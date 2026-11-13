# SDK-5-83 定时伤害免疫验收

2026-11-07。SDK 0.85.0、机制 100、schema 7、协议 6。

DI01～DI05 通过。Rust 默认和全特性各通过 1728 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 99 NPC 命中复验。 [批次范围](../batches/SDK-5-83-member-damage-immunity.md) · [机器验收](../../artifacts/sdk-5-83/acceptance.json)

普通时钟 Node 与 H5 在固定种子的四人和八人房观察 NPC 对 P00 的三发弹道。成员施放 1300ms 伤害免疫，伤害均为 66 → 0 → 66，分别发生在免疫前、有效期间和到期后。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-83/public/report.json) · [H5](../../artifacts/sdk-5-83/browser/report.json) · [机制 99 H5](../../artifacts/sdk-5-83/legacy99-browser/report.json)

Rust 专项覆盖自身技能绑定、免疫先于闪避与暴击、护盾与附带状态、近战间隔、同源刷新、精确到期、私有归属和退场清理；DOT 按实际执行时刻检查免疫，免疫期间推进周期进度。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-83/native-verification.json)

归档 SHA-256 为 `a317a63291678370d2a5b5adb19a194a879f25a260c56556f605ff80930d9c3d`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --damage-immunity`。 [交付包](../../artifacts/sdk-5-83/delivery/match-kernel-0.85.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-83/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4350/)
