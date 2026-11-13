# SDK-5-74 自身技能移速加成验收

2026-11-07。SDK 0.76.0、机制 91、schema 7、协议 6。

HS01～HS05 通过。Rust 默认和全特性各通过 1692 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 90 减速复验。 [批次范围](../batches/SDK-5-74-member-movement-haste.md) · [机器验收](../../artifacts/sdk-5-74/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证同一移动指令跨过两个状态到期点。四人房依次实测 189、135、180，八人房依次实测 198、108、180 世界单位/秒。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-74/public/report.json) · [H5](../../artifacts/sdk-5-74/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-74/legacy90-browser/report.json)

Rust 专项覆盖绑定、强度边界、方向移动、导航、追击、硬控制、最强实例、同源刷新、精确到期、目标私有、退场清理、免疫、缩时和净化。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-74/native-verification.json)

归档 SHA-256 为 `47c8863319914584cd40806b89e879c2ed3dbb096c7b352b7c062187df33432d`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --movement-haste`。 [交付包](../../artifacts/sdk-5-74/delivery/match-kernel-0.76.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-74/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4332/)
