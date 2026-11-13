# 多人圆形区域计划（机制41）

0.26.0/schema7/协议6，[GA01～GA12](../batches/SDK-5-24-member-circle-plans.md)。复用[统一多人战斗](unified-member-combat-v1.md)、既有ground_circle/ground_field/ContactBudget与cast_ground{skill_id,position:[x,y]}。

有限可行走落点/中心射程/视线接受与提交重验；提交前失败无成本/CD/状态/计划。单次预警后一次接触，零预警最早下一固定步；持续首跳在预警后等待间隔，截止排除、最多128跳。提交冻结有效ATK，当前位置/DEF/盾与统一敌对接触/伤害/附带/归功链逐次结算。持续必须有contact_policy：每跳/总命中/重复间隔由配置初始化，0=每生命身份一次，正数用scheduled_at_ms含等号，闪避不耗、全盾合法接触耗预算。

members.area_capacity仅机制41必配1～8，单次/持续共享全World槽；超工程输出预算在I/O前拒绝；容量×持续单区最大总接触次数≤128，限制全World历史额度。其它技能不占槽。未提交准备取消/后台/重连中断；已提交中心/预算/绝对期限不因移动/后台/同World重连重置。来源settling/死亡/退出/断线超时清自己的计划，其他成员继续；绝对终局/Close清全部，无退款。重启仅Closed金融责任。

SnapshotV4新增Option<Vec<AreaPlanViewV4>> areas，41含数组包括空、旧省略、null拒绝。delta.areas存在时替换整个数组，[]清空、省略不变。公开计划为World生成id和来源EntityRef，包含技能/几何/时序/进度/有限接触预算历史，不含私有request/action ID、攻击/绑定/灵力/CD或状态ID；准备/ActionCommitted仍仅本人。公开area_started/area_tick/area_ended用于观察真实计划和成功目标。严格Rust/JS解码器原子重建；Rust结构体字面量补Option字段，六类Client和金融/恢复DTO保持。

固定1～8成员PVE，仅玩家圆形区域；不含友军/PvP、线形/移动区域或穿透。Node/H5只提交坐标意图和显示Rust结果，所有玩法数值与绑定由配置初始化。
