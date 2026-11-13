# B1 公开接口

crate 0.3.0 支持协议 1、资产协议 2 和配置结果协议 3。以下历史条目描述协议 1，后续差异见 [资产契约](assets-v2.md) 与 [结果契约](results-v3.md)。新接入通过 `Config::protocol_version()` 选择创建协议。

权威 schema 为 `src/protocol.rs` 中的 Rust enum/struct。消息包括无参数消息都拒绝未知字段和枚举；位置仅接受有限数；ID 最多 128 UTF-8 字节，matchId 最多 64，代和输入序号使用 u32，避免 Node 整数精度损失。JSONL 单行最多 256KiB；超长行按有界缓冲丢弃至换行并返回 INVALID_MESSAGE，下一条合法消息仍可处理，不能因此丢弃活跃 World。B1 不承诺 v1.1 历史草案或现有客户端 wire 的逐字段兼容。

推荐显式加载 `ConfigCatalog`，保留 `Runtime::with_catalog(...)` 所有者，再取得 `.client()`；便捷入口 `Runtime::spawn()` 的默认目录规则见下文。单 Runtime 对应一个对局，生命周期、包版本和业务耦合审计见 [SDK 接入指南](../guides/sdk-integration.md)。Client 仅暴露：

| 方法 | 输入与结果 |
|---|---|
| create | Create → Status；协议 1、准确 configId/hash、认证 principal、唯一 MatchKey/连接；只发读取请求 |
| submit_input | Input → Status/明确错误；有限 Action，不允许位置补丁/阶段驱动 |
| submit_business_receipt | BusinessReceipt → Status/错误；匹配原请求、尝试、revision 和提交凭据 |
| submit_transport_receipt | TransportReceipt → Status/错误；enqueued 不推进，sent 只证明宿主声明发送边界 |
| drain_outputs | Vec<Output>；冻结业务请求、完整状态和事件，无 World 引用 |
| close | Close → 幂等终态；撤销 World，保留已发责任 |

调用返回的是已处理结果；满 mailbox 未接纳返回 QUEUE_FULL。等待结果有 2 秒上限，超时返回 EXTERNAL_UNKNOWN，原命令可能继续完成；使用原 requestId 核对/重试，不能改 ID 重放资产操作。

JSONL 命令形态为 `{"type":"create","request":{...}}`、`submit_input`、`submit_business_receipt`、`submit_transport_receipt`、`close` 或 `{"type":"drain_outputs"}`。每行恰好返回 `{"ok":{"status":...}}`、`{"ok":{"outputs":[...]}}` 或 `{"error":"ERROR_CODE"}`。命令与返回顺序对应；宿主轮询输出不等于推进 tick。

Publication 包含当前 recipient/connection、完整 Snapshot 和独立 output ID；全量发布不依赖增量基线。Snapshot 同时提供 `basic_ready_at_ms`、`skill_ready_at_ms`、`recovery_until_ms` 三个权威绝对逻辑截止时间。服务端另记录 readiness epoch；同一 epoch 中已 sent 且仍在有界历史中的完整输出均可用于 CombatReady，更新的在途快照不会使它失效。后台清移动/动作并保护，同时撤销上一 epoch 的 ready 资格；必须先 Foreground，再以当前 epoch 的快照 ready，才能解除保护。部署期间切后台、前台恢复未 ready 又切后台也必须撤销旧轮资格。旧 epoch 的传输回执返回 STALE_PUBLICATION，不能解除保护。

B2-1 在原 submit_input 增加宿主生命周期 Action：`Reconnect{previous_connection,authenticated_principal}` 与严格空结构 `ConnectionLost{}`。前者的 Input.connection 是新连接，必须是当前前驱的 generation+1，principal 必须匹配原创建身份；认证由宿主负责，普通浏览器操作白名单不开放这两项。重连成功保留原 create 的幂等身份与 World，只替换当前连接并撤销旧发布资格；相同请求在有界历史内重复幂等，已被下一代接替的旧重连请求拒绝。普通输入和传输回执先检查当前连接，因此旧代即使重放历史成功请求也返回 STALE_CONNECTION。

