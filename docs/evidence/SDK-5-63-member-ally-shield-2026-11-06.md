# SDK-5-63 指定友方吸收护盾验收

2026-11-06。SDK 0.65.0、机制 80、schema 7、协议 6。

AS01～AS05 通过。Rust 默认和全特性各通过 1648 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 79 指定友方 HOT 兼容复验。 [批次范围](../batches/SDK-5-63-member-ally-shield.md) · [机器签收](../../artifacts/sdk-5-63/acceptance.json)

| 样例 | 实授盾 | NPC 首次吸收 | 当次气血伤害 | 清理原因 |
|---|---:|---:|---:|---|
| 四人 | 220 | 67 | 0 | 来源离场 |
| 八人 | 200 | 67 | 0 | 目标离场 |

Node 和 H5 验证显式选人、来源与目标授盾通知、仅目标收到吸收明细及最终结算。四人房清理后，下一次同类攻击直接扣 67 点气血。 [Node](../../artifacts/sdk-5-63/public/report.json) · [H5](../../artifacts/sdk-5-63/browser/report.json)

Rust 专项验证配置门槛、目标重验、容量、共享上限、同来源补足、盾与气血的伤害顺序及双方退场清理。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-63/native-verification.json)

归档 SHA-256 为 `fd7b2471fe9543f7f70f508cbb394ad7014cb4ee5a158c5c778c50a1fe2f5b43`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --ally-shield`。 [交付包](../../artifacts/sdk-5-63/delivery/match-kernel-0.65.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-63/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4308/)
