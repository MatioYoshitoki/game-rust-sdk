# SDK-5-86 直接攻击忽略护甲验收

2026-11-07。SDK 0.88.0、机制 103、schema 7、协议 6。

IA01～IA05 通过。Rust 默认和全特性各通过 1740 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 102 成员不可闪避复验。 [批次范围](../batches/SDK-5-86-direct-ignore-armor.md) · [机器验收](../../artifacts/sdk-5-86/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房验证 P00 先普攻再施放方向弹道。四人技能开启忽略护甲，伤害为 66 → 100；八人关闭，伤害为 66 → 66。技能 ID 和费用从配置读取，费用各扣除一次，所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-86/public/report.json) · [H5](../../artifacts/sdk-5-86/browser/report.json) · [机制 102 H5](../../artifacts/sdk-5-86/legacy102-browser/report.json)

Rust 专项复验原 `defense-true` 向量，覆盖普攻、六种伤害技能与 NPC 弹道的严格配置、序列化和策略冻结，核验暴击、取整、免疫、闪避、受伤减伤、护盾、扣血、附带状态与重复接触预算。逐项核对历史数值边界。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-86/native-verification.json)

归档 SHA-256 为 `8c36f14b4cf3a89805c7858fdd082ddca9f54a8ee944e9c5f431392e7e15d265`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --ignore-armor`。 [交付包](../../artifacts/sdk-5-86/delivery/match-kernel-0.88.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-86/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4356/)
