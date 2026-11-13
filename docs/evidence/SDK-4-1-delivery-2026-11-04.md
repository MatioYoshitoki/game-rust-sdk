# SDK-4-1 独立交付包验收（2026-11-04）

先冻结 [DL01～DL08](../batches/SDK-4-1-delivery.md)，再交付兼容补丁版 0.3.1。[接入与交付说明](../guides/sdk-delivery.md)

本地分发包包含经 Cargo package 验证的真实 `.crate` 与展开源码、21 项锁定第三方源、当前 aarch64-apple-darwin 普通 release 二进制、Rust 消费方、Python 持久参考宿主、独立配置示例、支持矩阵与 SHA256SUMS。二进制从展开源包在空 CARGO_HOME、离线模式下重新构建，未复制工作区构建结果。[交付包](../../artifacts/sdk-4-1/match-kernel-0.3.1-aarch64-apple-darwin.tar.gz)

新增 `sdk_info()`/`--sdk-info` 数据查询无需配置或 worker，声明实际版本、协议/配置映射、机制/恢复版本、策略及编译特性。预编译宿主 replay=false，显式加速请求拒绝；六类 Client 方法与原玩法/业务契约保持。

验收通过：

- 从交付源包执行默认与 all-features 同一组 643 项 Rust 测试和 Clippy `-D warnings`。源码包摘要与编译器完全相同时，原生结果及日志摘要复用；不是累计 1286 项。
- 验证副本删除 SDK 的 config、examples JSON 及外层 samples 后，再用另一空 Cargo 缓存离线构建消费方。依赖 manifest 均位于交付副本内，无原仓库或用户 registry 缓存依赖。
- 包外自有默认别名/id/hash、账户属性和资产 ID，通过普通时钟完成 23 个消费结果阶段：协议 1 六个 Rust 场景，协议 2/3 完整战斗/拾取/撤离/死亡，未知查询、原正文重试、补偿、激活后关闭及拒绝核对。
- 3 个真实强杀边界（结算前、提交后、活跃局）连续两次重启，按包外账本中的原配置/结果/金额/业务 ID 一次结算或退款；改变外部结果配置不影响原责任，不重建 World。
- 配置缺失、协议错配、未知字段、超长 JSONL 明确拒绝；SDK 信息查询在无 config 工作目录及错误默认配置环境变量下正常完成。文件被修改或删除时包校验失败，恢复原文件后通过。

原生验收期间补充了外层交付清单的 Python 版本记录，并修正 macOS `/var` 与 `/private/var` 的路径归属比较。SDK `.crate` 字节及编译器未变化，按摘要复用已通过原生测试；独立消费完整重新执行。

最终核对 125 个源码/文档输入、扫描整包 1225 个文件，无工作区绝对路径引用。原 20 份场景及协议 2/3 示例 raw/hash 保持。当前压缩包约 4.4MB，准确字节数、依赖、编译器及摘要见机器报告。[机器结果](../../artifacts/sdk-4-1/verification.json) · [源码/内容核对](../../artifacts/sdk-4-1/source-audit.json) · [原生结果绑定](../../artifacts/sdk-4-1/native-proof.json)

该包为本地工程交付，publish=false；工具链由接入环境提供，当前预编译目标为 Apple Silicon macOS，参考工具链为本次记录的 Rust/Python。其他平台、生产存储、正式发布和跨游戏实际接入另行验收。