ConnectionLost 清移动、中断未提交动作和撤离、进入保护，并发出 `ConnectionLost{generation,resume_until_ms}`。Reconnect 发出 `ConnectionChanged{generation}` 和新连接的全量，但仍须 Foreground/sent/ready。断线和新换代连接未 ready 最多保留 30000ms 逻辑时间，重复断线不延长，成功 ready 清除期限；到期发出 ConnectionExpired 并走幂等 close，释放 World、冻结结算/补偿责任。Draining/Closed/Faulted 不接受玩法重连，宿主只恢复冻结结果/责任视图。进程重启后的战斗热恢复不在本接口范围。

Create 可选 `publication_mode`，只接受 `batched`（默认）与 `interactive`，创建后不可切换。前者按至少 250ms 的逻辑间隔周期发布（30ms 固定步下通常 270ms），后者每个 30ms 固定步发布；事件仍可立即触发完整输出。此项只控制冻结输出频率，不改变固定步、玩法时钟、输入处理、碰撞或结算规则；队列与发布历史上限仍然适用。H5 实时试玩选择 interactive，加速验收保留 batched。

传输结果单调：enqueued 可以进入 sent、failed 或 cancelled；相同终态重复幂等；迟到 enqueued 不反转终态；不同终态冲突拒绝。最新完整输出失败/撤销时生成一份替代全量，重复失败不反复补发。较旧输出失败不能撤销较新的成功输出。关闭后迟到发送回执返回 CLOSED，不重新建立发布能力。

业务目录为 LoadParticipant、CommitAdmission、CommitSettlement、QueryOperation、CompensateAdmission、ReconcileOperation，仍通过原六类能力交互。入场/结算均要求单次原子写；Committed 使用精确下一 revision 和稳定 commitId，补偿的下一 revision 以原入场提交为基准。重复相同终态幂等，相反普通回执拒绝。

not_committed 必须是对本尝试的权威、已隔离迟到写入的未提交事实；单纯查不到数据库记录只能返回 unknown。写入 unknown 先 QueryOperation；读取可重试。同 ID/冻结载荷最多自动尝试三次。已关闭的入场被查证未提交时直接结束责任，不能启动新扣费；已知 committed 后，旧查询不得再安排写入。

结算/补偿被拒绝、三次重试耗尽或查询仍 unknown 时，输出 OperationNeedsReconciliation 和唯一 ReconcileOperation{operation_id}。其引用原冻结请求，不重算金额、不生成另一份奖励。存储服务核对或纠正原责任后返回 **仅能匹配该对账请求** 的 Reconciled{revision,commit_id}；普通 committed 不能直接反转此前拒绝。错误版本、提交凭据、实例或尝试拒绝，不部分应用。

对账 unknown 保持待处理；不能提供提交证明时，不允许把财产责任标成成功。对账 not_committed 可结束尚未准入、无扣费的实例；不能取消已经欠付的结算/补偿。核对成功后只应用一次，发出 OperationResolved 清除原请求和关联查询/对账的未决身份。World 已关闭时只处理冻结责任，不重建 World。受控宿主模拟人工核对服务；生产人工操作、事务耐久性及审计由后续真实存储接入验收。

B1 单人终局进入 Draining 后暂停玩法时间，只等待冻结责任；不继续生成周期快照或接受玩法/连接动作。该规则不意味着未来多玩家中一个 I/O 可以暂停整局。B2-2 另按下面的服务期限处理自动 I/O 和 World 回收。调用方不能将无回执的 Preparing/Draining 当作成功。CLOSED 表示 World 已关闭，仍应核对 Closed.pending_operations 与后续 OperationResolved；只有使用 B2-3 持久宿主，才可恢复退出后的资产责任。

## B2-2 外部期限

Rust 内部 poll_io 使用 worker 的 Instant 单调经过时间，宿主不提交时钟或 tick。自动业务请求/查询的单次期限为 5000ms，原尝试超时发出 IoTimedOut{operation_id,attempt_id}。期限在 Draining/Closed/Faulted 继续生效，玩法逻辑时间不推进它；加速验证的 I/O 等待仍是实际经过时间。合法重试重新计时，无效回执、重复 unknown 和过期查询不刷新期限。

静默读取退休为未知读取并结束准入，不产生扣费；迟到 Loaded 返回冲突，不新建入场。写入超时先 QueryOperation 同一原业务，再幂等关闭 World，未决责任继续留存。查询保存其发起时的原 attempt，旧负结果不推进新 attempt；进入人工核对后负结果不自动取消或重启原写入。查询超时只申请一次 ReconcileOperation，人工请求不被自动当作成功或无限重发。

