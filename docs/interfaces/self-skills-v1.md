# 自身施法接入（SDK-5-9）

crate0.12.0 / 机制27 / schema4 / 协议3 / 单人PVE。范围及SS01～SS10见[本批需求](../batches/SDK-5-9-self-skills.md)。

主skill与唯一可选additional_skills均支持self_status，与原projectile可交换槽位，总技能数仍1～2。

```json
{"mechanism":"self_status","id":"renewing-ward","cost":8,"windup_ms":120,"recovery_ms":240,"cooldown_ms":2200,"on_commit_self_statuses":["ward","renewal"]}
```

每项数值均由配置初始化，成本/时间/ID上限沿原技能约束。绑定必须非空、去重、最多2个已登记且允许self的状态；复用ATK/DEF/受治疗修正、盾和HOT，DOT自施加禁止。self_status严格拒绝damage_bps/range/speed/radius/on_hit_target_statuses等字段（含null/空数组），不通过虚设弹道参数表达无弹道技能。未知/混合字段、缺参、非法数字/引用、旧机制选择self_status均启动拒绝。原projectile的字段及必填项保持。

```json
{"kind":"cast_self","skill_id":"renewing-ward"}
```

新输入没有方向或目标，额外字段严格拒绝。旧Cast继续必须提供合法方向；两种意图与配置机制不匹配在动作可接纳时为INVALID_MESSAGE，未知ID为UNSUPPORTED，旧机制的新意图在可接纳状态为UNSUPPORTED。未就绪/保护/死亡、共享准备/后摇、独立CD或资源不足按原门禁拒绝。

接受时冻结技能、参数、绑定与遭遇轮次；固定步到期时先预检当前灵力及整组状态容量，成功才整组生效、扣费、启动自身CD/共享后摇。满槽或资源减少拒绝时不能部分刷新/授予/扣费。即使windup_ms=0也在下一固定步提交，不在接受回执中同步改变状态。ignore沿原策略保持状态实例，但仍属于成功施法，正常消费成本/CD。自身技能不创建弹道、直接攻击、击杀、拾取或命中收益，也不消费攻击随机数。

取消/后台/死亡/终局/关闭及实际遭遇换轮释放未提交准备，不扣成本/CD；取消复位倒计时保留同轮动作。提交后取消不退款或撤销效果。移动/导航/切技能/取消/前后台/重连不能缩短已提交后摇。状态实例、HOT/盾/属性算术及到期/来源清理沿现有机制。

Snapshot沿用player_skills/pending_action及主技能skill_ready_at_ms别名；ActionCommitted使用skill_id、skill=true和spirit_cost。H5/Node按配置mechanism选择意图，读取Rust权威状态/CD/资源。重连保持同World已提交状态/CD/后摇及身份，准备沿原连接策略中断。进程重启只核对原冻结资产，清除临时状态/技能视图，不恢复World，不用新配置重算旧金额。

完整样例见[arena.json](../guides/config-cookbook.md#player-skills)arena.json)。其护盾/HOT组合属于数据，不是地图专用代码。

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4182 --state .local/sdk-5-9-demo.sqlite --configs examples/self-skill-host/catalog.json --account examples/self-skill-host/account.json
```

Q/2或对应触屏按钮按槽位施放，界面标注“自身施法”；自身技能不依赖瞄准或附近敌人。普通Node参考为adapters/node/asset-host.mjs；独立包样例位于samples/self-skills，可复制到包外消费。脚本玩家只保证所交付有限战斗样例，不保证可通关任意配置。

本批不含严格单位/地面目标、更多技能数、引导/蓄力/再施放、多技能AI、新状态族、HP成本、多人或完整技能目录。
