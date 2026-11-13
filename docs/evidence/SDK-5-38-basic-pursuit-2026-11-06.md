# SDK-5-38 锁定普攻短追击验收

2026-11-06。SDK 0.40.0，协议 6，schema 7，机制 55。

通过配置为显式锁定的敌方 NPC 开启短追击。Rust 沿现有移动和碰撞规则直线接近，进入射程后以同一动作进入普攻前摇。

BP01～BP06 通过。默认和全特性各通过 1561 项 Rust 测试，含新增 6 项。112 项 Node 测试含新增 2 项投影用例。Clippy 和格式通过。交付包通过空缓存离线构建、包外 Node、真实四页和八页 H5，以及机制 54 包外回归。 [批次范围](../batches/SDK-5-38-basic-pursuit.md) · [机器签收](../../artifacts/sdk-5-38/acceptance.json)

| 样例 | Node / H5 移动距离 | Node / H5 普攻提交数 | Node / H5 结算人数 |
|---|---:|---:|---:|
| 四人追上并提交 | 165.0 / 165.0 | 1 / 1 | 4 / 4 |
| 八人中途取消 | 10.8 / 27.0 | 0 / 0 | 8 / 8 |

四人房追上指定目标并以原动作发射一枚弹道。八人房移动后取消，超过原追击截止及前摇后仍为零次提交。两组同伴均可移动，成员离场后各结算一次。 [普通宿主](../../artifacts/sdk-5-38/public/report.json) · [浏览器](../../artifacts/sdk-5-38/browser/report.json)

Rust 专项核对移动与扫掠碰撞、射程边界、视线、代次、期限、实际命中及冷却，也核对重连、幂等、成员隔离及私有投影。Rust 与 JS 对非法追击投影原子拒绝。默认和全特性分别有 10、10 项普通时钟用例分组运行。 [原生测试](../../artifacts/sdk-5-38/native-verification.json)

包外验证使用外置配置并隐藏交付样例与源码配置。机制 54 的锁定目标和队列换目标样例通过。10991 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-38/delivery/independent-verification.json) · [机制 54 回归](../../artifacts/sdk-5-38/legacy54/report.json)

归档 SHA-256 为 `f8dc41e65f416b09d4ff710be4d750d1284aa1523da73836b948b8e6930d8b0a`。源码 crate SHA-256 为 `4e01d1b9f1e5cc8c6d429658bb4ef752f017c5e3169b01a101f9ff903d6e3b16`。二进制 SHA-256 为 `4e3fbfc6877f9b856dd82e4b570fcc2b996989db8f57ea2733b2c5be0327addb`。 [交付包](../../artifacts/sdk-5-38/delivery/match-kernel-0.40.0-aarch64-apple-darwin.tar.gz)

演示使用本批独立账本。通过「普攻目标」选敌人再点击普攻，追击中可点击「取消追击」。 [打开演示](http://127.0.0.1:4249/)
