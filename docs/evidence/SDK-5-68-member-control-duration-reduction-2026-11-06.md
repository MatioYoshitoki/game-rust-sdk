# SDK-5-68 控制时长减免验收

2026-11-06。SDK 0.70.0、机制 85、schema 7、协议 6。

CD01～CD05 通过。Rust 默认和全特性各通过 1668 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 四人及八人验证，以及机制 84 控制免疫兼容复验。 [批次范围](../batches/SDK-5-68-member-control-duration-reduction.md) · [机器验收](../../artifacts/sdk-5-68/acceptance.json)

普通时钟 Node 与 H5 均验证 P00 首次 NPC 沉默的期限。四人样例用 2500 bps 将 2800ms 缩短到 2100ms，八人样例用 4000 bps 缩短到 1680ms。减免先到期，沉默按命中时确定的期限结束，第二次命中回到 2800ms。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-68/public/report.json) · [H5](../../artifacts/sdk-5-68/browser/report.json)

Rust 专项覆盖配置限制、叠加上限、毫秒取整、精确到期、同源刷新、免疫优先、同次其他效果和目标私有归属。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-68/native-verification.json)

归档 SHA-256 为 `86f631e6b83f987cea3d0b547e4dab01bddd84092adee6ea3d3d4be0a2e922e6`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --control-duration-reduction`。 [交付包](../../artifacts/sdk-5-68/delivery/match-kernel-0.70.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-68/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4320/)
