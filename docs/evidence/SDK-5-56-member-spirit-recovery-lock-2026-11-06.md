# SDK-5-56 多人灵力回复封锁验收

2026-11-06。SDK 0.58.0、机制 73、schema 7、协议 6。

SL01～SL05 通过。Rust 默认和全特性各通过 1623 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 72 的自然恢复强化兼容复验。 [批次范围](../batches/SDK-5-56-member-spirit-recovery-lock.md) · [机器签收](../../artifacts/sdk-5-56/acceptance.json)

| 样例 | 封锁中光球气血 / 灵力 | 到期后光球气血 / 灵力 |
|---|---:|---:|
| 四人房 | 30 / 0 | 30 / 30 |
| 八人房 | 150 / 0 | 150 / 120 |

两房先自身施法，再分别于封锁中和到期后击杀拾取。普攻命中回灵独立核对，均从 0 恢复到 2。光球单次消费，气血正常回复，背包内容一致，退出结算完成。 [Node](../../artifacts/sdk-5-56/public/report.json) · [H5](../../artifacts/sdk-5-56/browser/report.json)

Rust 专项覆盖四条回灵路径、成员隔离、精确到期、纯回灵药原子拒绝、双恢复药的气血回复和附带状态生效顺序。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-56/native-verification.json)

归档 SHA-256 为 `2d8d78c1b5eeb58b79d740f0227d23d5810ca7aa17b03ef7227defb5383d6336`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --spirit-recovery-lock`。 [交付包](../../artifacts/sdk-5-56/delivery/match-kernel-0.58.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-56/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4292/)
