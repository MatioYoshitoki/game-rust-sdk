# 方向型直线穿透接入（SDK-5-14）

crate0.17.0 / 机制32 / schema4 / 协议3 / 单人PVE。范围与验收见[PP01～PP09](../batches/SDK-5-14-piercing-projectile.md)。

```json
{"mechanism":"piercing_projectile","id":"piercing-bolt","cost":12,"windup_ms":120,"recovery_ms":300,"cooldown_ms":1200,"damage_bps":10000,"range":500,"speed":600,"radius":6,"ttl_ms":3000,"max_hits":3,"on_hit_target_statuses":["exposed"]}
```

主副槽均支持，沿用两技能上限、共享准备/后摇及独立CD。ttl_ms为2×step_ms～60000，max_hits为1～128整数；其它数值/引用沿原projectile校验。必填字段不能省略/null；禁止contact_policy、target、地面参数和未知字段。通用身份预算组件复用持续区域实现，本批固定每身份一次，不提供正重复间隔或每步配额。

Node/H5仅提交方向意图：

```json
{"kind":"cast","skill_id":"piercing-bolt","direction":[1,0]}
```

有限非零方向在接受时归一化冻结，提交取当前施法者位置；参数/绑定/轮次随接受冻结。提交检查灵力、整组self容量及最多2发新穿透在途容量，失败不产生部分状态/扣费/CD/弹道。成功self状态后冻结有效ATK，当前DEF/盾/闪避在每次命中读取。创建步不移动。移动/取消输入/后台保留已提交弹道；准备阶段沿原取消规则。

每步连续扫掠移动段，按首次接触t、ID、代次排序。墙体采用扫掠圆对阻挡格/边界的连续交点（含圆角），同距墙优先；不穿墙。每身份每发弹最多一次成功接触；闪避/失效不占预算，全盾0损失仍算命中并沿合法hit状态。达到总预算立即结束，射程端点接触可计入，TTL截止前才可移动。伤害/状态/击杀/掉落仍由Rust旧直接接触链裁决。预算、墙、射程、期限结束回收；死亡/终局/关闭/实际遭遇换轮清理所有剩余穿透弹，无退款或跨轮收益。

PveView.piercing_projectiles仅≥32输出，每发含action_id/skill_id/position/direction/radius/speed/remaining_range/expires_at_ms/max_hits/used_hits/remaining_hits与contacts。历史按ID/代次排序，每条target和hit_at_ms，最多128条。PiercingStarted建立、PiercingHit提供接触点及余量、PiercingEnded按action_ids和reason结束；reason为hit_budget_exhausted/wall/range_exhausted/expired/owner_died/terminal/closed/encounter_reset。公开DTO/事件枚举增加分支，源码消费者需适配，旧协议/机制输出保持。

普通JSONL的submit_input响应在ok.status中返回当前logical_ms。自动消费方应等待该接受时刻之后的新快照，再按pending_action/共享后摇/自己的CD决定下一次施法；不能仅从可能滞后的旧快照递推节流时间。已接受的操作不换序号重发。

同World重连保留真实位置/方向/期限/行程/历史/余量及CD；精确重复请求只执行一次，旧连接隔离。重启不恢复战斗飞行/历史，仅处理原Create/hash/金额/operation ID的冻结资产责任。

H5点击或触摸地图设方向，未选择时禁用穿透技能；准备中改选不改变已接受方向。画面只展示Rust快照，不裁决碰撞或伤害。演示：

```sh
cargo build --locked --offline --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4187 --configs examples/piercing-host/catalog.json --account examples/piercing-host/account.json --state .local/sdk-5-14-demo.sqlite
node adapters/node/asset-host.mjs --binary target/debug/kernel-host --configs examples/piercing-host/catalog.json --account examples/piercing-host/account.json --extraction
```

[交付包指南](../guides/sdk-delivery.md)中的普通二进制不含replay特性，samples/piercing可复制到包外更换所有数值/绑定。
