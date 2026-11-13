# SDK-5-61 多人指定友方即时治疗验收

2026-11-06。SDK 0.63.0、机制 78、schema 7、协议 6。

AH01～AH05 通过。Rust 默认和全特性各通过 1640 项，含新增 4 项。114 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 77 自身即时治疗兼容复验。 [批次范围](../batches/SDK-5-61-member-ally-heal.md) · [机器签收](../../artifacts/sdk-5-61/acceptance.json)

| 样例 | 来源 | 目标 | 治疗前缺口 | 实回气血 |
|---|---|---|---:|---:|
| 四人固定量 | P00 | P01 | 160 | 120 |
| 八人最大气血比例 | P00 | P01 | 360 | 300 |

Node 和 H5 验证显式选人、来源和目标的身份、私有结果隔离及退出结算。H5 在选择成员前禁用施放，治疗后仅施法者和目标显示结果。首次包外验证发现前摇帧的友方身份被按 NPC 解析，改用专用 `ally_target` 后增加全量和增量回归并重新打包。 [Node](../../artifacts/sdk-5-61/public/report.json) · [H5](../../artifacts/sdk-5-61/browser/report.json)

Rust 专项验证目标代次、就绪、保护、死亡和位置或视线变化，提交失败保留费用与冷却，成功时使用目标倍率并按气血缺口截断。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-61/native-verification.json)

归档 SHA-256 为 `ff2c7bd26a3ed077742e440357b7fda0f10af12960cb171c0dd30637ab0adc88`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --ally-heal`。 [交付包](../../artifacts/sdk-5-61/delivery/match-kernel-0.63.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-61/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4304/)
