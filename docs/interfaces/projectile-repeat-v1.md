# 移动弹道重复命中接入（SDK-5-15）

crate0.18.0 / 机制33 / schema4 / 协议3 / 单人PVE。[需求与RP01～RP09](../batches/SDK-5-15-projectile-repeat.md)。

```json
{"mechanism":"piercing_projectile","id":"repeating-bolt","cost":12,"windup_ms":120,"recovery_ms":300,"cooldown_ms":1200,"damage_bps":7500,"range":500,"speed":20,"radius":42,"ttl_ms":22000,"max_hits":4,"repeat_interval_ms":600,"on_hit_target_statuses":["exposed"]}
```

repeat_interval_ms可省略；显式提供仅机制≥33的piercing_projectile支持，0～60000整数，null/负数/小数及其它技能顶层携带均拒绝。0为每身份每发一次；正数为最新成功接触+间隔（等值合法）。省略时继续[SDK-5-14](piercing-projectiles-v1.md)原语义及输出，主副槽均可，所有数值与绑定由配置初始化。

复用方向Cast、提交后冻结ATK、原子SP/self/两发容量与独立CD/共享后摇。命中频率不创建周期伤害；每步移动段必须真实接触当前合法目标，每身份每步最多尝试一次。小于step的正间隔不会同一步多发；不整步在下一接触固定步生效。离开再入不清时钟，未接触时错过的间隔不补跳。身份为本World的ID/代次，新代独立，各发弹独立。

原max_hits同时计首次和重复成功接触，总成功上限128。闪避/失效/墙/未到期不耗不刷新时间，0HP损失/全盾吸收仍耗一次且沿原hit绑定。按当前DEF/盾等结算，合法状态刷新/忽略由原定义决定，不因附带伤害重复计预算。预算0立即结束，死亡/终局/关闭/实际换轮清理；射程/墙/TTL截止与原规则相同。重复接触不再扣SP或开CD。

仅显式配置时，PiercingProjectileView.repeat_interval_ms输出配置值；contacts[].hit_at_ms为最新成功时刻，正间隔另有next_eligible_at_ms，表示资格门槛而非预约命中。PiercingHit.hit_at_ms输出本次实际固定步逻辑毫秒。used_hits累计成功次数，contacts仅保存每身份最新成功，故重复时contacts.length可小于used_hits。旧1～32及省略配置的视图/事件省略新增字段；源码消费需适配公开DTO/枚举，Client六类和协议1～3保持。

Node/H5只提交意图，Rust裁决碰撞、计时和伤害。普通JSONL返回ok.status.logical_ms，自动消费方等待接受时刻后的新快照，再按准备/CD继续。已接受原请求只精确去重，旧连接明确拒绝的精确重复可验证新代后继续，禁止换序号重发已接受操作。同World重连保留位置/TTL/行程/历史/余量与间隔；进程重启仅履行原Create/hash/金额/operation ID，不恢复弹道或重复时钟。

```sh
cargo build --locked --offline --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4188 --configs examples/repeat-host/catalog.json --account examples/repeat-host/account.json --state .local/sdk-5-15-demo.sqlite
node adapters/node/asset-host.mjs --binary target/debug/kernel-host --configs examples/repeat-host/catalog.json --account examples/repeat-host/account.json --extraction
```

H5点选/触摸地图设方向，显示Rust间隔/最新接触/下一资格/预算，不在前端模拟重复伤害。[独立交付](../guides/sdk-delivery.md)提供无replay普通二进制与samples/projectile-repeat；复制到包外可更换数值/ID/主副位置/绑定。
