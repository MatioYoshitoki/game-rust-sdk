# SDK-5-67 定时控制免疫验收

2026-11-06。SDK 0.69.0、机制 84、schema 7、协议 6。

CI01～CI05 通过。Rust 默认和全特性各通过 1664 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 83 NPC 弹道沉默兼容复验。 [批次范围](../batches/SDK-5-67-member-control-immunity.md) · [机器签收](../../artifacts/sdk-5-67/acceptance.json)

四人和八人 Node/H5 样例均验证 P00 施加 2600ms 免疫。第一发 NPC 弹道分别造成 19 和 19 点伤害，沉默被拒，前摇中的技能继续提交。免疫到期后第二发正常施加沉默，所有成员各结算一次。 [Node](../../artifacts/sdk-5-67/public/report.json) · [H5](../../artifacts/sdk-5-67/browser/report.json)

Rust 专项验证配置门槛、免疫优先于状态容量、保留同次其他效果和已有沉默的期限、刷新、精确到期与离场清理。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-67/native-verification.json)

归档 SHA-256 为 `24d5d29072796b6e0bdc2b45c3e4b5b70828eb65a0700d1e612051f604042fee`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --control-immunity`。 [交付包](../../artifacts/sdk-5-67/delivery/match-kernel-0.69.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-67/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4318/)