Draining 另有 15000ms 服务期限，届满发出 IoDrainExpired 并回收 World，保留冻结账本。初次 World 建立后也设置 30000ms 逻辑就绪窗口，成功 ready 清除；超时按未激活入场补偿。Preparing 允许认证换代而不重发读取/扣费，未建立 World 时 ConnectionLost.resume_until_ms 为 null，I/O 期限继续约束准入。输出空间不足会延后到期处理但不丢失责任；有足够空间后继续查询/收尾。生产 I/O fencing 和崩溃耐久性仍由后续存储接入验收。


## B2-3 持久责任恢复

六类 Client 能力不变。非 Publication 的 Output 新增可选 `recovery`：`IoCheckpoint{state,sha256}`，包含版本、原 Create 身份摘要、逻辑/输入/输出序号、恢复代及有限 `FrozenOperation{request,terminal,refused,queried_attempt}`。它不含 World、实体位置或可修改玩法字段。宿主必须先将整批输出中的最新检查点、原业务请求和唯一事件一起持久提交，再执行任何库存 I/O 或交付页面。仅排空到宿主内存不构成持久接受终局。

Create 可选 `recovery:{generation,checkpoint}`，仅用于可信持久宿主的启动；新代必须大于检查点代。Rust 校验摘要、schema、配置/MatchKey/principal/原创建身份、操作目录、冻结金额、尝试与提交凭据后，原子建立 Closed 的责任账；校验失败不部分初始化。新代输出 ID 加入 `r{generation}`，避免旧输出碰撞，查询复用既有有限身份。浏览器没有恢复写入口。摘要检测意外损坏，不构成对主机特权者的签名。

恢复先查询未决原资产操作；仅有可信未提交 fencing 证明才可有限重试。持久结算保持原金额；没有持久结算的已扣费中断局只补偿入场，不创造战利品和修为。已转人工继续原 ReconcileOperation；持久核对证明可幂等回填，否则保持未决。发出 `RecoveryRestored{generation,compensating_entry}` 与 Closed/OperationResolved 等既有事件；整个过程不建立 World。

H5 的所有者 cookie、当前实例、结果和关键闭环事件落盘，HTTP 连接在服务重启时换代。默认 SQLite 文件独占，旧进程已退出后才隔离其未提交尝试；原库存变更、提交证明和人工修正必须同事务。生产异步队列/远程库存必须提供自身 fencing 契约，不能直接沿用本地进程退出证明。

## B2-4 宿主调度与返回责任

Client 的六类能力保持统一。Rust mailbox 容量由清单默认场景的 `input_capacity` 在构造时确定，样例基线为 64。QUEUE_FULL 表示命令未接纳，EXTERNAL_UNKNOWN 表示已接纳但完成状态未知，按原 ID 核对。worker 向已断开的 drain 响应接收者发送失败时放回原输出批次，保留 output ID 与恢复检查点。收到响应后的落盘由宿主负责。

H5 每会话有序任务最多 64，普通输入/ACK 共 56，控制任务预留 8；溢出 HTTP 429/QUEUE_FULL。控制也达到总上限时仍拒绝，无隐式排队。普通请求与 close 不插队，已接纳工作按 FIFO 执行。自动 I/O 不等待于这条队列；回执暂时无法入队时保留，之后继续处理。

外部 I/O 调度总保留 32、并行 8、每会话并行 2；饱和后冻结责任仍留在会话，不重算或取消。Promise 异常按 unknown，不能证明未提交；迟到完成仍送原业务/attempt 回执。仍在执行的远程写不会因 Rust 的 5 秒期限而释放槽位或自动重发。

宿主工厂支持可信代码注入 `ioExecute({sessionId,key,principal,request}, executeLocal) -> Promise<ReceiptOutcome>`；当前默认仍是同步 SQLite 参考实现。真实适配器的事务、幂等、CAS、fencing、人工核对与非阻塞要求见 [B2-4 契约](../batches/B2-4-flow-control.md)。HTTP 没有远程 provider 选择或凭据写入口。

## B3-1 场景与多物品

