# SDK-5-76 自身技能破甲验收

2026-11-07。SDK 0.78.0、机制 93、schema 7、协议 6。

AP01～AP05 通过。Rust 默认和全特性各通过 1700 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 92 冷却缩减复验。 [批次范围](../batches/SDK-5-76-member-armor-penetration.md) · [机器验收](../../artifacts/sdk-5-76/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证同一成员三次真实弹道普攻。四人房伤害为 66、80、66，八人房为 66、72、66。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-76/public/report.json) · [H5](../../artifacts/sdk-5-76/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-76/legacy92-browser/report.json)

Rust 专项复现原 `defense-basic` 与 `defense-pierce` 两组算例，并覆盖配置、命中读取、叠加归零、有效 DEF、小数护甲、护盾顺序、目标私有、精确到期和周期及 NPC 伤害兼容。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-76/native-verification.json)

归档 SHA-256 为 `3d382bee0f5a3fa9f1c332a73ccd8fa447664eedf67b329eb9e4396bb5fb1697`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --armor-penetration`。 [交付包](../../artifacts/sdk-5-76/delivery/match-kernel-0.78.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-76/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4336/)
