# 共享期限DEF叠层（SDK-5-2 / 0.5.0）

机制20、schema4、协议3、单人PVE。沿用[定时DEF状态](statuses-v1.md)的定义、触发、身份、容量、到期、清理与资产责任。验收范围为[SK01～SK10](../batches/SDK-5-2-status-stack.md)。

在已通过校验的状态配置中使用：

```json
{"id":"guard","mechanism":"timed_defense_modifier","duration_ms":2400,"modifier_bps":2500,"duplicate_policy":"stack","source_scoped":false,"max_stacks":3}
```

`modifier_bps`为每层DEF修正；`duration_ms`为整组时长。首次一层，合法重施加一层至上限，每次重施都刷新整组期限（封顶亦然），保持instance_id，全部层到期共同移除。max_stacks必须是1～128整数，refresh/ignore禁止携带该字段，机制19禁止stack和max_stacks。未开放逐层计时或其它叠加策略。

`effective_def = floor(base_def × (10000 + clamp(Σ(modifier_bps × stacks),-10000,defense_bonus_cap_bps)) / 10000)`。先汇总再一次取整，施加当次伤害仍读取旧状态，禁止宿主逐层取整或改永久属性。旧单层实例按一层计算。

source_scoped=true按来源实体身份各自叠层；false同定义合并，每次合法重施更新最新来源，整组随该来源失效清理。一个叠层组占一个实例槽与期限，不按层占槽。混合绑定容量不足不部分加层/刷新/换来源；self失败不扣成本/CD，命中失败保留已发生伤害。

stack实例增加`stacks`、`max_stacks`，旧refresh/ignore的JSON省略它们。首次沿用status_applied；重施输出`status_stacked {status,added_stacks}`，封顶为0。Node/H5显示层数/上限、来源、有效DEF与截止，不能直接施加/改层数或推动时钟。0.5.0新增公开Rust字段与Event变体，源码消费方适配匹配分支；六类Client调用、协议1/2/3和恢复schema1保持。

完整[配置](../guides/config-cookbook.md#statuses)arena.json)、[清单](../guides/config-cookbook.md#statuses)catalog.json)、[账户事实](../guides/config-cookbook.md#statuses)account.json)：

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4175 \
  --configs examples/stack-host/catalog.json \
  --account examples/stack-host/account.json \
  --state /tmp/match-stack-lab.sqlite
```

选择实时模式，连续施放主技能即可看到层数增长，停止后整组到期。后台/断线和同World重连保留原期限；进程重启只恢复资产责任，不恢复战斗层数。普通交付二进制未编译replay，包内samples/stacks可复制到包外由普通Node参考宿主消费。

这只是DEF状态族的有限叠层策略。其它属性、控制、护盾、DOT/HOT及完整86状态内容仍有缺口，不因本批签收视为完成。
