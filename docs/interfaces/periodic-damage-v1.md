# 周期物理伤害（SDK-5-5 / 0.8.0）

机制23、schema4、协议3的单人PVE使用。复用[状态生命周期](statuses-v1.md)、[属性修正](attribute-modifiers-v1.md)与[护盾](absorption-shields-v1.md)，有限范围见[PD01～PD10](../batches/SDK-5-5-periodic-damage.md)。

## 配置

在合法PVE配置内声明并绑定玩家主技能或怪物弹道的命中目标：

```json
{
  "pve": {
    "status_definitions": [
      {"id":"wound","mechanism":"periodic_damage","tick_interval_ms":600,"damage_bps":5000,"duration_ms":2200,"duplicate_policy":"refresh","source_scoped":true}
    ],
    "status_rules":{"max_active_per_entity":4,"defense_bonus_cap_bps":10000}
  },
  "skill":{"on_hit_target_statuses":["wound"]}
}
```

伤害、间隔、时长和重复/来源策略全部来自配置。damage_bps为来源有效ATK的万分比，0～100000；间隔为整数且至少一个逻辑步；duration严格大于间隔，计划次数floor((duration−1)/interval)为1～128。只接受refresh/ignore和敌对命中目标；禁止self绑定、stack/max_stacks、attribute、modifier_bps和shield_bps，包括null。原16定义、每绑定2引用、实体32槽/全局128实例上限保持。旧机制/其它定义拒绝新周期字段。

合法直接命中后施加，取消、墙挡、闪避、保护或目标已死时不施加。零HP伤害或被盾完全吸收的合法直接命中仍可施加。同组容量预检失败时整组保持原状。

## 计时与伤害

每次绑定组开始时读取来源有效ATK，冻结raw_damage=floor(ATK×damage_bps/10000)，不受同组定义排列或之后ATK变化影响。refresh保留ID、重取来源ATK、次数归零并重置完整节拍/期限；ignore保持原实例、来源和计划。来源分组由source_scoped决定。

首跳等待一个间隔。例中计划600/1200/1800三跳，0/2200不跳。到达每个计划点的首个固定步执行，下一计划仍按原节拍累计。每步先清到期/失效实例，再按计划时刻、实例ID字典序执行DOT，之后进入原移动/AI/弹道流程；该步新施加不立即跳。仍有效的逾期点按顺序补齐；到期/关闭不补历史跳。非整步末计划点若首次执行步已到期，优先到期取消，因此total_ticks为上限。

每跳不判闪避/暴击，不消耗伤害随机数；冻结物理量经过目标当前DEF、原守方常数、floor/minimum_damage，再消耗当前盾，最后扣HP。不会重复触发命中状态、回灵/剑印/吸血、反击/主动命中窗口或支援打断。实际HP伤害仍触发原HP阶段、交互/撤离中断、死亡、一次击杀/掉落链。完全吸收不会作为主动命中打断撤离。

后台、断线未就绪或保护期间消耗到期次数但跳过伤害/盾；怪物归途和遭遇复位倒计时同样跳过。恢复不补跳、不延寿。来源/目标死亡、移除、换代/复位、终局/关闭清理；本批不保留死亡来源DOT。放弃和绝对终局优先，DOT伤害先于同一步撤离完成。

## 权威输出与宿主

DOT实例的modifier_bps为0，不贡献派生属性，新增可省略periodic：

```json
{"kind":"physical_damage","interval_ms":600,"next_tick_at_ms":1200,"ticks_done":1,"total_ticks":3,"raw_damage":50}
```

next_tick_at_ms为null表示无后续计划；ticks_done包括跳过的计划点。periodic_tick事件带实例/定义/来源/目标、tick_index、scheduled_at_ms、executed_at_ms、raw_damage、absorbed、hp_damage、outcome(applied/protected/returning)。Damage.amount仍为实际HP伤害。旧19～22实例省略periodic。

Node/H5只发送意图、处理存储证明、展示进度和实际扣盾/血；不设置周期时点或计算伤害。同World重连保留实例和节拍，进度随Rust时钟继续；旧连接拒绝。进程重启只履行原冻结资产责任，清除临时DOT，不恢复World或按新配置补跳。0.8.0新增公开Rust DTO字段、事件及枚举，源码消费方需适配；六类Client、协议1/2/3、恢复schema不变。

完整[配置](../../examples/dot-host/arena.json)、[清单](../../examples/dot-host/catalog.json)、[账户](../../examples/dot-host/account.json)。实时验证：

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4178 \
  --configs examples/dot-host/catalog.json \
  --account examples/dot-host/account.json \
  --state /tmp/match-dot-lab.sqlite
```

一次施加后停手可观察周期伤害，重新命中会刷新节拍。普通交付包samples/periodic可复制到包外，由hosts/node/asset-host.mjs消费。本批只注册物理DOT；HOT、DOT叠层/其它公式、控制驱散、地形DOT及完整来源目录仍另行验收。
