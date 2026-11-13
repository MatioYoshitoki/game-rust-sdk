# SDK-5-20 多成员增量发布执行记录

2026-11-05，crate 0.22.0 / 协议6 / schema7 / 机制37。实现前冻结[DP01～DP12](../batches/SDK-5-20-member-delta.md)，[逐项机器映射](../checklists/SDK-5-20-acceptance.json)与[最终机器签收](../../artifacts/sdk-5-20/acceptance.json)绑定实际源码、日志、交付包摘要；只有全部条件通过才签收。

本批实现固定1～8名必需成员PVE的通用发布与恢复。Rust先生成本人成员权限视图，再生成有类型full/delta；每席只有一个Sent基线、一个在途帧与最新目标。Enqueued不推进基线；未drain帧可合并，已drain正文不可变。失败、取消、回执超时、TTL或request_full提升本人stream_epoch并补full；周期full与体积回退保持有界。只有当前实际Sent的full可ready。私有字段不广播，Node不生成差异或补算玩法。

客户端Rust与Node/H5共用的JS展示解码器均原子应用：错身份/连接/epoch/revision/base、冲突重复及非法集合更新拒绝且保留旧缓存。精确重复幂等；缺基线通过认证request_full恢复。重连新连接从full开始，旧连接帧不能覆盖新页面。公开契约及迁移见[协议6](../interfaces/publications-v6.md)、[SDK.md](../../SDK.md)。

退出视图从settling起冻结，未知结算期间的重同步也不能继续观察生存者新位置；后续只更新本人确认终局阶段、Root阶段和逻辑时间。World Closed立即释放，金融责任继续核对。grace从成员退出或Root关闭较早时刻计算，重试不续期；到期私有publication_expired释放发布缓存，不伪报Sent，也不修改金融结果。输出队列满时保留到期通知。普通宿主只在金融收尾且终局已Sent或明确到期后释放Runtime；进程重启不恢复World/基线。

实际修复了三处边界：合并未drain替代全量时保留重建原因；预备阶段重连后仍正确Join；被拒绝的request_full不清除宿主已确认的终局送达。H5定期账本查询仅返回账本，增量SSE不附加隐藏全量。

验收证据：

- 原生与Node：[完整回归](../../artifacts/sdk-5-20/native-verification.json)，默认/全特性同1240项Rust，保留原1209、新增31；42项Node保留原30，Clippy与格式检查。包含1/2/4/8成员精确重建、发送回执边界、超时/TTL/周期/体积回退、有界合并、100次真实Runtime创建/关闭、终局/未知资产与输出背压。
- 跨语言：[607份真实Rust帧](../../artifacts/sdk-5-20/interop-verification.json)，8名接收者，40 full、567 delta；JS重建逐字段/顺序等于Rust目标快照，重复幂等，默认/全特性向量摘要相同。覆盖实际弹道、掉落与背包修订。
- 公共边界：[31项公开用例](../../artifacts/sdk-5-20/public/report.json)，含I/O前拒绝、失败/取消/回执丢失、缓存丢失、单席未知核心、准备期重连、Closed终局替代全量及明确到期。另7席可继续输入，账户不重复准入/结算。
- 真实页面：[四人/八人H5](../../artifacts/sdk-5-20/browser/report.json)，两套外部配置、独立页面，实际丢弃一个服务器已Sent的delta、清除显示缓存、重连并重放旧连接帧；恢复后完成战斗、竞争同一掉落、撤离和逐人离场结算。console/page异常0、未决0、宿主释放。
- 崩溃：[三处SIGKILL与六次恢复](../../artifacts/sdk-5-20/crashes/report.json)。部分准入、最后一席终局未决、核心已提交但回执丢失，各使用同一冻结检查点恢复两次；活跃边界已有delta，恢复为Closed金融账，发布数量0，外部改参不改变原责任。
- 旧协议：[协议4的16项](../../artifacts/sdk-5-20/legacy4/report.json)、[协议5的20项](../../artifacts/sdk-5-20/legacy5/report.json)与包外协议1～3完整消费链。
- 独立交付：[源码验证](../../artifacts/sdk-5-20/delivery/source-verification.json)、[包外消费](../../artifacts/sdk-5-20/delivery/independent-verification.json)。空Cargo缓存、离线源码默认/全特性同集与Clippy；36个包外普通消费用例，SDK回归目录与包内samples在普通消费时隐藏；另用交付的H5服务、页面和纯解码器进行包外只读启动验证。普通release未启用replay。

本次H5发布JSON统计（包含恢复场景；只统计发布帧，不包含SSE/HTTP封装、地图或账本，不表示CPU/网络生产性能）：

| 场景 | full | delta | 发布JSON字节 | 相同帧重建为full的字节 | 减少 |
|---|---:|---:|---:|---:|---:|
| 四人/配置A | 27 | 712 | 727473 | 1756193 | 58.6% |
| 八人/配置B | 58 | 1966 | 2342757 | 6159969 | 62.0% |

交付包SHA-256为`542987dc429c8f980f99249d217c848216f579208c428df21c107c985113c2c4`；源crate为`e4e2be84cb38085ae97597a37d39d7f5ba64c4a492c31f33c728c051fe61278a`；普通二进制为`e6e8cf0be244b1e32ee47374362f46e9ed4cbda16ed07d0677a008ef549cb088`。所有接入报告、包内/工作区源码与最终签收复核为同一候选。

[独立预览](http://127.0.0.1:4195/)使用`.local/sdk-5-20-demo.sqlite`和同一普通二进制；[只读预览检查](../../artifacts/sdk-5-20/local-preview.json)无建局、无浏览器异常。旧4173/4190/4193验证台及账本未替换。本批起点实际存在的3523份旧配置、示例JSON、SDK-5-17～19归档/证据与17份原来源逐字节保护；用户已清理历史artifacts的例外继续登记，不补造证据。

R2.3仅full/delta与恢复切片，R2的3包仍部分实现，19包待实现；观战/完整隐私组合、完整匹配/自动机器人、16/24模式及其余战斗/地图/资产组合、生产负载与正式接入继续开放。本批原48场景/50向量新增执行数仍为0，不关闭完整M01/M14或完整原需求。
