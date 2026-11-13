# SDK-1 配置接入边界验收（2026-11-04）

接入方可独立提供配置清单。清单的 `default_scenario` 决定 Runtime 的启动配置、JSONL 的 `--config-hash` 结果和 H5 默认选择。旧清单省略该字段时使用 `b1`。Rust 根据配置声明校验结构、引用和机制版本。原 20 个验收场景的专用能力检查由测试执行。[配置接口](../interfaces/configuration-v2.md)

受控接入包只提供 `arena` 和 `outpost`，使用接入方自己的配置 ID。H5 自动选择 `outpost` 并完成战斗和结算。编辑地图配置后，同一二进制重启并加载新哈希，旧对局仍使用原配置原文、冻结结算请求和原业务 ID 恢复，资产只提交一次。[配置加载报告](../../artifacts/config-loading/verification.json)

默认与全特性各 612 项相同 Rust 测试通过，配置专项包含 93 项 Rust 与 10 项进程和浏览器检查。B1、配置加载、H5 三组通过，H5 基线覆盖 9 个浏览器场景。原 20 份场景配置的原文哈希核对一致。[回归记录](../../artifacts/sdk-1/regression-run.json) · [源码与报告核对](../../artifacts/sdk-1/source-audit.json)

本地 H5 已更新，原 SQLite 账本已备份并通过完整性检查，原场景入口和默认场景通过只读检查。[预览检查](../../artifacts/sdk-1/local-preview-smoke.json) · [备份记录](../../artifacts/sdk-1/local-upgrade-backup.json)
