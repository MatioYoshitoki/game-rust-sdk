# SDK-5-20 多成员增量发布

2026-11-05；先冻结DP01～DP12，再实现。起点0.21.0/协议5/schema6/机制36；目标0.22.0/协议6/schema7/机制37。主要工作包R2.3，只补1～8名必需成员PVE的私有full/delta与恢复；不增加观战、模式、匹配机器人或新战斗能力。原协议1～5保留原全量语义，旧配置/hash/金融责任不改。

## 配置、契约与权限

新schema7继续使用members块和固定完整名单，新增严格replication块：ack_timeout_ms（100～60000）、baseline_ttl_ms（ack_timeout_ms～60000）、full_interval_ms（100～baseline_ttl_ms）、terminal_grace_ms（2×ack_timeout_ms～60000）。这些为初始化的工程预算。CreateV6字段沿CreateV5，但版本必须6，必须匹配schema7/机制37；混用在业务I/O前拒绝。Six Client方法和金融DTO/责任保持。

新PublicationV6明确recipient_id、connection、stream_epoch、revision，Output.id仍为publication_id；payload区分full与delta。full携带完整本人SnapshotV4及原因；delta引用准确base_publication_id/base_revision，包含逻辑时间、变更的头部/本人块，以及公共成员/NPC/资源/弹道的有类型upsert/remove/order。空集合、移除、可空字段和顺序均有明确语义；没有任意JSON路径、World补丁或宿主筛字段接口。Rust先生成本人权限视图，再计算差异，私有数据不能因增量基线泄漏。

每成员独立拥有一个已Sent基线、一份在途帧和最新目标视图。未drain的输出可同成员合并为最新，已drain帧正文不可变；未回执时只保留最新目标，不建立依赖未Sent帧的增量链。Enqueued不推进基线；Sent只推进该成员当前连接/epoch、已drain的对应帧，重复Sent幂等、矛盾终态/旧代/错成员/未drain回执明确拒绝。缓存有界，一人慢发不阻塞别人。

基线缺失/过期、发送失败/取消、在途回执超时，均由Rust清除该成员基线、提升stream_epoch并补最新全量；旧epoch帧/回执不能恢复旧基线。按配置周期发全量；增量编码不小于全量则选全量。共同激活/重新ready只认可当前连接/epoch已Sent的full，delta不能冒充ready。流重同步本身不扣费、不改CD、不重置成员玩法状态。

## 客户端恢复与生命周期

公共有限Action增加request_full{stream_epoch}：当前认证成员可请求自己的当前流重建。仍走原key/participant/entity/connection/sequence/request_id校验和幂等；拒绝旧连接、错epoch或未知字段，不能给别人设基线。请求使Rust提高本人成员流epoch并补全量，不暂停其它成员。

SDK提供有类型Rust解码器及浏览器/Node共用的纯展示解码器：验证MatchKey/recipient/connection/epoch/revision/base身份，完整重建精确Snapshot。精确重复帧无副作用；冲突重复、旧帧、未知/错基线、非法集合更新不能部分污染缓存。缺基线时客户端保留原状态，发request_full后用新full恢复；Node只转发原始帧及执行I/O，不计算差异或补玩法。重连只接受新连接full起步，旧页/旧流不能覆盖当前缓存。

成员终局采用self-contained full，并冻结其退出视图，不让重同步成为继续观察生存者的旁路。关闭后World仍立即释放；原金融责任继续核对，发布失败不得变成结算失败。普通宿主在金融未决归零且终局full已Sent后释放Runtime；若接收者缺席，在terminal_grace_ms内有限尝试，到期明确发本人成员publication_expired通知、释放瞬时发布缓存，不能伪报Sent。结算未知可继续收尾。过期后玩法/发布恢复拒绝，财务检查点仍可恢复；进程重启不恢复临时基线/World。

## 验收标准

| ID | 通过条件 |
|---|---|
| DP01 | 新typed DTO/schema/sdk-info及配置上下界/配对正确；未知/混用字段和不支持配置在I/O前拒绝，旧协议1～5仍只发原full。 |
| DP02 | 同World的1/2/4/8成员各自full→Sent→delta；增量按权限后视图计算，重建与Rust目标Snapshot逐字段/顺序相等，覆盖新增、更新、删除、空集合、可空值、状态/CD/背包和终局。 |
| DP03 | Enqueued、未drain、重复/矛盾、错成员/连接/epoch/ID/乱序回执不推进错误基线；只有实际当前Sent建立后续依赖，ready仅用当前已Sent full。 |
| DP04 | full/delta发送failed或cancelled、回执丢失/超时分别走新epoch全量，旧帧/迟到Sent不能复活基线；另7人继续正常输入/发布，不重复费用/状态/收益。 |
| DP05 | 客户端丢失基线/遗漏一帧/缓存损坏时不部分应用delta，request_full幂等且仅影响本人；收到新full后继续delta、实际战斗/拾取到结算；请求旧epoch/旧连接明确拒绝。 |
| DP06 | 前后台与重连重置本人的基线及ready；新连接先full，旧连接任何帧/回执/输入不能覆盖新页，其他成员继续活动。 |
| DP07 | 周期full、基线TTL、delta较大回退full及慢接收者合并有界；每成员最多一份drain后在途、一个Sent基线和最新目标；100次公共Runtime创建/关闭无World/线程/未决/遗漏发布。 |
| DP08 | Rust和独立JS解码器对真实Rust帧重建结果一致；严查重复/基线/身份/移除/顺序/版本，错误不污染旧缓存；SDK/Node不使用任意patch接口。 |
| DP09 | 撤离/死亡/Leave/绝对终局仍各自可靠；终局full发送失败可在Closed时补发，到期明确expired；不能伪报成功/活World，终局重同步不泄漏生存者新状态；未知核心不阻塞其他人。 |
| DP10 | 普通Node和四人/八人真实多页面、两套外部配置，实际收到且应用delta；注入合法传输failed/延迟和客户端丢基线/重连，完成战斗、竞争资源及独立结算，console/page异常0，无未声明未决。报告原始full/delta字节与数量，在声明场景证明增量较小，不以Node附加full或关闭玩法取巧。 |
| DP11 | 含正在发布delta的八人对局，部分准入/一席终局未决/核心回执丢失三个SIGKILL边界，各同一检查点恢复两次；原资产/成员/hash/操作保持，Closed财务收尾，不恢复旧发布基线或战斗。 |
| DP12 | 原1209项Rust/30项Node集合保留，default/all-features同集、Clippy及旧协议1～5关键链通过；同源普通二进制/源crate/vendor/包外Node与空缓存离线源码验证，旧配置、来源和现存SDK-5-17～19归档/证据逐字节保护；新预览独立端口/账本。 |

DP01～DP12全通过才签收。R2.3仅full/delta切片，不关闭观战/完整隐私组合、完整R2/M01/M14；原48场景/50向量本批不增加执行通过数，不寻找第二款游戏。历史artifacts用户清理例外沿原记录，仅保护开始时实际存在文件。

终局显示的grace期限从成员开始settling/terminal或Root关闭的较早时刻计算；失败/重同步不得延长。到期只结束该成员瞬时发布，不暂停同伴或改变金融责任。

退出视图从成员进入settling时即冻结；后续只更新本人确认终局阶段、房间阶段与逻辑时间。延迟/未知结算期间的request_full也不得读取生存者新位置。
