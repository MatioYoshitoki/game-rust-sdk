# 地图启动加载验收（2026-11-03）

本步按用户确认交付“启动读取、校验、预处理、只读缓存、每局版本固定”。需求为 PRD v1.10 §3.13，验收标准为 [C01～C09](../batches/B3-1-config-loading.md)。原 v1.1 资料包与 B1/B3-1 导出原文保持不变。

## 实现与结果

- Rust 显式 `ConfigCatalog::load` 在工作线程启动前完成加载；同进程共享 `Arc` 配置和展开后的地形。删除外部文件后，两份 Runtime 仍可创建和排空输出，错误版本拒绝。
- 运行中改文件后再创建 Rust 子进程，仍使用 Node 服务启动时校验的私有快照。重启同一服务后，新局采用新地图ID、出生点、速度与哈希，二进制哈希不变。
- SQLite 保存精确原文。新地图上线后，旧待核对局按原哈希绘图与恢复资产责任，原 operationId/载荷不变；核对完成并重复强杀重启后，仍只结算一次，World保持释放。
- 非法RLE/索引/巨量计数、越界/不可达点位、速度/尺寸、未知规则、清单逃逸/符号链接、超大文件均拒绝。缺少旧版本、损坏原文与非法当前地图不能监听HTTP，原资产记录不变。旧局配置读取受所有者隔离。

专项机器报告：[verification.json](../../artifacts/config-loading/verification.json)。包含7项新增 Rust 测试、9项真实进程/浏览器检查和全候选源码哈希。浏览器异常0，截图为 [旧版待核对](../../artifacts/config-loading/old-version-pending.png) 与 [换图后恢复旧结果](../../artifacts/config-loading/old-version-restored.png)。

## 测量口径

本机 debug 构建中，加载两份基线配置并展开一份256×256地形约88.310ms；10万次缓存配置获取共15.754ms（约0.158µs/次）。该数字是一次开发机微测量，含当前机器调度影响；缓存共享由 Arc 身份断言证明，不以耗时阈值决定正确性。准确值以报告 `cacheMeasurement` 为准。

当前 H5 每局独立 Rust 进程，进程间分别加载；显式共享目录的多个 Runtime 才在同进程共享地图内存。这不构成正式 release 容量/长稳报告，也没有签收跨进程共享或战斗帧率提升。

## 回归

B1、H5、B2-1～B2-4、B3-1 与配置专项全部重新通过；8份报告均与当前候选源码匹配，见 [源码审计](../../artifacts/config-loading/source-audit.json)。默认/all-features测试唯一数量为123（原116+配置7），不相加重复运行。另通过12条Node链、2项协议、4项投影、9项H5、6项重连、12项超时、19项强杀恢复、6项SQLite、7项队列/I/O、8项并发浏览器和7项PVE浏览器/重启验收。历史批次 Markdown 中的116等数量保留当时口径；本步机器报告与本记录提供当前状态。

复现命令：

```sh
python3 scripts/verify_config_loading.py
python3 scripts/verify_b1.py --date 2026-11-03
python3 scripts/verify_h5.py
python3 scripts/verify_reconnect.py
python3 scripts/verify_io_timeouts.py
python3 scripts/verify_durability.py
python3 scripts/verify_flow_control.py
python3 scripts/verify_b3.py
```

地图文件/清单须随独立二进制交付。独立临时目录回放会携带声明的配置包；不依赖原工作目录。当前仅开放现有地图数据与点位，技能/掉落/结算仍按支持的批次规则拒绝越界变更；长期版本保留策略与后续玩法另批交付。

本地验证台 `http://127.0.0.1:4173/` 已用当前实现重启，账本在升级前备份。默认两个场景的API哈希与原文件一致，浏览器初始化异常0，见 [本地烟测](../../artifacts/config-loading/local-preview.json)。

B3-2扩展后的当前回归口径：配置Rust由7增至10，新增3项已计入B3-2的23项；完整Rust总数146。当前源码审计与测量以 [B3-2记录](B3-2-pouch-2026-11-03.md) 和对应机器报告为准，上文保留配置加载首次交付时的123项/微测量。
