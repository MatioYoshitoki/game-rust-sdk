# 最终施法拦截

从 SDK 0.101.0（配置 schema 7，机制 105，发布协议 6）起，游戏可以把一次性施法反制放进 `combat_rules.cast_interceptions`。它只读可信规则，Node 仍转发原有玩家输入并处理连接，H5 继续显示 Rust 状态及私有事件。

```json
{
  "cast_interceptions": {
    "cast-mark": {
      "action_filter": {"basic": false, "skills": {"mode": "only", "ids": ["ordinary", "npc-ordinary"]}},
      "default": {"kind": "interrupt", "status_id": "cast-seal"},
      "profile_overrides": {
        "elite": {"kind": "allow_and_apply", "status_id": "cast-delay"}
      }
    }
  }
}
```

键是状态定义 ID，最多 16 个；每条规则最多配置 16 个目标战斗 profile。标记是来源隔离、非叠层、无属性和控制收益的有害定时状态，来自现有技能命中或提交触发。后续状态是来源隔离的有害定时状态，受现有状态准入、控制替换、缩时和清理约束；不能再次生成拦截标记。`allow_and_apply` 不能追加硬控制或改变资源上限。缺省没有拦截规则。

仅初次技能通过最终目标、资源和容量检查后才查找标记。适用于成员初次技能和现有 NPC 弹道初次施法。按最早到期、施加时刻和实例 ID 选择一份；标记和持有者必须仍是同代、存活且可参与的敌对对象。重复请求沿现有协议去重，普攻、取消准备、后续引导和重施放不进入该入口。

- `interrupt`：消耗标记、施加后续状态，原技能不落账、不进入冷却、不创建主效果。若后续状态在消费后的预检无法通过，则保留标记并按原路径处理这次施法。
- `allow_and_apply`：消耗标记、施加后续状态，原技能正常提交。
- 已有控制免疫：消耗标记、原技能正常提交，无后续状态。持续免疫仍保留；仅有首控免疫时沿既有一次性耗尽规则。

私有 `cast_interception_resolved` 只发给有权接收的来源或持有成员，带 `source`、`target`（含代次）、`skill_id`、`mark_instance_id`、`mark_definition_id`、`outcome`。成员事件带 `action_id`；NPC 弹道未分配玩家动作 ID。有后续状态才带 `followup_status_id`，该值是规则请求的状态 ID，实际状态以 `status_applied` 等事件和权威视图为准。重连只恢复当前状态，旧私有事件不会重放。

[Schema](cast-interception-v1.schema.json) 定义规则和事件形状。四人样例使用 5 秒标记、700 ms 沉默，以及首领 1200 ms 内降低移速 25%；八人样例使用另一组配置。包内样例位置为 `samples/combat/interception-{a,b}.json`，用对应 `interception-catalog.json` 和 `interception-host.json` 启动现有 H5 宿主。这些样例验证通用机制和原规则数值。
