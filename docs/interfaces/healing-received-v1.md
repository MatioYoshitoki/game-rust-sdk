# 受治疗修正（SDK-5-7 / 0.10.0）

机制25、schema4/协议3、单人PVE。复用[定时状态](statuses-v1.md)、[属性修正](attribute-modifiers-v1.md)和[HOT](periodic-healing-v1.md)，有限范围见[HM01～HM10](../batches/SDK-5-7-healing-received.md)。

## 配置

```json
{
  "pve": {
    "status_definitions": [
      {"id":"care","mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":-5000,"duration_ms":3600,"duplicate_policy":"refresh","source_scoped":true}
    ],
    "status_rules":{"max_active_per_entity":4,"defense_bonus_cap_bps":10000}
  },
  "skill":{"on_commit_self_statuses":["care"]}
}
```

modifier_bps为受治疗变化的万分比，整数−10000～100000。允许已有self提交和hit-target绑定；来源分组、refresh/ignore、期限和清理沿原状态规则。此选择器只接受refresh/ignore，禁止stack/max_stacks（含null）；不得混入heal_bps、damage_bps、shield_bps或未知字段。旧机制禁止此选择器，不设新的healing_cap字段。定义、绑定、实例槽和发布预算保持原上限。

## 结算

HOT、怪物预算自疗、支援每份治疗都读取目标当时全部有效修正：

```text
multiplier_bps = max(0, 10000 + sum(modifier_bps))
adjusted_heal = floor(raw_heal * multiplier_bps / 10000)
hp_restored = min(adjusted_heal, max(0, maxHp - hp))
```

只回复存活目标。raw100在−50%/+50%下为50/150，−50%与+25%合并为75%；总削弱至−100%及以下为0。没有额外正向玩法帽，数值仍受单定义范围和有限实例槽控制；宽整数计算调整量再裁剪到HP缺口。满血、零量不套伤害保底，不转盾/灵力，也不把超上限HP倒扣。

基础量沿原冻结规则：HOT基础最大HP比例、引导总预算和份额分配都保持。倍率只在实际治疗时应用一次；总预算100分三份34/33/33后各乘−50%，得到17/16/16。修正到期后后续份额立即恢复，不重建HOT或引导，不重扣成本/CD。相同截止先清到期；DOT同计划时刻仍先于HOT，致死不复活。

自然恢复、回复光球、归位满血和其它恢复路径保持原公式。新乘区不触发伤害/主动命中/吸血/反击，不中断交互/撤离/支援、不回退Boss阶段或额外消费随机数；各治疗路径沿原资格和中断规则执行。

## 输出与恢复

机制25 Snapshot/MonsterView提供healing_received_bps，无修正为10000。状态实例attribute为healing_received。每次合法治疗尝试新增heal_resolved事件，含source/target实体身份、heal_kind(hot/self_channel/support_channel)、raw_heal、multiplier_bps、adjusted_heal(u64)、hp_restored。HOT原periodic_heal_tick及MonsterHealed/SupportPulse保持原字段并提供相同实际回复；旧1～24不增加新事件或字段。

Node/H5只显示权威基础量、倍率、调整量和实回，不补算HP。重连保持实例与原期限，旧连接拒绝；来源/目标失效、归位/复位和关闭清理。重启仅恢复冻结资产责任，删除临时状态及healing_received_bps，不恢复World或按新配置重算旧金额。0.10.0包含公开DTO/枚举源码变更，六类Client、协议1～3、资产/恢复schema保持。

完整[配置](../../examples/heal-modifier-host/arena.json)、[清单](../../examples/heal-modifier-host/catalog.json)、[账户](../../examples/heal-modifier-host/account.json)。实时验证：

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4180 \
  --configs examples/heal-modifier-host/catalog.json \
  --account examples/heal-modifier-host/account.json \
  --state /tmp/match-heal-modifier-lab.sqlite
```

先受伤，再施放技能并停手观察受治疗倍率和每跳回复；修正先于HOT到期，后续跳恢复100%倍率。普通交付包samples/healing-modifiers可复制到包外消费。本批不覆盖恢复强化、其它治疗公式/复杂叠层、友军目标、过量转盾或完整来源状态目录。
