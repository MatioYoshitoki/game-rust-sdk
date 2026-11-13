# SDK-5-78 自身技能暴伤加成验收

2026-11-07。SDK 0.80.0、机制 95、schema 7、协议 6。

CB01～CB05 通过。Rust 默认和全特性各通过 1708 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 94 暴击率复验。 [批次范围](../batches/SDK-5-78-member-critical-damage-bonus.md) · [机器验收](../../artifacts/sdk-5-78/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证暴击率持续有效时的三次真实弹道普攻。四人房伤害为 100、150、100，八人房为 100、200、100。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-78/public/report.json) · [H5](../../artifacts/sdk-5-78/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-78/legacy94-browser/report.json)

Rust 专项覆盖配置、暴击及非暴击、护甲与护盾、聚合封顶、刷新、精确到期、命中读取、目标私有、概率流和周期及 NPC 伤害兼容。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-78/native-verification.json)

归档 SHA-256 为 `645ee93f133405804afbf18c85dfe7922d6df58ccc330fa14821048e960e2de0`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --critical-damage-bonus`。 [交付包](../../artifacts/sdk-5-78/delivery/match-kernel-0.80.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-78/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4340/)
