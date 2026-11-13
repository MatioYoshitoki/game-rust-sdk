# SDK-5-33 方向弹道单次再施放验收

2026-11-06。SDK 0.35.0，协议 6，schema 7，机制 50。

普通方向弹道支持一次限时转向。Rust 校验成员、连接、动作 ID、窗口与灵力，保留已发弹道的位置、伤害、射程与 TTL。

RC01～RC06 通过。默认和全特性各通过 1531 项 Rust 测试，含新增 6 项。102 项 Node 测试含新增 2 项投影用例。Clippy 和格式通过。交付包通过空缓存离线构建、包外 Node、真实四页和八页 H5，以及机制 49 包外回归。 [批次范围](../batches/SDK-5-33-projectile-recast.md) · [机器签收](../../artifacts/sdk-5-33/acceptance.json)

| 样例 | Node 实际创建弹道 | Node 初次费用 | Node / H5 再施放费用 | Node / H5 结算人数 |
|---|---:|---|---:|---:|
| 四人转向 | 1 | 3 HP 和 12 灵力 | 5 / 5 灵力 | 4 / 4 |
| 八人过期 | 1 | 3 HP 和 9 灵力 | 0 / 0 灵力 | 8 / 8 |

普通宿主记录同一弹道 ID 的移动。四人转向后沿新方向前进，八人过期后保持原方向。两组的同伴可继续移动，各自完成一次准入与结算。H5 使用点选目标和转向按钮，消费权威事件和快照。 [普通宿主](../../artifacts/sdk-5-33/public/report.json) · [浏览器](../../artifacts/sdk-5-33/browser/report.json)

Rust 验证转向后的实际命中、后摇期间转向、窗口等号、资源失败重试、机会替换与终局清理。重连保留在途机会与原截止，新连接恢复就绪后可消费。原连接请求拒绝。Rust 和 JS 对私有机会的非法引用与期限做原子拒绝。 [原生测试](../../artifacts/sdk-5-33/native-verification.json)

包外测试隐藏交付样例与源码配置，以外置配置运行普通宿主。机制 49 的四人自然引导和八人取消引导通过。1933 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-33/delivery/independent-verification.json) · [机制 49 回归](../../artifacts/sdk-5-33/legacy49/report.json)

归档 SHA-256 为 `8e0aa42df82d0f30eea066a00c0fe33a1426441f0f2873068cddd3e7dfd6c5de`。源码 crate SHA-256 为 `c9865bf45abd8bf42f9c7ea974b09adb922543c487e7a2fb595b9aca5f94c7e5`。二进制 SHA-256 为 `7938b90725c6e32718f0d2c2dd062629e62e08651646d24b4c143016371e7260`。 [交付包](../../artifacts/sdk-5-33/delivery/match-kernel-0.35.0-aarch64-apple-darwin.tar.gz)

4234 使用本批独立账本。选四人房施放主槽技能，再选择目标点并点击「再施放 · 转向」。 [打开演示](http://127.0.0.1:4234/)
