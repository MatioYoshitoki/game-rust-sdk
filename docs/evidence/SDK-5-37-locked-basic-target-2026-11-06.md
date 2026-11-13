# SDK-5-37 普攻锁定敌方单位验收

2026-11-06。SDK 0.39.0，协议 6，schema 7，机制 54。

普通攻击支持指定已发布的敌方 NPC 和代次。Rust 依次校验受理、排队释放和实际提交，使用提交时位置和原普通弹道。

BT01～BT06 通过。默认和全特性各通过 1555 项 Rust 测试，含新增 6 项。110 项 Node 测试含新增 2 项投影用例。Clippy 和格式通过。交付包通过空缓存离线构建、包外 Node、真实四页和八页 H5，以及机制 53 包外回归。 [批次范围](../batches/SDK-5-37-locked-basic-target.md) · [机器签收](../../artifacts/sdk-5-37/acceptance.json)

| 样例 | Node / H5 首发目标 | Node / H5 排队提交目标 | Node / H5 普攻提交数 | Node / H5 结算人数 |
|---|---|---|---:|---:|
| 四人锁定 | `status-target-2` #1 / `status-target-2` #1 | — / — | 1 / 1 | 4 / 4 |
| 八人换目标 | `status-target-2` #1 / `status-target-2` #1 | `status-target-1` #1 / `status-target-1` #1 | 2 / 2 | 8 / 8 |

四人房从候选中选择非最近敌人。八人房在同一普攻冷却内替换排队目标，第二发提交使用最新有效身份。Node 核对准备、实际提交和公共弹道。H5 通过目标选择和普攻按钮走普通宿主。两组同伴可移动，所有成员主动离场后各结算一次。 [普通宿主](../../artifacts/sdk-5-37/public/report.json) · [浏览器](../../artifacts/sdk-5-37/browser/report.json)

Rust 验证了指定非最近目标的实际命中、非法代次、目标在准备中换代、队列替换与重连隔离。Rust 与 JS 对混合瞄准和非法投影原子拒绝。默认套件首轮有 3 项旧用例未通过墙钟门禁，隔离复验通过。全特性套件将 28 项墙钟用例分开验证，最终覆盖同一测试集合。 [原生测试](../../artifacts/sdk-5-37/native-verification.json)

包外验证隐藏交付样例与源码配置，以外置配置运行普通宿主。机制 53 的普通普攻跟进和取消样例通过。9143 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-37/delivery/independent-verification.json) · [机制 53 回归](../../artifacts/sdk-5-37/legacy53/report.json)

归档 SHA-256 为 `74ad75eb781d0ae43ab670f8c4cb0e09f8c1581c318ea0cb05e9348c2e6fe85c`。源码 crate SHA-256 为 `6818339b37b6ec32bbe0f9b50c8eac5eb88c1767f450c351fda708a0f38b5496`。二进制 SHA-256 为 `3ac2a69b104632205db863e31294834662c85e35d22c897ade958c5c7ae13793`。 [交付包](../../artifacts/sdk-5-37/delivery/match-kernel-0.39.0-aarch64-apple-darwin.tar.gz)

4246 使用本批独立账本。通过「普攻目标」选敌人，再点击普攻。 [打开演示](http://127.0.0.1:4246/)