仍为六类 Client 能力。Create 可选启动目录中的 `b3-1-misty-wolves-v1` 及其精确 hash；不会接受任意地图 JSON 或 World 补丁。候选配置与恢复载荷一并原子校验，选择场景不能丢掉 recovery。B1 的基线行为保持；当前schema2原文哈希已改变，旧raw/hash归档供责任恢复。

新增有限意图 `Navigate{destination:[x,y]}`：只在 B3-1 的合法地面点上由 Rust 有界寻路，不能直接设置位置；Move/Stop/Background 取消旧导航。普通输入仍检查当前连接、顺序、保护和阶段。H5 的“前往撤离点”和脚本玩家只提交该意图，不驱动 AI 或 tick。

Snapshot 可选 `pve`，含稳定怪物 ID、位置、HP/MP、状态/施法、各物品背包、资源与 itemId 对应、击杀数和剑印层数。旧 `target` 继续作为最近存活目标的兼容视图，B3 的完整成员以 pve.monsters 为准。地形配置从只读配置接口传输，Native 决定可走、速度、视线、弹道阻挡与碰撞。

CommitSettlement 保留玄铁 carried/looted/kept/dropped 字段，新增可选 `items:[{item_id,looted,kept,dropped}]` 表示其他实际拾取物品；空数组不序列化，不改变 B1 请求。每个 itemId 唯一且不得重复主 item_id，逐项守恒，整批和修为在同一个原业务事务提交。SettlementApplied 同样可携带 items。库存服务不能把这些数量折算或合并成玄铁。

恢复校验根据配置中的怪物数量、冻结击杀修为、原掉落表数量上限、逐物品守恒和死亡选格约束检查多物品载荷。重试、查询与人工核对保持原 ID 和全部物品行不变。没有持久终局的中断仍只补偿原入场，不根据旧快照补发战利品。

H5 创建会话可选启动目录中的场景别名，省略时使用清单的默认场景。宿主保存所选场景并在重启时恢复。`/api/config?scenario=b3-1` 提供服务启动时加载的该场景配置，已有对局可按 sessionId 读取原版本。外部客户端的字段兼容由各自的接入验收覆盖。


## 启动配置与固定版本

配置加载属于构造阶段，Client 六类能力不变。推荐先 `ConfigCatalog::load(manifest)?`，将返回值放入 `Arc`，再调用 `Runtime::with_catalog(catalog.clone())`；同进程可向多个 Runtime 传入同一 Arc。加载失败返回 I/O/InvalidData 错误，不启动内核线程。`Runtime::spawn()` 使用进程级 OnceLock，第一次从 `MATCH_CONFIG_MANIFEST` 或默认 `config/catalog.json` 读取；调用者需保证文件存在，需自行处理启动错误时使用显式加载。`active(场景别名)` 返回共享配置，`id()` 与 `content_hash()` 用于 Create。Create 只选择目录中精确匹配的 id/hash；不提供运行中注册/热替换能力。

清单 `default_scenario` 指向默认启动别名，Rust 库可通过 `ConfigCatalog::default_config()` 取得其配置。`kernel-host --configs /绝对路径/catalog.json --catalog` 输出 `defaultScenario`、active、版本原文、规范化配置和加载统计，`--config-hash` 输出默认场景的配置哈希。库和宿主使用接入方提供的清单及其引用的 JSON，独立回放将验收配置包复制至临时目录。

H5 `GET /api/catalog` 发布 `defaultScenario` 和可选场景。`/api/current` 和会话信息携带 `configId/configHash`。`GET /api/config` 可用 `scenario` 选择启动版，省略时返回默认场景。传入 `sessionId` 时返回该所有者已有对局固定的版本，其他所有者返回 404。恢复旧画面时前端按会话版本重新加载地图和渲染缓存。SQLite `config_versions` 保存准确原文并核验摘要。源清单归档提供首次升级时的基线版本。[配置格式](configuration-v2.md)


## B3-2 普通储物袋

六类 Client 能力不变。Action 新增严格结构 `OpenChest{chest_id,expected_generation}`（JSON kind=`open_chest`）；箱ID最长64个UTF-8字节，代为正u32。不接受奖励、坐标、进度、成本或箱状态回写。仅新b3-2场景启用，旧场景明确拒绝。

