# 命中预算与重复接触策略接入（SDK-5-13）

crate0.16.0 / 机制31 / schema4 / 协议3 / 单人PVE。[CP01～CP09](../batches/SDK-5-13-contact-policy.md)，通用身份/时间/计数组件首批接入ground_field，沿现有两槽/两地面计划。

```json
{"mechanism":"ground_field","id":"budget-circle","cost":12,"windup_ms":120,"recovery_ms":300,"cooldown_ms":1200,"damage_bps":7500,"range":500,"radius":80,"warning_ms":600,"duration_ms":2400,"tick_interval_ms":600,"contact_policy":{"max_targets_per_tick":1,"max_total_hits":3,"repeat_interval_ms":1200},"on_hit_target_statuses":["exposed"]}
```

contact_policy可省略；一旦提供必须包含三项整数，不能null/不完整/额外字段。max_targets_per_tick为1～32，max_total_hits为1～128，repeat_interval_ms为0～60000。仅≥31、schema4/PVE的ground_field支持，主副槽均可；其它类型和旧版本携带新字段启动拒绝。省略时保持[SDK-5-12](persistent-fields-v1.md)原规范化/输出/稳定索引顺序，每周期接触所有合法目标，无本批预算历史。全部数值/绑定由配置初始化。

沿用[CastGround严格世界坐标](ground-targets-v1.md)、接受/提交重验和原子门禁。预算/间隔随接受的技能冻结，成功提交才初始化区域和计数，成本/CD/后摇只处理一次。每周期上限计成功直接接触；全区域总上限亦计成功直接接触。只有原participant Damage事件标识真实接触：闪避/遮挡/目标不合法/重复冷却不耗额度和历史，继续检查后续候选；全盾吸收或原最低伤害为0产生的0HP损失仍消耗一次并沿原合法hit状态。击杀只算一次，附带DOT/状态/反击/阶段追加不重复扣预算，状态容量失败不会退还已发生直接接触预算。

配置策略的区域每周期按当前到中心的距离平方、稳定目标ID字节序、代次（最后索引兜底）依序尝试；每目标同周期最多一次，达到周期配额或总预算0立即停止剩余候选及状态。无策略保留旧索引顺序；同一步原单次圆形优先，区域依计划时刻/创建顺序，多区域独立历史/预算。

历史按当前World的{id,generation}记录。repeat_interval_ms=0：同一身份每区域只允许一次成功接触；正数：本次scheduled_at_ms≥上次成功接触的计划时刻+间隔时可重试，等于合法。独立于扫描tick_interval_ms，不以实际晚到时刻累加，不因离开/再入/改选或重连清历史。闪避不开始重复冷却。新代为新身份，不复用旧代记录；每区域历史至多128条，保留至区域结束。

预算0时发最后一次PlayerFieldTick，再以PlayerFieldEnded{reason:"hit_budget_exhausted"}结束并释放区域/历史，后续无周期或额外目标接触。最后合法周期在到期后补跑并耗尽预算时仍以预算原因结束，不再重复expired。未耗尽则沿原到期；移动/取消/后台/来源死亡/终局/关闭/实际换轮、恢复和资产责任沿原规则，不退款/回滚已发生效果。

仅配置策略的PlayerFieldView增加contact_policy，含冻结三项配置、used_hits/remaining_hits、contacts数组（稳定ID/代次顺序）。每项为target{id,generation}、last_hit_at_ms、next_eligible_at_ms；一次性规则next为null，正间隔为计划下一资格时刻。PlayerFieldTick仅策略区域增加hit_targets（成功接触顺序）和remaining_hits。原字段/事件保持，省略策略及旧1～30无新字段；公共DTO/枚举源码消费需适配，Client六类/协议1～3/资产/恢复schema保持。

同World重连保留历史和余量，准备沿原策略取消；重复输入不重复建区、扣成本或预算，旧连接拒绝。死亡/终局/关局/实际换轮移除区域/历史；进程重启仅履行旧Create/hash/金额/operation ID责任，不恢复临时区域/历史/预算/战斗或重算旧金额。

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4186 --state .local/sdk-5-13-demo.sqlite --configs examples/contact-host/catalog.json --account examples/contact-host/account.json
```

[完整配置](../guides/config-cookbook.md#statuses)arena.json)：点选/触摸地图后Q建区，2自身状态。示例每周期最多1目标、总共3次接触、同一身份间隔1200ms；H5显示Rust预算和历史，Node仅提交坐标并记录Rust目标/余量，客户端不裁决接触。独立包samples/contacts可复制到包外，验收包含一次性与正间隔重复两套配置。

本批不含弹道/陷阱/其它形状的策略接入、来源保留可选策略、周期费用/引导、多人、完整接触家族/器型/技能目录或完整M05。
