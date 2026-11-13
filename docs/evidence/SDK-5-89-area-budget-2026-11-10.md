# DELIVERY-02 区域发布预算首项验收

2026-11-10，R3.4，覆盖 WE01 的陷阱接触与来源回收、WE02 的区域预算。普通圆形技能、陷阱和纯施法锚点共用区域池。验证器按陷阱实际使用的接触策略计入单步上限，纯锚点只计区域槽位。 [验收规格](../batches/SDK-5-89-world-effects-and-summons.md)

- 合成 World 有两名成员、三个 NPC 和两个区域槽。陷阱单步最多接触一个 NPC，区域发布容量 152 可通过装载，151 以 `CONFIG_MEMBER_AREA_OUTPUT_BUDGET` 拒绝。普通无限制圆形技能在相同的 152 容量下拒绝。真实 JSONL 二进制的 `--configs` 和 `--catalog` 复验了两个容量端点。
- 固定步中，陷阱启动时只命中一个 NPC，持续留在区域内时不会再次命中。陷阱来源退出后只回收自己的计划，另一成员的锚点仍在。 [合成回归](../../src/world/two_members/mechanism_contract_tests.rs)
- Rust 默认与全特性各 11 项通过，格式检查和 Clippy 通过。召唤生命周期与 DELIVERY-02 整体公开接入继续按 WE03、WE04 验收。
