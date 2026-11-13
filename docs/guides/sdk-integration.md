# SDK 接入指南（协议 1/2/3/4/5/6 / crate 0.135.0）

接入方通过六个 Client 方法提交意图与外部事实、读取不可变输出。Rust 持有 World 并裁决玩法；宿主负责认证、连接、配置包以及原子业务存储。协议1～3支持单成员，协议4支持同一World两名必需成员、协议5支持配置化1～8名必需成员的有限PVE；一个Runtime对应一个对局、一个调度线程。多局可共享同一个 `Arc<ConfigCatalog>`，各自保留 Runtime 和独立业务身份。

Node 宿主的十分钟可运行路径（建账本 → 就绪 → 战斗拾取 → 撤离结算）见 [Node 快速上手](node-quickstart.md)。剩余配置、机制和验收工作见[三类矩阵](../checklists/sdk-requirements-gap.md)。机制19提供[定时防御状态](../interfaces/statuses-v1.md)，机制20增加[共享期限DEF叠层](../interfaces/status-stack-v1.md)，机制21增加[ATK/DEF通用修正](../interfaces/attribute-modifiers-v1.md)，机制22增加[吸收护盾](../interfaces/absorption-shields-v1.md)，机制23增加[通用周期调度与物理DOT](../interfaces/periodic-damage-v1.md)，机制24增加[自身周期治疗HOT](../interfaces/periodic-healing-v1.md)，机制25增加[受治疗修正](../interfaces/healing-received-v1.md)，仅适用schema4/协议3的PVE；具体范围和验收见ST01～ST12、SK01～SK10、AT01～AT10、SH01～SH10、PD01～PD10、HT01～HT10、HM01～HM10；机制26的[配置化双技能](../interfaces/player-skills-v1.md)按MS01～MS10验收。机制27增加[自身施法](../interfaces/self-skills-v1.md)，SS01～SS10冻结范围。机制28增加[严格敌方单位目标](../interfaces/unit-targets-v1.md)，按UT01～UT10冻结范围。其它属性与状态族继续另批。

本指南的启动命令使用宿主提供的配置和账户。后续按批次记录的旧样例和路径属于历史验收。

## 运行最小消费方

仓库根目录执行：

```sh
cargo run --locked --manifest-path examples/minimal-host/Cargo.toml -- \
  /absolute/path/to/game/catalog.json /absolute/path/to/game/account.json
```

独立消费方的 [Cargo.toml](../../examples/minimal-host/Cargo.toml) 以 `match-kernel = { version = "=0.135.0", path = "../..", default-features = false }` 引用 SDK。将其移到其他目录时，把 path 改为 SDK 检出目录即可。程序参数为 `<清单> <账户事实JSON> [模式] [场景别名]`；省略别名使用清单 `default_scenario`。JSON 初始化账户属性、库存、已有修为和入场年限；内核数值由清单引用的配置提供。

样例使用普通 Runtime，先执行读取/入场业务，再提交 Join，接收完整状态并回报 Sent，使用该发布 ID 提交 CombatReady。最后脚本玩家提交 Leave，由 Rust 按当前协议生成死亡结算。最终输出 JSON 包括配置哈希、原业务正文及凭据、账户余额、World 与未决操作数。完整战斗/撤离另见现有 [B1 规格](../batches/B1-minimal-loop.md)。

模式包括 `normal`、`unknown-settlement`、`refused-admission`、`close-before-ready`、`failed-publication`、`config-mismatch`。验收运行 `python3 scripts/verify_sdk_2.py`；它在仓库外复制、构建消费方并核对八项标准。脚本需宿主历史验收资料 `examples/minimal-host/account.json` 与 `config/b1.json`（游戏配表与账户事实按边界不入 SDK 源码与交付包，由宿主持有提供）。三类矩阵 [sdk-requirements-gap.md](../checklists/sdk-requirements-gap.md) 随交付包提供（包内版本链接指回 GitHub 仓库全量）。

## 初始化与六个调用

```rust,ignore
use match_kernel::{ConfigCatalog, Runtime};
use std::sync::Arc;

let catalog = Arc::new(ConfigCatalog::load(manifest_path)?);
let config = catalog.default_config(); // 或 catalog.active(alias)
let runtime = Runtime::with_catalog(catalog);
let client = runtime.client();
// Create 中使用 config.id()、config.content_hash() 和 config.protocol_version()。
// 保留 runtime；完成关闭和责任收尾后才 drop(runtime)。
```

先显式加载可在启动 worker 前返回配置错误。`Runtime::spawn()`、`Config::frozen()` 和 `config_hash()` 使用进程级默认目录，首次读取后固定；消费方应优先采用上面的显式方式。原文配置必须通过目录加载和校验；反序列化 `Config` 不能代替目录版本注册。[配置契约](../interfaces/configuration-v2.md)

| 调用 | 接入动作和成功含义 |
|---|---|
| `create(Create、CreateV4 或 CreateV5)` | 发送由配置确定的协议版本、目录精确 id/hash、可信身份、种子、连接代及唯一 MatchKey。成功表示创建已接纳；读取与扣费由后续业务输出驱动。 |
| `submit_input(Input)` | 发送有限 Action，带实体代、当前连接代、单调 sequence、稳定 request_id。重试同一输入须保留全部字段。普通客户端没有 recovery、事实或 World 写入口。 |
| `submit_business_receipt(BusinessReceipt 或 MemberBusinessReceipt)` | 对原 operation_id/attempt_id 回报外部事实；Rust 验证 revision/凭据并推进原责任。 |
| `submit_transport_receipt(TransportReceipt)` | 按原 output_id/接收者/连接回报 Enqueued、Sent、Failed 或 Cancelled。Sent 对应宿主声明的实际发送边界；不等于玩家已读。 |
| `drain_outputs()` | 有序取走冻结业务请求、Publication 与 Event。收到返回后，这批输出的保存和处理责任转到宿主。 |
| `close(Close)` | 同一请求重复幂等。回收 World 并保留已发业务责任；关闭后继续处理业务输出/回执。 |

