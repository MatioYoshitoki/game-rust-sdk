# SDK-5-69 首控免疫验收

2026-11-07。SDK 0.71.0、机制 86、schema 7、协议 6。

FC01～FC05 通过。Rust 默认和全特性各通过 1672 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 四人及八人验证，以及机制 85 控制时长减免兼容复验。 [批次范围](../batches/SDK-5-69-member-first-control-immunity.md) · [机器验收](../../artifacts/sdk-5-69/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房验证 P00 施加 10000ms 首控免疫。首发 NPC 弹道照常扣血，阻止沉默并消耗免疫，准备中的技能照常提交。下一发在免疫的原截止时间前正常施加 2800ms 沉默。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-69/public/report.json) · [H5](../../artifacts/sdk-5-69/browser/report.json)

Rust 专项覆盖配置限制、同次多沉默、普通命中、定时免疫优先、确定性选取、消耗后缩时、刷新、精确到期及退场清理。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-69/native-verification.json)

归档 SHA-256 为 `07e49f76a6c2c60e4ee89dc0fa3ac3e7dce61b4ca770e4b227e696e2f0bfe9aa`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --first-control-immunity`。 [交付包](../../artifacts/sdk-5-69/delivery/match-kernel-0.71.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-69/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4322/)
