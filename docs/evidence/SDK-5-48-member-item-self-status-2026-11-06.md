# SDK-5-48 恢复道具附带自身状态验收

2026-11-06。SDK 0.50.0、机制 65、schema 7、协议 6。

SR01～SR06 通过。Rust 默认和全特性各通过 1598 项，含新增 3 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 64 品质恢复回归。 [批次范围](../batches/SDK-5-48-member-item-self-status.md) · [机器签收](../../artifacts/sdk-5-48/acceptance.json)

| 样例 | 实际恢复 | 属性变化 | 到期 | 道具入账 | 结算人数 |
|---|---:|---:|---:|---:|---:|
| 四人赤髓丸 | 气血 200 | 攻击 100 → 130 | 回到 100 | 1 | 4 |
| 八人灵力护剂 | 灵力 6 | 防御 100 → 120 | 回到 100 | 1 | 8 |

两房通过实际击杀拾取两份道具，使用一份。Node 核对本人收到状态施加和到期事件，H5 核对实际属性变化和到期恢复。队友未收到该用药私有事实。 [Node](../../artifacts/sdk-5-48/public/report.json) · [H5](../../artifacts/sdk-5-48/browser/report.json)

Rust 专项验证已登记的属性、盾和持续治疗引用，非法或敌对状态拒绝，满值、修订冲突、整组容量不足时的原子拒绝，以及重连和净额结算。两种构建配置各单独运行 10 项真实时钟压力测试。 首次回归发现升版误改两个旧周期次数上限，已恢复为 64，并以修正候选完成回归和包外验收。 [原生验证](../../artifacts/sdk-5-48/native-verification.json)

四人样例取自原赤髓丸恢复加属性修正的组合，验收配置将状态时长设为 3 秒以观察到期。八人样例用灵力和防御验证同一机制的另一种组合。 [原始注册表](../../rust-match-kernel-v1.1/source-config.json#L48)

归档 SHA-256 为 `503040eda50b8053d928e42eb66bbcf38d2dcd64b2c3967c54bed4c7b2918eb2`。包外验收可运行 `python3 scripts/verify_sdk_5_45_delivery.py --status`。 [交付包](../../artifacts/sdk-5-48/delivery/match-kernel-0.50.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-48/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4276/)
