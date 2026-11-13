# SDK-5-50 恢复道具附带直接减伤验收

2026-11-06。SDK 0.52.0、机制 67、schema 7、协议 6。

MR01～MR06 通过。Rust 默认和全特性各通过 1604 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 66 品质状态回归。 [批次范围](../batches/SDK-5-50-member-item-damage-reduction.md) · [机器签收](../../artifacts/sdk-5-50/acceptance.json)

| 样例 | 减伤率 | 用药前伤害 | 有效期内伤害 | 到期后伤害 | 入账 |
|---|---:|---:|---:|---:|---:|
| 四人玄甲丸 | 25% | 100 | 75 | 100 | 1 份 |
| 八人品质覆盖 | 32% | 100 | 68 | 100 | 1 份 |

两房通过实际击杀拾取玄甲丸两份，恢复气血并使用一份。Node 和 H5 读取同一个 NPC 的实际伤害事件，对比用药前、状态有效期和到期后的伤害。私有状态仅本人可见，剩余道具入账一份。 [Node](../../artifacts/sdk-5-50/public/report.json) · [H5](../../artifacts/sdk-5-50/browser/report.json)

Rust 专项验证版本、状态绑定与数值边界、防御和护盾顺序、50% 聚合上限、周期伤害隔离、到期边界、种子舍入、品质刷新、重连和净额结算。两种构建配置各单独运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-50/native-verification.json)

25% 和 32% 来自玄甲丸原资料中的两个品质。验收样例配置 3.3 秒以观察到期，范围限定为 NPC 对成员的直接伤害。 [原始配置](../../rust-match-kernel-v1.1/source-config.json)

归档 SHA-256 为 `fb5f94c9bd28242e58ee2326e7e7e95bf8a03699cc78a94781f6a92a24cff1e2`。包外验收可运行 `python3 scripts/verify_sdk_5_45_delivery.py --mitigation`。 [交付包](../../artifacts/sdk-5-50/delivery/match-kernel-0.52.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-50/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4280/)
