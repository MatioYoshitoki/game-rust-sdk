# SDK-5-65 友方持续治疗溢出转护盾验收

2026-11-06。SDK 0.67.0、机制 82、schema 7、协议 6。

AHOT01～AHOT05 通过。Rust 默认和全特性各通过 1656 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 81 友方即时转盾兼容复验。 [批次范围](../batches/SDK-5-65-member-ally-hot-overheal-shield.md) · [机器签收](../../artifacts/sdk-5-65/acceptance.json)

| 样例 | 三跳实回气血 | 三跳实授盾 | 清理原因 |
|---|---|---|---|
| 四人满血 | 0 / 0 / 0 | 55 / 55 / 0 | 来源离场 |
| 八人缺 160 | 100 / 60 / 0 | 0 / 20 / 50 | 目标离场 |

Node 和 H5 核对三跳结果顺序、施法者与目标身份、私有实例、退场清理和结算。每组退场均清理该次 HOT 与其转换盾。 [Node](../../artifacts/sdk-5-65/public/report.json) · [H5](../../artifacts/sdk-5-65/browser/report.json)

Rust 专项验证实时倍率、气血优先、取整、单盾和总盾上限、零新增时保留期限、容量不足保留本跳气血、保护跳过，以及独立来源的退场清理。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-65/native-verification.json)

归档 SHA-256 为 `f54a311ea751fdd27654eb3a0f63ccec74e14b71cc127a6e2d1345f1b19c0236`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --ally-hot-overheal-shield`。 [交付包](../../artifacts/sdk-5-65/delivery/match-kernel-0.67.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-65/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4314/)
