# B3-3 分层封印容器验收（2026-11-03）

对应PRD v1.14 §3.16，先完成[B3-3需求与U01～U08](../batches/B3-3-layered-container.md)再实施。状态：U01～U08受控验收通过；24项新增Rust、13项浏览器/进程专项和11组最终回归全部通过，报告与最终源码/二进制哈希一致。

## 实现和来源

新增独立 `b3-3-layered-container-v1`；raw SHA-256为 `1dabc72c1bf08e9ee364dfdbe7230f070a6ee2b5e30aab7fc3d640ef0e5b1c49`。导出器核验原包manifest，采用来源玉匣两层、每层1200ms、当前层中断保留比例0、600000ms重生和loot-chest-rare奖池，并登记所有物品堆叠。使用原已验证的森林双点和独立模板 `verification-layered-cache`；不把该容器声明为包含守护/优先领取的完整原玉匣。

配置schema仍为2，机制版本2新增layered_seal_ground_loot；原版本1、B1/B3-1/B3-2原文/哈希均保持。seal字段强制对象，层数、时长、保留比例及跨字段约束启动校验，未知guarded能力或字段不静默忽略。即时容器不能携带seal，分层能力不能遗漏seal或使用旧能力版本。

Rust内部LayeredProgress只管理分层时钟和中断，不拥有资产。已完成层保留，当前层按配置比例乘后向下取整；0、0.5、1和不同层数/时长均有行为验收。正伤害先于该步层完成，实际0伤害不中断；非法导航不取消，合法移动/导航、主动取消、保护、死亡与关闭释放交互。补齐了Arena暂时从World取出时Leave仍须显式取消的边界。

解封本身不动RNG/背包/仓库/修为，全部解封后显式OpenChest才进入原原子落地路径。满地面/落点失败保留解封并回滚RNG，满背包拾取仍保留地面。每代只领一次，重生自成功开启计时并重置进度，陈旧取消不能影响新代。Engine关闭为活跃交互中断额外保留输出槽，仍有限回收World。

H5显示权威层数、当前层条形进度与中断原因，提供F开始/继续/开启、Escape/按钮取消和手机操作。一键流程在击杀后逐箱解封、显式开启、拾取、撤离；无H5进度计时或奖励裁决。一键脚本按怪物ID追踪本次目标，并在提交攻击前记录计数，避免弹道击杀后的目标切换或快速回执导致等待卡住。查看[移动端](../../artifacts/b3-3/mobile.png)、[中断](../../artifacts/b3-3/interrupted.png)、[完整撤离](../../artifacts/b3-3/extraction.png)。

## 验收映射

| 标准 | 证据 |
|---|---|
| U01 | 4项配置专项：新旧共存、版本/缺字段/未知字段/数组伪结构/非法层数时长比例、合法变体和即时机制误配；来源导出一致 |
| U02 | 16项World专项中的生命周期、施法/撤离、范围边界/墙、一个活跃交互、攻击/拾取互斥、非法导航与合法移动；旧连接取消被拒绝 |
| U03 | 前/到期层提交、0/0.5/1保留、完成层保留、不同层数/时间、同刻正伤害先中断/0伤害继续；重复公开输入幂等、刷新保护后不自动继续 |
| U04 | 解封无奖励、显式开启一次、满地面回滚后重试、代重置与陈旧取消；原通用满包/旧修订保留地面测试继续通过，落地/拾取复用同一实现 |
| U05 | 三杀双封印箱正常撤离45修为、死亡11，逐物品守恒；只解封关闭不产生额外物品/修为；关闭/绝对终局不再推进 |
| U06 | 4项公开协议/关闭/恢复Rust测试；刷新/原连接、结算提交前后SIGKILL、丢回执查询、人工责任换版/重复重启、未冻结破解中断只退入场 |
| U07 | 权威进度/原因/开始取消开启、手机无横向溢出、一键闭环；真实伤害和真实逻辑钟重生的公开输入验证 |
| U08 | 186项相同默认/all-features Rust测试（原162+新增24），Clippy与原全部专项；11份最终报告源码/二进制审核 |

专项[机器报告](../../artifacts/b3-3/verification.json)含24项新增Rust（World16/协议恢复4/配置4）、13项真实浏览器/进程场景，页面异常0。正常种子下稀有必掉寒潭合计5，其他物品按原概率独立生成；结算只含实际拾取物，不把掉落或解封等同入库。默认/all-features重复运行不相加；配置专项当前14项中新增4项也已计入24。

服务重启只恢复Closed资产/结果，不恢复封印现场。恢复物品上界加入首次最低破解时长与“重生+再次破解”周期；新seal时长/层数或成长系数上线后，旧责任仍用原raw/hash核对，金额/物品和逻辑operationId不变。SQL提交前已证明未提交时可在有限预算内增加attempt_id，这不改变冻结业务正文。

## 复现与最终证据

```sh
python3 scripts/export_b3_3.py --check
python3 scripts/verify_b3_3.py
python3 scripts/verify_b1.py --date 2026-11-03
python3 scripts/verify_h5.py
python3 scripts/verify_reconnect.py
python3 scripts/verify_io_timeouts.py
python3 scripts/verify_durability.py
python3 scripts/verify_flow_control.py
python3 scripts/verify_b3.py
python3 scripts/verify_config_loading.py
python3 scripts/verify_b3_2.py
python3 scripts/verify_parameters.py
```

最终机器报告与11组命令日志位于 `artifacts/b3-3/`，源码审核为[source-audit.json](../../artifacts/b3-3/source-audit.json)。原专项统计：12条Node链、2项JSONL、来源包/导出/独立回放；4项投影/9项H5功能性能；6项连接、12项超时、6项SQLite/19项强杀、7项队列I/O/8项并发、7项地图、14项配置Rust/9项配置进程、23项普通袋Rust/13项浏览器、16项参数Rust/18项加载/9项参数浏览器。各专项Rust为186的子集，不能重复相加。

本地4173预览已启用4个场景，封印参数为两层/1200ms/保留比例0。升级前停止旧服务并备份 `.local/h5-lab.before-b3-3-20261103-130750.sqlite`，完整性检查ok，保留1个当前会话与6份历史配置，启动后加入新配置。只读冒烟无新会话、页面异常0；服务启用情况见[预览检查](../../artifacts/b3-3/local-preview-smoke.json)。完整守护/优先领取/多人竞争、混合AI模板、生产库存/客户端、容量和战斗热恢复仍在后续范围。
