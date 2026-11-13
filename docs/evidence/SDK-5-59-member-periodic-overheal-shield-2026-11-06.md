# SDK-5-59 自身持续治疗溢出转护盾验收

2026-11-06。SDK 0.61.0、机制 76、schema 7、协议 6。

HT01～HT05 通过。Rust 默认和全特性各通过 1632 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 75 用药转盾兼容复验。 [批次范围](../batches/SDK-5-59-member-periodic-overheal-shield.md) · [机器签收](../../artifacts/sdk-5-59/acceptance.json)

| 样例 | 第 1 跳（回复／授盾） | 第 2 跳（回复／授盾） | 第 3 跳（回复／授盾） |
|---|---:|---:|---:|
| 四人房满血 | 0／50 | 0／50 | 0／0 |
| 八人房缺 160 气血 | 100／0 | 60／20 | 0／50 |

Node 和 H5 验证自身施法、连续三跳、护盾到期和退出结算。每跳按顺序发布本人回复和实际授盾，同房其他成员的盾量保持为零。 [Node](../../artifacts/sdk-5-59/public/report.json) · [H5](../../artifacts/sdk-5-59/browser/report.json)

Rust 专项验证受治疗倍率、气血优先、向下取整、护盾上限、保留余额、实际授盾刷新、状态槽不足时保留本跳气血回复及保护期跳过。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-59/native-verification.json)

归档 SHA-256 为 `d06958325bb605b3c83656e4d4abb2569fa8b198a7595be89907fe0dc1740726`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --periodic-overheal-shield`。 [交付包](../../artifacts/sdk-5-59/delivery/match-kernel-0.61.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-59/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4298/)
