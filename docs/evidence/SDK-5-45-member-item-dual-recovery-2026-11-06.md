# SDK-5-45 局内双恢复道具验收

2026-11-06。SDK 0.47.0、机制 62、schema 7、协议 6。

DR01～DR06 通过。Rust 默认和全特性各通过 1589 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 61 共享冷却回归。 [批次范围](../batches/SDK-5-45-member-item-dual-recovery.md) · [机器签收](../../artifacts/sdk-5-45/acceptance.json)

| 样例 | 实际气血恢复 | 实际灵力恢复 | 消耗 | 入账 | 结算人数 |
|---|---:|---:|---:|---:|---:|
| 四人双恢复 | 40 | 8 | 1 | 1 | 4 |
| 八人仅灵力有缺口 | 0 | 6 | 1 | 1 | 8 |

Node 和 H5 从实际击杀掉落拾取两份玉泉散，使用一份后退出。Node 验证满值和冷却中的拒绝。H5 核对本人同时显示两项恢复量，其他页面收不到用药事件。 [Node](../../artifacts/sdk-5-45/public/report.json) · [H5](../../artifacts/sdk-5-45/browser/report.json)

Rust 专项覆盖仅气血有缺口、两项均满、配置版本和边界、旧修订、重复输入、冷却、重连和净拾取结算。两个全量配置各将 10 项真实时钟压力用例单独运行。 [原生验证](../../artifacts/sdk-5-45/native-verification.json)

样例取自原物品注册表的玉泉散双恢复行为，使用配置的固定量。 [原始注册表](../../rust-match-kernel-v1.1/source-config.json#L48)

归档 SHA-256 为 `7b0de1ac33c414925d44186bd6e421e5ef1a8b7375759a1b5b196a6b08acd141`。 [交付包](../../artifacts/sdk-5-45/delivery/match-kernel-0.47.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-45/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4270/)
