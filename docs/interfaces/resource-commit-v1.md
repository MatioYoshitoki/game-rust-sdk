# 通用资源预留与提交 v1

SDK 0.93.0，schema7 / mechanism105 / protocol6，同包 Rust/Node/H5 消费。

每个技能形式（含技能库）可选 `resource_commit`：

```json
{"reserve_at":"accept","consume_at":"commit","cooldown_starts_at":"commit","refund_before_commit":{"kind":"full"}}
```

reserve_at 为 accept 或 none。当前只接受 commit 消费、commit 冷却与 full 未提交释放；其它字符串、未知字段和显式 null 拒绝。未声明时保留旧技能费用路径。

accept 在进入真实准备时，按当时HP及既有 hp_cost 公式冻结HP需求、灵力需求和最低剩余生命；只记录私有占用，不提前扣除HP/SP，不调用外部资产IO。前摇、蓄力/松手与重瞄准期间保留这份费用；排队意图在提升为真实准备时才生成预留。none 不保存预留，HP费用继续按提交时当前生命求值。

提交先重验目标、来源、资源、冷却和投放容量，再原子扣除HP/SP、写入一次冷却、生成提交事件及效果。accept 的HP费用不因等待期间治疗或受伤而重新计算；若无法按原费用保留最低生命，整笔拒绝。外部资源变化或冷却变化也可使提交失败，失败不能留下部分扣费、冷却或运行时。none 及旧路径继续使用原有非致死费用截断规则。

例如100HP、50%费用，accept 冻结50；受伤到80后提交剩30。none 在80HP提交则费用40、剩40。accept 若提交时仅有50HP且下限1，整笔失败。被治疗至100后，已冻结的费用仍保持原量。

取消、目标失效、控制、死亡/退出、重连、技能撤销及关局均释放未提交占用；释放不扣费，也不生成可重复领取的退款。提交后的费用和冷却不会因后续中断撤销。进程恢复保持现有财务关闭/补偿政策，不恢复战斗或旧预留。

显式声明新策略的私有 pending_action 携带 action_id；accept 另有 resource_reservation `{spirit,hp,minimum_hp_after_cost}`。字段出现时不允许null，当前HP/SP可以因合法外部变化低于预留，需要等待提交重验。旁观者不获得这些私有明细。同一准备身份的费用快照不可通过完整或增量消息改变、抹除；替换动作重新报价。旧策略的投影保持原结构。

H5 展示权威预留，无本地费用结算。普通输入无法声明预留、费用或改状态。两套 reservations-four/eight 配置通过真实NPC伤害、友方治疗、两种HP求值、重连与取消验证。原资料的58项accept、20项none和8项无生命周期分组已登记；这是资源策略映射与阶段投影，完整技能威力、数量扩展和原内容效果并不据此全部关闭。

[片段Schema](resource-commit-v1.schema.json)校验结构，冷装载与Rust事务还执行跨字段和生命周期验证。
