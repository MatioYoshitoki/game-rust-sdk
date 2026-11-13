# SDK-5-60 多人自身技能即时治疗验收

2026-11-06。SDK 0.62.0、机制 77、schema 7、协议 6。

IS01～IS05 通过。Rust 默认和全特性各通过 1636 项，含新增 4 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 76 自身 HOT 转盾兼容复验。 [批次范围](../batches/SDK-5-60-member-instant-self-heal.md) · [机器签收](../../artifacts/sdk-5-60/acceptance.json)

| 样例 | 气血费用 | 基础量 | 实回气血 | 实授护盾 |
|---|---:|---:|---:|---:|
| 四人房固定量 | 0 | 100 | 0 | 50 |
| 八人房最大气血比例 | 160 | 300 | 160 | 70 |

Node 和 H5 验证自身施法、本人治疗和授盾的事件顺序、护盾到期和退出结算。同房其他成员盾量为零，浏览器本人状态区显示实际回复。 [Node](../../artifacts/sdk-5-60/public/report.json) · [H5](../../artifacts/sdk-5-60/browser/report.json)

Rust 专项验证技能扣费优先、取整、有效倍率、满血的零回复、冷却、取消、保护、退出、资源不足及状态槽不足。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-60/native-verification.json)

归档 SHA-256 为 `7200ce49a0b03a3a2e24260263c5ce315dc5e4841362b1e790583c9f7b90011f`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --instant-self-heal`。 [交付包](../../artifacts/sdk-5-60/delivery/match-kernel-0.62.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-60/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4300/)
