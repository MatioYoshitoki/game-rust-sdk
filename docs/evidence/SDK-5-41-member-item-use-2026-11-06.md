# SDK-5-41 局内恢复道具验收

2026-11-06。SDK 0.43.0，协议 6，schema 7，机制 58。

同一 World 可配置用于本人的即时恢复道具。Rust 原子提交恢复量和库存变化，退出以净拾取量结算。

IU01～IU06 通过。默认和全特性各通过 1576 项 Rust 测试，含新增 5 项。112 项 Node 测试、Clippy 和格式检查也已通过。交付包通过空缓存离线构建、包外 Node、四页和八页 H5，以及机制 57 包外回归。 [批次范围](../batches/SDK-5-41-member-item-use.md) · [机器签收](../../artifacts/sdk-5-41/acceptance.json)

| 样例 | Node / H5 使用者 | Node / H5 恢复生命 | Node / H5 消耗份数 | Node / H5 入账份数 | Node / H5 结算人数 |
|---|---|---:|---:|---:|---:|
| 四人拾取后使用 | P00 / P00 | 40 / 40 | 1 / 1 | 2 / 2 | 4 / 4 |
| 八人转移后使用 | P01 / P01 | 40 / 40 | 1 / 1 | 2 / 2 | 8 / 8 |

实际击杀后拾取 3 份青华露。四人房由拾取者使用，八人房由同伴拾取转移的一份后使用。满血返回 `INVALID_STATE`。生命成本生效后恢复 40 点并扣一份，剩余 2 份入账。两组成员均各结算一次。 [普通宿主](../../artifacts/sdk-5-41/public/report.json) · [浏览器](../../artifacts/sdk-5-41/browser/report.json)

Rust 专项验证了道具配置与输入、生命缺口截断、背包修订、带入份额、保护状态、幂等、重连、成员隔离及金融恢复。默认和全特性分别有 10 和 10 项真实时钟压力用例分组运行。 [原生测试](../../artifacts/sdk-5-41/native-verification.json)

包外验证使用外置配置并隐藏交付样例与源码配置。机制 57 的四人转移和八人争抢通过。16618 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-41/delivery/independent-verification.json) · [机制 57 回归](../../artifacts/sdk-5-41/legacy57/report.json)

归档 SHA-256 为 `2ef826950b973da86cb4d3e4da99bfb4a8c059b9fa92a10bfe07d4b92e88e7ce`。源码 crate SHA-256 为 `d7848bb7322a8fa0c25fd763e26fa8af2b7e9c52ad0c3add3f0544f77a72f273`。二进制 SHA-256 为 `b5093e1b3d45f08ba08ef2fbd76650a72efae007ce6310ecefcd511f69f968e5`。 [交付包](../../artifacts/sdk-5-41/delivery/match-kernel-0.43.0-aarch64-apple-darwin.tar.gz)

演示使用本批独立账本。普攻击杀近处 NPC 后拾取青华露。点击「自身技能」消耗生命，再点击「使用恢复道具」。 [打开演示](http://127.0.0.1:4258/)
