# SDK-5-54 多人回复光球验收

2026-11-06。SDK 0.56.0、机制 71、schema 7、协议 6。

RO01～RO05 通过。Rust 默认和全特性各通过 1617 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 70 的全局光球和即时用药兼容回归。 [批次范围](../batches/SDK-5-54-member-recovery-orb.md) · [机器签收](../../artifacts/sdk-5-54/acceptance.json)

| 样例 | 气血比例 | 灵力比例 | 实回气血 | 实回灵力 |
|---|---:|---:|---:|---:|
| 四人房 | 3% | 15% | 30 | 30 |
| 八人房 | 15% | 60% | 150 | 120 |

两房击杀者的最大气血为 1000，最大灵力为 200。施法消耗后，击杀仅生成光球，成功拾取才回复。Node 验证队友代领和重复拾取均被拒绝，H5 核对实回和背包。两房均完成退出结算。 [Node](../../artifacts/sdk-5-54/public/report.json) · [H5](../../artifacts/sdk-5-54/browser/report.json)

Rust 专项覆盖倍率配置、击杀时冻结、当前缺口、满值消费、过期边界、归属和库存修订。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-54/native-verification.json)

归档 SHA-256 为 `bdf7e6876bcf1b09bb874e844e7697f607c384119f55fa2cb8453c8e8662099a`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --recovery-orb`。 [交付包](../../artifacts/sdk-5-54/delivery/match-kernel-0.56.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-54/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4288/)
