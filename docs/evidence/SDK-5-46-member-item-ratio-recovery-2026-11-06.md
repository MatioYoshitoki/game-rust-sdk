# SDK-5-46 按最大属性比例恢复验收

2026-11-06。SDK 0.48.0、机制 63、schema 7、协议 6。

RR01～RR06 通过。Rust 默认和全特性各通过 1592 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 62 固定量双恢复回归。 [批次范围](../batches/SDK-5-46-member-item-ratio-recovery.md) · [机器签收](../../artifacts/sdk-5-46/acceptance.json)

| 样例 | 实际气血恢复 | 实际灵力恢复 | 消耗 | 入账 | 结算人数 |
|---|---:|---:|---:|---:|---:|
| 四人双比例取整 | 200 | 40 | 1 | 1 | 4 |
| 八人按灵力缺口截断 | 0 | 6 | 1 | 1 | 8 |

四人房最大气血 1003、最大灵力 203，使用自身技能制造 300、50 点缺口。配置 20% 后分别取整为 200、40。八人房气血已满，灵力仅缺 6 点。两房通过击杀拾取两份玉泉散，使用一份，退出结算一份。Node 验证满值和冷却中拒绝，H5 核对本人实际数值与队友事件隔离。 [Node](../../artifacts/sdk-5-46/public/report.json) · [H5](../../artifacts/sdk-5-46/browser/report.json)

Rust 专项验证宽整数、100% 比例、取整为零、同属性配置冲突、混合配置、重连和重复请求。两种构建配置各单独运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-46/native-verification.json)

样例复用原始玉泉散的两个基础 2000 万分值。首次包外校验发现原 5000 毫秒冷却无法对齐 30 毫秒步长，样例向上对齐到 5010 毫秒。品质倍率待后续适配。 [原始注册表](../../rust-match-kernel-v1.1/source-config.json#L48)

归档 SHA-256 为 `abf472059816d825a254fe82de1a3cf5a0fc933fa14be3b8905bb2fa03004073`。包外验收可运行 `python3 scripts/verify_sdk_5_45_delivery.py --ratio`。 [交付包](../../artifacts/sdk-5-46/delivery/match-kernel-0.48.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-46/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4272/)
