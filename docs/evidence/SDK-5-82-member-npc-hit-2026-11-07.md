# SDK-5-82 NPC 命中属性验收

2026-11-07。SDK 0.84.0、机制 99、schema 7、协议 6。

NH01～NH05 通过。Rust 默认和全特性各通过 1724 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 98 成员命中加成复验。 [批次范围](../batches/SDK-5-82-member-npc-hit.md) · [机器验收](../../artifacts/sdk-5-82/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房验证 NPC 对 P00 的三发弹道。成员闪避增益均为 5000 bps。四人配置 100 点 NPC 命中，伤害为 66 → 66 → 66；八人配置 50 点 NPC 命中，伤害为 66 → 0 → 66。约 29% 的相同抽取分别高于 25% 和低于约 33.33% 的有效闪避概率。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-82/public/report.json) · [H5](../../artifacts/sdk-5-82/browser/report.json) · [机制 98 H5](../../artifacts/sdk-5-82/legacy98-browser/report.json) · [机制 97 H5](../../artifacts/sdk-5-82/legacy97-browser/report.json)

Rust 专项覆盖基础怪物和附加模板的字段校验、省略与显式零、上界、来源隔离、目标保护、精确到期和旧机制；近战与弹道共用闪避公式，并验证攻击间隔、暴击、护盾、附带状态和周期结算。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-82/native-verification.json)

归档 SHA-256 为 `69baa22ffd9ab514436eabec060a8f5bec89966142c1efb8fe0caa1e1e43cbad`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --npc-hit`。 [交付包](../../artifacts/sdk-5-82/delivery/match-kernel-0.84.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-82/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4348/)
