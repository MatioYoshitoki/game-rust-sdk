# SDK-5-53 即时用药的受治疗倍率验收

2026-11-06。SDK 0.55.0、机制 70、schema 7、协议 6。

IH01～IH06 通过。Rust 默认和全特性各通过 1614 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 69 的 HOT 兼容回归。 [批次范围](../batches/SDK-5-53-member-item-healing-received.md) · [机器签收](../../artifacts/sdk-5-53/acceptance.json)

| 样例 | 品质取整后 | 受治疗倍率 | 生效期实回 | 到期后实回 |
|---|---:|---:|---:|---:|
| 四人房固定量 | 53 | +50% | 79 | 53 |
| 八人房按最大气血比例 | 53 | −50% | 26 | 53 |

两房击杀 NPC 拾取一份调息饮及三份回春丸。调息饮当次回复 10，随后施加状态。受治疗倍率生效和到期后各用一份回春丸，退出时各有一份回春丸入账，其他成员无该道具收入。 [Node](../../artifacts/sdk-5-53/public/report.json) · [H5](../../artifacts/sdk-5-53/browser/report.json)

Rust 专项覆盖配置门禁、双重取整、气血缺口、到期时刻、宽整数、关闭倍率、零回复的原子拒绝和双恢复灵力成功。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-53/native-verification.json)

归档 SHA-256 为 `46e98ec5f50441dab553205d5916abdf760ccffb641cc1bc43f4f365adafe118`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --item-healing-received`。 [交付包](../../artifacts/sdk-5-53/delivery/match-kernel-0.55.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-53/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4286/)