`runtime.client()` 可克隆；这些句柄共享同一有界邮箱。保留 Runtime 所有者，单独保留 Client 不能延长 worker 生命周期。`drop(runtime)` 停止并等待线程退出，不替代 `close`、资产收尾或持久化。当前 Rust 调用是同步等待；异步服务应在受控工作线程调用，外部 I/O 使用独立有界执行队列。[详细接口与调度](../interfaces/independent-match-kernel.INTERFACE.md)

## 输出、回执与关闭

宿主按输出顺序处理：

1. 持久宿主先把整批最新 recovery 检查点、冻结请求和唯一事件持久提交，再执行任何库存 I/O 或发送快照。Publication 本身没有恢复检查点。协议4使用独立ResponsibilityV4聚合检查点，必须与整批输出原子保存。
2. `BusinessRequest` 按原 ID 和正文交给认证的账户存储适配器；原子提交库存、成长与幂等凭据，回报原尝试结果。金额、奖励、消耗与结果直接使用冻结请求。
3. `Publication` 只交给对应连接代的接收方。发送队列接受可回报 Enqueued；完成声明的发送边界才报 Sent。客户端收到有效全量后用该 ID 发 CombatReady。失败必须报 Failed/Cancelled，使用替代全量重新就绪。
4. `Event::OperationResolved` 结束对应责任；`SettlementApplied` 表示内核已应用验证后的提交证明。`Closed` 的 live_worlds=0 证明 World 已释放，pending_operations 仍可能非空。
5. 关闭后持续排空和回报原责任。记录 `Closed.pending_operations` 与后续 `OperationResolved`，未决归零才完成本进程的正常收尾；仍有未知责任则持久保留并交接核对。

不能仅在收到 `Status::Closed` 后退出；未决清零也不能代替持久化要求。业务查询可能晚于 Closed 才返回。进程重启用原 Create 身份、精确配置版本和可信检查点恢复 Closed 责任账，恢复流程不会重建战斗 World。[持久恢复契约](../batches/B2-3-durable-responsibilities.md)

最小宿主的本地脚本玩家是快照接收方，接收后才回报 Sent。其账户与日志均在内存中，适合验证接入流程；它没有生产鉴权、持久检查点和崩溃恢复能力。生产样例可参考现有 [H5 SQLite 适配](../../adapters/h5/persistence.mjs) 与 [有界调度契约](../batches/B2-4-flow-control.md)，按实际存储/传输系统重新验收。

## 业务结果和错误处理

| 结果 / 错误 | 宿主策略 |
|---|---|
| `Loaded` | 返回可信账户事实，principal 必须匹配认证身份。读取不能覆盖活跃 World。 |
| `Committed` | 业务事务已提交，带准确下一 revision 与稳定 commit_id。原操作重复返回同一证明。 |
| `Refused` | 存储明确拒绝原操作，由 Rust 决定关闭、补偿或核对。不能直接改报成功。 |
| `Unknown` | 写入可能成功或仍在执行；Rust 发 QueryOperation。无记录不能证明未提交。 |
| `NotCommitted` | 只有已隔离迟到写入的权威未提交证明才可返回；Rust 决定有限重试。 |
| `Reconciled` | 仅对应 ReconcileOperation 的授权调查/修正结果，关联原冻结责任；最小内存适配器无此服务，返回 Unknown。 |
| `QUEUE_FULL` | 本次命令未接纳，按原身份有界延后，不能无界堆积。 |
| `EXTERNAL_UNKNOWN` | 同步调用已接纳但响应超时，不能假定没有执行。保留原请求与顺序，查询输出/原业务并按既有幂等规则重试，不能换 ID 再扣费。 |
| `CONFIG_MISMATCH` / `VERSION_MISMATCH` | 创建前修正选中的版本，禁止静默换成默认图。 |
| `STALE_*` / `UNAUTHORIZED` / `CONFLICT` | 拒绝旧身份、相反终态或冲突正文；记录原因并重新同步可信连接/状态，不能覆盖内核结果。 |
| `NOT_READY` / `INVALID_STATE` / `OUT_OF_RANGE` / `NO_RESOURCE` / `GUARDIAN_*` / `ENCOUNTER_LOCKED` / `CAPACITY` / `REVISION_CONFLICT` | 业务/玩法拒绝，向玩家显示并依据新快照重新操作；修正内容时使用新的合法输入序号和请求 ID。 |
| `INVALID_MESSAGE` / `UNSUPPORTED` / `STALE_SEQUENCE` | 修正 schema/能力选择或输入序列；避免无限重试同一非法请求。 |
| `CLOSED` / `FAULTED` | 停止新玩法，保留冻结资产责任，检查输出和持久恢复/核对流程；不能据此宣告结算成功。 |

最小宿主对预设边界进行显式处理，其他错误直接退出并给出失败；其十秒看门期限用于发现验收挂起，不是生产服务的未决责任处置策略。生产宿主需要独立的超时、背压、持久化和告警实现。自动 I/O 期限、有限重试和人工核对流程由内核已有契约决定。

## 能力与业务耦合审计

| 当前接口 / 配置 | 当前定位 | 后续兼容方案 |
|---|---|---|
| MatchKey、Connection、request/operation/attempt/output ID，六方法和有界邮箱 | 可复用的生命周期、隔离及责任机制。单 Runtime 一个对局。 | 保留数据边界；多局调度由宿主构造多个 Runtime，容量需单独测量。 |
| manifest、默认别名、目录 id/hash、不可变地图、机制版本 | 可复用加载/版本选择；几何、模板、技能、掉落、波次与节点由配置初始化。 | 在现有注册能力内只改配置；新效果/事件需要实现和验收机制版本。当前配置格式仍承载首款游戏的成长公式。 |
| `ParticipantFacts.spirit_points`、`warehouse_quantity`；Snapshot.spirit | 当前一个能量池、一个主入场物品库存的事实模型。 | 未来以版本化资源/属性 schema 表达多资源，旧协议字段保持原义，不能自行把仓库数量解释为其他资产。 |
| `CommitAdmission.entry_years`、`CompensateAdmission.entry_years` | 首款游戏的入场成长成本；补偿绑定原提交。 | 后续用明确的类型化成本项替代，并建立旧字段映射及恢复测试；本批继续执行原正文。 |
| `Outcome::{Extraction,Death}`、Extract/Leave 相关 Action | 当前搜打撤终局与输入目录，Leave 被内核判定为 Death。 | 若新增胜负、竞速等结果，先定义新的版本化模式/结果能力与结算规则，禁止将新结果塞进既有 Death。 |
| `CommitSettlement.cultivation`、`ScoreView.*cultivation`、配置修为系数/境界属性 | 首款游戏的成长计算；SDK 当前会裁决其数值。 | 后续提供有限、类型化的收益规则与业务摘要，旧版本继续原公式。配置改名称不能完成语义解耦。 |
| `PveView.sword_marks`、map_progress/reset_progress 等具名摘要 | 当前机制及恢复校验需要的结构，部分仍带游戏命名。 | 分清通用计数/节点能力和游戏展示名，逐项迁移为版本化机制 DTO；旧冻结摘要须有读取路径。 |
| operation_id 由 MatchKey 加后缀生成，无 `game_id` | 内核未提供独立游戏命名空间。 | 现阶段宿主分配全局唯一 MatchKey，并按项目隔离配置目录、认证与持久账本。未来若加 game_id，需同步原创建摘要、操作 ID、检查点及存储键迁移。 |
| H5 页面、地图标签、受控账号与验收脚本 | 接入参考和能力测试内容。 | 游戏自行实现前端、账户/认证适配与内容配置；SDK 核心不根据场景别名硬编码地图能力。 |

