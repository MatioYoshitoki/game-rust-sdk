# SDK-5-43 灵力恢复道具验收

2026-11-06。SDK 0.45.0，协议 6，schema 7，机制 60。

机制 60 允许把恢复道具配置为恢复本人灵力。Rust 按缺口计算恢复量，成功扣一份并启动冷却，结算按剩余净量。

IS01～IS06 通过。默认和全特性各通过 1583 项 Rust 测试，含新增 3 项。113 项 Node 测试、Clippy 和格式检查也已通过。交付包通过空缓存离线构建、包外 Node、四页和八页 H5，以及机制 59 包外回归。 [批次范围](../batches/SDK-5-43-member-item-spirit.md) · [机器签收](../../artifacts/sdk-5-43/acceptance.json)

| 样例 | Node / H5 使用者 | Node / H5 恢复灵力 | Node / H5 消耗份数 | Node / H5 入账份数 | Node / H5 结算人数 |
|---|---|---:|---:|---:|---:|
| 四人拾取后使用 | P00 / P00 | 8 / 8 | 1 / 1 | 2 / 2 | 4 / 4 |
| 八人转移后使用 | P01 / P01 | 6 / 6 | 1 / 1 | 2 / 2 | 8 / 8 |

实际击杀后拾取 3 份凝气散。四人房由拾取者使用，八人房由同伴拾取转移的一份后使用。满灵力返回 `INVALID_STATE`。技能成本生效后按缺口恢复并扣一份，剩余 2 份入账。两组成员均各结算一次。 [普通宿主](../../artifacts/sdk-5-43/public/report.json) · [浏览器](../../artifacts/sdk-5-43/browser/report.json)

Rust 专项覆盖机制门禁、单属性配置、灵力缺口、满值和冷却拒绝、库存修订、幂等、转移、重连及结算。默认和全特性分别有 10 和 10 项真实时钟压力用例分组运行。 [原生测试](../../artifacts/sdk-5-43/native-verification.json)

包外验证使用外置配置并隐藏交付样例与源码配置。机制 59 的四人到期再用和八人独立使用通过。20448 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-43/delivery/independent-verification.json) · [机制 59 回归](../../artifacts/sdk-5-43/legacy59/report.json)

归档 SHA-256 为 `40ac654ef645bd4222fee1d01f9063cd5e3ae90860a619742c5e76756f0bf0c1`。源码 crate SHA-256 为 `8e27ac94bbc826b3f23aa8c7e78d6435584c5df5a578f26901755415d7cea2a5`。二进制 SHA-256 为 `2a1936f9fdd3805169b61471aea24f41a40db9878f83aaf94d583f48317b2118`。 [交付包](../../artifacts/sdk-5-43/delivery/match-kernel-0.45.0-aarch64-apple-darwin.tar.gz)

演示使用本批独立账本。普攻击杀近处 NPC 后拾取凝气散。点击「自身技能」消耗灵力，再点击「使用恢复道具」。 [打开演示](http://127.0.0.1:4264/)
