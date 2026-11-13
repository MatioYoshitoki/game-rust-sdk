# SDK-5-70 道具净化自身沉默验收

2026-11-07。SDK 0.72.0、机制 87、schema 7、协议 6。

CL01～CL05 通过。Rust 默认和全特性各通过 1676 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 四人及八人验证，以及机制 86 首控免疫兼容复验。 [批次范围](../batches/SDK-5-70-member-item-cleanse.md) · [机器验收](../../artifacts/sdk-5-70/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证 P00 击杀后拾取两件净化道具。第一次沉默期间消费一件，移除实例并在原截止时间前完成新的施法。下一发 NPC 弹道施加新的 2800ms 沉默。所有成员各完成一次准入和结算，净化道具净收益一件。 [Node](../../artifacts/sdk-5-70/public/report.json) · [H5](../../artifacts/sdk-5-70/browser/report.json)

Rust 专项覆盖配置限制、多个来源、私有事件、其他状态、同刻到期、库存版本、未拾取库存及冷却。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-70/native-verification.json)

归档 SHA-256 为 `3128d018193e916ec07c48fc6a83c270c57a6344c8b1c70e0f682c58d5f1163a`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --item-cleanse`。 [交付包](../../artifacts/sdk-5-70/delivery/match-kernel-0.72.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-70/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4324/)
