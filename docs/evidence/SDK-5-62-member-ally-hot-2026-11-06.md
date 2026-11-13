# SDK-5-62 指定友方持续治疗验收

2026-11-06。SDK 0.64.0、机制 79、schema 7、协议 6。

AT01～AT05 通过。Rust 默认和全特性各通过 1644 项，含新增 4 项。116 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 78 指定友方即时治疗兼容复验。 [批次范围](../batches/SDK-5-62-member-ally-hot.md) · [机器签收](../../artifacts/sdk-5-62/acceptance.json)

| 样例 | 来源 | 目标 | 首跳实回 | 退场结果 |
|---|---|---|---:|---|
| 四人 | P00、P02 | P01 | 110、110 | P00 退出移除其 HOT，P02 的下一跳实回 110 |
| 八人 | P00 | P01 | 100 | P01 退出，P00 收到目标失效事件，超过下一跳时间仍无追加回复 |

Node 和 H5 验证施法、实例归属、每跳及移除事件隔离、来源或目标先离开及最终结算。 [Node](../../artifacts/sdk-5-62/public/report.json) · [H5](../../artifacts/sdk-5-62/browser/report.json)

Rust 专项验证配置门槛、目标代次、射程、视线、状态容量、同来源刷新、目标实时倍率、保护消耗节拍、缺口截断和退场清理。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-62/native-verification.json)

归档 SHA-256 为 `adc267d88c5ec486fab649dcffe47399c067ad59dfa34d940817bd73435d2b77`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --ally-hot`。 [交付包](../../artifacts/sdk-5-62/delivery/match-kernel-0.64.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-62/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4306/)
