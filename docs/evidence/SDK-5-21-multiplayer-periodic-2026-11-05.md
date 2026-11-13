# SDK-5-21 多人DOT/HOT执行记录

2026-11-05，crate0.23.0 / 协议6 / schema7 / 机制38。实现前冻结[MP01～MP12](../batches/SDK-5-21-multiplayer-periodic.md)，[逐项映射](../checklists/SDK-5-21-acceptance.json)及[最终机器签收](../../artifacts/sdk-5-21/acceptance.json)绑定实际源码、配置、日志、普通release与交付包；全部条件通过才签收。

本批复用同一个Statuses存储和take_periodic_due队列，在同一1～8必需成员World中接入玩家↔NPC的普通方向弹道敌对DOT和玩家/NPC自身HOT。定义、绑定、比例、时长、间隔、refresh/ignore、source_scoped和预算由配置初始化；Node/H5只提交意图、物化展示和处理真实存储证明。没有按成员复制World或在Node补算HP。

目标/来源按kind/id/generation解析；连接换代不换实体，NPC实际归位复位清旧状态并提高代次。DOT冻结施加组开始时的有效来源ATK，每跳读取当前DEF；HOT冻结自身基础maxHp，按当前缺口裁剪。统一队列保持计划时刻、DOT→HOT、实例ID顺序；保护/后台/未ready或归途消耗相关计划但跳过效果，其他成员继续。DOT走原伤害、撤离中断、死亡、一次击杀/掉落/归功链，不重复直接命中奖励或随机数。同刻致死DOT清HOT后不能复活。

来源从进入settling立即清理，无需金融回执；目标与来源分别清理，不能删除其他来源实例。unscoped refresh转移后的实例归新来源，ignore保留原来源/计划；旧来源退出不会误删已转移实例。NPC视图的可选statuses仅包含本人来源和NPC自身状态，private事件/移除ID按同一权限路由，Rust投影后再diff。退出视图冻结，重启只恢复Closed金融责任，不恢复World/周期/基线。详见[接入契约](../interfaces/multiplayer-periodic-v1.md)与[SDK说明](../../SDK.md)。

本批补齐的边界包括：NPC提交前容量预检不扣费/CD、不创建弹道；非法友军/旧代目标不施加；NPC复位旧代弹道失效；移除通知按实际实例权限拆分；NPC状态列表存在时拒绝null。恢复过程允许HOT按原期限自然到期，不能重新授予或延长。

证据：

- [原生/Node回归](../../artifacts/sdk-5-21/native-verification.json)：默认/全特性同1275项Rust（保留1240、新增35），48项Node（保留42、新增6）、Clippy与格式检查。覆盖1/2/4/8成员、双向DOT与自身HOT、有效攻击冻结/当前防御、来源覆盖/转移/保留、计划边界、局部保护/归途、真实死亡/退出/复位及独立资产收尾。
- 100次公共普通Runtime：每名成员实际收到过活跃HOT，随后关闭、终局送达、金融未决归零、Runtime线程结束；不要求异步接收者在同一观察瞬间仍同时持有未到期HOT，不作生产负载声明。
- [跨语言精确重建](../../artifacts/sdk-5-21/interop-verification.json)：8接收者、1520份真实Rust帧（57 full、1463 delta），JS逐字段/顺序等于Rust目标快照，精确重复幂等，默认/全特性向量摘要相同。包含本人HOT/受击DOT、本人来源NPC状态、NPC自身HOT、进度与到期/清空；非法来源、目标代次、重复/空列表及null等错误保持原缓存。
- [29项公开边界](../../artifacts/sdk-5-21/public/report.json)：13项非法Create不部分创建，14项配置/绑定/预算拒绝，以及四人/八人普通Node真实战斗、来源退出、带周期重连/缓存重建、共享拾取和独立终局。单席核心延迟并丢回执时，同伴周期继续；每账户准入/结算各一次。持久化Rust原始发布帧另行解码：四人731帧含23次NPC实例刷新，八人929帧使用ignore、不出现刷新；两者均有新增/移除/清空及到期通知，并保留接收者权限。
- [四人/八人H5](../../artifacts/sdk-5-21/browser/report.json)：真实独立页面，两套配置，实际观察玩家/NPC DOT及各自HOT和实际回复。P00带DOT提前退出后不再产生来源伤害，同伴继续；P01重连保留原实例/期限，重放旧连接帧被拒绝；丢一个已Sent delta与清缓存均通过request_full恢复。之后真实战斗、争抢同一掉落、撤离/逐人离场，console/page异常0、未决0、宿主释放。周期计数按接收者观察事件，NPC自身HOT向多个成员展示，不把这些重复观察计作多次游戏效果。
- [三处SIGKILL/六次恢复](../../artifacts/sdk-5-21/crashes/report.json)：部分准入、最后一席终局未决、核心已提交回执丢失，各同一冻结检查点恢复两次。后两处此前已实际产生DOT/HOT；部分准入尚未激活World。改外部配置不改旧责任，原资产/成员/hash保持；恢复为Closed，周期和发布计数均0，无World热恢复。
- 旧边界：[协议4的16项](../../artifacts/sdk-5-21/legacy4/report.json)、[协议5的20项](../../artifacts/sdk-5-21/legacy5/report.json)、[协议6原机制37的31项](../../artifacts/sdk-5-21/legacy6/report.json)及包外协议1～3完整消费链。
- [离线源码](../../artifacts/sdk-5-21/delivery/source-verification.json)与[独立消费](../../artifacts/sdk-5-21/delivery/independent-verification.json)：同一普通release、真实源crate/vendor，空Cargo缓存离线默认/全特性同集及Clippy；包外38项普通消费保留原36项并加入两套多人周期。普通消费时SDK回归目录与包内samples隐藏，显式使用包外配置/账户；另用交付H5服务/页面/纯模块只读启动。普通二进制未启用replay。

交付包SHA-256：`427a6266cc381ae3f58d1871d3528f77dc2b39dd78b04baa86d26253a86657ca`；源crate：`cbfdb53bd2cb4b08503ba70a215bd2f0bc8c972f2f6e8236af124ac9e37db5bc`；普通二进制：`e2b82364a1088f77a43c90401fd99761b264aa8afbe944599d37ab41d1c009f7`。原生/接入/交付报告与最终源码复核为同一候选。

[独立预览](http://127.0.0.1:4198/)使用`.local/sdk-5-21-demo.sqlite`与同一普通二进制；[只读预览检查](../../artifacts/sdk-5-21/local-preview.json)无建局、无浏览器异常。旧4173/4190/4193/4195验证台/账本不替换。起点现存5320份旧配置/示例JSON、原来源及SDK-5-17～20归档/证据逐字节保护；历史artifacts用户清理例外保持，不补造证据。

范围仍是普通PVE敌对DOT与自身HOT；友军治疗/PvP、护盾/受治疗修正、区域/穿透/单位技能及其他周期公式未迁移。R2三包和R3.2/R3.3共5包部分实现、17包待实现；完整M01/M03/M04/M14及原需求继续开放。原48场景/50向量新增执行数仍为0，不寻找第二款游戏。