PveView 可选 `chests`（旧场景为空时不输出），每项 `ChestView{id,type_id,position,generation,opened,respawn_at_ms}`。普通袋无封印读条，opened=true表示本轮已领取且处于重生冷却；逻辑时间到期后新代可开。事件为 `ChestOpened{chest_id,generation,resource_ids}` 和 `ChestRespawned{chest_id,generation}`。开启本身不写背包/仓库；只有已有 Pickup 成功后才进入本局资产。

同一输入幂等，其他请求重开已开启的一代返回NO_RESOURCE；代不匹配为REVISION_CONFLICT；距离/阻挡为OUT_OF_RANGE，地面空间不足为CAPACITY；角色/施法/撤离门禁沿用现有错误。失败不提交部分掉落或消费RNG。成功原子生成本代唯一资源、发事件并即时发布完整状态。

PVE配置有 `chest_spawns` 与 `treasure_rules`，点位、奖池、范围、容量和重生支持启动配置与版本固定；只支持即时落地普通容器机制，未知机制/字段拒绝。恢复检查按原配置、冻结击杀数、逻辑时间和有界重生次数核验物品上限，不再假定所有物品来自击杀；仍不恢复World或重新开箱。完整规则见 [B3-2](../batches/B3-2-pouch.md)。

## CONFIG-1 参数与展示

配置schema2、机制版本1，严格字段与注册机制见[参数目录](configuration-v2.md)。清单1～16个active，必有b1，active+archives≤32；原有三个别名保留种类约束，可新增其他别名。不新增运行期Client能力。schema1旧raw/hash只经历史兼容解释使用，不能用新版数值重新解释旧责任。

Snapshot新增 `max_hp/max_spirit`，来自Rust读取的角色事实；H5据此显示上限，旧持久快照缺失时回退到该会话原配置。`GET /api/catalog`返回启动别名/configId/configHash/展示名；`GET /api/config`另有params、inventoryCapacity、stackLimit、itemId和carriedQuantity，仍是只读DTO。按sessionId读取受所有者隔离。

死亡恢复按原配置堆叠/选格/损失策略核验逐物品载荷，支持多个完整堆叠和尾格，不重算冻结随机选择。成长系数0时不以修为0断定零击杀，改用可行击杀范围与实际掉落上界约束；零概率不贡献产量。未知物品、超容量、数量不守恒、非法损失或旧版本缺失仍拒绝。

## B3-3 分层交互

Client仍为六类。版本2的分层容器用原 `OpenChest{chest_id,expected_generation}` 开始/继续破解，全部解封后同一意图请求开启；不同阶段由权威配置/状态确定。新增严格 `CancelChestUnlock{chest_id,expected_generation}`，只取消匹配的活跃交互；原CancelAction也可取消。容器ID/代边界同OpenChest，未知progress、奖励或阶段字段拒绝。

ChestView可选 `seal:{layers,completed_layers,layer_progress_ms,layer_duration_ms,unlocking,interruption_reason}`，旧即时容器不输出seal，旧快照缺失按None读取。中断原因为cancelled/movement/damage/background/closed/out_of_range。新增ChestUnlockStarted、ChestUnlockInterrupted、ChestLayerCompleted、ChestUnlocked事件，均含箱ID/代；层完成含已完成层数，中断含原因。仅原ChestOpened表示奖励已成功生成地面实例，未拾取物不进入资产。

破解中攻击/技能/拾取/撤离拒绝，移动可中断；刷新/连接换代保护中断且不自动继续。正伤害优先于同一步进度完成。成功开启失败可原状态重试，不能重抽后部分提交。范围与U01～U08见[B3-3规格](../batches/B3-3-layered-container.md)。

## B3-4 混合模板和守护资格

Client仍为六类能力，无新增任意伤害/怪物状态或World写入口。机制版本3配置新模板和守护计划；普通Attack/Cast、OpenChest/CancelChestUnlock继续使用原公开意图与连接/代校验。

MonsterView在版本3可选提供 `template_id`、`radius`，绑定守护者另有 `guardian:{chest_id,generation}`。实例ID稳定，守护ID包含箱ID/代/序号。HP/MP上限、施法/自疗/冷却均来自实例自己的模板和状态，不能由浏览器写入。PveView.kills是计分击杀数，不等于全部死亡数；不计分模板死亡仍正常掉落配置物品/光球。

