# SDK-5-75 自身技能冷却缩减验收

2026-11-07。SDK 0.77.0、机制 92、schema 7、协议 6。

CR01～CR05 通过。Rust 默认和全特性各通过 1696 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 91 加减速复验。 [批次范围](../batches/SDK-5-75-member-cooldown-reduction.md) · [机器验收](../../artifacts/sdk-5-75/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证首次 2000ms 冷却分别缩至 1500、1200ms。加成先到期，已登记截止时间保持，下一次提交使用完整 2000ms。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-75/public/report.json) · [H5](../../artifacts/sdk-5-75/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-75/legacy91-browser/report.json)

Rust 专项覆盖绑定、聚合封顶、取整、正冷却下限和零冷却、提交时读取、截止时间、失败原子性和其他计时。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-75/native-verification.json)

归档 SHA-256 为 `87ddf0717b7f72eb82c4030db7f0d185e49866dc317c22474234d3165cc52a50`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --cooldown-reduction`。 [交付包](../../artifacts/sdk-5-75/delivery/match-kernel-0.77.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-75/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4334/)
