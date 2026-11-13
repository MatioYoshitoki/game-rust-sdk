# SDK-5-47 恢复道具品质倍率验收

2026-11-06。SDK 0.49.0、机制 64、schema 7、协议 6。

QR01～QR06 通过。Rust 默认和全特性各通过 1595 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 63 比例恢复回归。 [批次范围](../batches/SDK-5-47-member-item-quality-recovery.md) · [机器签收](../../artifacts/sdk-5-47/acceptance.json)

| 样例 | 实际气血恢复 | 实际灵力恢复 | 低品质入账 | 高品质入账 | 结算人数 |
|---|---:|---:|---:|---:|---:|
| 四人使用 1.5 倍品质 | 300 | 60 | 2 | 1 | 4 |
| 八人使用 0.7 倍品质 | 0 | 28 | 1 | 2 | 8 |

两房通过实际击杀拾取低、高两种品质各 2 份，使用所选品质的一份。四人房最大气血 1003、最大灵力 203，按 30% 取整。八人房气血已满，最大灵力 200 按 14% 恢复 28 点。两种品质共享冷却并按各自 ID 入账。 [Node](../../artifacts/sdk-5-47/public/report.json) · [H5](../../artifacts/sdk-5-47/browser/report.json)

Rust 专项验证默认 1 倍、倍率上限和宽整数、固定量、比例量、混用、取整为零、重连、重复请求和结算。两种构建配置各单独运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-47/native-verification.json)

样例以原始玉泉散的 20% 基础比例和 14%～30% 说明为依据，用 0.7 倍与 1.5 倍覆盖区间两端。品质按物品 ID 配置，实例品质和生成规则另批处理。 [原始注册表](../../rust-match-kernel-v1.1/source-config.json#L48)

归档 SHA-256 为 `03a71a9af81614c23b0c0ff9a98d9ee2b00d61d705508f6db150944fdb5395da`。包外验收可运行 `python3 scripts/verify_sdk_5_45_delivery.py --quality`。 [交付包](../../artifacts/sdk-5-47/delivery/match-kernel-0.49.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-47/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4274/)