ChestView可选 `guardians:{pending,planned_count,alive,monster_ids}`；首次计划冻结前planned_count为null，受阻后保留已抽数量，monster_ids只列当前代已启用实例。零守护计划就绪后为pending=false、alive=0、空ID列表；无此能力的旧场景省略整个字段。

新增 `GuardiansReady{chest_id,generation,monster_ids}` 和 `GuardianSpawnBlocked{chest_id,generation}`，JSON kind分别为guardians_ready/guardian_spawn_blocked。Ready每代一次（含零守护），Blocked每代最多一次，重复空间重试不刷事件。pending时OpenChest返回GUARDIAN_PENDING，本箱本代任一守护存活返回GUARDIAN_ALIVE；代不匹配仍为REVISION_CONFLICT。归位、其他箱死亡、陈旧代或旧连接不能解除门禁；全部死亡后仍需显式破解/开启，击杀不直接发箱奖。

同World刷新/断线保留计划、绑定和实例；旧连接被隔离。重生清理旧绑定槽位的施法/弹道及玩家旧锁定，复用槽位建立新代ID。进程重启只恢复Closed责任/结果，不重新生成守护或World。完整G01～G08与配置预算见[B3-4](../batches/B3-4-guarded-container.md)和[配置目录](configuration-v2.md)。

## B3-5 区域遭遇与奖励门禁

Client仍为六类，浏览器只有Navigate/Move/Attack/Cast和原容器输入；没有spawn、complete或World写入口。配置版本4增加一次性区域遭遇，按权威位置和实例死亡推进。

PveView可选 `encounters:[{id,phase,monster_ids,required_monster_ids,defeated_required,reward_chest_ids}]`，空数组省略；phase为dormant/spawning/active/completed。monster_ids仅列已实际启用的本组实例，未生成预留槽位不进入MonsterView、target、碰撞或AI。MonsterView另可选encounter_id，旧怪物/旧快照省略。目标进度基于本组指定实例是否启用并HP=0，不以全局kills代替。

ChestView可选 `encounter:{encounter_id,completed}`。未完成时OpenChest返回ENCOUNTER_LOCKED，陈旧箱代仍先返回REVISION_CONFLICT；完成只解除此锁，守护、距离、封印和容量继续检查，不直接生成箱奖励。已完成资格不随箱重生撤销。

新增事件EncounterTriggered{encounter_id}、EncounterSpawnBlocked{encounter_id}、EncounterStarted{encounter_id,monster_ids}、EncounterCompleted{encounter_id,reward_chest_ids}，JSON kind使用snake_case。每遭遇各生命周期事件只发一次，生成受阻也只报一次；完成后重复输入/进出/伤害不重发。恢复仍只建立Closed责任，不重放触发或生成，范围见[B3-5](../batches/B3-5-encounter-loop.md)。

## B3-6 顺序波次

六类Client能力及有限Action不变；没有wave index、spawn、complete、clear enemies或奖励写入口。机制版本5配置显式waves，原版本4单组仍按原输出/原raw/hash恢复。

EncounterPhase增加waiting。新波次形式的EncounterView附加可选 `wave:{id,index,total,completed_waves,ready_at_ms,monster_ids,required_monster_ids,defeated_required}`。index从1开始，ready_at_ms仅Waiting给出绝对逻辑截止时间，否则null；其他字段为当前波。父monster_ids汇总所有已启用波实例，父required/defeated汇总整个遭遇；旧单组省略wave。MonsterView可选wave_id，与encounter_id共同组成波绑定，旧怪物省略。

新增EncounterWaveScheduled{encounter_id,wave_id,wave_index,ready_at_ms}、EncounterWaveStarted{encounter_id,wave_id,wave_index,monster_ids}、EncounterWaveSpawnBlocked{encounter_id,wave_id,wave_index}和EncounterWaveCompleted{encounter_id,wave_id,wave_index}，JSON kind为snake_case。每波安排/开始/完成各一次，受阻每波最多一次；父EncounterTriggered/EncounterStarted/EncounterCompleted仍整个遭遇各一次。旧单组不发新波事件，旧EncounterSpawnBlocked仍保持原规则。

