# 多人回复光球的模板倍率

机制 71 / schema 7 在启用 `drop_orb` 的 `pve.additional_templates` 条目下接受以下配置片段。

```json
{
  "drop_orb": true,
  "recovery_orb": {
    "hp_bps": 1500,
    "spirit_bps": 6000
  }
}
```

字段单位为万分比。两项都需提供，范围为 0～10000，至少一项大于零。击杀时以击杀者的两项属性上限计算整数恢复量。示例中 1000 气血、200 灵力上限生成 150 和 120 的冻结恢复量。省略模板倍率时使用 `params.orb_hp_ratio` 和 `params.orb_spirit_ratio`。

光球复用协议 6 的地面资源、归属和拾取。宿主读取拾取后本人的气血及灵力快照。能力通过 `kernel-host --sdk-info` 的 `memberRecoveryOrb` 查询。 [范围与验收](../batches/SDK-5-54-member-recovery-orb.md)
