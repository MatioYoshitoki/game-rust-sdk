# SDK-5-6 周期治疗HOT验收（2026-11-04）

先冻结[HT01～HT10](../batches/SDK-5-6-periodic-healing.md)，再实现0.9.0/机制24。有限范围全部通过；[逐项映射](../checklists/SDK-5-6-acceptance.json)与[机器汇总](../../artifacts/sdk-5-6/acceptance.json)绑定源码、原生输入、版本、报告与交付包。

## 已交付行为

periodic_heal在技能提交时给自身施加，玩家与怪物复用同一状态机制。基础maxHp×heal_bps的每跳量在施加组开始时冻结，首跳等待间隔；refresh保留ID并重取/重置，ignore保持原来源与节拍。比例、间隔、时长、重复和绑定均从配置初始化。

HOT与DOT共用周期游标、容量、到期和身份清理。按计划时刻、效果优先级、实例ID执行；相同计划时刻DOT先HOT，致死后不能复活。每跳按当前HP缺口裁剪，满血/零量消耗计划并记录0，不套伤害保底，不转盾/灵力、不触发主动命中、不打断交互/撤离/支援或回退Boss阶段，也不消费随机数。

保护/后台/未就绪、归途/复位倒计时跳过到期点，恢复不延寿/补跳。死亡、换代、归位/复位、终局和关闭清理。重连保持同World计划，重启只履行冻结资产责任，不恢复临时状态或World。

HOT的periodic包含kind=max_hp_heal/raw_heal，periodic_heal_tick提供实际hp_restored。旧DOT字段保持，包括raw_damage=0；旧19～22省略periodic。0.9.0新增公开Rust DTO字段/枚举，六类Client、协议1～3及资产/恢复schema保持。

[配置与接入](../interfaces/periodic-healing-v1.md) · [完整样例](../../examples/hot-host/catalog.json)

## 执行证据

| 门禁 | 结果 |
|---|---|
| 原生/配置 | 默认/全特性相同800项Rust（保留767、新增33）、Clippy -D warnings和22项Node通过。覆盖28种非法字段/版本/时间/绑定、0/10000比例、128跳上界、旧19～23规范化及容量原子性。[默认日志](../../artifacts/sdk-5-6/default.log) · [原生摘要](../../artifacts/sdk-5-6/native-verification.json) |
| 精确计时/金额 | interval600/duration2200在600/1200/1800执行；非整步不漂移，末点执行步已到期则取消。maxHp1000/bps1000冻结100，HP850实回100/50/0；覆盖零量/minimum_damage、大整数、超上限HP不倒扣、ATK/DEF/护盾独立性、刷新重取/忽略保持。[HT02～HT04映射](../checklists/SDK-5-6-acceptance.json) |
| 排序/生命周期 | 颠倒实例ID也由同计划DOT先执行；致死清理HOT，非致死先伤后回复，较早计划的HOT仍先于较晚DOT。玩家/怪物真实self提交、准备取消、重复输入/重连、归途/复位、Boss阶段保持、100次关闭和合法容量下周期事件峰值回收通过。自然致死产生原结算并在凭据提交后释放World。同上原生集合 |
| 普通Node/H5 | 两套包外配置4条实时链。A为raw100、600ms、6000ms、refresh，受伤934后实回66至1000；B为raw50、700ms、2500ms、ignore，934实回50至984。随后战斗→拾取→撤离/死亡结算，页面异常0、一次入场/结算、World/未决=0。A含同World重连、原ID/期限/节拍保持及旧连接拒绝。[报告cases](../../artifacts/sdk-5-6/verification.json) |
| 纯触屏 | 390×844仅用触屏控件，真实受伤→HOT实回→战斗拾取→双终局，HTTP仅只读观察。页面异常0，回收完成。同上touchCases；[手机截图](../../artifacts/sdk-5-6/outside-b-hot-0-hot.png) |
| 强杀/恢复 | 活跃已回复且有后续计划、冻结终局意图后、资产提交后三处强杀，各连续两次恢复；改变外部HOT比例/节拍不重算旧Create/hash/金额/结果/operation ID，无重复扣费/发奖或HOT/World恢复。同上crashes |
| 旧版普通链 | 旧19～23各撤离/死亡，共10条；原DEF/叠层/属性/护盾/DOT与序列化形状保持。同上legacy |
| 独立交付 | Cargo源包、21项锁定第三方源和普通release二进制；Rust源与候选一致、包内无工作区绝对路径。删除回归目录和samples后，包外24/23/22/21/20/19配置各Node双终局，共12条。空缓存离线Rust协议1六模式、协议2/3十四场景与各自重启、两次核对通过。[报告](../../artifacts/sdk-5-6/delivery/independent-verification.json) |
| 解压源码/保护 | 空Cargo缓存、离线执行解压源码的相同800项默认/全特性及Clippy通过，不把重复运行计作新增。原20场景、旧样例、v1.1来源包及历史0.3.1～0.8.0交付69项原文/hash保持。[源包报告](../../artifacts/sdk-5-6/delivery/source-verification.json) · [保护清单](../../artifacts/sdk-5-6/baseline-verification.json) |

首轮浏览器脚本在2200ms短时HOT重连后读取到空状态，且未记录当时截止诊断，[原日志](../../artifacts/sdk-5-6/first-browser-attempt.log)保留。主验收将包外A观察时长延长到6000ms，并显式检查读取时逻辑时间仍早于原截止、原实例与节拍保持；完整浏览器链重新执行通过。另以原2200ms窗口单独复验，逻辑3810→4080、截止5260、重载墙钟480ms，实例保留、无移除、页面异常0。[短时重连诊断](../../artifacts/sdk-5-6/short-hot-reconnect-diagnostic.json)

这次调整仅涉及验证脚本及包外观察配置；Rust/Node/Web/仓库样例与原生输入未改。原生2200/600精确时序仍有独立检查；复用原生摘要前逐项核对输入和日志hash，不重复计数。旧协议2的包外副本仍将撤离解锁/读条调为3000/500ms以适配参考玩家期限，原数据不变。

## 交付和范围

[0.9.0本地交付包](../../artifacts/sdk-5-6/delivery/match-kernel-0.9.0-aarch64-apple-darwin.tar.gz)，4,521,201字节；SHA256为`6c0fe66118a6c84e217908b9f047970049acbdf1a23bfc330ca61f511fb47b11`。预编译平台为Apple Silicon macOS，replay=false；其它平台与生产发布分别验收。[交付清单](../../artifacts/sdk-5-6/delivery/package.json)

试玩：[127.0.0.1:4179](http://127.0.0.1:4179/)，独立账本`.local/sdk-5-6-demo.sqlite`。先受伤，再施放技能并停手观察逐跳回血。启动检查未创建对局、页面异常0，旧4173～4178预览及账本保持。[预览检查](../../artifacts/sdk-5-6/local-preview.json)

本批只关闭基础maxHp比例、refresh/ignore的自身HOT。其它公式/治疗乘区、友军目标、复杂叠层、过量转盾、控制/驱散、完整来源目录与生产接入继续按[剩余矩阵](../checklists/sdk-requirements-gap.md)跟踪。D-03/D-05未变；不扩新地图或第二款游戏内容。
