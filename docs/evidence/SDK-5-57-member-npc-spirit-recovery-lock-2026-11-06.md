# SDK-5-57 NPC 弹道灵力回复封锁验收

2026-11-06。SDK 0.59.0、机制 74、schema 7、协议 6。

NL01～NL05 通过。Rust 默认和全特性各通过 1626 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 73 自身封锁兼容复验。 [批次范围](../batches/SDK-5-57-member-npc-spirit-recovery-lock.md) · [机器签收](../../artifacts/sdk-5-57/acceptance.json)

| 样例 | 封锁中光球气血 / 灵力 | 到期后光球气血 / 灵力 |
|---|---:|---:|
| 四人房 | 30 / 0 | 30 / 30 |
| 八人房 | 150 / 0 | 150 / 120 |

Node 核对 NPC 弹道发射、命中伤害事件和本人状态依次出现。H5 核对状态中的来源及代次、两次拾取和退出结算。两房的普攻命中回灵均由 0 恢复到 2。 [Node](../../artifacts/sdk-5-57/public/report.json) · [H5](../../artifacts/sdk-5-57/browser/report.json)

Rust 专项覆盖配置门禁、护盾全吸收、受保护和致死目标、精确到期、来源死亡及归位重置。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-57/native-verification.json)

归档 SHA-256 为 `a0548d4d0f763af5e53377e124049e524a60534485916795bb50216ef091ffbc`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --npc-spirit-recovery-lock`。 [交付包](../../artifacts/sdk-5-57/delivery/match-kernel-0.59.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-57/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4294/)
