# SDK-5-97 死源弹道命中结算（6B 尾巴收口）

SDK 0.183.0，机制 105，schema 7，协议 6。SDK-5-96 登记的 6B 尾巴（`end_on_source_invalid` 弹道保留飞行但命中结算被死源校验拦截）收口。 [裁定记录](../decisions/SDK-5-95-divergence-arbitration.md) · [前批证据](SDK-5-96-divergence-resolution-2026-11-12.md)

| 改动 | 内容 |
|---|---|
| `DamageMode::DetachedShot` | 新增投递模式：跳过「来源必须是有效状态实体」的校验（命中链其余部分——护盾、减伤、免疫、暴击、归功——全部照走）。 |
| `DirectContact.dead_source` | 接触意图携带死源豁免位；`resolve_detached_damage` 公共入口把带位接触送入 DetachedShot 投递。 |
| `direct_contact_eligible` | 豁免位同步穿过接触资格门槛（来源存活子句按位跳过；所有权、敌对与目标资格不变）。 |
| 分发点 | `advance_shots` 命中分发以 `Shot.end_on_source_invalid` 填充豁免位——只有配置开启的技能弹道受益，默认路径与所有既有调用点（12 处接触、2 处外部资格调用）固定 `false`，行为不变。 |

S13 翻转为与源 oracle 完全一致：`end_on_source_invalid=true` 时成员退场后冻结载荷继续飞行并命中（B 900，命中消耗弹道）；`false` 时随源清除（B 1000）。

门禁：World 层 22 + 协调器 38 场景测试全绿；合成 122、Node/H5 246、格式、Clippy 全目标全特性、数据目录全量 186 过且失败集与基线一致；真实内核与最小宿主通过。八处分歧至此全部闭合且无开放尾巴。
