# SDK-5-40 局内战利品丢弃与再拾取验收

2026-11-06。SDK 0.42.0，协议 6，schema 7，机制 57。

同一 World 可配置主动丢弃本局拾取的堆叠物品。Rust 原子变更背包与公共资源，退出以净拾取量结算。

LD01～LD06 通过。默认和全特性各通过 1571 项 Rust 测试，含新增 5 项。112 项 Node 测试、Clippy 和格式检查也已通过。交付包通过空缓存离线构建、包外 Node、四页和八页 H5，以及机制 56 包外回归。 [批次范围](../batches/SDK-5-40-member-loot-drop.md) · [机器签收](../../artifacts/sdk-5-40/acceptance.json)

| 样例 | Node / H5 丢弃者入账 | Node / H5 获胜者入账 | Node / H5 失败拾取 | Node / H5 结算人数 |
|---|---:|---:|---:|---:|
| 四人转移 | 1 / 1 | 2 / 2 | 0 / 0 | 4 / 4 |
| 八人争抢 | 1 / 1 | 2 / 2 | 1 / 1 | 8 / 8 |

实际击杀后拾取 3 份血灵芝。丢弃 2 份，同伴拾取后双方合计入账仍为 3 份。八人房同时提交两个拾取，落败请求返回 `INVALID_TARGET`。两组成员均各结算一次。 [普通宿主](../../artifacts/sdk-5-40/public/report.json) · [浏览器](../../artifacts/sdk-5-40/browser/report.json)

Rust 专项验证了丢弃配置和输入形状、背包修订、碰撞、地面配额、到期、容量、幂等、重连、成员隔离与金融恢复。默认和全特性分别有 10 和 10 项真实时钟压力用例分组运行。 [原生测试](../../artifacts/sdk-5-40/native-verification.json)

包外验证使用外置配置并隐藏交付样例与源码配置。机制 56 的自动和锁定追击通过。14726 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-40/delivery/independent-verification.json) · [机制 56 回归](../../artifacts/sdk-5-40/legacy56/report.json)

归档 SHA-256 为 `0917e6ac8a1d365cd05e71e1147a75a20ab0941a0c4d3a0c9d26b4b095d69292`。源码 crate SHA-256 为 `ed5282a0f21ce27dd5e04b6156efe3b978f70140f6d5f257babef7e16c7f7ea9`。二进制 SHA-256 为 `26bee6be9dc56ba839562f59660a4e8a89a1b6fb1ed90071deba9f36f885d4d0`。 [交付包](../../artifacts/sdk-5-40/delivery/match-kernel-0.42.0-aarch64-apple-darwin.tar.gz)

演示使用本批独立账本。普攻击杀近处 NPC 后拾取血灵芝，在背包选择血灵芝与数量后丢弃。另一成员点击拾取。 [打开演示](http://127.0.0.1:4255/)