中间波完成只安排下一波，Waiting/Spawning/Active仍ENCOUNTER_LOCKED。最后一波完成才解除父箱锁，依然显式解封/开箱/拾取，箱重生不重新启动波次。连接换代保留同World波与deadline，旧连接隔离；重启仅恢复Closed资产责任，不重放安排/生成。见[B3-6范围](../batches/B3-6-wave-encounter.md)。

## B3-7 Boss阶段视图

MonsterView可选 `boss:{phase_id,phase_name,phase_index,phase_count,epoch,attack_interval_ms,cast_chance}`，index从1开始，epoch新实例为1；普通怪省略，尚未生成的波次/守护仍不展示。字段来自Rust当前状态，客户端不能设置阶段/HP/epoch。HTTP多余Action字段拒绝INVALID_MESSAGE，未知动作拒绝UNSUPPORTED；Rust类型不会将额外数据转换成状态写操作。

事件 `BossPhaseChanged{monster_id,epoch,from_index,phase_id,phase_name,phase_index}`只在非致死正伤害实际推进时发一次，跨阈值不发中间阶段事件；`BossReset{monster_id,epoch,phase_id,phase_name}`只在活怪真正完成归位时发一次。DTO与事件沿原snake_case JSON规则，首次生成仍由原波/守护事件表示。

同World重连保留阶段/epoch和实际战斗状态，逻辑钟继续，已有归位到家仍重置；旧连接和重复输入不能重写。关闭后不推进，持久恢复只恢复Closed资产责任，不重建Boss或重放阶段事件。六类Client能力不变，见[B3-7](../batches/B3-7-boss-phases.md)。

## B3-8 模板积分与冻结摘要

机制7仅PVE启用；MonsterView可选kill_score为该实例有效分值，PveView可选scoring为 `{kills,credited_kills,kill_score,extraction_cultivation,death_cultivation}`。kills表示实际死亡，credited_kills保持原kill_credit计数，后两项是当前战果在不同结果下的权威修为预览。旧PveView.kills仍为原计分击杀，旧版本不附加这些字段。

事件 `KillScored{monster_id,template_id,score,total_score,kills}` 每次实际死亡一次，0分仍发；JSON沿原snake_case。CommitSettlement与SettlementApplied可选scoring为 `{kills,credited_kills,kill_score,templates:[{template_id,kills}]}`，新机制必填、旧机制禁止；模板行按ID严格排序，数量正、唯一、引用和时点生成上界合法，计数/积分/修为与原配置一致。提交/查询/核对和旧记录恢复都保留完整原正文与operationId。

六类Client不增加能力，无设置击杀/积分动作；公开HTTP拒绝多余字段和未知动作。新摘要进入既有受保护的I/O检查点和SQLite责任账，不保存World供重启发奖。恢复按实际模板次数校验掉落，不从修为反推击杀；旧版本继续旧校验。见[B3-8](../batches/B3-8-template-scoring.md)。

## B3-9 范围预警视图与事件

MonsterView可选area_warning={cast_id,skill_id,name,center,radius,started_at_ms,resolves_at_ms}，无预警省略。活动期间cast返回area技能ID，原两技能语义保持。monster_id+cast_id联合标识一次攻击，新守护代使用新monster_id/序号1。

事件AreaStarted含monster_id/cast_id/skill_id/center/radius/started_at_ms/resolves_at_ms；AreaResolved含monster_id/cast_id/skill_id/hit/damage；AreaCancelled含casts:[{monster_id,cast_id}]与reason(background/returning/owner_died/closed/replaced)。JSON仍snake_case；hit表示几何与效果通路通过，伤害可为0，沿原命中打断撤离/正伤害打断封印语义。命中护甲/暴击/吸血复用，不增加剑痕。

Background/断线/接管保护取消，不冻结时钟或返MP/CD；范围锁点不跟随移动，当前非致死伤害/阶段变化不取消。关闭可同时取消封印与预警，Engine先预留两项有界事件空间；32圈保护取消聚合一次。故障回收以终局状态为准，预警不进入I/O检查点，不重启重放伤害。六类Client和公开玩家动作不增加直接施放/伤害写入口，见[B3-9](../batches/B3-9-area-telegraph.md)。

## B3-10 地图目标与材料消耗

