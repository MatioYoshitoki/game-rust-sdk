# SDK-5-66 NPC 弹道沉默验收

2026-11-06。SDK 0.68.0、机制 83、schema 7、协议 6。

SI01～SI05 通过。Rust 默认和全特性各通过 1660 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 82 友方 HOT 转盾兼容复验。 [批次范围](../batches/SDK-5-66-member-npc-silence.md) · [机器签收](../../artifacts/sdk-5-66/acceptance.json)

四人和八人 Node/H5 样例均验证 NPC 弹道命中 P00，前摇被打断且未扣灵力，沉默 2800ms 期间可移动和普攻，到期后同一技能重新提交。状态事件只发送给 P00，所有成员各结算一次。 [Node](../../artifacts/sdk-5-66/public/report.json) · [H5](../../artifacts/sdk-5-66/browser/report.json)

Rust 专项验证配置门槛、技能及排队打断、已提交弹道与费用、引导周期、到期边界、来源清理、保护和容量失败。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-66/native-verification.json)

归档 SHA-256 为 `b1e0666f5bd77ac84577f4979d57d5cb4da1c58342fb311ba1fdfd203adb3529`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --npc-silence`。 [交付包](../../artifacts/sdk-5-66/delivery/match-kernel-0.68.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-66/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4316/)
