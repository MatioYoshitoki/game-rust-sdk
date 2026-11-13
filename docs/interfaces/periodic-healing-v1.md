# 周期治疗HOT（SDK-5-6 / 0.9.0）

机制24、schema4/协议3、单人PVE。复用[周期调度](periodic-damage-v1.md)和[状态生命周期](statuses-v1.md)，有限范围见[HT01～HT10](../batches/SDK-5-6-periodic-healing.md)。

## 配置与授予

在合法PVE配置内定义自身HOT并绑定技能提交：

```json
{
  "pve": {
    "status_definitions": [
      {"id":"renewal","mechanism":"periodic_heal","heal_bps":1000,"tick_interval_ms":600,"duration_ms":2200,"duplicate_policy":"refresh","source_scoped":true}
    ],
    "status_rules":{"max_active_per_entity":4,"defense_bonus_cap_bps":10000}
  },
  "skill":{"on_commit_self_statuses":["renewal"]}
}
```

玩家主技能与怪物弹道均可在提交时向自己施加。只支持self提交，禁止hit-target或任意友军/单位目标；取消准备不施加。沿原成本/CD及整组容量原子预检，重复输入/回执不重复授予。

heal_bps为基础最大HP的万分比，整数0～10000。在施加组开始时冻结raw_heal=floor(maxHp×heal_bps/10000)，不读取ATK/DEF或minimum_damage。refresh保留ID、重取基础量并重置全部节拍/期限/次数；ignore保持原实例参数。拒绝damage_bps、modifier_bps、attribute、shield_bps、max_stacks（包括null）或stack；旧机制/其它定义禁止heal_bps。上限仍为16定义、绑定2引用、实体32槽/全局128实例。

## 调度与实际回复

首跳等待一个间隔，随后按原计划累计；interval至少一个逻辑步，duration严格大于interval，计划次数floor((duration−1)/interval)为1～128。例中600/1200/1800三跳，0/2200不跳。非整步在首次达到的固定步执行，下一计划不漂移；若执行步已到期则优先取消，total_ticks是上限。仍有效的逾期计划有界补齐，到期/关闭不补历史跳。

统一队列按计划时刻、效果优先级、实例ID字典序执行。相同计划时刻先DOT后HOT，致死DOT清理状态后HOT不能救回；不同计划时刻按原时间先后。绝对终局/主动放弃优先，周期在原AI/动作/弹道与本步撤离完成之前执行。

实际回复=min(raw_heal,max(0,当前maxHp−当前HP))，只作用于存活者。maxHp1000/raw100/HP850依次实回100/50/0。满血或零量消耗计划并记录0，不把过量转成盾/MP，也不会倒扣超过上限的HP。HOT不触发伤害、主动命中、吸血/回灵/剑印/反击，不消费随机数，不中断交互/撤离/支援或回退Boss阶段。

后台/保护/断线未就绪、怪物归途/复位倒计时消耗计划但跳过回复，不延寿或补跳。死亡/移除/换代/归位/复位、终局与关闭沿统一状态清理。

## 权威输出与恢复

HOT的periodic视图：

```json
{"kind":"max_hp_heal","interval_ms":600,"next_tick_at_ms":1200,"ticks_done":1,"total_ticks":3,"raw_heal":100}
```

next为null表示无后续计划；ticks_done包括保护/归途跳过的点。HOT省略raw_damage，DOT保留旧raw_damage（包括0）并省略raw_heal，旧19～22继续省略periodic。HOT的modifier_bps=0，不贡献派生属性。

periodic_heal_tick事件提供instance_id、definition_id、source、target、tick_index、scheduled_at_ms、executed_at_ms、raw_heal、hp_restored、outcome(applied/protected/returning)。满血是applied/0，跳过为相应原因/0。Node/H5只展示进度和实际回复，不补算HP或驱动时点。

同World重连保持实例/原期限/节拍，旧连接拒绝；重启只履行原冻结资产责任，移除HOT/DOT，不恢复World或按新配置/墙钟补跳。0.9.0新增公开Rust DTO字段和枚举分支；六类Client、协议1～3、资产和恢复schema保持。

完整[配置](../guides/config-cookbook.md#statuses)arena.json)、[清单](../guides/config-cookbook.md#statuses)catalog.json)、[账户](../guides/config-cookbook.md#statuses)account.json)。实时验证：

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4179 \
  --configs examples/hot-host/catalog.json \
  --account examples/hot-host/account.json \
  --state /tmp/match-hot-lab.sqlite
```

先让怪物造成伤害，再用技能施加HOT并停手观察。普通交付包samples/healing可复制到包外，用hosts/node/asset-host.mjs完成消费。治疗强化/衰减、其它公式/叠层、友军目标、过量转盾及完整状态目录另行注册验收。
