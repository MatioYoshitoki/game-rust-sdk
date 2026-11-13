# SDK-5-84 NPC 弹道不可闪避验收

2026-11-07。SDK 0.86.0、机制 101、schema 7、协议 6。

UD01～UD05 通过。Rust 默认和全特性各通过 1732 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 100 定时伤害免疫复验。 [批次范围](../batches/SDK-5-84-member-npc-unavoidable.md) · [机器验收](../../artifacts/sdk-5-84/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房观察 NPC 对 P00 的四发弹道。四人配置不可闪避，伤害为 66 → 66 → 0 → 66；八人配置可闪避，伤害为 66 → 0 → 0 → 66。四次接触分别覆盖基础伤害、成员闪避增益、伤害免疫和免疫到期。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-84/public/report.json) · [H5](../../artifacts/sdk-5-84/browser/report.json) · [机制 100 H5](../../artifacts/sdk-5-84/legacy100-browser/report.json)

Rust 专项覆盖基础怪物与附加模板的布尔字段、版本和投放位置校验，以及释放时冻结、随机流、墙挡、来源失效、目标保护和伤害免疫；费用、冷却、护盾与附带状态沿用原结算顺序。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-84/native-verification.json)

初次全量回归发现机制版本扩展误改数值边界。已逐项恢复原边界，重新打包并按以上口径全部复验。 [初次回归记录](../../artifacts/sdk-5-84/superseded-initial-candidate/default.log)

归档 SHA-256 为 `3bc86ebd964cc568737986dee42239335a84fc689133810e31e5e60e4ba1bc1b`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --unavoidable`。 [交付包](../../artifacts/sdk-5-84/delivery/match-kernel-0.86.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-84/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4352/)