通用机制已覆盖移动/导航、投射物/近战、施法中断、掉落/背包、容器/守护、遭遇/波次/阶段、范围预警、有限条件/动作节点、定时站位、治疗/重击/反击与冻结结算责任。准确的可选能力及组合校验见 [configuration-v2](../interfaces/configuration-v2.md)。现有机制只接受已经注册的结构，配置不能定义任意脚本或回调，协议1～3仍限制单成员；协议4只开放双成员白名单，均不能扩充既有终局目录。

本批形成耦合清单和迁移约束。当前可供相同规则族的消费方接入。[SDK-3-1 资产契约](../interfaces/assets-v2.md) 已将业务请求的成本/收益改为配置绑定的资产事务；[SDK-3-2 结果契约](../interfaces/results-v3.md) 进一步分离触发与策略，冻结配置结果 ID；现有数值参数名、成长视图和新增终局机制仍按后续批次处理。

## 交付与版本策略

| 版本维度 | 当前值 / 选择方式 | 升级约束 |
|---|---|---|
| Rust crate | `match-kernel 0.21.0`，`publish=false`；开发期以准确版本和本地 path 引用 | 当前未发布 registry 包、npm 包或 FFI。交付检出来源摘要、Cargo.lock、配置包及证据；消费方同时保存自己的 Cargo.lock。path 版本号不能锁定路径内容。 |
| JSONL 进程适配 | `cargo build --locked --bin kernel-host`，启动 `kernel-host --configs /绝对路径/catalog.json` | 每个进程一局，六种 type 请求逐行返回 `ok`/`error`，业务输出显式 drain；256KiB 行上限。启动 EOF 会结束 Runtime，退出前先关闭并交接责任。Node 消费方参考 `adapters/node/demo.mjs`；该脚本使用 replay 验收时钟。 |
| 公共消息 | `Create.protocol_version = config.protocol_version()`（1、2 或 3），权威类型为 `src/protocol.rs` | 未知输入字段严格拒绝。变更输入、枚举、字段语义须显式定义兼容版本；serde 可选字段不保证 Rust struct literal 或穷尽 match 的源码兼容。 |
| 配置格式 / 机制 | 配置 schema 2～7 对应协议 1～6，清单 schema 1。支持信息见 `kernel-host --sdk-info` | 能力由所选配置声明，启动校验；同一 id 下原文改变会改变 hash，每局固定准确版本。 |
| 恢复状态 | `IoState.schema_version = 1`，精确 id/hash 与创建身份绑定 | 升级前保存旧原文和检查点，验证旧责任读取；不能用新配置重新计算旧奖励。 |

SDK-4-1 本地交付版为 0.3.1，协议 1/2/3 的业务正文与历史责任保持。[独立交付说明](sdk-delivery.md) 覆盖 Cargo 源包、供应依赖和包外配置接入；SDK 支持信息由 `sdk_info()` 或 `kernel-host --sdk-info` 查询。本地独立打包和支持矩阵已通过 SDK-4-1；正式发布前仍需登记发布渠道、许可证并完成生产门禁。0.x 的公开源码或协议破坏性变更须提升次版本并附迁移说明；只有明确验证兼容的修复才使用补丁版本。涉及资产/恢复时，保留旧版本读取路径或提供经过验收的迁移工具，再切换新局。

机制26在schema4/协议3 PVE中保留skill主技能，并允许additional_skills配置一个附加弹道技能。两项独立成本/CD，共用灵力、单个准备动作与后摇；移动、取消、切技能、前后台或重连不能缩短已提交后摇。Snapshot.player_skills提供按配置顺序的绝对CD，pending_action显示准备动作；ActionCommitted.skill_id标识已提交技能。旧skill_ready_at_ms仍仅表示主技能，机制1～25省略新字段并保持原动作策略。重启恢复仅核对冻结资产，清除临时技能视图。公开DTO增加可选字段，Rust结构体字面量需适配。

机制27新增玩家self_status技能和CastSelf输入（kind=cast_self、skill_id），主/副槽均可使用。配置只含ID/成本/前后摇/CD及1～2项on_commit_self_statuses；严格拒绝弹道几何/伤害和hit绑定字段。固定步提交时沿原整组容量/灵力预检施加合法自身状态，不创建弹道或直接攻击；与弹道技能共用准备/后摇和灵力、各自CD。0前摇仍在下一固定步提交。机制1～26的配置和输出保持，旧Cast必须有方向，新CastSelf禁止方向/目标等多余字段。Action新增枚举分支，Rust穷尽match需适配；六类Client/协议1～3/资产及恢复schema保持。

