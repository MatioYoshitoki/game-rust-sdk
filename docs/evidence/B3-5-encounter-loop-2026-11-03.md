# B3-5 区域遭遇验收（2026-11-03）

对应PRD v1.16 §3.18，先明确[B3-5需求与E01～E08](../batches/B3-5-encounter-loop.md)，再实施Rust/配置/H5。状态：E01～E08受控验收通过；31项新增Rust、16项浏览器/进程检查和13组完整回归最终通过，报告与最终源码/配置/二进制一致。

## 来源与实现

新增独立 `b3-5-encounter-loop-v1`，raw SHA-256为 `8736981919da33d89ad11ed1d29d6bf31313ca4aaa673923ca7acdcd8acddfcc`。导出前校验不可变来源manifest；复用已验证的原森林、妖狼点位/模板、封印双箱/奖池，增加显式验证区域和目标/箱绑定。区域中心为第一狼点，96世界单位半径为验证布局参数，全部写入配置；不签收来源完整森林圣所、Boss阶段、波次、调谐成本、贡献或优先权。

配置schema仍2，机制版本4增加encounters，原B1/B3-1/B3-2/B3-3/B3-4原文和哈希保持。区域、模板/怪物实例/点位、必杀列表、箱绑定和属性/奖励均配置初始化；Rust维护状态，无浏览器生成/完成/直接发奖入口。六类Client能力不变。

Arena预留有界槽位，初始敌人不进入快照/碰撞/AI。权威移动后按距离采样触发，dormant→spawning→active→completed每局一次；固定点位整组满足自身足迹/组内间距/玩家和活怪避让才提交。受阻保留计划、离开继续重试，不消耗RNG、只报一次受阻事件。刷新/保护继续同World状态和ID，关闭后不再推进。

必杀进度只认本组指定实例实际生成并死亡，不使用全局计分；归位、其他组/静态怪死亡不贡献，不计分怪仍能完成目标。最后目标死亡且玩家仍可操作时完成一次；完成不改物品、积分、RNG或箱代。ENCOUNTER_LOCKED解除后仍显式破解/开箱/拾取，所有原守护/封印/距离/代/容量门禁和资产守恒继续生效。箱重生保留遭遇完成，不再生成第二组。

恢复产量/成长上界包含所有一次性遭遇配置实例，并保留混合模板/不计分物品语义。服务重启只恢复Closed责任和结果，无持久终局只退原入场，不重建现场或重放触发。

补查8遭遇同一步触发的峰值后，修复了原固定16输出预留不足时可能触发断言的边界。Engine在发送该步事件前按实际批量检查余量，容量不足时冻结原结算并受控Fault/回收World，不部分发布后越界；32容量封存、64容量正常完成同时触发均有证据。

## 验收映射

| 标准 | 证据 |
|---|---|
| E01 | 6项配置测试：原五版本/缓存、能力/严格结构、全局ID/模板/目标/箱引用、区域和足迹路径/重叠、独立组不得共享奖励、8组/32槽位/资源128预算 |
| E02 | 20项World中的圈外/边界/移动后采样、出生圈内首合法步、ready/保护/前台/死亡/终局、重复进出；浏览器初始无敌人、公开导航触发 |
| E03 | 未启用槽位不可战斗、异模板/半径独立、玩家与静态敌人阻挡无部分实体、受阻刷新/离区清障恢复、不消耗RNG/重复事件 |
| E04 | 指定目标部分死亡/归位/别怪不完成，跨组隔离、必杀子集、不计分目标、离开不重置、最后死亡一次完成、重复伤害不记双份 |
| E05 | 完成前公开开启/伪造完成字段拒绝；完成只解锁、原分层/显式开箱/落地拾取；未触发提前撤离22修为且无箱奖、部分击杀死亡3、完整三杀撤离45/死亡11；不计分目标0杀/22修为且晶石6实际入库 |
| E06 | 5项公开协议/恢复/峰值测试；浏览器刷新部分进度/ID与旧连接、箱重生资格保留、提交前后强杀、丢回执、目标/半径/成长换版后原责任、重复重启及无终局原入场退款 |
| E07 | 等距触发圈、阶段/目标进度/箱锁、前往区域和一键公开输入；桌面/390px手机无横向溢出，最终页面异常0 |
| E08 | 默认/all-features各247项相同Rust（原216+新31），新专项31项Rust/16项浏览器，原全部专项与13份报告的最终源码/二进制审核通过 |

新增Rust31项：World20、协议/恢复/输出峰值5、配置6。目录配置专项26项中的新6项属于31的子集，不重复相加。原满地面/满包/拾取修订等测试继续通过后才签收复用门禁。

## 复现

```sh
python3 scripts/export_b3_5.py --check
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
```

最终日志写入 `artifacts/b3-5/regression-*.log`，参数、守护与遭遇专项分别独立执行。新专项[机器报告](../../artifacts/b3-5/verification.json)包含31项新Rust和16项真实浏览器/进程场景；全部13份报告的声明源码逐文件匹配当前源码，带二进制哈希的报告及本地预览使用同一kernel-host。最终86个源码/配置/脚本文件和原五份配置哈希审核见[source-audit.json](../../artifacts/b3-5/source-audit.json)，命令退出状态、用时和单独复核记录见[regression-run.json](../../artifacts/b3-5/regression-run.json)。

默认/all-features各247项相同Rust、格式、Clippy `-D warnings`、来源包/导出/独立回放、12条Node链/2项协议、原H5功能/性能及全部连接/超时/持久/并发/地图/配置/普通袋/分层/参数/守护专项通过。查看[待触发/箱锁](../../artifacts/b3-5/dormant.png)、[移动端](../../artifacts/b3-5/mobile.png)、[完整撤离](../../artifacts/b3-5/extraction.png)。

本地4173已升级6场景。先停止旧服务，备份 `.local/h5-lab.before-b3-5-20261103-145822.sqlite` 并检查完整性ok，保留1个当前Closed会话和8份已有配置。新目录/配置只读冒烟确认林间遭遇、半径96、3个必杀目标、2个奖励箱，页面异常0、未新建会话；见[预览检查](../../artifacts/b3-5/local-preview-smoke.json)和[备份记录](../../artifacts/b3-5/local-upgrade-backup.json)。

最终并行首轮中，B3-2最后一个旧场景输入与B3-4首次导航遇到浏览器尚未完成的ready输入，发生STALE_SEQUENCE/直接输入竞争。使用不变源码单独复核；首次失败日志保留在 `regression-b3_2-initial-sequence-race.log` 与 `regression-b3_4-initial-sequence-race.log`，最终命令记录保留两次执行信息。该用例混用页面与直接HTTP序号，最终复核结果以对应机器报告为准。

当前范围仍为每遭遇每局一次、单人独立测试库存，生产库存/客户端、正式容量、跨机与战斗热恢复继续按批交付。
