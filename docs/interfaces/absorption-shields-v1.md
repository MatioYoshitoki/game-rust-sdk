# 吸收护盾（SDK-5-4 / 0.7.0）

机制22、schema4、协议3的单人PVE使用。复用[状态生命周期](statuses-v1.md)，范围与边界见[SH01～SH10](../batches/SDK-5-4-absorption-shields.md)。

## 配置与授予

在合法PVE配置内声明：

```json
{
  "pve": {
    "status_definitions": [
      {"id":"ward","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":4000,"duplicate_policy":"refresh","source_scoped":false}
    ],
    "status_rules":{"max_active_per_entity":4,"defense_bonus_cap_bps":10000,"shield_cap_bps":3000}
  },
  "skill":{"on_commit_self_statuses":["ward"]}
}
```

shield_bps为目标基础maxHp的万分比，1～10000；本批只有这一种盾量公式。shield_cap_bps为每目标合计盾帽，0～10000；常规来源样例用3000。首次盾量=min(floor(maxHp×shield_bps/10000),合计帽−其它组剩余量)。只裁剪本组，不倒扣其它来源。

refresh保留实例ID、按上述可用量补足本组并更新来源/期限；ignore原样保持部分消耗量、来源和期限。破盾立即移除，下次授予新ID。零授予不创建实例/期限；绑定整组先按原槽位规则预检，容量失败不部分刷新或扣成本/CD，合法零授予仍付原成本/CD。

盾定义不接受attribute、modifier_bps、max_stacks或stack；旧属性定义禁止shield_bps，旧机制禁止新字段（包括null）。仍限16定义、每绑定2引用、每实体最多32实例、预留实体×实例上限≤128，旧时长与ID约束保持。

## 消耗和命中

先完成原资格/闪避/ATK/DEF/暴击/元素/取整/minimum_damage，再按截止时间、applied_at_ms、稳定实例ID字典序消耗有效盾，最后把剩余伤害夹到HP。不能先按HP截断超杀伤害再扣盾。盾60受100伤害：吸收60、HP扣40；盾60受40：剩20、HP扣0。

Damage.amount继续表示实际HP伤害，完全吸收为0。shield_absorbed事件给出source、target实体身份、吸收合计和逐组consumptions `{instance_id,absorbed,remaining}`；该事件在对应Damage之前输出。盾耗尽发status_removed/depleted。shield_granted的requested为计算请求量、granted为本次实际新增量、remaining为授予后余额，零授予的instance_id为null；状态视图shield `{granted,remaining}`中的granted为本次裁剪后的完整额度。

合法接触触发与HP伤害触发保持分离：完全吸收仍可施加命中状态、打断支援/撤离或触发反击；吸血、血量阶段与原正HP条件使用实际扣血。零伤害、墙挡/闪避/保护不扣盾，重复输入/回执不重复授予或消费。放弃与绝对终局仍按原规则关闭，不受护盾阻止。

## 输出与接入

机制22的Snapshot/MonsterView提供shield_total（无盾为0），盾实例增加shield；旧机制/旧实例省略新字段。盾实例modifier_bps=0，不贡献ATK/DEF派生。0.7.0新增公开Rust字段、事件及depleted枚举分支，源码消费方须适配；六类Client、协议1/2/3、资产/结果/恢复契约保持。

Node/H5只转交意图、存储证明和展示权威盾量、吸收/HP事件。没有直接设置盾量入口。前后台/重连保留余额与绝对截止；到期、来源/目标失效、归位/复位、关闭清理。进程重启只履行原资产责任，Closed视图移除临时状态和shield_total，不恢复World或按新配置重算旧责任。

完整[配置](../guides/config-cookbook.md#statuses)arena.json)、[清单](../guides/config-cookbook.md#statuses)catalog.json)、[账户](../guides/config-cookbook.md#statuses)account.json)。实时验证：

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4177 \
  --configs examples/shield-host/catalog.json \
  --account examples/shield-host/account.json \
  --state /tmp/match-shield-lab.sqlite
```

普通交付包samples/shields可复制到包外，由hosts/node/asset-host.mjs消费。该有限切片不覆盖PvP/秘境额外容量、反伤、驱散、控制、DOT/HOT及完整状态目录。