机制28新增unit_projectile与CastUnit（kind=cast_unit、skill_id、target{id,generation}）。只支持当前World已发布的存活敌人；接受/提交重验身份代次、资格、中心射程及视线，提交前失败免扣。提交时按合法当前位置冻结方向，沿原弹道/伤害/合法状态执行，不追踪或换人。非目标先接触只阻挡不受伤，目标失效/换代中止在途弹道，不退款或回滚CD。MonsterView、准备视图与单位提交事件增加可省略unit_target；旧1～27不输出该字段，旧配置/责任保持。Action/Error枚举与视图字段为源码变更，Rust消费方需适配；六类Client/协议1～3/资产及恢复schema保持。

机制29增加ground_circle / CastGround，明确世界坐标position；接受/提交双重校验地图半开边界、可行走落点、射程与视线，严格拒绝而不裁剪。提交前原子预检灵力/self状态/两计划容量；成功冻结中心/攻击量、扣费/CD/后摇并发布预警，至少下一固定步才按当前敌人位置结算一次圆形伤害，沿原直接命中/DEF/盾/状态/掉落链。PveView.player_area_warnings和pending_action/ActionCommitted.ground_position仅机制29输出；新增PlayerAreaStarted/Resolved/Cancelled。死亡/终局/关闭/实际换轮清计划，移动/后台/同World重连保留已提交计划且不退款，重启不恢复临时战斗。公开Action/事件/视图源码消费需适配，协议和资产责任保持。

地面目标使用[机制29接入契约](../interfaces/ground-targets-v1.md)，不通过方向弹道输入模拟；选择、显示与薄转发归宿主，落点/计划/命中/清理归Rust。

机制30新增ground_field，复用CastGround严格选点及接受/提交重验，配置必填duration_ms/tick_interval_ms。首跳在预警结束后等待一个间隔，按计划时刻周期结算且截止点排除，最多128跳；当前目标位置/DEF/盾/闪避、原伤害/命中状态/击杀掉落链保持，直接攻击量提交冻结。只提交扣费/CD一次，与单次圆形共用每World两个地面计划槽。PveView.player_fields和PlayerFieldStarted/Tick/Ended仅机制30输出；来源失效固定结束，死亡/终局/关闭/实际换轮清理，后台/移动/同World重连保留已提交区域，重启不恢复临时战斗。旧1～29保持，公开视图/事件枚举源码消费需适配，协议和冻结资产责任保持。

[机制30持续区域](../interfaces/persistent-fields-v1.md)复用严格地面输入，按固定计划周期接触当前目标；建区/伤害/生命周期归Rust，宿主只负责输入/传输/持久责任和视图。

机制31允许ground_field配置可选contact_policy（max_targets_per_tick 1～32、max_total_hits 1～128、repeat_interval_ms 0～60000）。0为同一ID/代次每区域一次，正数按上次成功接触的计划时刻计算，等值合法，进出/重连不清历史。每周期按距离平方/ID/代次排序；真实直接接触（含0HP损失/全盾吸收）消耗名额和预算，闪避/阻挡不消耗，额度满停止后续目标/状态。总预算0以hit_budget_exhausted立即只结束一次；成本/CD仍只在提交时处理。通用身份/时间/计数组件首批仅接入持续区域，历史≤128；配置时FieldView.contact_policy和Tick.hit_targets/remaining_hits输出权威事实，省略策略及旧1～30保持原形状/顺序/语义。公开DTO/枚举源码消费需适配，协议与冻结资产责任保持。

[机制31命中策略](../interfaces/contact-policy-v1.md)首批接入持续区域：Rust持有身份历史、计划间隔、成功接触预算及结束责任，宿主只提交意图和显示权威余量/历史；省略策略保持旧行为。

机制32新增piercing_projectile（方向Cast，必填ttl_ms与max_hits），按连续扫掠顺序接触敌方身份、墙同距优先截断，每身份每发弹一次、总成功接触耗尽结束。复用ContactBudget，闪避不耗、全盾0损失耗预算。最多2发在途，提交原子扣费/开启独立CD，死亡/终局/关闭/实际换轮结束。同World重连保留位置/期限/行程/历史/预算，重启仅恢复冻结责任。所有数值/绑定由配置初始化，旧1～31省略新增视图/事件；公开DTO/枚举源码消费需适配。本批仅方向型直线穿透，不代表完整弹道家族或M05完成。

机制33允许piercing_projectile显式配置repeat_interval_ms（0～60000整数）：0每身份一次，正数按该发弹最新成功接触时刻+间隔给资格；仍需真实扫掠，未接触不补跳，每步每身份最多尝试一次。每次成功含全盾0损失消耗原max_hits总预算并刷新最新时刻，闪避/失效不耗不刷新，成本/CD只提交一次。配置随接受冻结，同World重连保留历史/计时/预算，死亡/终局/关闭/实际换轮回收；重启仅恢复冻结责任。显式配置才输出弹道repeat_interval_ms、正间隔contacts.next_eligible_at_ms和Hit.hit_at_ms，旧1～32及省略配置的形状保持。公开DTO/事件源码消费需适配；本批只扩展移动接触频率，不代表完整M05。

SDK-5-16 / 0.19.0 / 机制34：提供[固定线形区域](../interfaces/line-fields-v1.md)，复用原周期/身份预算和资产契约。

SDK-5-18 / 0.20.0 / 机制35：新增[同World双成员契约](../interfaces/two-members-v4.md)。协议4配schema5，两名必需成员共同激活、各自输入/资产/终局、逐接收者私有全量；普通Node和双页面H5接入命令见[源包指南](../../SDK.md)。原协议1～3仍为单人。新增OutputBody变体的Rust源码迁移和封闭能力清单见该契约；未包含标准8席、delta、旁观及全部单人机制的双人组合。

SDK-5-19 / 0.21.0 / 机制36：[协议5配置化成员](../interfaces/members-v5.md)，固定完整名单1～8、独立责任、共同激活与私有全量。原协议4仍严格两名，输出/回执和金融DTO复用*_v4标签；新Rust CreateV5与CreateRequest::Members迁移见契约。普通Node/H5命令见[SDK说明](../../SDK.md)。


SDK-5-20 / 0.22.0 / 协议6/schema7/机制37：[逐成员增量契约](../interfaces/publications-v6.md)。CreateV6配replication预算，金融DTO保持；新增PublicationV6、RequestFull及PublicationExpired枚举分支需源码适配。只有实际Sent推进本人基线，ready只用当前已Sent full；客户端使用Rust ReplicaV6或交付web/replication.mjs原子重建。缺基线请求本人当前epoch全量，重连新连接full起步；终局到期明确结束显示同步，不能回滚已提交资产。普通Node/H5命令和升级说明见[SDK.md](../../SDK.md)。


