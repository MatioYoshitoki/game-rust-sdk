# SDK-5-58 恢复道具溢出治疗转护盾验收

2026-11-06。SDK 0.60.0、机制 75、schema 7、协议 6。

OH01～OH05 通过。Rust 默认和全特性各通过 1629 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 74 NPC 封锁兼容复验。 [批次范围](../batches/SDK-5-58-member-overheal-shield.md) · [机器签收](../../artifacts/sdk-5-58/acceptance.json)

| 样例 | 实回气血 | 申请护盾 | 实授护盾 |
|---|---:|---:|---:|
| 四人房满血 | 0 | 100 | 100 |
| 八人房缺 360 气血 | 360 | 120 | 100 |

Node 和 H5 验证击杀、拾取两份、使用一份、护盾到期和退出结算。护盾和用药结果只进入本人视图。Node 还验证达到上限后重试无收益时，道具和冷却原样保留。 [Node](../../artifacts/sdk-5-58/public/report.json) · [H5](../../artifacts/sdk-5-58/browser/report.json)

Rust 专项验证品质及受治疗倍率、溢出转换、单盾和总盾上限、保留余额并刷新、吸收、槽位不足时原子拒绝及精确过期。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-58/native-verification.json)

归档 SHA-256 为 `d69b25bff20b7934967c8d02311caa93ca808b6d8643ba3b65a6188ba0affa99`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --overheal-shield`。 [交付包](../../artifacts/sdk-5-58/delivery/match-kernel-0.60.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-58/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4296/)
