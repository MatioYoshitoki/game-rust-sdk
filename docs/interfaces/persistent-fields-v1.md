# 固定圆形持续区域接入（SDK-5-12）

crate0.15.0 / 机制30 / schema4 / 协议3 / 单人PVE，[GF01～GF09](../batches/SDK-5-12-ground-field.md)。主副槽均支持，最多两个技能。

```json
{"mechanism":"ground_field","id":"lasting-circle","cost":12,"windup_ms":120,"recovery_ms":300,"cooldown_ms":1200,"damage_bps":3000,"range":500,"radius":80,"warning_ms":600,"duration_ms":2400,"tick_interval_ms":600,"on_hit_target_statuses":["exposed"]}
```

全部玩法数值及已有合法self/hit绑定从配置初始化。持续时间1～60000ms，间隔step_ms～60000ms且持续时间>间隔，结束前计划跳数(duration_ms-1)/tick_interval_ms为1～128。半径0.1～128、射程0.1～10000、预警0～60000ms，其余沿原两槽约束；不允许speed/脚本/周期成本/来源保留策略等额外字段。旧机制选择新类型启动拒绝。

```json
{"kind":"cast_ground","skill_id":"lasting-circle","position":[450,400]}
```

复用[严格地面目标](ground-targets-v1.md)的世界坐标、半开地图边界/可行走中心/射程/视线、准备和提交重验、错误码及冻结参数；不吸附/裁剪/导航。准备期间移动保留准备，提交时重验施法者当前位置。提交原子预检灵力、整组self状态及两地面计划总容量（单次圆形+持续区域共享），失败免扣；不误拦普攻/自身技能。成功施加self状态后冻结直接伤害ATK比例量/中心/半径/生命周期/绑定/来源/轮次，成本/CD/后摇只处理一次，不产生弹道或周期费用。

计划激活时刻=提交+预警，结束=激活+持续时间。首跳=激活+间隔，后续加固定间隔；只处理计划时刻≤当前固定步且严格<结束的周期。0预警也等待间隔。非整步周期在首个满足条件的固定步执行，有界补齐不漂移、最多128跳/区域。同一步现有单次圆形先结算，持续区域再按计划时刻/创建顺序，区域每周期按稳定怪物索引结算。

每周期取当前存活/启用、非归途/非复位倒计时、原轮次敌人，以radius+单位半径及弹道遮挡判定。每区域每周期每敌一次，多区域独立；走出可躲避，再入参与后续周期，无进出补跳，空区域也推进。直接攻击量已冻结，DEF/盾/闪避每次实时读取；合法hit状态沿原施加时攻击量捕获及重复策略，不递归生成区域、不重复动作级奖励。死亡目标不会再次命中或再掉落。

机制30增加PveView.player_fields数组：action_id/skill_id/center/radius、started_at_ms/activates_at_ms/expires_at_ms、next_tick_at_ms（最后一跳后为null）、completed_ticks/total_ticks。PlayerFieldStarted{field}、PlayerFieldTick{action_id,skill_id,tick_index,scheduled_at_ms,hits}、PlayerFieldEnded{action_ids,reason}表示权威进度；hits为真实直接接触数，闪避不计，全盾吸收的0HP损失接触仍计。结束原因expired/owner_died/terminal/closed/encounter_reset，各区域只结束一次，有界聚合并纳入关闭输出预算。

本批来源固定当前participant且失效策略固定end；未提交取消/后台/死亡/终局/关闭/实际换轮免扣，提交后移动/CancelAction/后台/同World重连保留区域与CD/后摇、不退款。死亡/终局/关闭/实际换轮停止未来周期，不回滚已发生伤害；复位倒计时敌人暂不可接触。重连保留中心/计划时刻/周期进度，旧连接拒绝、重复输入不重复建区或扣费。重启只履行旧Create/hash/金额/operation ID责任，清临时区域/准备/状态/World。旧1～29省略新字段/事件，原单次圆形与NPC时序保持；公共视图/事件枚举源码消费需适配，Client六类/协议1～3/资产/恢复schema保持。

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4185 --state .local/sdk-5-12-demo.sqlite --configs examples/field-host/catalog.json --account examples/field-host/account.json
```

[完整配置](../guides/config-cookbook.md#player-skills)arena.json)：先点选/触摸地图，Q创建持续区域；2为自身护盾/HOT。H5显示Rust预警/激活/剩余时间/周期，不自行判伤。Node只转发意图。独立包samples/fields可复制到包外使用。

本批不含治疗区域/光环、移动/跟随/附着、陷阱、其它形状、独立重复接触间隔/目标总预算、周期扣费/引导、来源失效可选策略、多人或完整M05/技能目录。
