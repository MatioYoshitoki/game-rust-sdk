# SDK-5-81 自身技能命中加成验收

2026-11-07。SDK 0.83.0、机制 98、schema 7、协议 6。

AC01～AC05 通过。Rust 默认和全特性各通过 1720 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 97 成员闪避率复验。 [批次范围](../batches/SDK-5-81-member-accuracy.md) · [机器验收](../../artifacts/sdk-5-81/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房验证 P00 对同一 NPC 的三发弹道。命中增益分别为 10000 和 7500 bps，伤害均为 0 → 66 → 0。中间一发发生在增益有效期间。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-81/public/report.json) · [H5](../../artifacts/sdk-5-81/browser/report.json) · [机制 97 H5](../../artifacts/sdk-5-81/legacy97-browser/report.json) · [机制 96 H5](../../artifacts/sdk-5-81/legacy96-browser/report.json)

Rust 专项复验原 `dodge-half` 向量，并覆盖闪避压制、命中修正、封顶和零概率抽取顺序，以及刷新、到期、在途弹道、成员隔离和原接触副作用。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-81/native-verification.json)

归档 SHA-256 为 `c5e9e323f7c9c199d97e9e574f8d769719487642f51ed9a7d2da76350d65e78b`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --accuracy`。 [交付包](../../artifacts/sdk-5-81/delivery/match-kernel-0.83.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-81/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4346/)
