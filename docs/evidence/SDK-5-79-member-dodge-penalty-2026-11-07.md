# SDK-5-79 自身技能闪避压制验收

2026-11-07。SDK 0.81.0、机制 96、schema 7、协议 6。

EP01～EP05 通过。Rust 默认和全特性各通过 1712 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 95 暴伤复验。 [批次范围](../batches/SDK-5-79-member-dodge-penalty.md) · [机器验收](../../artifacts/sdk-5-79/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房验证同一成员三次真实弹道普攻。每组只有压制有效的中间一发产生 66 点伤害。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-79/public/report.json) · [H5](../../artifacts/sdk-5-79/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-79/legacy95-browser/report.json)

Rust 专项复现原 `dodge-penalty` 与 `dodge-cap` 算例，并覆盖配置、先压制后封顶、零率、叠加、刷新、精确到期、攻击者隔离、概率流及闪避时无伤害、回灵和附带状态。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-79/native-verification.json)

归档 SHA-256 为 `363ca8da61ee2f79c4ba6b4fc054cb3f6899385ba391917257fdd8cd12a76ab8`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --dodge-penalty`。 [交付包](../../artifacts/sdk-5-79/delivery/match-kernel-0.81.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-79/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4342/)
