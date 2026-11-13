# SDK-5-87 自身技能威力加成验收

2026-11-07。SDK 0.89.0、机制 104、schema 7、协议 6。

SP01～SP05 通过。Rust 默认和全特性各通过 1744 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 103 忽略护甲复验。 [批次范围](../batches/SDK-5-87-member-skill-power.md) · [机器验收](../../artifacts/sdk-5-87/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房验证基础普攻、威力增益期间的技能、期间的普攻及到期后的技能。四人加成 5000 bps，伤害为 66 → 100 → 66 → 66；八人加成 10000 bps，伤害为 66 → 133 → 66 → 66。技能 ID 和费用从配置读取，核验自身技能及两次伤害技能的费用和普攻命中回灵。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-87/public/report.json) · [H5](../../artifacts/sdk-5-87/browser/report.json) · [机制 103 H5](../../artifacts/sdk-5-87/legacy103-browser/report.json)

Rust 专项覆盖自身状态配置及绑定、威力取整先于技能倍率、加成叠加、饱和、刷新、精确到期、成员隔离、退出清理，以及六种伤害技能和引导的冻结预算。逐项核对历史数值边界。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-87/native-verification.json)

归档 SHA-256 为 `82e5d2ff3ff2c797cce36a48020b5116eb95d221c7d49a8e4479ce5edaf4c116`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --skill-power`。 [交付包](../../artifacts/sdk-5-87/delivery/match-kernel-0.89.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-87/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4358/)
