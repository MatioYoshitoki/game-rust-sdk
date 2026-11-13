# 多人统一战斗与指定单位弹道（机制40）

0.25.0/schema7/协议6，沿[UC01～UC12](../batches/SDK-5-23-unified-member-combat.md)、[多人盾](multiplayer-shields-v1.md)与[原指定单位契约](unit-targets-v1.md)。六类Client、金融/恢复DTO保持。

内部统一kind/id/generation实体解析、self/friendly/hostile模式关系、直接/DOT伤害、HOT回复与状态归属。当前PVE成员互为友方、NPC互为友方，跨类敌对；本批不开放PvP或友军输入。既有方向/自身/周期/盾复用同一结算入口，后续技能只负责选择目标与计划接触，不另复制一套成员伤害规则。历史单成员协议适配保持。

新多人输入复用cast_unit{skill_id,target:{id,generation}}，只对已发布的存活非归途敌对NPC。接受、提交重验当前资格、中心射程/视线；准备期失败无成本/CD/自身状态/新弹道。提交按当前目标位置冻结方向与有效ATK，不跟踪、不转绑新代。其它合格敌对单位先挡住只结束弹道，不伤害/附带/随机/回灵；墙同时接触优先。来源/目标失效结束在途，不退款；同World重连保留已提交计划，未提交准备中断。重启只恢复Closed金融账。

本人pending_action及私有action_committed使用原unit_target。公共ProjectileViewV4新增Option<UnitTarget> unit_target，仅单位计划存在；null拒绝，普通/NPC/旧机制省略。Rust字面量补Option字段，严格JS消费方用随包解码器。目标只有公开NPC{id,generation}；成员私有准备/数值不新增公共字段。full/delta仍以Rust投影与Sent基线为准，坏帧保留缓存。

所有数值、技能槽、状态绑定与预算沿配置。Node/H5只从发布快照选择NPC身份并提交意图，不计算伤害/敌我或回写World。范围不包含友军/PvP/地面区域/穿透等输入。
