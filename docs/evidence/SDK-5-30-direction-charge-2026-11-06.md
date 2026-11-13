# SDK-5-30 方向蓄力验收

2026-11-06。SDK 0.32.0，协议 6，schema 7，机制 47。

普通和穿透方向弹道可配置最低与最高蓄力时间和最低威力。开始时 Rust 冻结方向。提前释放取消，达到最低时间可手动释放，达到上限自动提交。手动释放按实际持有时间冻结倍率，原提交事务负责资源、冷却、后摇和弹道。

CH01～CH06 通过。默认和全特性各通过同一套 1509 项 Rust 测试，含新增 8 项。96 项 Node 测试含 2 项新增私有准备投影测试。Clippy 和格式通过。交付包通过离线源码、普通包外宿主、真实 4 页和 8 页 H5，以及机制 46 的两套包外回归。 [批次范围](../batches/SDK-5-30-direction-charge.md) · [机器签收](../../artifacts/sdk-5-30/acceptance.json)

| 配置 | 提交方式 | 普通宿主持有时长与威力 | H5 持有时长与威力 | 普通宿主与 H5 结算人数 |
|---|---|---|---|---|
| 四人 | 手动普通弹道 | 540 ms / 4514 bps | 570 ms / 4685 bps | 4 / 4 |
| 八人 | 自动穿透弹道 | 1800 ms / 10000 bps | 1800 ms / 10000 bps | 8 / 8 |

每次成功提交的实际生命成本为 3。普通宿主核对实际灵力成本、跨成员 ID 拒绝、提前取消、迟到释放、实际弹道及终局。H5 用开始和释放按钮验证手动流程，用实际提交事件验证自动流程，并等待权威准备清空。 [普通宿主](../../artifacts/sdk-5-30/public/report.json) · [浏览器](../../artifacts/sdk-5-30/browser/report.json)

Rust 回归覆盖配置门禁、最小值等号、跨步自动提交、释放时间冻结、阶段位移、实际起点、失败原子性、两成员同时蓄力、重复输入和重连清理。Rust 与 Node 读取本人准备时钟并拒绝坏帧，保留上一份快照。 [原生测试](../../artifacts/sdk-5-30/native-verification.json)

离线验证从空 Cargo 缓存构建，包外普通时钟消费使用显式外部配置。旧机制 46 的准备和后摇速度及独立结算回归通过。 28715 份起点配置、原资料和旧验收证据哈希一致。原始 48 场景与 50 数值向量本批执行增量均为 0。 [包外验证](../../artifacts/sdk-5-30/delivery/independent-verification.json) · [旧机制回归](../../artifacts/sdk-5-30/legacy46/report.json)

归档 SHA-256 为 `73cb09201f68be07c23c348c15f0e6cced94ae00f357aec10da1d2b69414980b`。源 crate SHA-256 为 `4843ded2b66c74142fe192a6514d3410230587e1fab3088cefdebf5b017c54ee`。二进制 SHA-256 为 `1a2735cafe42295e7b99c9efa63af62bcddbb4613e0d87cecf0c50626fd11a5f`。 [交付包](../../artifacts/sdk-5-30/delivery/match-kernel-0.32.0-aarch64-apple-darwin.tar.gz)

4225 使用本批独立账本。 [打开演示](http://127.0.0.1:4225/)
