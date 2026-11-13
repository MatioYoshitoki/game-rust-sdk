# SDK-5-55 多人自然恢复强化验收

2026-11-06。SDK 0.57.0、机制 72、schema 7、协议 6。

RS01～RS05 通过。Rust 默认和全特性各通过 1620 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 71 的光球兼容复验。 [批次范围](../batches/SDK-5-55-member-recovery-strength.md) · [机器签收](../../artifacts/sdk-5-55/acceptance.json)

| 样例 | 修正 | 生效期每周期气血 / 灵力 | 到期后每周期气血 / 灵力 |
|---|---:|---:|---:|
| 四人近敌房 | +100% | 10 / 2 | 5 / 1 |
| 八人远敌房 | −50% | 7 / 1 | 15 / 3 |

两房最大气血为 1000，最大灵力为 200。成员自身施法扣除属性并施加状态。Node 和 H5 读取每周期前后的本人快照，核对恢复量、队友状态隔离及退出结算。 [Node](../../artifacts/sdk-5-55/public/report.json) · [H5](../../artifacts/sdk-5-55/browser/report.json)

Rust 专项覆盖近敌边界、死亡 NPC、修正相加、零倍率、只取整一次、保护、到期、缺口和死亡成员。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-55/native-verification.json)

归档 SHA-256 为 `1b35f9aa1f049e75f80c199503bfda4d4559052cfc90c1c89fc6a8de78b79036`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --recovery-strength`。 [交付包](../../artifacts/sdk-5-55/delivery/match-kernel-0.57.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-55/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4290/)
