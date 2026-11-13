# SDK-5-71 NPC 弹道定身验收

2026-11-07。SDK 0.73.0、机制 88、schema 7、协议 6。

RT01～RT05 通过。Rust 默认和全特性各通过 1680 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 87 净化沉默复验。 [批次范围](../batches/SDK-5-71-member-npc-root.md) · [机器验收](../../artifacts/sdk-5-71/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证 P00 被 NPC 弹道命中并定身 2800ms。定身期间位置保持，原地技能和普攻可提交。定身到期后用新指令移动，所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-71/public/report.json) · [H5](../../artifacts/sdk-5-71/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-71/legacy87-browser/report.json)

Rust 专项覆盖绑定、移动与追击门禁、清空旧路径、精确到期、状态私有、定时与首控免疫、缩时、刷新及净化。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-71/native-verification.json)

归档 SHA-256 为 `fc6f31b5ca053c1f6be64582fa748c43f96aaada2bfab496bf5d80e069b9127f`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --npc-root`。 [交付包](../../artifacts/sdk-5-71/delivery/match-kernel-0.73.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-71/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4326/)
