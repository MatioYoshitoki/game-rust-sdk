# SDK-5-72 NPC 弹道眩晕验收

2026-11-07。SDK 0.74.0、机制 89、schema 7、协议 6。

ST01～ST05 通过。Rust 默认和全特性各通过 1684 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 88 定身复验。 [批次范围](../batches/SDK-5-72-member-npc-stun.md) · [机器验收](../../artifacts/sdk-5-72/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证 P00 被 NPC 弹道命中并眩晕 2800ms。正在准备的技能中断且未扣费、未开启冷却，眩晕期间位置保持。到期后发出新的移动与施法指令，所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-72/public/report.json) · [H5](../../artifacts/sdk-5-72/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-72/legacy88-browser/report.json)

Rust 专项覆盖绑定、未提交动作和排队清理、追击与撤离、引导终止和已付费弹道、精确到期、目标私有、控制免疫、缩时、刷新及净化。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-72/native-verification.json)

归档 SHA-256 为 `731d37fd2cdcba8e323d1a5544499873b297538f36b4fdc93ff3f61661e9c4d5`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --npc-stun`。 [交付包](../../artifacts/sdk-5-72/delivery/match-kernel-0.74.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-72/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4328/)
