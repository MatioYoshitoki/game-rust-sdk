# SDK-5-49 恢复道具状态品质配置验收

2026-11-06。SDK 0.51.0、机制 66、schema 7、协议 6。

SQ01～SQ06 通过。Rust 默认和全特性各通过 1601 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 65 默认状态回归。 [批次范围](../batches/SDK-5-49-member-item-status-quality.md) · [机器签收](../../artifacts/sdk-5-49/acceptance.json)

| 样例 | 气血恢复 | 攻击变化 | 两次状态时长 | 到期 | 低、高品质入账 |
|---|---:|---:|---:|---:|---:|
| 四人先低后高 | 140 → 190 | 100 → 115 → 139 | 4.5 秒 → 7.5 秒 | 回到 100 | 各 1 份 |
| 八人先高后低 | 300 → 60 | 100 → 139 → 115 | 7.5 秒 → 4.5 秒 | 回到 100 | 各 1 份 |

两房实际击杀后拾取两种品质各两份，各使用一份。Node 核对施加、刷新和到期事件仅给本人，H5 核对实际属性、同一实例 ID、截止变化和到期恢复。八人房第二次用药缩短了原截止。 [Node](../../artifacts/sdk-5-49/public/report.json) · [H5](../../artifacts/sdk-5-49/browser/report.json)

Rust 专项验证版本和数值边界、未覆盖绑定沿用原定义、两种刷新顺序、满值与旧修订拒绝、重复请求、重连和净额结算。两种构建配置各单独运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-49/native-verification.json)

两种品质的攻击修正取自赤髓丸原资料的低品质 15% 与高品质 39%。原时长 20 秒与 35 秒可直接配置。验收样例使用较短时长，以便观察到期。 [原始配置](../../rust-match-kernel-v1.1/source-config.json)

归档 SHA-256 为 `4e592b7848900d5087edd013187ba5fc1a5024ee85ad049a708e4144b523b075`。包外验收可运行 `python3 scripts/verify_sdk_5_45_delivery.py --status-quality`。 [交付包](../../artifacts/sdk-5-49/delivery/match-kernel-0.51.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-49/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4278/)
