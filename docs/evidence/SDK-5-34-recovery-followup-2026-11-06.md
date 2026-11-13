# SDK-5-34 后摇跟进施法验收

2026-11-06。SDK 0.36.0，协议 6，schema 7，机制 51。

普通方向技能可在前一技能后摇末段缓冲另一技能。每人保留最新有效意图，释放时进入原准备，成功提交才扣费。

FU01～FU06 通过。默认和全特性各通过 1537 项 Rust 测试，含新增 6 项。104 项 Node 测试含新增 2 项投影用例。Clippy 和格式通过。交付包通过空缓存离线构建、包外 Node、真实四页和八页 H5，以及机制 50 包外回归。 [批次范围](../batches/SDK-5-34-recovery-followup.md) · [机器签收](../../artifacts/sdk-5-34/acceptance.json)

| 样例 | Node 实际弹道 | Node / H5 生命总费用 | Node 跟进灵力 | Node / H5 结算人数 |
|---|---:|---:|---:|---:|
| 四人替换 | 2 | 5 / 5 HP | 7 灵力 | 4 / 4 |
| 八人取消 | 1 | 3 / 3 HP | 0 灵力 | 8 / 8 |

四人房先排队再换技能，Node 检查释放时的准备、固定方向和实际弹道。八人房排队后取消，观察原后摇和首发继续。两组的同伴均可移动，所有成员各结算一次。H5 验证器使用同一权威窗口的 DOM 更新触发按钮，防止把第二个后摇窗口中的操作计为替换。 [普通宿主](../../artifacts/sdk-5-34/public/report.json) · [浏览器](../../artifacts/sdk-5-34/browser/report.json)

Rust 覆盖未对齐后摇的固定步取整、窗口等号、被拒输入保留已有意图、排队不扣费、释放和提交分别重验、取消及重连清除。首次专项的非法起点断言使用了错误码 OutOfRange，调整为既有弹道起点契约的 InvalidTarget 后通过。 [原生测试](../../artifacts/sdk-5-34/native-verification.json)

包外验证隐藏交付样例和源码配置，以外置配置运行普通宿主。机制 50 的四人转向和八人过期回归通过。3702 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-34/delivery/independent-verification.json) · [机制 50 回归](../../artifacts/sdk-5-34/legacy50/report.json)

归档 SHA-256 为 `a57c9ce0f97792a8dc39c20355cb3e57fe46859757b51004bb0648990c6701d1`。源码 crate SHA-256 为 `442345231314500a89207ca91b3a00bec5e3220e0de9c4eaeb7238b3c074ccb8`。二进制 SHA-256 为 `3024d344e2a4aa43ae46cc91542c91306ddea212658ef89725ca6c3b11a003ba`。 [交付包](../../artifacts/sdk-5-34/delivery/match-kernel-0.36.0-aarch64-apple-darwin.tar.gz)

4237 使用本批独立账本。施放首个技能后切换到跟进技能，等「排队跟进」按钮出现。 [打开演示](http://127.0.0.1:4237/)
