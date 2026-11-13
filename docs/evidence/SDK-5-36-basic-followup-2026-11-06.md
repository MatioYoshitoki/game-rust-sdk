# SDK-5-36 普攻冷却跟进验收

2026-11-06。SDK 0.38.0，协议 6，schema 7，机制 53。

普攻提交后可在当前冷却内缓冲下一击。Rust 冻结目标代次，在冷却释放与实际提交两次重验后使用原弹道事务。

BA01～BA06 通过。默认和全特性各通过 1549 项 Rust 测试，含新增 6 项。108 项 Node 测试含新增 2 项投影用例。Clippy 和格式通过。交付包通过空缓存离线构建、包外 Node、真实四页和八页 H5，以及机制 52 包外回归。 [批次范围](../batches/SDK-5-36-basic-followup.md) · [机器签收](../../artifacts/sdk-5-36/acceptance.json)

| 样例 | Node / H5 普攻提交数 | Node 实际弹道 | Node / H5 结算人数 |
|---|---:|---:|---:|
| 四人排队 | 2 / 2 | 2 | 4 / 4 |
| 八人取消 | 1 / 1 | 1 | 8 / 8 |

四人 Node 连续排队只提交最新请求，按冷却和前摇时钟核对实际第二发。四人 H5 通过「排队普攻」完成相同闭环。八人通过取消让首发之外的提交数保持为零。两组同伴可移动，成员各结算一次。 [普通宿主](../../artifacts/sdk-5-36/public/report.json) · [浏览器](../../artifacts/sdk-5-36/browser/report.json)

Rust 验证未对齐的冷却固定步、零宽限等号、释放与提交重验、实际命中、技能接管、取消与重连隔离。两端对非法身份和时序原子拒绝。修复 SDK-5-35 在发布 JSON Schema 中放错位置的 `recoveryCancel` 定义，本批逐一解析包内 Schema 的本地引用。 [原生测试](../../artifacts/sdk-5-36/native-verification.json) · [发布 Schema](../interfaces/publications-v6.schema.json)

包外验证隐藏交付样例与源码配置，以外置配置运行普通宿主。机制 52 的四人接管和八人显式取消回归通过。7316 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-36/delivery/independent-verification.json) · [机制 52 回归](../../artifacts/sdk-5-36/legacy52/report.json)

归档 SHA-256 为 `e08c445270167203fea2037ebf93ddf298d6c70941dc2490b732c020d71a7a87`。源码 crate SHA-256 为 `d58aad8ee122f3fa7a9f4f0d4880e921f676874b39ca62f5876a9b9a4f12ae8f`。二进制 SHA-256 为 `523c5807feb92dad9fabef401b15960be58fb89166ea4eb43156512d5b481033`。 [交付包](../../artifacts/sdk-5-36/delivery/match-kernel-0.38.0-aarch64-apple-darwin.tar.gz)

4243 使用本批独立账本。点击普攻后，在冷却内点击「排队普攻」；也可在释放前取消。 [打开演示](http://127.0.0.1:4243/)