新增版本9能力：attune_objective{objective_id,expected_revision}、cancel_attunement{}仍通过原认证输入能力提交，无目标推进/发奖写入口。EncounterView.blocked_by、PveView.map_objective显示Rust权威依赖、完成、读条、成本与领取状态；AttunementStarted/Interrupted/Completed表达生命周期。只有一次成功完成扣材料，所有中断通过原交互清理路径执行；关闭预留同时覆盖调谐/破解与范围技能取消。

CommitSettlement与SettlementApplied可选map_progress，严格按有地图目标配置要求，其消耗用于逐物品守恒；主物品仍用carried/looted/kept/dropped，其他物品仍用ItemSettlement。Node直接存保留金额，不能从looted推断应返数量、重算消耗或把调谐完成当永久业务。Closed恢复须验证原配置、击杀/完成/成本与死亡剩余格，原body及operationId不变。旧配置不接受新增摘要。

## B3-11 支援治疗与主动命中

六类Client能力不变，无新增“直接打断/完成治疗”输入。Attack/Cast在Rust接受时记录逻辑时间，实体弹道携带该动作来源；只有窗口开始后发起的合法首次直接接触才可打断。MonsterView.support与SupportStarted/Pulse/Completed/Interrupted/Cancelled输出权威目标、预算次数、时点与生命周期；旧配置省略新投影。

准备不提交MP/CD/次数，首跳重验并一次提交，分跳钳上限。保护/关闭保留已生效回复与已用次数，取消未完成动作。关闭为支援聚合取消独立预留空间，拒绝关闭不能先取消或扣费。资产正文/核对责任不新增治疗业务，重启不重放回复；旧十一raw与旧责任继续按原版处理。

## B3-12 定向重击与借隙窗口

MonsterView可选heavy={skill_id,name,ready_at_ms,cast}；cast={cast_id,origin,end,front,half_width,started_at_ms,commits_at_ms,ends_at_ms,committed}。准备结束才释放，提交当步不推进；实际释放逻辑时点决定结束时间。PveView可选counter={monster_id,cast_id,started_at_ms,ends_at_ms,raw_damage}，仅当前有效实例/窗口有值。H5按权威坐标/时点投影，不裁决资格、消费或伤害。

事件HeavyStarted/Committed/Resolved与CounterOpened/Triggered表达实际动作和追加伤害；HeavyResolved区分contacted、damage、blocked、counter_awarded。HeavyCancelled聚合≤32个重击身份及至多一个反击身份，reason为background/returning/owner_died/closed/replaced/resource/expired；expired只结束机会，不取消同施法者后来发起的另一重击。

没有新的授予、反击或修改伤害输入。Attack仍序列化为`{"kind":"attack"}`，Rust变体改为`Action::Attack {}`以拒绝额外字段；合法旧JSON保持，伪造counter_damage/kill_score/boss_phase等数据在解码即拒绝。窗口前动作、错误目标、闪避、过期或无直接接触不消费。服务重启不重建重击、窗口或追加伤害，只履行原资产责任。

## B3-13 遭遇轮次、复位与收益证据

EncounterView可选reset={round,reset_at_ms,delay_ms,pursuit_radius,actual_defeats,rewarded_defeats}；MonsterView可选encounter_round。稳定monster_id跨轮保留，epoch与能力动作序号单调；客户端不传轮次，Rust在玩家动作接纳时固定轮次并在弹道选目标前过滤新轮成员。没有新的强制复位、清状态、授予奖励输入。

事件EncounterResetScheduled含encounter_id/round/resets_at_ms；EncounterResetCancelled聚合≤8个encounter_ids；EncounterReset发布新轮。计时内取消按原Returning理由清理动作，到期用Replaced语义清理残留；关闭单独预留一个计时取消事件。保护状态在圆内不启动复位，圆外已有计时继续推进，成功撤离/死亡/关闭清理。

CommitSettlement与SettlementApplied新增可选reset_progress={actual_kills,encounters:[{encounter_id,round,completed,defeats:[{monster_id,deaths,last_round}]}]}，旧配置禁止该字段，可复位配置必须完整提供所有已配置组。ScoreSummary保留收益击杀及首次模板计数；实时ScoreView.kills与reset_progress.actual_kills显示真实累计死亡，credited_kills/积分不重复增长。恢复核对排序/归属/时间与次数上界/当前轮完成、按模板首次击杀与非复位生成上界、物品守恒及map_progress，不恢复活跃战斗。
