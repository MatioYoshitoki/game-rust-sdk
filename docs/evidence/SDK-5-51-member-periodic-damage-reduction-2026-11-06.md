# SDK-5-51 周期伤害减伤验收

2026-11-06。SDK 0.53.0、机制 68、schema 7、协议 6。

PM01～PM06 通过。Rust 默认和全特性各通过 1607 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 67 直伤兼容回归。 [批次范围](../batches/SDK-5-51-member-periodic-damage-reduction.md) · [机器签收](../../artifacts/sdk-5-51/acceptance.json)

| 样例 | 用药前扣血 | 减伤后伤害 | 首跳盾吸收 | 首跳扣血 | 盾耗尽后扣血 | 减伤到期后扣血 |
|---|---:|---:|---:|---:|---:|---:|
| 四人玄甲丸 | 100 | 75 | 75 | 0 | 75 | 100 |
| 八人品质覆盖 | 100 | 68 | 40 | 28 | 68 | 100 |

NPC 通过实际弹道命中施加持续伤害。两房都击杀拾取玄甲丸两份，基准跳结束后使用一份，取得减伤和吸收盾。Node 和 H5 核对每跳护盾吸收和生命损失，剩余道具各入账一份。 [Node](../../artifacts/sdk-5-51/public/report.json) · [H5](../../artifacts/sdk-5-51/browser/report.json)

Rust 专项覆盖机制 67 与 68 的周期差异、护盾与生命事件顺序、迟到执行和精确到期、保护跳过及种子舍入。两种构建配置各单独运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-51/native-verification.json)

样例沿用玄甲丸 25% 和 32% 的减伤值，另配置 75 和 40 点护盾验证结算顺序。范围为 NPC 对成员的物理持续伤害。 [原始配置](../../rust-match-kernel-v1.1/source-config.json)

归档 SHA-256 为 `609ba29168bf721e5f81accf26932aeeba57b8fcbd1d94e9b768e87d2d7bc37c`。包外验收可运行 `python3 scripts/verify_sdk_5_45_delivery.py --periodic-mitigation`。 [交付包](../../artifacts/sdk-5-51/delivery/match-kernel-0.53.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-51/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4282/)