SDK-5-21 / 0.23.0 / 机制38：[多人周期契约](../interfaces/multiplayer-periodic-v1.md)。保持schema7/协议6，开放成员↔NPC敌对DOT与各自自身HOT；NPC视图新增可省略statuses数组，严格JS/Rust结构体字面量按契约迁移。Rust投影本人来源及NPC自身状态后再diff，private周期/移除事件按实际来源和目标路由。来源退出不等待金融回执；同World重连保持原实例/期限，正常到期不重新授予；重启不热恢复周期。数值/绑定沿配置初始化。命令见[SDK.md](../../SDK.md)。


SDK-5-22 / 0.24.0 / 机制39：[多人盾接口](../interfaces/multiplayer-shields-v1.md)。保持schema7/协议6，配置shield_bps、shield_cap_bps、时长、来源分组与refresh/ignore；玩家/NPC提交自身及敌对方向弹道命中目标可授盾。本人及NPC视图增加可选shield_total，39输出整数含0，旧机制省略、null拒绝；Rust字面量补Option字段，严格JS消费使用随包解码器。NPC合计公开，可见实例及consumptions只含本人来源/NPC自身；各接收者私有amount不得相加作为游戏总吸收。普通命中与DOT统一DEF→盾→HP，DOT.absorbed为真实总吸收。重连不重授，settling起清理来源与目标，重启只冻结金融责任。命令见[SDK.md](../../SDK.md)。


SDK-5-23 / 0.25.0 / 机制40：[统一多人战斗接口](../interfaces/unified-member-combat-v1.md)。schema7/协议6新增多人cast_unit{skill_id,target:{id,generation}}，目标使用当前发布的存活敌对NPC身份。准备/提交均由Rust验证当前资格/射程/视线，提交冻结方向/攻击，在途不跟踪或转绑新代；阻挡不误伤。内部[统一入口](../../src/world/two_members/combat.rs)承接既有直接/DOT/HOT/盾及新单位计划。ProjectileViewV4新增可选unit_target，仅单位计划存在、null拒绝，普通/NPC/旧机制省略；Rust字面量补Option字段，严格JS用随包解码器。现有Client六方法与金融恢复DTO不变。Node/H5发送发布身份和意图，不裁决敌我/命中/数值。当前PVE关系策略与友军/PvP/其它动作限制以接口为准。


SDK-5-24 / 0.26.0 / 机制41：[多人圆形计划接口](../interfaces/member-circle-plans-v1.md)。schema7/协议6开放ground_circle/ground_field/cast_ground{skill_id,position:[x,y]}；接受与提交校验由Rust执行，预警/有限周期和接触预算共用[计划](../../src/world/two_members/areas.rs)，接触/伤害沿统一入口。配置members.area_capacity 1～8及持续contact_policy，并满足输出和容量×单区总命中≤128历史工程预算。SnapshotV4.areas与delta.areas新增Option<Vec<AreaPlanViewV4>>：41 full含数组包括空，旧省略、null拒绝；delta存在完整替换、[]清空、省略不变，Rust字面量补Option字段、严格JS用随包解码器。公开计划使用全局id，不含私有请求/冻结攻击/绑定/成本；准备/提交仅本人。后台/重连不重授已提交计划，来源settling立即清理，重启只金融责任。Node/H5选择坐标并显示Rust结果，数值与绑定由配置初始化。


## SDK-5-25多人固定胶囊（2026-11-05）

SDK-5-25按LA01～LA12限定签收，0.27.0/协议6/schema7/机制42。在同一World区域计划上增加line_field固定胶囊，复用连续扫圆墙裁剪、统一调度/ContactBudget及直接接触/伤害/状态链。长度/宽度、预警/周期、重复间隔/预算、绑定和容量均从配置初始化。公开line几何随areas完整替换增量发布，旧circle/field省略该字段；重连保持几何/期限/预算，来源settling立即清理，重启仅金融责任。

默认/全特性同1408项Rust（保留1376、新增32）、74项Node（保留68），3416份真实帧精确跨语言重建；64项公共边界、4页/8页H5及Node实际施法/战斗/共享拾取/独立终局、三处强杀各两次同检查点恢复、100次带线形Runtime释放、46项包外普通消费及空缓存离线源码全部通过。原48场景/50数值向量新增执行数0；3包映射完成、7包部分实现、15包待实现，22包尚未完整关闭。

[批次范围](../batches/SDK-5-25-member-line-plans.md)、[接口](../interfaces/member-line-plans-v1.md)、[验收记录](../evidence/SDK-5-25-member-line-plans-2026-11-05.md)。Node使用member-line-host.mjs，H5使用member-line-server.mjs与独立4210端口/账本；配表examples/member-line-host（交付samples/member-line）。来源映射498项、14M/9I/9P完整缺口及22个未闭合工作包保持，不把几何适配计为完整玩法/正式接入完成。


## SDK-5-26多人穿透弹道（2026-11-05）

SDK-5-26按PP01～PP12限定签收，0.28.0/协议6/schema7/机制43。多人piercing_projectile沿cast方向输入，复用同一World弹道容器、连续扫圆与敌对目标迭代器、ContactBudget、直接伤害/盾/DOT/死亡归功链。所有速度/半径/射程/TTL、预算/重复间隔、成本/CD/绑定由配置初始化；普通/单位/NPC/穿透共享容量及128份历史工程预算。每代一次或按实际逻辑步含等号重复，一步同身份一次且不补跳，墙同刻优先、TTL截止先结束。重连保在途余程/预算/历史/绝对期限，来源settling立即清自己的穿透，重启仅金融责任。

默认/全特性同1442项Rust（保留1408、新增34）、82项Node（保留74），3496份真实帧精确跨语言重建；64项公共边界、4页/8页H5及Node实际施法/战斗/共享拾取/独立终局、三处强杀各两次同检查点恢复、100次带穿透Runtime释放、48项包外普通消费及空缓存离线源码全部通过。原48场景/50数值向量新增执行数0；3包映射完成、7包部分实现、15包待实现，22包尚未完整关闭。

