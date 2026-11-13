# B2-3 本地持久责任与重启恢复验收

日期：2026-11-03（实现于 11 月 2 日开始，最终回归跨日完成）。状态：本批受控范围通过。规格见 [B2-3](../batches/B2-3-durable-responsibilities.md)，机器结果见 [源码绑定报告](../../artifacts/durability/verification.json)。

## 交付行为

Rust 冻结的原业务与提交证明可导出为 I/O 检查点。恢复入口校验配置、创建身份、摘要、有限操作目录、金额与凭据后，只建立 Closed 责任账，不创建 World。Node 先将整批责任落盘，再执行库存操作或通知 H5；写盘失败保留原输出批次等待重试。

SQLite 将余额、寿元、修为和幂等/人工凭据一起提交。进程重启后先查询原业务；已提交的入场、结算、补偿或人工修正不会重复执行。没有持久终局的中断局核实并退还原入场；已有持久终局按原载荷履行。仍待人工的责任保持原 ID，不能创建新局绕过。H5 恢复结果、余额、闭环进度与核对入口；旧连接和其他所有者隔离。

## 本轮执行

| 层次 | 结果 |
|---|---|
| Rust 恢复专项 | 9 项通过，包含 100 次恢复查询有界/输出身份验证 |
| SQLite 存储专项 | 6 项通过：重开去重、事务回滚、人工修正原子性、双进程互斥、损坏封闭、整批输出回滚后重试 |
| 真实进程/浏览器专项 | 19 项通过；实际 SIGKILL 与同文件重启，不以正常关闭模拟崩溃 |
| Rust 完整回归 | 默认/all-features 各 94 项相同测试通过，唯一测试数为 94；Clippy `-D warnings` 通过 |
| B1 回归 | 12 条 Node 成功/故障链、2 项 JSONL 检查、独立临时目录回放、来源包哈希和冻结配置通过 |
| H5 等距回归 | 4 项坐标测试、9 项浏览器功能/性能检查通过，页面异常 0 |
| B2-1 回归 | 8 项 Rust、6 项浏览器检查通过，页面异常 0 |
| B2-2 回归 | 16 项 Rust、12 项浏览器故障检查通过，页面异常 0 |

19 项专项包括：入场意图落盘/SQL 提交前/提交后/内核回执后 4 项；活跃中断退款与连接隔离 1 项；终局排空未落盘/意图落盘/SQL 前后/回执后/结果落盘后 6 项；补偿已提交未回执 1 项；人工待办恢复 1 项；人工 SQL 前后/回执后 3 项；检查点/库存/配置损坏拒绝启动 3 项。终局和人工场景再次多次重启，金额与次数保持不变。

实测完整撤离恢复结果为仓库 **102**、修为 **30**、一次入场/结算、World=0、未决=0、闭环 5/5；未冻结终局的中断恢复为仓库 **100**、寿元扣除归零、一次入场/补偿、无结算和修为。待人工恢复仍是仓库 95、原冻结责任和核对入口；人工完成后才显示成功。

实时等距性能检查：SSE 间隔中位数约 31.6ms、p95 36.2ms；按键到权威位置变化约 20.1ms，无浏览器长任务。仅为此本机场景的观测，不代表生产容量。原始样本见 [性能报告](../../artifacts/h5/performance-isometric.json)。

## 复现与证据

```sh
python3 scripts/verify_durability.py
python3 scripts/verify_b1.py --date 2026-11-03
python3 scripts/verify_reconnect.py
python3 scripts/verify_io_timeouts.py
python3 scripts/verify_h5.py
```

最终报告中的所有文件 SHA-256 已核对当前源码；测试结束后清理临时服务和数据库。截图：[结果恢复](../../artifacts/durability/restored-result.png)、[中断退款](../../artifacts/durability/interrupted-refunded.png)、[待核对恢复](../../artifacts/durability/restored-pending.png)。完整回归报告：[B1](b1-results.json)、[H5](../../artifacts/h5/verification.json)、[连接恢复](../../artifacts/reconnect/verification.json)、[I/O 超时](../../artifacts/io-timeouts/verification.json)。

本地演示已更新至 `http://127.0.0.1:4173/`。默认状态文件为 `.local/h5-lab.sqlite`；旧版内存实例不迁移，新建实例开始保存。新实例仍使用独立的初始 100 库存，未将验证账户改为跨局账号经济。

本批没有签收战斗热恢复、跨机接管、磁盘/断电容灾、生产库存和远程队列 fencing、现有客户端、正式容量或全部玩法。下一步应收敛 B2 的并发、背压与生产 I/O 适配契约，再进入下一批内容。
