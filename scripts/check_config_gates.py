#!/usr/bin/env python3
"""配置手册门禁：校验 config-cookbook.md 的机制版本门槛表与 --sdk-info 一致。

用法：
  python3 scripts/check_config_gates.py --sdk-info-json <support.json>   # 校验（CI/打包门禁）
  python3 scripts/check_config_gates.py --binary <kernel-host> [--fix]   # 从二进制取信息；--fix 就地重写表格

漂移时非零退出；--fix 用最新 sdk-info 重生成标记间表格（勿手改标记间内容）。
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COOKBOOK = ROOT / 'docs/guides/config-cookbook.md'
BEGIN = '<!-- GATES-BEGIN（generated，勿手改；漂移请运行 --fix） -->'
END = '<!-- GATES-END -->'
TITLES = {'solo': '单人族（schema 4 / 协议 3）', 's5': 'schema 5 / 协议 4（双人）',
          's6': 'schema 6 / 协议 5（1~8 席）', 's7': '成员族（schema 7 / 协议 6，1~8 席）'}
ORDER = ['solo', 's5', 's6', 's7']


def load_info(binary):
    out = subprocess.run([str(binary), '--sdk-info'], capture_output=True, text=True, check=True).stdout
    return json.loads(out)


def gate_rows(info):
    rows = []

    def dig(obj, path=''):
        if isinstance(obj, dict):
            mv = obj.get('minimumMechanismVersion')
            if mv is not None:
                mx = obj.get('maximum', obj.get('maxSkills', obj.get('maxMs')))
                rows.append((str(mv), path.lstrip('.'), obj.get('schemaVersion'), mx))
            for key, value in obj.items():
                dig(value, f'{path}.{key}')

    dig(info)
    groups = {}
    for mv, name, sv, mx in rows:
        key = f's{sv}' if sv else 'solo'
        groups.setdefault(key, []).append((int(mv), name, mx))
    lines = []
    for key in ORDER:
        if key not in groups:
            continue
        lines.append(f'**{TITLES[key]}**')
        lines.append('')
        lines.append('| 门槛 | sdk_info 键 | 上限 |')
        lines.append('|---|---|---|')
        for mv, name, mx in sorted(groups[key]):
            shown = f'上限 {mx}' if mx is not None else '—'
            lines.append(f'| {mv} | `{name}` | {shown} |')
        lines.append('')
    return '\n'.join(lines).rstrip('\n')


def expected_block(info):
    return f'{BEGIN}\n{gate_rows(info)}\n{END}'


def strip_legacy_table(text):
    """删除章节标题到清单说明行之间的遗留手写表；不存在则原样返回。"""
    note = text.find('> 完整机器可读清单')
    heading = text.find('## 版本门槛速查')
    if note < 0 or heading < 0:
        return text, False
    between = text[text.find('\n', heading) + 1:note]
    if TITLES['solo'] in between:
        return text[:text.find('\n', heading) + 1] + '\n' + text[note:], True
    return text, False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--binary', default=None, help='kernel-host 路径；缺省自动探测 target/{release,debug}')
    ap.add_argument('--sdk-info-json', default=None, help='直接提供 --sdk-info 的 JSON 文件')
    ap.add_argument('--fix', action='store_true', help='漂移时就地重写手册表格')
    args = ap.parse_args()

    if args.sdk_info_json:
        info = json.loads(Path(args.sdk_info_json).read_text())
    else:
        binary = args.binary
        if not binary:
            for candidate in ('target/release/kernel-host', 'target/debug/kernel-host'):
                if (ROOT / candidate).exists():
                    binary = ROOT / candidate
                    break
            else:
                print('找不到 kernel-host；先构建或用 --binary/--sdk-info-json 指定', file=sys.stderr)
                return 2
        info = load_info(binary)

    expected = expected_block(info)
    text = COOKBOOK.read_text()
    match = re.search(re.escape(BEGIN) + r'\n(.*?)\n' + re.escape(END), text, re.S)
    if match is None:
        if not args.fix:
            print('FAIL: 手册缺少 GATES-BEGIN/END 标记', file=sys.stderr)
            return 1
        # 首装引导：表格插在「完整机器可读清单」说明行之后、「移动与寻路」章节之前，
        # 并删除章节标题到说明行之间的遗留手写表（否则会出现新旧两张表）。
        # 纯 str.find 定位——不能用跨行正则，DOTALL 懒惰量词在长文档上会灾难性回溯。
        note = text.find('> 完整机器可读清单')
        heading = text.find('## 版本门槛速查')
        head = text.find('## 移动与寻路')
        if note < 0 or heading < 0 or head < 0 or not heading < note < head:
            print('FAIL: 未找到首装插入锚点（章节标题 + 清单说明行 + 移动章节）', file=sys.stderr)
            return 1
        heading_line_end = text.find('\n', heading) + 1
        text = text[:heading_line_end] + '\n' + text[note:]
        COOKBOOK.write_text(text)
        print('gates inserted（首装引导；已移除遗留表）')
        return 0
    stray = text.count(TITLES['solo']) > 1
    current = f'{BEGIN}\n{match.group(1)}\n{END}'
    if current == expected and not stray:
        cells = expected.count('|') - len(expected.splitlines()) * 2
        print(f'gates ok: 与 --sdk-info 一致（{cells} 个数据单元）')
        return 0
    if args.fix:
        text = text[:match.start()] + expected + text[match.end():]
        text, removed = strip_legacy_table(text)
        COOKBOOK.write_text(text)
        print('gates rewritten（--fix）' + ('；已移除遗留表' if removed else ''))
        return 0
    if stray:
        print('FAIL: 存在标记外的遗留手写门槛表（误导读者）；运行 '
              'python3 scripts/check_config_gates.py --fix 清理', file=sys.stderr)
    else:
        print('FAIL: 配置手册版本门槛表与 --sdk-info 漂移；运行 '
              'python3 scripts/check_config_gates.py --fix 更新', file=sys.stderr)
    return 1


if __name__ == '__main__':
    raise SystemExit(main())