[批次范围](../batches/SDK-5-26-member-piercing.md)、[接口](../interfaces/member-piercing-v1.md)、[验收记录](../evidence/SDK-5-26-member-piercing-2026-11-05.md)。Node使用member-piercing-host.mjs，H5使用member-piercing-server.mjs与独立4213端口/账本；配表examples/member-piercing-host（交付samples/member-piercing）。Rust字面量适配piercing/结束reason，严格前端使用新随包解码器；六类Client和金融/恢复契约保持。来源映射498项、14M/9I/9P完整缺口与22个未闭合工作包保持开放，不将有限穿透适配计为完整多人/模式或正式接入完成。


## SDK-5-27配置化多技能槽（2026-11-05）

SDK-5-27按MS01～MS12限定签收，0.29.0/协议6/schema7/机制44。skill＋additional_skills数组初始化1～8个静态技能，顺序和全部数值由配置决定；复用现有按ID施法、每成员/每技能独立CD与费用、共享准备/后摇及区域/弹道/状态/伤害流程。末槽的类型/数值/绑定/最坏预算完整校验，不裁掉末槽；旧1～43保留两槽与原存在规则。私有有序列表、准备/CD与重连/退出沿原投影和事务，无新Action/Client方法或金融/恢复DTO。移动保留准备并提交重验，取消/保护中断未提交无成本；重连保已提交CD/后摇/计划，重启仅金融责任。

默认/全特性同1474项Rust（保留1442、新增32）、90项Node（保留82），2488份真实帧跨语言逐字段精确重建；64项公共边界、四槽/四人与八槽/八人Node/H5实际逐槽提交及战斗/共享拾取/独立终局、三处强杀各同检查点两次金融恢复、100次多槽Runtime释放、50项包外普通消费与空缓存离线源码全部通过。原48场景/50数值向量新增执行数0；3包映射完成、7包部分实现、15包待实现，22包尚未完整关闭。

[批次范围](../batches/SDK-5-27-member-skill-slots.md)、[接口](../interfaces/member-skill-slots-v1.md)、[验收记录](../evidence/SDK-5-27-member-skill-slots-2026-11-05.md)。Node入口member-skills-host.mjs、H5入口member-skills-server.mjs，独立4216端口/账本，配表examples/member-skills-host（交付samples/member-skills）。客户端按配置遍历技能，使用新随包严格解码器；页面处理无有效单位目标并显示INVALID_TARGET，无未捕获异常。来源498项路由、14M/9I/9P完整缺口和22个未闭合工作包保持，不将静态技能数量扩展计为完整施法/多人/模式签收。


## SDK-5-28技能生命成本（2026-11-06）

SDK-5-28按HC01～HC10限定签收，0.30.0/协议6/schema7/机制45。每个既有技能可选配置hp_cost（fixed或current_hp_ratio，value及min_hp_after_cost完整提供）；接受冻结定义，提交按当前HP先floor再最低生命截断，在目标/灵力/状态/计划全部重验后与灵力/CD/效果原子生效。声明成本的本人ActionCommitted.hp_cost发布实际整数（含0），未声明与普攻省略，公共费用不泄漏。成本不走伤害链，不消耗盾或触发归功；未提交无费用，成功后重连/取消不退款或重复收费。既有HP/CD full/delta、金融及仅Closed重启责任保持。

保留原1474项并新增20项Rust，默认/全特性同1494项；原90项加4项Node同94项，Clippy/fmt通过。2488份真实发布逐字段跨语言重建，77项公共边界、四槽/四人与八槽/八人H5、三处强杀各同检查点两次金融恢复、原机制1～44关键链、52项包外普通消费、空缓存离线源码/最小消费者全部通过。原hp-ratio/hp-min/hp-clamped三向量从原JSON精确绑定并经真实Rust提交；本批新增执行3/50向量、0/48场景，498内容项路由与14M/9I/9P完整缺口保持。3包映射完成、7包部分实现、15包待实现，仍有22包未完整关闭。

Node入口skill-hp-cost-host.mjs，H5入口skill-hp-cost-server.mjs，examples/skill-hp-cost-host（交付samples/skill-hp-cost），新4219独立预览/账本；web/hp-costs.mjs只观察实际私有费用，不按HP差反算。Rust构造旧ActionCommitted字面量补hp_cost:None，旧JSON省略不变。

[规格](../batches/SDK-5-28-skill-hp-cost.md)、[接口](../interfaces/skill-hp-cost-v1.md)、[验收记录](../evidence/SDK-5-28-skill-hp-cost-2026-11-06.md)、[原向量执行绑定](../checklists/SDK-5-28-source-vectors.json)。


## 施法阶段移动

机制 46 的技能配置支持准备和后摇策略。服务端沿用 Move、Navigate、Stop 和原技能动作。客户端显示原快照的位置、准备和后摇时钟。 [配置接口](../interfaces/cast-movement-v1.md)


## 方向蓄力

机制 47 的技能支持开始锁向、手动释放和自动提交。客户端使用本人准备快照中的 ID 释放，并读取本人提交事件中的实际持有时间和威力。 [配置与 Action](../interfaces/direction-charge-v1.md)


## 蓄力重瞄准

机制 48 通过本人准备快照中的动作 ID 和权威方向承载更新。Rust 调用方构造 `Action::ReleaseCharge` 时用 `direction: None` 沿用当前方向，或用 `Some([x,y])` 提交最终方向。 [配置与 Action](../interfaces/charge-reaim-v1.md)


## 固定方向引导

机制 49 沿用普通方向施法与取消。Node 转发本人引导状态、脉冲和结束事件。展示层以权威逻辑时钟显示进度，以实际脉冲事件记录周期灵力。 [配置与协议](../interfaces/fixed-channel-v1.md)


## 方向弹道单次再施放

机制 50 从本人快照取得在途弹道和原动作 ID，使用 `recast_projectile` 发送新方向。成功事实通过原私有事件发布，客户端沿用原弹道 ID 更新位置。 [配置与协议](../interfaces/projectile-recast-v1.md)


## 后摇跟进施法

