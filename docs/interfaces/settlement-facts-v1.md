# 结算附属事实

SDK 0.146.0 在机制 105、schema 7 和协议 6 支持可选顶层 `settlement_facts`。宿主用 `result_contract` 的结果 ID 作为键，为每个启用的结果登记 1～8 个不同的事实 ID。ID 为 1～64 个 UTF-8 字节且无控制字符。事实 ID 与下游用途由宿主维护。

Rust 确认核心结算提交后，为该席位发出一笔 `append_facts`，正文包含 `settlement_operation_id`、`result_id` 和按 UTF-8 字节排序的 `fact_ids`。可用原结算操作 ID 从持久账本读取相应的库存、积分及结果。宿主应原子校验关联的核心提交凭据、追加事实并保存幂等凭据。成功回执为 `facts_appended`，带 `commit_id`。附属追加独立于账户资产修订。

相同 `operation_id` 的正文固定，`attempt_id` 只在确认前一次未提交后递增。未知结果或超时先发 `query_operation`。查询发现已提交时返回原 `facts_appended`。宿主确认旧尝试已经隔离后，可返回 `not_committed`。拒绝、查询仍未知或自动尝试耗尽时，使用 `reconcile_operation` 继续核对原附属操作。授权修复成功也返回原追加的 `facts_appended`。

附属责任可随已结束的 World 继续收尾。成员终局以核心提交凭据为准。宿主需要继续接收业务请求和持久化检查点，直至附属责任结束。恢复校验结果路由、原核心提交凭据及冻结正文，对附属责任单独查询或恢复人工核对。 [配置与消息 Schema](settlement-facts-v1.schema.json)
