# 配置化玩家双技能接入（SDK-5-8）

crate0.11.0 / 机制26 / schema4 / 协议3 / 单人PVE。规格与验收见[本批范围](../batches/SDK-5-8-player-skills.md)。

保留必填`skill`主技能，可省略`additional_skills`；显式提供时数组须恰有一个SkillConfig对象，最多两项，ID唯一。每项使用已有projectile机制、windup_ms/recovery_ms/cooldown_ms/cost/damage_bps/range/speed/radius和on_commit_self_statuses/on_hit_target_statuses。参数/引用/绑定/未知字段与主技能同样严格校验，状态容量在提交时原子预检。null、空数组、多项及旧机制携带该字段拒绝。只有单技能的机制26仍须schema4/PVE。

完整可复制配置见[arena.json](../guides/config-cookbook.md#player-skills)arena.json)，账户与清单在同目录。示例主技能ward-bolt产生护盾，附加renewal-bolt产生HOT；这是数据组合，SDK没有按示例ID分支。

```json
{"kind":"cast","skill_id":"renewal-bolt","direction":[1,0]}
```

宿主仅提交意图。Rust接受时冻结所选定义、瞄准和遭遇轮次；成功提交才扣该技能成本、设置自身CD及共享后摇，弹道按当时有效ATK冻结。准备中或共享后摇内的新普攻/任意技能为INVALID_STATE；CD边界相等可用，未知ID为UNSUPPORTED，灵力不足为NO_RESOURCE。状态容量不足以ACTION_REJECTED/CAPACITY拒绝，不扣费或开CD。取消准备不消费资源；提交后取消不撤销弹道、不退款、不缩短CD/后摇。移动、导航、前后台和重连不提供后摇捷径。遭遇实际换轮取消未提交动作且不扣费/开CD；复位倒计时在换轮前取消则保留同轮准备。在途弹道仍沿原轮次资格命中。

Snapshot机制26增加：

```json
{"skill_ready_at_ms":1920,"player_skills":[{"skill_id":"ward-bolt","ready_at_ms":1920},{"skill_id":"renewal-bolt","ready_at_ms":3300}],"pending_action":{"skill_id":"renewal-bolt","commits_at_ms":600}}
```

主技能在前，旧别名只对应主技能。pending_action仅准备时出现，普攻的skill_id为null；提交后消失。recovery_until_ms为共享后摇绝对期限。ActionCommitted保持原skill布尔/spirit_cost，并仅为机制26技能提交添加skill_id。显示读取权威时间/资源；客户端不得自行启动CD、扣灵力或计算HP。HTTP接受回执不等于动作已成功提交，须读取事件。

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4181 --state .local/sdk-5-8-demo.sqlite --configs examples/dual-skill-host/catalog.json --account examples/dual-skill-host/account.json
```

H5显示主技能Q与第二技能2，触屏使用对应按钮，独立CD与共享准备/后摇来自快照。Node参考入口为`adapters/node/asset-host.mjs`。独立交付包将本例放在`samples/player-skills`，可复制到包外消费。

机制1～25保持原配置和输出形状，包括原允许取消后摇的动作策略；资产/协议1～3/恢复schema1保持。机制26是显式选择新策略，不能把旧配置原文悄悄升版；老责任保留原Create/hash与金额。进程重启不恢复World、CD表或准备动作，仅履行冻结资产责任；Closed临时字段清除。

本批仅1～2个方向弹道技能、既有绑定。不包含任意技能数、严格单位/地面/自身目标、引导/蓄力/再施放、多技能AI或完整技能目录。
