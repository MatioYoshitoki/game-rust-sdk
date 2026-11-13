# B3-7 Boss血量阶段验收（2026-11-03）

对应PRD v1.18 §3.20，先明确[B3-7需求与P01～P08](../batches/B3-7-boss-phases.md)，再实施Rust/配置/H5。状态：P01～P08受控验收通过。新增35项Rust、20项浏览器/进程场景与15组完整回归全部通过，最终报告和预览绑定同一源码/配置/二进制。

## 来源与行为

独立场景 `b3-7-boss-phases-v1`，raw SHA-256 `bd330bc50814f359fcbe7dbea60f1edcf1d7311ea7c617799d5c7bdfdf8b3e66`。离线导出先校验不可变manifest，复用森林、两狼、原境界七属性、既有近战/弹道/治疗、封印容器和奖池。守将HP450是来源炼气HP×3的显式验证配置；阈值10000/5000、近战2000/900ms、施法0.1/0.45均由配置初始化。

boss字段只增加阶段能力，不附加原Boss类型积分/移速/光球，当前模板kill_credit=true、无独立掉落，仍计一次普通击杀。完整原Boss、精英/Boss独立计分、范围技/危险区/召唤、多人和生产库存另批。

schema2/mechanism6，旧七raw/hash不变。每实例单向阶段与epoch独立；非致死正伤害只推进一次到最深满足阶段，致死优先，回血不回退。切阶段不清时序/动作/CD或返成本；真正到家才满血/初始阶段/epoch+1，不返MP/CD。阶段不算目标/奖励，Boss真实死亡后才完成最终波，仍显式解封/开箱/拾取和结算。

## 验收映射

| 标准 | 已通过证据 |
|---|---|
| P01 | 6项配置测试：原七hash、1/2/3/8阶段、能力/严格结构、身份、阈值、间隔/概率边界；普通模板兼容 |
| P02 | World阈值前/等值/后、跨多个阈值、miss/零伤害、自然回复/治疗/吸血不倒退与不额外抽RNG |
| P03 | 既有近战期限与后续间隔、概率0/1、施法/治疗/后摇/CD/成本保留，致死优先/收益一次，实例隔离 |
| P04 | 开始归位不重置、家外免伤/合法重激活、真正归位HP/阶段/epoch/近战，不返MP/CD、新守护代初始化 |
| P05 | 实际Boss死亡才完成波/放行箱，刷新/旧连接/保护动作不重置，已有归位到家照常重置；死亡/关闭停止，不增加实体/资源/计分 |
| P06 | 公开协议与恢复5项；浏览器完整搜刮撤离45/死亡11、部分阶段撤离37/死亡7、非计分Boss含晶石、提交前后强杀/丢回执/换版人工责任/重复重启/中断退款 |
| P07 | 桌面/手机Boss阶段/HP/有效节奏，公开输入自动闭环，页面异常与截图 |
| P08 | 默认/all-features唯一Rust、Clippy、原全部专项、新专项及最终源码/配置/二进制审核 |

新增Rust35项：World24、公开协议/恢复5、配置6。配置专项38项中的新6项已计入35；其他专项也是317项总数的子集，不将默认/all-features重复相加。

## 复现

```sh
python3 scripts/export_b3_7.py --check
python3 scripts/verify_b1.py --date 2026-11-03
python3 scripts/verify_h5.py
python3 scripts/verify_io_timeouts.py
python3 scripts/verify_durability.py
python3 scripts/verify_reconnect.py
python3 scripts/verify_flow_control.py
python3 scripts/verify_b3.py
python3 scripts/verify_config_loading.py
python3 scripts/verify_b3_2.py
python3 scripts/verify_b3_3.py
python3 scripts/verify_parameters.py
python3 scripts/verify_b3_4.py
python3 scripts/verify_b3_5.py
python3 scripts/verify_b3_6.py
python3 scripts/verify_b3_7.py
```

实现期先校正验收夹具与接口已有语义：未知动作应返回UNSUPPORTED；混用直接HTTP输入与页面暂停前刷新同步序号；归位夹具将警戒半径与牵引距离一致配置，避免离开牵引后又自动入警戒；Boss存活时显式经旁路前往撤离点，绕过动态足迹。中断恢复保留历史事件，核验历史阶段事件数量不增加，而非要求删除历史。阶段/资产规则未因这些夹具调整而放宽。

15组回归全部通过；日志为 `artifacts/b3-7/regression-*.log`，退出状态和用时见[regression-run.json](../../artifacts/b3-7/regression-run.json)。基础/H5/持久恢复/并发先单独运行，旧地图/连接/超时/配置/容器最多两组并行，参数、守护、单组、多波和Boss专项独立运行，避免加速时钟夹具受到外部CPU争用。

[新专项机器报告](../../artifacts/b3-7/verification.json)包含35项新Rust与20项真实浏览器/进程场景，页面异常0。默认/all-features各317项相同Rust，格式、Clippy `-D warnings`、来源/导出/独立回放、12条Node链/2项协议检查、原全部专项和H5性能检查通过。旧七raw/hash保持；15份报告逐文件核对最终100个源码、配置、脚本及B1验收映射，所有带二进制哈希的报告与预览一致，见[source-audit.json](../../artifacts/b3-7/source-audit.json)。

本机实时等距性能采样：快照间隔中位数30.7ms/P95 32.7ms，按键到权威移动17.9ms，长任务0；这是本地受控采样，未覆盖生产容量。截图：[阶段2](../../artifacts/b3-7/phase-two.png)、[390px手机](../../artifacts/b3-7/mobile.png)、[完整撤离](../../artifacts/b3-7/extraction.png)。

本地4173预览升级为8场景，选择“迷雾森林 · 阶段守将”后可手动或一键验证。先正常停止旧宿主，再备份原SQLite并通过完整性检查，保留原会话/账本；备份信息见[local-upgrade-backup.json](../../artifacts/b3-7/local-upgrade-backup.json)。只读冒烟核验8场景、新Boss配置/hash、等距页面和页面异常0，未创建新局，见[local-preview-smoke.json](../../artifacts/b3-7/local-preview-smoke.json)。

本批只签收HP阶段及既有战斗/搜刮/资产闭环；完整原圣所Boss、独立类型计分和新特殊技能不包含在此结果中。