机制 51 沿用 `cast`，本人快照提供释放固定步和已接受队列。宿主按真实提交事件记录成本。 [配置与协议](../interfaces/recovery-followup-v1.md)


## 可取消后摇

机制 52 提供本人取消开放时刻。沿用 `cancel_action` 或另一普通方向技能的 `cast`。宿主按权威快照显示状态，按实际提交事件记录成本。 [配置与协议](../interfaces/cancelable-recovery-v1.md)


## 普攻冷却跟进

机制 53 沿用 `attack`，本人快照提供排队目标代次、释放与截止时刻。观察实际提交事件确认第二击。 [配置与协议](../interfaces/basic-followup-v1.md)


## 普攻锁定敌方单位

机制 54 通过 `attack_unit` 提交已发布的 NPC 身份和代次。本人准备、队列和提交事件提供相应身份。 [配置与协议](../interfaces/locked-basic-target-v1.md)


## 锁定普攻短追击

机制 55 的对局配置可通过 `basic_pursuit` 设置追击范围和时长。宿主继续发送 `attack_unit` 与 `cancel_action`。本人快照提供追击目标、动作 ID 和截止时间。 [配置与协议](../interfaces/basic-pursuit-v1.md)


## 自动选敌短追击

机制 56 的对局可配置 `basic_pursuit.auto_max_range`，从原 `attack` 入口发起自动选敌追击。显式锁定继续使用 `max_range`，两者使用同一个本人快照字段和取消入口。 [配置与协议](../interfaces/auto-basic-pursuit-v1.md)


## 局内战利品丢弃

机制 57 的对局可配置 `member_loot_drop.max_active`。宿主使用 `drop_item` 提交本人库存修订，地面资源继续使用 `pickup`。结算 `looted` 按净拾取量计算。 [配置与协议](../interfaces/member-loot-drop-v1.md)


## 局内恢复道具

机制 58 的对局可配置 `member_item_use`。宿主使用 `use_item` 提交物品 ID 和本人库存修订，实际恢复量由本人 `item_used` 事件提供。 [配置与协议](../interfaces/member-item-use-v1.md)


## 恢复道具冷却

机制 59 的恢复道具可配置 `cooldown_ms`。宿主继续使用 `use_item`，本人快照通过 `item_cooldowns` 提供逻辑截止时间。 [配置与协议](../interfaces/member-item-cooldown-v1.md)


## 灵力恢复道具

机制 60 的道具可配置 `restore_spirit`。宿主继续使用 `use_item`，本人 `item_used` 提供实际 `spirit_restored`。 [配置与协议](../interfaces/member-item-spirit-v1.md)


## 恢复道具共享冷却

机制 61 的恢复道具可配置 `cooldown_group`。同组条目使用相同的 `cooldown_ms`，客户端按本人快照中的道具 ID 查找截止时间。 [配置与协议](../interfaces/member-item-cooldown-group-v1.md)


## 局内双恢复道具

机制 62 允许同一条 `member_item_use` 配置 `heal_hp` 和 `restore_spirit`。本人 `item_used` 发布两项实际恢复量。 [接入](../interfaces/member-item-dual-recovery-v1.md)


## 按最大属性比例恢复

恢复道具可以按本人最大气血和最大灵力的比例恢复。两项独立向下取整，再按当前缺口截断。 [配置与接入](../interfaces/member-item-ratio-recovery-v1.md)


## 恢复道具品质倍率

恢复道具可按物品 ID 配置品质倍率。固定或比例恢复量乘倍率后一次取整，再按缺口截断。 [配置与接入](../interfaces/member-item-quality-recovery-v1.md)


## 恢复道具附带自身状态

恢复道具可引用已登记的自身状态。实际恢复大于零且整组状态容量足够时，一次提交恢复、消耗、冷却及状态。 [配置与接入](../interfaces/member-item-self-status-v1.md)


## 恢复道具状态品质配置

恢复道具可按品质配置攻击或防御状态的强度和时长。换品质沿用同一实例，按本次成功用药刷新。 [配置与接入](../interfaces/member-item-status-quality-v1.md)


## 恢复道具附带直接减伤

恢复道具可附带自身直接减伤。NPC 对成员的直伤在防御后、护盾前应用配置减伤。 [配置与接入](../interfaces/member-item-damage-reduction-v1.md)


## 周期伤害减伤

NPC 对成员的物理持续伤害在每跳执行时，按防御、减伤、护盾、生命的顺序结算。 [配置与接入](../interfaces/member-periodic-damage-reduction-v1.md)


## 多人成员持续治疗倍率

多人成员的自身 HOT 每跳应用执行时有效的受治疗倍率，取整后按气血缺口截断。 [配置与接入](../interfaces/member-periodic-healing-received-v1.md)


## 即时用药的受治疗倍率

恢复道具可显式配置即时气血回复应用执行时的受治疗倍率。 [配置与接入](../interfaces/member-item-healing-received-v1.md)


## 多人回复光球的模板倍率

多人 PvE 怪物模板可配置光球的气血和灵力比例。击杀时冻结恢复量，拾取时按当前缺口裁剪。 [配置与接入](../interfaces/member-recovery-orb-v1.md)


## 多人自然恢复强化

多人 PvE 成员的自然恢复在每周期应用执行时有效的恢复强化。 [配置与接入](../interfaces/member-recovery-strength-v1.md)


## 多人灵力回复封锁

多人 PvE 成员的灵力回复封锁统一作用于自然恢复、普攻、光球及即时用药。 [配置与接入](../interfaces/member-spirit-recovery-lock-v1.md)


## NPC 弹道施加灵力回复封锁

多人 PvE 的 NPC 普通弹道可向合法命中的存活成员施加灵力回复封锁。 [配置与接入](../interfaces/member-npc-spirit-recovery-lock-v1.md)


## 恢复道具溢出治疗转护盾

多人 PvE 的即时气血恢复道具可按显式配置将溢出治疗转换为护盾。 [配置与接入](../interfaces/member-overheal-shield-v1.md)


## 自身持续治疗溢出转护盾

多人 PvE 的成员自身持续治疗可按配置将每跳溢出量转换为护盾。 [配置与接入](../interfaces/member-periodic-overheal-shield-v1.md)


## 多人自身技能即时治疗

