# 通用ATK/DEF定时修正（SDK-5-3 / 0.6.0）

机制21、schema4、协议3的单人PVE使用。沿用[状态生命周期](statuses-v1.md)与[共享期限叠层](status-stack-v1.md)；范围与精确读取边界见[AT01～AT10](../batches/SDK-5-3-attribute-modifiers.md)。

## 配置

在已有合法PVE配置中声明：

```json
{
  "pve": {
    "status_definitions": [
      {"id":"power","mechanism":"timed_attribute_modifier","attribute":"atk","duration_ms":2400,"modifier_bps":2500,"duplicate_policy":"stack","max_stacks":3,"source_scoped":false},
      {"id":"exposure","mechanism":"timed_attribute_modifier","attribute":"def","duration_ms":2400,"modifier_bps":-2500,"duplicate_policy":"refresh","source_scoped":true}
    ],
    "status_rules":{"max_active_per_entity":4,"attack_bonus_cap_bps":10000,"defense_bonus_cap_bps":10000}
  },
  "skill":{"on_commit_self_statuses":["power"],"on_hit_target_statuses":["exposure"]}
}
```

attribute只接受atk/def；ATK定义必须提供attack_bonus_cap_bps（0～100000整数）。旧timed_defense_modifier继续隐式DEF并禁止attribute字段。旧机制禁止新机制、attribute或ATK帽，包括显式null。原倍率、时长、定义/绑定/实例数量、发布预算及stack约束保持。

每属性分别汇总所有有效实例的modifier_bps×层数，夹到[-10000,该属性正帽]，再乘基础属性并统一floor。原单层按1；读取不改基础账户/模板。有效ATK可为0，普通伤害仍遵守配置minimum_damage；临时ATK不改变守方境界防御常数、百分比治疗/回蓝等非ATK量。

## 攻击计划

玩家普攻/技能和怪物弹道在提交时读取有效ATK一次；技能先原子施加self状态再读取，因此自身增攻影响本次弹道。准备期变化影响提交，提交后状态新增/到期不会二次乘算在途攻击。DEF继续在实际命中时读取，命中后减攻影响目标的后续计划。

其它既有计划保持物化边界：近战/范围在释放时，重击在预警创建时，反击在窗口创建时，限时站位/直接命中窗口在预警创建时读取一次有效ATK。计划创建后数值固定，原取消/死亡/归位/轮次规则保持；不为读取来源而复活实体。

## DTO与宿主

机制21的Snapshot/MonsterView新增effective_atk；通用属性实例增加attribute，事件仍携带原StatusView。旧19/20实例省略attribute，旧快照省略effective_atk，原JSON形状保持。0.6.0新增公开Rust字段与StatusAttribute枚举，源码消费方须适配；六类Client调用、协议和恢复schema保持。

Node/H5仅展示属性、有效ATK/DEF、层数、来源和剩余时间，不推算伤害或直接设置属性。同World重连保留绝对截止；进程重启只恢复原资产责任，Closed视图移除临时状态及有效属性，不按新配置重算旧责任。

完整[配置](../guides/config-cookbook.md#statuses)arena.json)、[清单](../guides/config-cookbook.md#statuses)catalog.json)、[账户](../guides/config-cookbook.md#statuses)account.json)。实时验证：

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4176 \
  --configs examples/attribute-host/catalog.json \
  --account examples/attribute-host/account.json \
  --state /tmp/match-attribute-lab.sqlite
```

普通交付包的samples/attributes可复制到包外，通过hosts/node/asset-host.mjs消费。其它属性、控制/盾/DOT、多技能槽与完整状态目录继续按后续批次实现。
