# SDK-5-77 自身技能暴击率验收

2026-11-07。SDK 0.79.0、机制 94、schema 7、协议 6。

CT01～CT05 通过。Rust 默认和全特性各通过 1704 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 93 破甲复验。 [批次范围](../batches/SDK-5-77-member-critical-chance.md) · [机器验收](../../artifacts/sdk-5-77/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证同一成员三次真实弹道普攻。四人房配置 1.5 倍暴伤，伤害为 66、100、66。八人房配置 2 倍，伤害为 66、133、66。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-77/public/report.json) · [H5](../../artifacts/sdk-5-77/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-77/legacy93-browser/report.json)

Rust 专项复现原 `defense-crit` 算例，并覆盖配置、命中读取、叠加封顶、有效护甲、单次取整、护盾顺序、概率流、闪避、精确到期和周期及 NPC 伤害兼容。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-77/native-verification.json)

归档 SHA-256 为 `5cdabceb89a86e1e4f1821933d2ce1bc8d57e56a0c329dd431478cff7d53205a`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --critical-chance`。 [交付包](../../artifacts/sdk-5-77/delivery/match-kernel-0.79.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-77/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4338/)