多人 PvE 的自身技能可按配置即时治疗，显式配置可将溢出转换为吸收盾。 Rust 端若穷尽匹配 `HealKind`，需要加入 `SelfSkill` 分支。 [配置与接入](../interfaces/member-instant-self-heal-v1.md)


## 多人指定友方即时治疗

多人 PvE 可指定同一对局内另一名成员，提交时重验目标后按配置即时恢复气血。 Rust 端若穷尽匹配 `HealKind`，需要加入 `AllySkill` 分支。直接构造 `PlayerActionView` 时填写可选的 `ally_target`。 [配置与接入](../interfaces/member-ally-heal-v1.md)


## 指定友方持续治疗

多人 PvE 可给同局另一名成员施加按来源独立的持续治疗，任一方退场时清理对应实例。 [配置与接入](../interfaces/member-ally-hot-v1.md)


## 指定友方吸收护盾

多人 PvE 可给同局另一名成员施加吸收护盾，沿用目标总量上限及伤害消耗顺序。 [配置与接入](../interfaces/member-ally-shield-v1.md)


## 友方即时治疗溢出转护盾

多人 PvE 的友方即时治疗可按配置将溢出量转换为施法者名下的目标护盾。 [配置与接入](../interfaces/member-ally-overheal-shield-v1.md)


## 友方持续治疗溢出转护盾

多人 PvE 的友方 HOT 可按配置将每跳溢出量转换为施法者名下的目标护盾。 [配置与接入](../interfaces/member-ally-hot-overheal-shield-v1.md)


## NPC 弹道沉默

多人 PvE 的 NPC 弹道可按配置施加定时沉默，打断未提交主动技能及引导。 [配置与接入](../interfaces/member-npc-silence-v1.md)


## 定时控制免疫

多人 PvE 的自身技能可提供定时控制免疫，拒绝 NPC 弹道的沉默并保留伤害。 [配置与接入](../interfaces/member-control-immunity-v1.md)


## 控制时长减免

多人 PvE 的自身技能可按配置缩短 NPC 弹道施加的沉默。命中时先判免疫，再按目标当前减免确定期限。 [配置与接入](../interfaces/member-control-duration-reduction-v1.md)


## 首控免疫

多人 PvE 的自身技能可按配置抵挡一次 NPC 弹道沉默。定时免疫优先，首控消耗后沿用控制缩时。 [配置与接入](../interfaces/member-first-control-immunity-v1.md)


## 道具净化自身沉默

多人 PvE 成员可通过配置的本局拾取道具移除指定的自身沉默，并立即恢复施法资格。 [配置与接入](../interfaces/member-item-cleanse-v1.md)


## NPC 弹道定身

多人 PvE 的 NPC 弹道可按配置定身，阻止移动与追击，保留原地施法和普攻。 [配置与接入](../interfaces/member-npc-root-v1.md)


## NPC 弹道眩晕

多人 PvE 的 NPC 弹道可按配置眩晕，打断当前动作并暂时限制移动和战斗输入。 [配置与接入](../interfaces/member-npc-stun-v1.md)


## NPC 弹道减速

多人 PvE 的 NPC 弹道可按配置减速，方向移动、导航和普攻追击共用步长预算。 [配置与接入](../interfaces/member-npc-movement-slow-v1.md)


## 自身技能移速加成

多人 PvE 的自身技能可按配置加速，加速与减速在同一速度乘区合成。 [配置与接入](../interfaces/member-movement-haste-v1.md)


## 自身技能冷却缩减

多人 PvE 的自身技能可按配置提供冷却缩减，成功提交时计算并固定主动技能冷却截止时间。 [配置与接入](../interfaces/member-cooldown-reduction-v1.md)


## 自身技能破甲

多人 PvE 的自身技能可按配置提供破甲，成员直接命中 NPC 时读取并结算有效护甲。 [配置与接入](../interfaces/member-armor-penetration-v1.md)


## 自身技能暴击率

多人 PvE 的自身技能可按配置提供暴击率，成员直接命中 NPC 时按护甲、暴击、取整与护盾的顺序结算。 [配置与接入](../interfaces/member-critical-chance-v1.md)


## 自身技能暴伤加成

多人 PvE 的自身技能可按配置提供额外暴伤，成员直接命中 NPC 成功暴击时计入当前有效倍率。 [配置与接入](../interfaces/member-critical-damage-bonus-v1.md)


## 自身技能闪避压制

多人 PvE 的自身技能可按配置压制 NPC 对当前攻击者的闪避，先扣除压制再应用 50% 上限。 [配置与接入](../interfaces/member-dodge-penalty-v1.md)

## 自身技能闪避率

自身技能可按配置提高施法成员对 NPC 弹道和近战的闪避率，合计封顶 50%。 [配置与接入](../interfaces/member-dodge-chance-v1.md)

## 自身技能命中加成

自身技能可按配置提供命中加成。成员直接接触 NPC 时使用原公式的闪避压制、命中修正和封顶顺序。 [配置与接入](../interfaces/member-accuracy-v1.md)

## NPC 命中属性

NPC 基础属性和附加模板可配置 `hit` 命中点，弹道和近战读取实际攻击来源的数值。 [配置与接入](../interfaces/member-npc-hit-v1.md)

## 定时伤害免疫

自身技能按配置授予定时伤害免疫。NPC 直接接触先检查免疫，周期伤害逐跳检查并继续推进节拍。 [配置与接入](../interfaces/member-damage-immunity-v1.md)

## NPC 弹道不可闪避

NPC 弹道在释放时冻结可选不可闪避策略，接触时先检查资格与免疫。 [配置与接入](../interfaces/member-npc-unavoidable-v1.md)

## 成员直接攻击不可闪避

成员普攻和伤害技能可配置不可闪避，提交时冻结并复用通用直接接触入口。 [配置与接入](../interfaces/member-unavoidable-v1.md)

## 直接攻击忽略护甲

直接攻击可配置忽略护甲，按免疫、闪避、伤害乘区、减伤、护盾和扣血顺序结算。 [配置与接入](../interfaces/direct-ignore-armor-v1.md)

## 自身技能威力加成

自身技能通过配置授予威力加成，伤害技能在提交时读取并冻结预算。 [配置与接入](../interfaces/member-skill-power-v1.md)
