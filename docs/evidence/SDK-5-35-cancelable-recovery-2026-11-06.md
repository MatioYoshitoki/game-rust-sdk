# SDK-5-35 可取消后摇验收

2026-11-06。SDK 0.37.0，协议 6，schema 7，机制 52。

普通方向技能可配置提交后何时开放取消。显式取消结束后摇，另一普通方向技能通过准入后可同一步接管。

CR01～CR06 通过。默认和全特性各通过 1543 项 Rust 测试，含新增 6 项。106 项 Node 测试含新增 2 项投影用例。Clippy 和格式通过。交付包通过空缓存离线构建、包外 Node、真实四页和八页 H5，以及机制 51 包外回归。 [批次范围](../batches/SDK-5-35-cancelable-recovery.md) · [机器签收](../../artifacts/sdk-5-35/acceptance.json)

| 样例 | Node / H5 提前结束时长 | Node 实际弹道 | Node 接管技能费用 | Node / H5 结算人数 |
|---|---:|---:|---:|---:|
| 四人接管 | 1770 / 1560 ms | 2 | 7 灵力 | 4 / 4 |
| 八人显式取消 | 1800 / 1560 ms | 1 | 0 灵力 | 8 / 8 |

四人房按权威时刻接管并完成新准备及提交。八人房取消后摇后恢复移动，原弹道继续飞行。两组的同伴均可移动，所有成员各结算一次。八人 Node 预验收首轮的弹道在取消前自然结束，调整试射方向后用仍在飞行的同一弹道完成复验。 [普通宿主](../../artifacts/sdk-5-35/public/report.json) · [浏览器](../../artifacts/sdk-5-35/browser/report.json)

Rust 验证早于开放、开放等号、自然截止、失败输入保留原队列、接管后提交重验及原费用与冷却保留。重连后使用新代身份，并保留此前提交的取消时刻。Rust 与 JS 对私有投影坏帧做原子拒绝。 [原生测试](../../artifacts/sdk-5-35/native-verification.json)

包外验证隐藏交付样例和源码配置，以外置配置运行普通宿主。机制 51 的四人跟进替换和八人取消跟进回归通过。5506 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-35/delivery/independent-verification.json) · [机制 51 回归](../../artifacts/sdk-5-35/legacy51/report.json)

归档 SHA-256 为 `a5b36c7ea46a3166946336ca3c0249f815b7aa0bf4c7f39afe467d2563b2c6c7`。源码 crate SHA-256 为 `c1771172bd0acdee8e24c34175a5088f13e65ac6cd8f22542c10b1d387d3b7ad`。二进制 SHA-256 为 `8ec727537a5752cf875d42615059f8cb7b963d3630f60617ecfa2991bee5fda2`。 [交付包](../../artifacts/sdk-5-35/delivery/match-kernel-0.37.0-aarch64-apple-darwin.tar.gz)

4240 使用本批独立账本。施放首个技能后观察后摇倒计时，开放后可取消或切换下一技能接管。 [打开演示](http://127.0.0.1:4240/)
