# 定时防御状态（SDK-5-1 / 0.4.0）

机制19、schema4、协议3 的单人PVE使用本能力；机制20的stack扩展另见[共享期限DEF叠层](status-stack-v1.md)；本页保留机制19规则。六类Client调用、资产/结果正文和恢复schema1保持。实现与验收范围见[ST01～ST12](../batches/SDK-5-1-timed-status.md)。

## 配置

完整样例：[arena.json](../guides/config-cookbook.md#statuses)arena.json)、[清单](../guides/config-cookbook.md#statuses)catalog.json)、[账户](../guides/config-cookbook.md#statuses)account.json)。已有配置的增量片段：

```json
{
  "pve": {
    "status_definitions": [
      {"id":"guard","mechanism":"timed_defense_modifier","duration_ms":1200,"modifier_bps":10000,"duplicate_policy":"refresh","source_scoped":false},
      {"id":"exposed","mechanism":"timed_defense_modifier","duration_ms":1800,"modifier_bps":-5000,"duplicate_policy":"ignore","source_scoped":true}
    ],
    "status_rules":{"max_active_per_entity":4,"defense_bonus_cap_bps":10000}
  },
  "skill": {"on_commit_self_statuses":["guard"],"on_hit_target_statuses":["exposed"]}
}
```

两个绑定也可用于基础及additional_templates的combat.projectile。最多16定义、每绑定最多2个不重复引用、每实体1～32实例；全部预留实体（含玩家）×实例上限≤128。ID为1～64 UTF-8字节且无控制字符；duration_ms为step_ms～min(terminal_ms,3600000)；modifier_bps为-10000～100000整数，正修正合计帽为0～100000。未知字段/触发/属性/stack、坏引用、非PVE和旧版本携带新字段（即使空数组）加载失败。

## 权威语义

`effective_def = floor(base_def × (10000 + clamp(有效modifier_bps之和,-10000,defense_bonus_cap_bps)) / 10000)`。基础属性保持原值，各物理伤害路径统一读取有效DEF；防御提升不吸收HP伤害。时间区间为[start,end)，同刻先到期再处理动作/命中。

refresh保留instance_id并更新本次来源/开始/截止；ignore保留原实例全部参数/时点。source_scoped=true按来源身份分组，否则同定义跨来源合并。仅单层。提交前取消不产生状态；self绑定整组容量不足时动作失败且不扣成本/CD，命中绑定不足时整组拒绝并保留已有伤害。施加当次使用旧DEF；闪避/保护/墙挡不施加，合法零伤害接触可施加，致死目标不施加。重复输入/传输回执不重复生效。

身份含kind(participant/monster)、id、generation与可选encounter_round。来源/目标死亡、移除/换代、真正归位和整组复位清理对应实例；旧轮弹道不能向新轮施加。后台/断线/同World换连接保留实例和绝对截止，逻辑时间继续；终局/关闭释放全部状态。

## 输出与接入

Snapshot及MonsterView提供可省略的statuses、effective_def，旧机制不输出新字段。每状态含instance_id、definition_id、source/target、applied_at_ms、expires_at_ms、modifier_bps，按instance_id排序。事件为status_applied/refreshed/ignored/rejected/removed及提交容量不足的action_rejected；移除按expired/target_invalid/source_invalid/reset/closed原因聚合instance_ids。

Node/H5仅展示输出和转交原操作。重启后的Closed视图移除临时状态；恢复检查点不保存战斗实例，重启只履行原资产责任，不重建状态/World或重算金额。

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4174 \
  --configs examples/status-host/catalog.json \
  --account examples/status-host/account.json \
  --state /tmp/match-status-lab.sqlite
```

页面选择实时模式，Node不会传加速标志；普通交付二进制不编译replay。账户是本地受控测试事实，真实认证/库存仍须独立验收。本批不签收完整buff/技能目录或生产存储。
