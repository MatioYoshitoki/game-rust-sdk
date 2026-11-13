# SDK-5-80 自身技能闪避率验收

2026-11-07。SDK 0.82.0、机制 97、schema 7、协议 6。

EV01～EV05 通过。Rust 默认和全特性各通过 1716 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 96 闪避压制复验。 [批次范围](../batches/SDK-5-80-member-dodge-chance.md) · [机器验收](../../artifacts/sdk-5-80/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房观察 NPC 对 P00 的三发弹道。成员增益分别为 2500 和 5000 bps，伤害均为 66 → 0 → 66。中间一发发生在增益有效期间。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-80/public/report.json) · [H5](../../artifacts/sdk-5-80/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-80/legacy96-browser/report.json)

Rust 专项覆盖配置、封顶、刷新、私有归属、到期和资格检查，并验证闪避先于护盾、近战暴击和附带状态，近战攻击间隔、周期伤害和旧机制沿用原规则。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-80/native-verification.json)

归档 SHA-256 为 `b02aab21c9dbe77a9b2bb9d8d35da200d8073980745cee8c2ee33da66a448d78`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --dodge-chance`。 [交付包](../../artifacts/sdk-5-80/delivery/match-kernel-0.82.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-80/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4344/)
