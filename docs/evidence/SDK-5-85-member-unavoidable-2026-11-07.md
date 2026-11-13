# SDK-5-85 成员直接攻击不可闪避验收

2026-11-07。SDK 0.87.0、机制 102、schema 7、协议 6。

MU01～MU05 通过。Rust 默认和全特性各通过 1736 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 101 NPC 不可闪避复验。 [批次范围](../batches/SDK-5-85-member-unavoidable.md) · [机器验收](../../artifacts/sdk-5-85/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房验证 P00 先普攻再施放不可闪避方向弹道。四人普攻也开启不可闪避，伤害为 66 → 66；八人关闭普攻的该策略，伤害为 0 → 66。技能费用各扣除一次，所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-85/public/report.json) · [H5](../../artifacts/sdk-5-85/browser/report.json) · [机制 101 H5](../../artifacts/sdk-5-85/legacy101-browser/report.json)

Rust 专项覆盖普攻及六种伤害技能的严格配置和序列化，实际提交普通、单位和穿透弹道以及圆形、持续圆形和线形区域，验证冻结策略、随机流、费用、回灵、护盾、附带状态、重复接触预算与来源退出。逐项核对历史数值边界。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-85/native-verification.json)

归档 SHA-256 为 `3250488de83f952144414c4b72dff22d16a7ce164cf0ddcece0193d450a65e10`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --member-unavoidable`。 [交付包](../../artifacts/sdk-5-85/delivery/match-kernel-0.87.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-85/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4354/)
