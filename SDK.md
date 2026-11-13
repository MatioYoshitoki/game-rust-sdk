# match-kernel 0.171.0

权威对局运行时 Rust SDK。一个 Runtime 对应一个对局，持有调度线程与一份 World；协议1～3为单成员，协议4恰为两名必需成员，协议5由配置冻结1～8名必需成员。宿主负责认证、配置、存储事务和传输；Rust 裁决玩法、结果和资产金额。

协议 6 的宿主可通过 `pve.world_geometry` 提供完整凸边界、装饰碰撞和定时阻挡。客户端可读取 `active_blockers` 展示当前生效的阻挡。 [几何配置](docs/interfaces/world-geometry-v1.md)

机制105增加[通用战斗配置](docs/interfaces/combat-rules-v1.md)，由配置提供阶差、元素/武器关系、抗性、命中/暴击与恢复公式；直接和周期效果共用结算。另提供临时上限/SPD、复合状态、可配置聚合上限、有害状态缩时、周期来源/目标与动态采样、纯增益道具/品质覆写，以及按接触或整次施法计数的有界被动触发。`attack_speed`通用属性实时调整普攻冷却推进，成员与NPC近战共用，保持ATK、SPD及技能时序独立。记录盾在同一盾实例内维护冻结上限的观察账，结束反击、后续状态、条件/概率和指定来源的层数消费均由配置声明。SDK-5-88整项仍按其范围矩阵推进，当前能力不代表原R3全部工作包已关闭。

`combat_rules.self_statuses_by_mastery` 支持按可信熟练度选择同一自身技能的提交状态，保留单一技能 ID 和费用、冷却。配置与边界见[来源资格接口](docs/interfaces/source-gates-v1.md)。

成员普攻和伤害技能可通过 `life_steal_timing` 选择逐次接触或按整次动作的实际扣血结算吸血。[通用战斗配置](docs/interfaces/combat-rules-v1.md)

成员普攻支持可配置穿透、定向扇形、径向波面、同刻多段接触、目标爆破、固定延迟、叠层周期状态与死亡扩散、地面周期领域、开放式方向引导、蓄力飞行爆炸和叠层终击。宿主可为同局成员指定不同普攻配置，绑定释放状态和基于实际伤害的终态状态。[多人穿透弹道](docs/interfaces/member-piercing-v1.md) · [普攻冷却跟进](docs/interfaces/basic-followup-v1.md)

可信宿主可按装备实例解析模型、家族和器型，并用带修订号的 `set_equipped_weapon` 在局内换装。机制 105 通用战斗配置支持最多 32 个状态定义，以便在同一场景组合器型与普通功法。 [装备接入](docs/interfaces/basic-followup-v1.md#装备实例与局内换装)

NPC 技能可配置移动区域，在成员进入时结算最大生命比例伤害、碰撞安全击退与状态。已释放区域可在来源失效后沿原代次继续命中。预警、在途区域和保留的减速沿用协议 6 的全量、增量和重连发布。 [移动区域接入](docs/interfaces/combat-rules-v1.md#npc-移动区域进入)

NPC 技能也可配置固定圆形领域。落点在预警时锁定，入场状态和周期伤害各有时钟，已释放领域可按配置在来源失效后继续结算。 [固定领域接入](docs/interfaces/combat-rules-v1.md#npc-固定地面领域)

固定落点单次打击在提交时选圆内最近的合格成员。伤害按目标最大生命结算，命中状态概率由宿主配置。 [单次打击接入](docs/interfaces/combat-rules-v1.md#npc-固定落点单次打击)

宿主可配置定时圆形检查点、预警躲避和反制。Rust 按 World 时钟独立登记各席位的到点或躲避进度。反制可随达标、手动请求或窗口内的首次直接接触结算。 [检查点接入](docs/interfaces/publications-v6.md#定时区域检查点)

宿主可声明顺序模式阶段，组合等待、检查点确认、原生目标清除和双席对抗。Rust 负责阶段期限、有限复活与胜负平，并用既有结果账本提交资产。 [模式接入](docs/interfaces/publications-v6.md#顺序模式)

宿主可授权独立旁观席位。Rust 生成公开视图，按旁观席发送全量或增量，并在配置期限内提供冻结的终局视图。 [旁观接入](docs/interfaces/publications-v6.md#旁观发布)

地面圆形技能可绑定成员陷阱。提交后按配置延迟启动，敌对目标进入时走共享命中和伤害链。 [陷阱接入](docs/interfaces/combat-rules-v1.md#成员地面陷阱)

宿主可配置定时施法锚点。成员使用自己的有效锚点发射弹道或布置圆形区域，也可按配置在窗口内迁移、返回或主动结束锚点。绑定领域随锚点移动或回收。 [锚点接入](docs/interfaces/combat-rules-v1.md#成员施法锚点)

宿主可在同一技能中按可信熟练阶段组合双资源周期恢复、减益拦截、净化、满血转灵和到期恢复。 [阶段配置](docs/batches/SDK-5-88-source-time-current.md)

即时敌对单位技能可按可信阶段选择命中状态。有害状态可配置物理和元素易伤。 [玄域绝法验收](docs/batches/SDK-5-88-source-primordial-seal.md)

成员可配置受击反制窗口。同一自身状态实例按实际攻击者分别冷却，并可配置反制成功后驱散和自然到期时的附加状态。 [战斗接口](docs/interfaces/combat-rules-v1.md#受击反制窗口) · [九阙封脉验收](docs/batches/SDK-5-88-source-retaliation-window.md)

记录盾可配置同来源、同目标的盾家族，仅由余额更强的盾替换，并可分别选择反击的结束原因。 [坤甲功与撼山劲验收](docs/batches/SDK-5-88-source-guard-families.md)

成员技能可按可信熟练阶段冻结冷却，状态保命可按实体代次与配置组维护独立期限。 [磐心守一验收](docs/batches/SDK-5-88-source-stillness-guard.md)

指定友方治疗可按可信熟练阶段组合即时治疗、冻结生息、目标周边群疗和溢出盾。 [友方治疗接口](docs/interfaces/combat-rules-v1.md#按熟练阶段组合友方治疗)

即时友方治疗支持按技能开放本人作为主目标，并可在提交时增加来源隔离的叠层状态，配置满层消费与恢复。 [自选与叠纹验收](docs/batches/SDK-5-88-source-ally-self-and-marks.md)

## 显式接入

```rust,ignore
use match_kernel::{ConfigCatalog, Runtime};
use std::sync::Arc;

let catalog = Arc::new(ConfigCatalog::load(manifest_path)?);
let config = catalog.default_config();
let runtime = Runtime::with_catalog(catalog);
let client = runtime.client();
// Create 使用 config.id()、config.content_hash()、config.protocol_version()。
// 保留 runtime，直到 World 关闭、未决责任处理完毕或已持久交接。
```

Client 的六类调用为 create、submit_input、submit_business_receipt、submit_transport_receipt、drain_outputs、close。调用交换数据，不暴露 World；权威类型位于 [protocol.rs](src/protocol.rs)、[资产/结果类型](src/protocol/assets.rs)。宿主使用稳定请求/业务/尝试/连接身份，处理真实发送与业务回执。

收到输出后先持久接受最新检查点、原请求及唯一事件，再执行存储 I/O 或交付快照。库存、成长和幂等证明在同一业务事务提交。Unknown 不能当作未提交；仅有隔离迟到写入的权威证明才可返回 NotCommitted。重试/查询/人工核对使用原冻结责任；不改金额或另起奖励 ID。

Closed 表示 World 已释放，资产责任可能仍未决。按 Closed.pending_operations 和后续 OperationResolved 继续收尾。重启使用原配置 id/hash、创建身份及可信检查点恢复 Closed 责任账，不恢复战斗 World。检查点摘要不是抗特权主机伪造的签名。

## 支持版本

| 配置 schema | 协议 | 业务边界 |
|---|---|---|
| 2 | 1 | 兼容原物品/成长及撤离/死亡结算 |
| 3 | 2 | 配置资产 ID 与有限原子资产事务 |
| 4 | 3 | 配置结果 ID、preserve/attrition 策略与资产事务 |
| 5 | 4 | 双成员独立准入、共享World、私有全量发布与聚合资产责任 |
| 6 | 5 | 固定完整名单，配置化1～8名必需成员，复用成员全量/金融责任DTO |
| 7 | 6 | 同一成员PVE白名单，逐成员有类型full/delta、Sent基线与恢复 |

清单 schema 1、机制版本 1～105、恢复 schema 1。机制19在schema4/协议3的PVE增加有限定时DEF状态（refresh/ignore、来源分组、提交/弹道命中触发），由配置初始化数值和绑定，Rust裁决有效防御及到期/清理。状态视图见[protocol/status.rs](src/protocol/status.rs)，机制20增加DEF叠层：stack必须配置max_stacks（1～128），每次合法重施加层并刷新整组期限，封顶仍刷新，到期整组移除；层数不占新实例槽。stack实例输出stacks/max_stacks，重施发status_stacked（封顶added_stacks=0）；refresh/ignore省略新字段。机制21增加timed_attribute_modifier和有限atk/def选择器，ATK定义必配attack_bonus_cap_bps。两属性共用聚合、叠层、来源和清理；弹道提交时self状态施加后读取有效ATK一次，在途不重算，DEF在命中时读取，其它攻击计划沿原物化边界。新通用实例输出attribute，机制21快照输出effective_atk，旧实例/旧机制省略新字段。机制22增加有限absorption_shield，以目标基础maxHp的shield_bps授予、shield_cap_bps限总量，refresh/ignore；先完成伤害减免/取整再消耗盾，最后扣HP，按到期/施加/实例ID消费。盾实例输出shield，机制22视图输出shield_total，并发shield_granted/absorbed与depleted移除事件。机制23增加有限periodic_damage：tick_interval_ms/damage_bps/duration_ms配置节拍与冻结的来源有效ATK比例，refresh/ignore仅敌对命中目标。首跳等待间隔，截止前执行；每跳读取当前DEF/盾，不再触发直接命中附带或消耗伤害随机数。DOT实例输出periodic进度，新增periodic_tick事件；保护/归途跳过到期点，失效/终局清理，重启不恢复临时状态。机制24增加有限periodic_heal：仅提交时施加给自身，冻结基础maxHp×heal_bps，每跳按当前缺口回复，满血为0，不转盾/灵力、不复活。复用周期队列，相同计划时刻DOT先HOT；HOT视图kind=max_hp_heal/raw_heal，新增periodic_heal_tick实际回复事件，旧DOT字段保持。机制25新增timed_attribute_modifier/healing_received（refresh/ignore），目标有效修正累加后下限归零，无额外正向玩法帽。HOT、自疗、支援在实际治疗时一次倍率/取整，再按缺口裁剪；基础冻结量/预算及其它恢复保持。新视图healing_received_bps与heal_resolved事件提供实际计算，重启移除临时派生。0.11.0新增公开DTO字段、事件与枚举分支，Rust源码消费方需适配；旧机制不输出新字段或消费新随机数，历史配置和资产保持原义。

配置必须由ConfigCatalog加载并校验，每局固定准确原文hash。注册机制不提供任意脚本或World修改入口；新增胜负等触发、多人、生产存储及跨机接管需要对应接入验收。

`match_kernel::sdk_info()` 或 `kernel-host --sdk-info` 可在无配置目录查询实际编译支持信息。普通 JSONL 宿主显式启动：

```sh
kernel-host --configs /absolute/path/to/catalog.json
```

JSONL 请求逐行发送，单行最多 256KiB，返回 ok/error；业务请求、状态和事件通过 drain_outputs 取出。进程 EOF 结束 Runtime，退出前先完成关闭/责任交接。replay 是显式验收特性，交付普通二进制不启用它。

## 宿主配置

地图、技能、状态、怪物、道具、账户和数值由宿主维护并注入。SDK 提供配置校验、机制执行和版本冻结。Rust 使用 `ConfigCatalog::load` 与 `Runtime::with_catalog`，JSONL 使用 `--configs` 或 `MATCH_CONFIG_MANIFEST`。兼容入口 `Runtime::spawn` 和 `Config::frozen` 也要求显式指定宿主清单。清单只有一个场景时可省略 `default_scenario`，多个场景时须指定默认场景。

配置 schema 2～7 可加载。旧配置 schema 1 曾依赖内置数值，升级时由宿主补齐参数。仍有旧 schema 1 未决责任时，使用原版本与原配置 hash 完成处理后再迁移。清单自身仍使用 schema 1。

本地交付包另含第三方源、当前平台普通二进制和独立参考宿主；目录及平台/编译器信息以外层 README、distribution.json、support.json 为准。Rust 包使用精确版本和接入方 Cargo.lock；path 本身不锁定文件内容。升级保留旧原文与检查点，按原版本履行旧责任。当前 publish=false，分发渠道为本地工程交付。

机制26在schema4/协议3 PVE中保留skill主技能，并允许additional_skills配置一个附加弹道技能。两项独立成本/CD，共用灵力、单个准备动作与后摇；移动、取消、切技能、前后台或重连不能缩短已提交后摇。Snapshot.player_skills提供按配置顺序的绝对CD，pending_action显示准备动作；ActionCommitted.skill_id标识已提交技能。旧skill_ready_at_ms仍仅表示主技能，机制1～25省略新字段并保持原动作策略。重启恢复仅核对冻结资产，清除临时技能视图。公开DTO增加可选字段，Rust结构体字面量需适配。

机制26遭遇实际换轮时取消未提交动作且不扣成本/CD；复位倒计时若在换轮前取消，同轮准备保留。在途弹道继续沿原轮次资格判定。

机制27新增玩家self_status技能和CastSelf输入（kind=cast_self、skill_id），主/副槽均可使用。配置只含ID/成本/前后摇/CD及1～2项on_commit_self_statuses；严格拒绝弹道几何/伤害和hit绑定字段。固定步提交时沿原整组容量/灵力预检施加合法自身状态，不创建弹道或直接攻击；与弹道技能共用准备/后摇和灵力、各自CD。0前摇仍在下一固定步提交。机制1～26的配置和输出保持，旧Cast必须有方向，新CastSelf禁止方向/目标等多余字段。Action新增枚举分支，Rust穷尽match需适配；六类Client/协议1～3/资产及恢复schema保持。

机制28新增unit_projectile与CastUnit（kind=cast_unit、skill_id、target{id,generation}）。只支持当前World已发布的存活敌人；接受/提交重验身份代次、资格、中心射程及视线，提交前失败免扣。提交时按合法当前位置冻结方向，沿原弹道/伤害/合法状态执行，不追踪或换人。非目标先接触只阻挡不受伤，目标失效/换代中止在途弹道，不退款或回滚CD。MonsterView、准备视图与单位提交事件增加可省略unit_target；旧1～27不输出该字段，旧配置/责任保持。Action/Error枚举与视图字段为源码变更，Rust消费方需适配；六类Client/协议1～3/资产及恢复schema保持。

机制29增加ground_circle / CastGround，明确世界坐标position；接受/提交双重校验地图半开边界、可行走落点、射程与视线，严格拒绝而不裁剪。提交前原子预检灵力/self状态/两计划容量；成功冻结中心/攻击量、扣费/CD/后摇并发布预警，至少下一固定步才按当前敌人位置结算一次圆形伤害，沿原直接命中/DEF/盾/状态/掉落链。PveView.player_area_warnings和pending_action/ActionCommitted.ground_position仅机制29输出；新增PlayerAreaStarted/Resolved/Cancelled。死亡/终局/关闭/实际换轮清计划，移动/后台/同World重连保留已提交计划且不退款，重启不恢复临时战斗。公开Action/事件/视图源码消费需适配，协议和资产责任保持。

机制30新增ground_field，复用CastGround严格选点及接受/提交重验，配置必填duration_ms/tick_interval_ms。首跳在预警结束后等待一个间隔，按计划时刻周期结算且截止点排除，最多128跳；当前目标位置/DEF/盾/闪避、原伤害/命中状态/击杀掉落链保持，直接攻击量提交冻结。只提交扣费/CD一次，与单次圆形共用每World两个地面计划槽。PveView.player_fields和PlayerFieldStarted/Tick/Ended仅机制30输出；来源失效固定结束，死亡/终局/关闭/实际换轮清理，后台/移动/同World重连保留已提交区域，重启不恢复临时战斗。旧1～29保持，公开视图/事件枚举源码消费需适配，协议和冻结资产责任保持。

机制31允许ground_field配置可选contact_policy（max_targets_per_tick 1～32、max_total_hits 1～128、repeat_interval_ms 0～60000）。0为同一ID/代次每区域一次，正数按上次成功接触的计划时刻计算，等值合法，进出/重连不清历史。每周期按距离平方/ID/代次排序；真实直接接触（含0HP损失/全盾吸收）消耗名额和预算，闪避/阻挡不消耗，额度满停止后续目标/状态。总预算0以hit_budget_exhausted立即只结束一次；成本/CD仍只在提交时处理。通用身份/时间/计数组件首批仅接入持续区域，历史≤128；配置时FieldView.contact_policy和Tick.hit_targets/remaining_hits输出权威事实，省略策略及旧1～30保持原形状/顺序/语义。公开DTO/枚举源码消费需适配，协议与冻结资产责任保持。

机制32新增piercing_projectile（方向Cast，必填ttl_ms与max_hits），按连续扫掠顺序接触敌方身份、墙同距优先截断，每身份每发弹一次、总成功接触耗尽结束。复用ContactBudget，闪避不耗、全盾0损失耗预算。最多2发在途，提交原子扣费/开启独立CD，死亡/终局/关闭/实际换轮结束。同World重连保留位置/期限/行程/历史/预算，重启仅恢复冻结责任。所有数值/绑定由配置初始化，旧1～31省略新增视图/事件；公开DTO/枚举源码消费需适配。本批仅方向型直线穿透，不代表完整弹道家族或M05完成。

机制33允许piercing_projectile显式配置repeat_interval_ms（0～60000整数）：0每身份一次，正数按该发弹最新成功接触时刻+间隔给资格；仍需真实扫掠，未接触不补跳，每步每身份最多尝试一次。每次成功含全盾0损失消耗原max_hits总预算并刷新最新时刻，闪避/失效不耗不刷新，成本/CD只提交一次。配置随接受冻结，同World重连保留历史/计时/预算，死亡/终局/关闭/实际换轮回收；重启仅恢复冻结责任。显式配置才输出弹道repeat_interval_ms、正间隔contacts.next_eligible_at_ms和Hit.hit_at_ms，旧1～32及省略配置的形状保持。公开DTO/事件源码消费需适配；本批只扩展移动接触频率，不代表完整M05。

机制34新增line_field，复用Cast方向输入与PlayerField调度/ContactBudget。配置length（0.1～10000）、width完整宽度（0.2～256）、warning_ms、duration_ms、tick_interval_ms及必填contact_policy；主副槽均可。接受冻结方向，提交取当前起点且重验width/2弹道足迹，按连续圆盘扫掠在首墙/地图边界截断，固定世界胶囊。每跳按路径进度/距离/ID/代次排序，以当前目标足迹/最近点视线进入原伤害链；成功含全盾0损失计预算，闪避不耗，0每身份一次、正间隔按计划时刻重复。与单次/持续圆形共享2份区域计划，成本/CD只提交一次，死亡/终局/关闭/实际换轮回收。PlayerFieldView新增可省略line:{start,end,width}，旧1～33圆形输出省略；公开DTO源码消费需适配，六类Client/协议1～3/资产恢复schema保持。


## 协议4：同一World的两名成员

0.20.0新增CreateV4和MemberBusinessReceipt，分别通过原create、submit_business_receipt调用；Input、TransportReceipt及close仍使用原入口。公共类型见[src/protocol/v4.rs](src/protocol/v4.rs)。旧Create/回执调用继续可用；Rust穷尽匹配OutputBody时须处理新增的BusinessRequestV4/PublicationV4/EventV4/ResponsibilityV4，serde泛型反序列化调用需显式指定请求类型。

schema5必须声明two_members，提供按成员稳定ID顺序的两处合法出生点、部署期限、每接收者队列和弹道容量。角色列表、主体、连接身份以及配置hash在创建时冻结；错误或本批不支持的配置在业务请求前拒绝。普通方向弹道、自身ATK/DEF、有限普通NPC和公共整数资源参与同一World；NPC/掉落各一份，成员状态/输入/成本/CD/背包各自独立。完整8席/模式容量、PvP、delta/旁观、实例遗物及其余双人玩法组合仍待实现。

发布的snapshot.self仅含本人私有状态，members仅公开位置/HP/阶段/保护。宿主严格按recipient_id送达，不能广播私有事件或ResponsibilityV4；旧Event包装为Private时只描述该成员/金融账本，只有顶层EventV4::Closed表示共享World结束。共同激活必须双方入场成功、实际sent且ready；enqueued不算。成员撤离核心未提交时保持settling，不暂停另一人的战斗时钟；成员终局不能关闭存活成员的World。

每批输出先同事务保存最新ResponsibilityV4检查点与全部原请求/唯一事件，再做I/O和传输。恢复时使用原CreateV4与RecoveryV4，仅建立Closed责任账，不恢复战斗。聚合检查点摘要按递归排序键的紧凑JSON UTF-8编码；成员schema1检查点原样保存，交由Rust按原类型规则校验。稳定operation身份绑定MatchKey/成员；未知提交先查询，重试/补偿保持原金额。宿主须确认旧进程退出或提供等价fencing，不能以“查不到”声称NotCommitted。

仓库内普通Node/SQLite消费方：

```sh
cargo build --locked --bin kernel-host
node adapters/node/two-member-host.mjs --binary target/debug/kernel-host \
  --configs /absolute/path/to/game/catalog.json \
  --accounts /absolute/path/to/game/accounts.json --scenario your-scenario \
  --database /absolute/path/outside-sdk/pair.sqlite
```

H5两成员验证台使用同一普通二进制：`node adapters/h5/two-member-server.mjs --port 4190 --database /absolute/path/pair-demo.sqlite`。创建后并排页面分别接收成员数据；WASD、施法、拾取、撤离/离场均提交意图，Rust裁决结果。参考宿主提供本地账户/票据和SQLite，并不代替真实认证、存储与正式客户端接入验收。


## 协议5：固定名单的1～8名必需成员

0.21.0/schema6/机制36增加严格members配置块：capacity为1～8整数，spawns长度相同且任意两点满足玩家足迹不重叠；共享弹道/每接收者发布和部署期限仍由配置初始化。创建名单必须恰capacity、全部必需、创建后冻结，不支持动态入席或匹配机器人。原协议4/schema5/two_members仍严格两人，协议1～3仍为单人。

新增有类型CreateV5，沿原create调用；协议版本5必须匹配schema6/机制36。CreateRequest增加Members枚举分支，Rust穷尽匹配需适配。协议5有意复用已有业务/发布/事件/聚合责任DTO及*_v4输出标签，宿主遍历实际成员/recipient；不得根据标签假定恰两名。资产回执每成员最多32行，沿原资产ID64字节限制；超限在应用前拒绝，恢复同样校验，以保留普通256KiB JSONL恢复预算。

能力白名单仍为上一节的普通PVE子集。全部成员准入committed、实际sent且ready后共同激活一次；成本/背包/状态/CD/序列/连接/终局独立，NPC/资源来自一个World。任意一席失败或缺席不自动缩容，准备局终止后逐人按冻结责任核对/补偿；独立终局与未知I/O不暂停生存成员，最后一人退出可操作集合后释放World。

普通Node消费：`node adapters/node/member-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/members.sqlite`；八人改accounts-b.json、scenario eight，可加--reverse验证顺序。H5：`node adapters/h5/member-server.mjs --port 4193 --database /absolute/path/members-demo.sqlite`。两者只提交意图、认证与I/O，Rust裁决结果。重启只恢复Closed金融责任；标准完整匹配、自动机器人、16/24模式、delta/观战与生产负载仍待验收。


## 协议6：成员增量发布与恢复

0.22.0/schema7/机制37增加CreateV6（CreateRequest::Replicated）及OutputBody::PublicationV6。仍用原六类Client调用、固定1～8名必需成员、原金融DTO/检查点。协议1～5全量行为保持；Rust穷尽匹配Action、OutputBody、EventV4需处理RequestFull、PublicationV6、PublicationExpired。公开类型见[src/protocol/v6.rs](src/protocol/v6.rs)，Rust纯解码器见[src/replication.rs](src/replication.rs)。

配置新增replication：ack_timeout_ms为100～60000，baseline_ttl_ms为ack_timeout_ms～60000，full_interval_ms为100～baseline_ttl_ms，terminal_grace_ms为2×ack_timeout_ms～60000。必须同时使用schema7/机制37/CreateV6，不兼容的配对在I/O前拒绝。工程预算由接入方配置，Rust不读取地图名称决定行为。

每个接收者独立维护一个已Sent基线和一个在途帧。payload.full携带本人完整SnapshotV4；payload.delta准确引用base_publication_id/base_revision，按权限视图变化发送state/own完整块，以及集合upsert/removed/order。缺失块不变，出现的块不可为null；state里的active_at_ms、own里的可空值可明确置空。发送失败、取消、回执超时、基线TTL或RequestFull提升本人成员stream_epoch并补full；周期full不改epoch，delta较大则回退full。Enqueued不能推进基线，ready只接受当前连接/epoch已Sent的full。宿主应先保存输出责任，再转发原始帧，按实际发送回执，不能把排队等同Sent。

客户端使用ReplicaV6，以MatchKey和recipient绑定，原子应用PublicationEnvelopeV6；Node/H5共享交付包web/replication.mjs。缺基线或丢帧保持原展示状态，使用当前认证连接提交Action::RequestFull{stream_epoch}，等待新full后再接收delta。错误基线、旧连接/epoch/revision、冲突重复及非法集合更新不会污染缓存。重连仍走原认证Reconnect，新连接full起步。Node不生成差异、不改伤害、不附加隐藏full。

终局发送冻结的本人full；Root Closed释放World后仍可在grace内补发，重同步不更新退出时看到的其他成员位置。grace从成员settling/terminal或Root Closed较早时刻起算，失败/重试不续期。未送达到期发私有publication_expired，释放发布缓存；资产账独立完成，不能伪报Sent。宿主等待金融未决为0且终局已送达或明确到期再释放Runtime。重启只恢复Closed金融责任，不恢复World、基线或在途帧。

仓库普通Node：`node adapters/node/delta-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/delta.sqlite`；八人改accounts-b.json和eight。H5：`node adapters/h5/delta-server.mjs --port 4195 --database /absolute/path/delta-demo.sqlite`。交付包分别改为hosts/node、hosts/h5及samples/delta路径。H5的“清除显示缓存”按钮可验证request_full恢复；屏幕统计实际JSON full/delta数量与字节，不代表网络/CPU生产性能。完整匹配/机器人、16/24模式、观战、其余战斗组合与生产负载仍未验收。


## 机制38：DOT/HOT多人适配

0.23.0仅在schema7/协议6开放固定1～8必需成员的周期物理伤害和自身周期回复，复用同一个Statuses队列及原六类Client/金融DTO。原机制1～37不改变能力或输出。periodic_damage仍绑定方向弹道命中目标（成员→NPC、NPC→成员），periodic_heal绑定提交者自身（成员或NPC）；NPC本批仅接受自身HOT与命中DOT引用，不包含友军治疗/PvP/盾/其它周期公式。所有比例、时间、来源分组、refresh/ignore和工程预算由配置初始化。

来源/目标以kind/id/generation查找真实实体。DOT施加时冻结有效来源ATK，每跳读取目标当前DEF；HOT按自身基础maxHp冻结并裁剪当前缺口。后台/保护/未ready和NPC归途仅跳过相关实例并消耗计划，重连不延寿或补跳。成员进入settling立即清其目标与实际来源状态，无需等待金融回执；refresh已转移来源的实例按新来源清理。NPC真实归位复位清原来源/目标并提高generation，旧代在途弹道不再生效。DOT按原伤害/撤离中断/击杀归属/一次掉落链执行，同刻致死DOT清HOT后不能复活；其他成员继续。

MonsterViewV4新增Option<Vec<StatusView>> statuses，存在时必须为数组（包括空数组），null拒绝，从机制38起输出，内容为该接收者本人来源在NPC上的状态和NPC自身状态；不包含他人来源的NPC状态。本人受击与自身HOT沿snapshot.self.statuses，members无私有状态。事件沿EventV4::Private按实际来源/目标路由，Removed按原实例权限拆分ID；Rust投影后再diff。Rust结构体字面量须补新Option字段，严格JS客户端使用0.23.0 web/replication.mjs；字段在旧机制省略。公开实现见[src/world/two_members/periodic.rs](src/world/two_members/periodic.rs)、[状态队列](src/status/periodic.rs)。

配置保持16定义、每绑定≤2引用、每实体≤32槽、每实例≤128跳；按成员数+固定NPC数校验全World≤128实例。终局立即清理/释放临时状态，重启仍只恢复Closed金融责任，不恢复World、周期或基线。

普通Node：`node adapters/node/multiplayer-periodic-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/periodic-members.sqlite`；八人改accounts-b.json和eight，可加--reverse。H5：`node adapters/h5/multiplayer-periodic-server.mjs --port 4198 --database /absolute/path/periodic-demo.sqlite`。交付包使用hosts/node、hosts/h5及samples/multiplayer-periodic路径。页面显示Rust周期事件和本人可见实例；观测统计不参与玩法计算。单人其他战斗能力尚未全部迁移到多人，本批不代表完整战斗/模式/观战完成。

## 机制39：多人吸收盾

0.24.0/schema7/协议6在同一World复用既有盾与周期队列，支持玩家/NPC提交自身、敌对普通方向弹道命中目标的盾绑定。配置初始化shield_bps、shield_cap_bps、时长、source_scoped与refresh/ignore；所有直接伤害与DOT依次经过原DEF/取整、盾、HP裁剪。DOT的absorbed是本次真实合计；HOT不转盾。直接接触全吸收仍触发原附带及撤离中断，DOT只在HP损失时打断。

PrivateMemberView和MonsterViewV4新增Option<u32> shield_total，39始终含整数（包括0），旧机制省略，null拒绝；Rust结构体字面量补字段。本人total等于自己目标盾合计，NPC total为全部来源公开合计；NPC statuses及吸收consumptions依本人来源/NPC自身权限切片，私有amount不得跨接收者相加。members不暴露本人盾细节。客户端先校验余额与身份再原子替换基线。

来源/目标退出从settling清理；刷新转移来源，忽略保持原来源。重连保持实例ID、余额与绝对截止，保护/归途不耗盾，重启仅金融责任。协议、六类Client接口、资产责任保持。详见[多人盾接口](docs/interfaces/multiplayer-shields-v1.md)和[验收规格](docs/batches/SDK-5-22-multiplayer-shields.md)。

Node：`node adapters/node/multiplayer-shield-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/shields.sqlite`。H5：`node adapters/h5/multiplayer-shield-server.mjs --port 4201 --database /absolute/path/shields-demo.sqlite`。交付包对应hosts/node、hosts/h5与samples/multiplayer-shield。尚不包括友军/PvP、其他玩家动作及完整多人战斗。

## 机制40：统一多人战斗与指定单位弹道

0.25.0/schema7/协议6增加多人unit_projectile/cast_unit。内部以完整EntityRef统一解析实体与self/friendly/hostile模式关系，直接双向伤害和DOT统一DEF/原取整→盾→HP，HOT统一实际基础maxHp与缺口回复；状态、来源退出与权限沿同一实体解析。直接接触的随机/回灵/附带只在接触层执行一次。当前PVE成员之间、NPC之间互为友方，跨类敌对；不开放任意阵营、PvP或友军输入。历史单成员协议适配保持。

cast_unit{skill_id,target:{id,generation}}只接受发布的存活非归途敌对NPC，接受/提交重验身份、资格、中心射程与视线；准备期失败无成本/CD/自身状态/弹道。提交按当前目标位置冻结方向与有效ATK，不追踪/转绑新代。其它合格敌对单位先挡住只结束弹道，无伤害/附带/RNG/回灵；墙同时接触优先。来源/目标失效结束，已付成本不退款；同World重连保已提交计划，保护中断未提交准备，重启只恢复Closed金融账。

本人pending_action与私有action_committed复用unit_target。公共ProjectileViewV4增加Option<UnitTarget> unit_target，仅单位计划存在，null拒绝；普通/NPC/旧机制省略。Rust字面量补Option字段，严格JS使用随包解码器。所有数值/绑定/预算由配置初始化。见[接口](docs/interfaces/unified-member-combat-v1.md)及[UC01～UC12](docs/batches/SDK-5-23-unified-member-combat.md)。未声明完整多人战斗、区域/穿透/友军/PvP已接通。

Node：`node adapters/node/multiplayer-combat-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/combat.sqlite`。H5：`node adapters/h5/multiplayer-combat-server.mjs --port 4204 --database /absolute/path/combat-demo.sqlite`。八人改accounts-b.json/eight；包内路径hosts/node、hosts/h5及samples/multiplayer-combat。前端只选发布身份、发送输入和显示Rust结果。

## 机制41：多人圆形区域计划

0.26.0/schema7/协议6新增多人ground_circle/ground_field与cast_ground{skill_id,position:[x,y]}。同一有界区域计划/ContactBudget复用统一敌对接触、直接伤害与状态链；所有半径/预警/周期/重复间隔/命中预算/绑定和容量由配置初始化。接受/提交重验严格可行走落点、中心射程/视线，提交前失败不扣费/CD/授状态。提交self绑定后冻结有效ATK、固定圆心，当前目标位置/DEF/盾在各次接触读取。

单次预警后一次，零预警最早下一固定步；持续首跳在预警后等待间隔，结束排除、最多128跳。持续必配contact_policy；repeat=0每生命身份一次、正数按scheduled时刻含等号，闪避不耗预算、全盾合法接触耗且有原附带。members.area_capacity仅41必配1～8，单次/持续共享全World槽、其他技能不占；输出和容量×单区总接触≤128的历史工程预算在I/O前校验。

SnapshotV4新增Option<Vec<AreaPlanViewV4>> areas，41含数组（含空），旧省略、null拒绝；delta.areas存在完整替换，[]清空、省略不变。公开计划用World全局id/来源及几何/时序/进度/接触历史，公开area_started/tick/ended，不暴露私有输入ID/攻击/状态绑定/成本；准备/ActionCommitted仍仅本人。Rust字面量补Option字段，严格JS使用随包解码器。后台/移动/同World重连保已提交区域/预算/原期限，来源settling/退出/死亡即时清理，Close/绝对终局清全部，重启仅金融责任。六类Client/协议/金融DTO保持。见[接口](docs/interfaces/member-circle-plans-v1.md)与[GA01～GA12](docs/batches/SDK-5-24-member-circle-plans.md)。

Node：`node adapters/node/member-area-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/areas.sqlite`。H5：`node adapters/h5/member-area-server.mjs --port 4207 --database /absolute/path/areas-demo.sqlite`。八人改accounts-b.json/eight，包内对应hosts/node、hosts/h5及samples/member-area。页面可选落点并显示Rust预警/活动区域；不含友军/PvP、线形/移动区域、穿透或完整模式。

## 多人线形区域（SDK-5-25）

0.27.0/协议6/schema7/机制42，将既有line_field接入同一个多人区域计划。沿cast方向输入，配置length/width、预警/周期/接触预算/绑定及共享members.area_capacity；Rust双检起点，提交按当前起点连续扫圆裁剪墙/边界，固定胶囊、一次成本，复用统一调度/ContactBudget/直接伤害链。目标按轴进度、轴距、ID/代次稳定处理，当前足迹/DEF/盾逐次读取。后台/同World重连保留几何/期限/历史，退出从settling立即清自己的计划，重启仅金融责任。

公开AreaPlanViewV4新增LineField种类及可省略line字段；仅line_field必须带非null几何，旧circle/field省略，Rust字面量补line:None。areas全数组delta替换规则不变。使用随包Rust/JS解码器；详见[接口](docs/interfaces/member-line-plans-v1.md)、[LA01～LA12](docs/batches/SDK-5-25-member-line-plans.md)。

`node adapters/node/member-line-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/lines.sqlite`。H5：`node adapters/h5/member-line-server.mjs --port 4210 --database /absolute/path/lines-demo.sqlite`；八人改accounts-b.json/eight。包内对应hosts与samples/member-line。线形和圆形共享容量/预算/生命周期，穿透/PvP/友军/移动区域和完整模式仍待实现。

## 机制43：多人穿透弹道（SDK-5-26）

0.28.0/协议6/schema7，在固定1～8成员PVE中接入piercing_projectile与cast方向输入。复用共享弹道容器/连续扫圆/敌对目标选择、ContactBudget及直接伤害/盾/DOT/归功链。所有速度/半径/射程/TTL/总预算/重复间隔、成本/CD和状态绑定从配置初始化；members.projectile_capacity与普通/单位/NPC弹道共享，在I/O前校验128份历史与输出工程预算。接受/提交起点双检，提交self绑定后冻结攻击，一次付费；提交步不动，墙同刻优先，TTL截止先结束。

重复省略或0每代一次，正数按实际逻辑步的上次成功+间隔含等号，每身份每步一次、不补跳，仍须实际扫过；闪避不耗预算，全盾耗且有原附带。后台/移动/同World重连保在途方向/速度/余程/预算/历史/绝对期限，来源settling立即清自己的计划，重启仅金融责任。

ProjectileViewV4新增可省略非null piercing对象（旧弹道省略、与unit_target互斥），公开实际几何/预算和目标代次历史；ProjectileContact与穿透结束reason沿公共事件，不暴露私有输入ID/攻击/绑定/成本。Rust字面量补piercing:None和结束reason:None。沿原projectiles列表增量，使用新随包Rust/JS解码器严格原子重建。详见[接口](docs/interfaces/member-piercing-v1.md)、[PP01～PP12](docs/batches/SDK-5-26-member-piercing.md)。

Node：`node adapters/node/member-piercing-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/flights.sqlite`。H5：`node adapters/h5/member-piercing-server.mjs --port 4213 --database /absolute/path/flights-demo.sqlite`；八人改accounts-b.json/eight。交付对应hosts/node、hosts/h5与samples/member-piercing。PvP/友军、追踪/反弹、移动区域和完整模式仍后续。


## 机制44：配置化多技能槽（SDK-5-27）

0.29.0/schema7/协议6，在固定1～8成员PVE中沿skill＋additional_skills配置1～8项，顺序与所有数值由配置初始化，不另设重复容量数值。附加数组可省略/空/0～7项；旧1～43的两槽与存在规则保持。末槽类型/数值/绑定和区域/弹道最坏预算全部在I/O前校验，原文/hash保持。

按ID施法与成员独立CD、共享灵力/一个准备/后摇沿原实现；切槽不覆盖准备、不缩短后摇/CD。取消/保护中断未提交无成本；移动保留准备、提交重验实际位置。所有槽复用既有弹道/区域/自身状态、接触与伤害规则。PrivateMemberView.player_skills按配置顺序发布，full/delta私有替换与公共权限保持；严格接入方使用新随包Rust/JS解码器，拒绝超长/重复/坏ID/非法时间或准备关联。无新协议字段、Action、Client方法或金融/恢复DTO。见[接口](docs/interfaces/member-skill-slots-v1.md)、[MS01～MS12](docs/batches/SDK-5-27-member-skill-slots.md)。

Node：`node adapters/node/member-skills-host.mjs --binary target/debug/kernel-host --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/skills.sqlite`。H5：`node adapters/h5/member-skills-server.mjs --port 4216 --database /absolute/path/skills-demo.sqlite`；八槽/八人改accounts-b.json/eight。交付对应hosts与samples/member-skills。选择器由配置生成，接入方只提交原有类型意图，Rust裁决成本/CD/目标与效果。动态换技能、成员不同技能表、友军/PvP与完整模式仍后续。


## 机制45：技能生命成本（SDK-5-28）

0.30.0/协议6/schema7，固定1～8成员与静态1～8槽，每技能可配置hp_cost（fixed或current_hp_ratio，value与min_hp_after_cost完整提供）。定义在接受时冻结，实际成本在提交瞬间按当前HP取整并截断至最低保留生命；目标/灵力/状态/计划校验后与灵力原子消费。成本不走伤害链，不消耗盾/触发归功；成功后后续计划不二次消费。原ActionCommitted私有事件可选hp_cost发布实际值（含0），旧未配置事件省略；Rust字面量补hp_cost:None。见[接口](docs/interfaces/skill-hp-cost-v1.md)、[HC01～HC10](docs/batches/SDK-5-28-skill-hp-cost.md)。

Node：`node adapters/node/skill-hp-cost-host.mjs --configs /absolute/path/to/game/catalog.json --accounts /absolute/path/to/game/accounts.json --scenario your-scenario --database /absolute/path/hp-cost.sqlite`。H5：`node adapters/h5/skill-hp-cost-server.mjs --port 4219 --database /absolute/path/hp-demo.sqlite`。八人改accounts-b.json/eight；交付hosts与samples/skill-hp-cost。客户端只传意图，web/hp-costs.mjs观察真实私有费用，不计算成本；完整真意/多发倍率和施法阶段仍待做。


## 机制 46 施法阶段移动

技能可配置准备和后摇的自由、减速或锁定策略。宿主继续发送原有输入。锁定保留最新移动意图，阶段结束后由 Rust 应用。交付含四人和八人配置。 [接口](docs/interfaces/cast-movement-v1.md)


## 机制 47 方向蓄力

普通和穿透方向弹道可配置蓄力时间与最低威力。Rust 记录开始方向，处理提前取消、手动释放和上限自动提交。本人快照提供动作身份和时钟，本人提交事件提供实际持有时间与威力。 [接入](docs/interfaces/direction-charge-v1.md)


## 机制 48 蓄力重瞄准

方向弹道可配置蓄力期间的更新间隔与角速度。本人准备快照带权威方向，自动提交使用最近有效方向，手动释放可带最终方向。 [接入](docs/interfaces/charge-reaim-v1.md)


## 固定方向引导

机制 49 为普通方向弹道提供可配置的固定时长、发射间隔和周期灵力费用。普通 `cast` 进入准备，提交后冻结方向，在本人协议 6 快照和事件中发布进度、实际费用和结束原因。 [接入](docs/interfaces/fixed-channel-v1.md)


## 方向弹道单次再施放

机制 50 可在初次提交后为普通方向弹道保留一次限时转向。接入方使用本人快照提供的原动作 ID 与索引，Rust 校验有效窗口和实际费用，并继续模拟同一弹道。 [接入](docs/interfaces/projectile-recast-v1.md)


## 后摇跟进施法

机制 51 允许在配置窗口内缓冲下一普通方向技能。Rust 在固定步消费最新有效意图并进入原准备与提交事务。 [接入](docs/interfaces/recovery-followup-v1.md)


## 可取消后摇

机制 52 为普通方向技能提供提交后可取消的时间点。本人可显式结束后摇，或由另一普通方向技能通过准入后接管。 [接入](docs/interfaces/cancelable-recovery-v1.md)


## 普攻冷却跟进

机制 53 可在已提交普攻的冷却内缓存下一击。Rust 在固定步重验冻结的目标代次，并沿用原普攻准备、提交和弹道。 [接入](docs/interfaces/basic-followup-v1.md)


## 普攻锁定敌方单位

机制 54 提供 `attack_unit` 输入，按已发布的 NPC 身份和代次进行普攻。该输入沿用现有准备、冷却、排队与弹道。 [接入](docs/interfaces/locked-basic-target-v1.md)


## 锁定普攻短追击

机制 55 为显式锁定 NPC 的普攻提供可配置的直线追击范围与时限，沿现有移动和普攻事务执行。 [接入](docs/interfaces/basic-pursuit-v1.md)


机制 56 支持为普通 `attack` 配置自动选敌短追击，保留独立的显式锁定范围和现有私有投影。 [接入](docs/interfaces/auto-basic-pursuit-v1.md)


机制 57 提供配置化的局内战利品丢弃。既有拾取处理同局竞争，结算按成员净拾取份额入账。 [接入](docs/interfaces/member-loot-drop-v1.md)


机制 58 支持按配置使用局内拾取的即时恢复道具。背包、实际生命恢复和净额结算由 Rust 统一提交。 [接入](docs/interfaces/member-item-use-v1.md)


机制 59 支持恢复道具冷却。成功使用后按成员和道具 ID 分别计时，本人快照提供截止时间。 [接入](docs/interfaces/member-item-cooldown-v1.md)


机制 60 支持配置灵力恢复道具。实际恢复量由本人事件发布。 [接入](docs/interfaces/member-item-spirit-v1.md)


机制 61 支持配置恢复道具共享冷却。本人快照逐条发布同组道具的截止时间。 [接入](docs/interfaces/member-item-cooldown-group-v1.md)

机制 62 支持一份道具同时恢复本人气血和灵力。本人 `item_used` 发布两项实际恢复量。 [接入](docs/interfaces/member-item-dual-recovery-v1.md)

机制 63 支持配置按最大气血和灵力的万分比用药恢复。每项独立取整并按缺口截断。 [接入](docs/interfaces/member-item-ratio-recovery-v1.md)

机制 64 支持为恢复道具 ID 配置品质倍率。倍率在取整前作用于固定或比例恢复量。 [接入](docs/interfaces/member-item-quality-recovery-v1.md)

机制 65 支持为恢复道具配置自身状态引用。正量恢复和整组状态容量通过校验后一次提交。 [接入](docs/interfaces/member-item-self-status-v1.md)

机制 66 支持按恢复道具品质 ID 配置可刷新攻击或防御状态的强度和持续时间。成功用药时沿用同一状态实例刷新。 [接入](docs/interfaces/member-item-status-quality-v1.md)

机制 67 支持恢复道具附带直接减伤。NPC 对成员的直接伤害在防御后、护盾前应用减伤。 [接入](docs/interfaces/member-item-damage-reduction-v1.md)

机制 68 将同一减伤扩展到 NPC 对成员的物理周期伤害，按每跳执行时状态结算。 [接入](docs/interfaces/member-periodic-damage-reduction-v1.md)

机制 69 支持受治疗状态影响成员自身 HOT，每跳应用执行时倍率并按当前气血缺口截断。 [接入](docs/interfaces/member-periodic-healing-received-v1.md)

机制 70 支持恢复道具配置即时气血回复受治疗倍率。应用品质、取整、应用当前受治疗倍率、取整后，按气血缺口裁剪。 [接入](docs/interfaces/member-item-healing-received-v1.md)

机制 71 支持多人 PvE 怪物模板配置回复光球倍率。击杀时冻结恢复量，拾取时按两项缺口裁剪。 [接入](docs/interfaces/member-recovery-orb-v1.md)

机制 72 支持多人 PvE 成员自然恢复读取生效中的恢复强化状态，并分别结算气血与灵力。 [接入](docs/interfaces/member-recovery-strength-v1.md)

机制 73 支持成员自身状态封锁自然恢复、普攻命中、回复光球和用药产生的灵力回复。 [接入](docs/interfaces/member-spirit-recovery-lock-v1.md)

机制 74 支持 NPC 普通弹道命中存活敌对成员后施加灵力回复封锁，状态记录来源代次。 [接入](docs/interfaces/member-npc-spirit-recovery-lock-v1.md)

机制 75 支持恢复道具按显式配置将溢出治疗转换为护盾，遵守单盾及总盾上限。 [接入](docs/interfaces/member-overheal-shield-v1.md)

机制 76 支持成员自身 HOT 将每跳溢出治疗转换为护盾。 [接入](docs/interfaces/member-periodic-overheal-shield-v1.md)


## 多人自身技能即时治疗

机制 77 支持自身技能按配置即时恢复气血，显式配置可将溢出转换为吸收盾。 [接入](docs/interfaces/member-instant-self-heal-v1.md)


## 多人指定友方即时治疗

机制 78 支持以单位施法输入选择同局另一名成员，重验身份和视线后即时恢复气血。 [接入](docs/interfaces/member-ally-heal-v1.md)


## 指定友方持续治疗

机制 79 的友方技能可绑定来源独立的 HOT。施加和每跳保留来源与目标身份，退场清理对应实例。 [接入](docs/interfaces/member-ally-hot-v1.md)


## 指定友方吸收护盾

机制 80 的 `ally_shield` 技能按配置为指定成员授予吸收盾，共享目标总量上限。 [接入](docs/interfaces/member-ally-shield-v1.md)


## 友方即时治疗溢出转护盾

机制 81 的 `ally_heal` 可把显式配置的即时治疗溢出转成施法者名下的目标吸收盾。 [接入](docs/interfaces/member-ally-overheal-shield-v1.md)


## 友方持续治疗溢出转护盾

机制 82 可把友方 HOT 每跳的溢出量按显式配置转为来源独立的吸收盾。 [接入](docs/interfaces/member-ally-hot-overheal-shield-v1.md)


## NPC 弹道沉默

机制 83 可为 NPC 弹道配置定时沉默，命中后打断目标的主动技能并在期限内拒绝施法。 [接入](docs/interfaces/member-npc-silence-v1.md)


## 定时控制免疫

机制 84 的自身技能可配置定时控制免疫。生效期间拒绝 NPC 弹道沉默，并沿用伤害结算。 [接入](docs/interfaces/member-control-immunity-v1.md)

## 控制时长减免

机制 85 的自身技能可配置定时控制时长减免。NPC 弹道命中时先判免疫，再按目标减免计算沉默期限。 [接入](docs/interfaces/member-control-duration-reduction-v1.md)

## 首控免疫

机制 86 的自身技能可按配置提供一次性控制免疫。定时免疫优先。首控抵挡 NPC 弹道沉默并消耗自身实例，伤害照常结算。 [接入](docs/interfaces/member-first-control-immunity-v1.md)

## 道具净化自身沉默

机制 87 通过配置的本局拾取道具移除指定的自身沉默。沿用库存版本、原子消费、道具冷却与结算。 [接入](docs/interfaces/member-item-cleanse-v1.md)

## NPC 弹道定身

机制 88 支持 NPC 弹道配置定身，阻止移动和需要位移的普攻追击。复用控制免疫、时长减免和显式净化。 [接入](docs/interfaces/member-npc-root-v1.md)

## NPC 弹道眩晕

机制 89 支持 NPC 弹道配置眩晕，打断未完成动作并阻止移动与战斗输入。复用控制免疫、缩时和显式净化。 [接入](docs/interfaces/member-npc-stun-v1.md)

## NPC 弹道减速

机制 90 支持 NPC 弹道按配置减速。成员的方向移动、导航和普攻追击共用实际位移预算，并沿用免疫、缩时和净化。 [接入](docs/interfaces/member-npc-movement-slow-v1.md)

## 自身技能移速加成

机制 91 支持自身技能按配置加速，与 NPC 减速在同一乘区合成。方向移动、导航和普攻追击沿用同一位移预算。 [接入](docs/interfaces/member-movement-haste-v1.md)

## 自身技能冷却缩减

机制 92 支持自身技能按配置提供冷却缩减。主动技能成功提交时计算冷却截止时间。 [接入](docs/interfaces/member-cooldown-reduction-v1.md)

## 自身技能破甲

机制 93 支持自身技能按配置提供破甲。成员直接命中 NPC 时读取破甲、计算护甲减伤，再处理护盾。 [接入](docs/interfaces/member-armor-penetration-v1.md)

## 自身技能暴击率

机制 94 支持自身技能按配置提供暴击率。成员直接命中 NPC 时按护甲、暴击、取整与护盾顺序结算。 [接入](docs/interfaces/member-critical-chance-v1.md)

## 自身技能暴伤加成

机制 95 支持自身技能按配置提供额外暴伤。成员直接命中 NPC 成功暴击时，按当时有效加成计算倍率。 [接入](docs/interfaces/member-critical-damage-bonus-v1.md)

## 自身技能闪避压制

机制 96 支持自身技能按配置提供闪避压制。成员直接接触 NPC 时，先扣除压制，再应用 50% 闪避率上限。 [接入](docs/interfaces/member-dodge-penalty-v1.md)

## 自身技能闪避率

机制 97 支持自身技能按配置提供闪避率。NPC 弹道和近战读取防守成员的闪避率，合计封顶 50%。 [接入](docs/interfaces/member-dodge-chance-v1.md)

## 自身技能命中加成

机制 98 支持自身技能按配置提供命中加成。成员直接接触 NPC 时，先扣闪避压制，再应用命中修正，最后将有效闪避率封顶 50%。 [接入](docs/interfaces/member-accuracy-v1.md)

## NPC 命中属性

机制 99 支持多人 PvE 的 NPC 模板配置命中点，弹道和近战共用修正后的成员闪避概率。 [接入](docs/interfaces/member-npc-hit-v1.md)

## 定时伤害免疫

机制 100 支持多人 PvE 的自身技能配置定时伤害免疫，覆盖 NPC 弹道、近战和周期伤害。 [接入](docs/interfaces/member-damage-immunity-v1.md)

## NPC 弹道不可闪避

机制 101 支持 NPC 弹道按配置跳过闪避，接触时仍检查资格、墙体和伤害免疫。 [接入](docs/interfaces/member-npc-unavoidable-v1.md)

## 成员直接攻击不可闪避

机制 102 支持成员普攻和六种伤害技能配置不可闪避，弹道及区域计划共用已冻结的接触策略。 [接入](docs/interfaces/member-unavoidable-v1.md)

## 直接攻击忽略护甲

机制 103 支持成员普攻、六种伤害技能与 NPC 弹道配置忽略护甲，物化时冻结并进入通用伤害链。 [接入](docs/interfaces/direct-ignore-armor-v1.md)

## 自身技能威力加成

机制 104 支持自身状态配置技能威力加成，伤害技能在预算物化时先计算威力，再应用技能倍率。 [接入](docs/interfaces/member-skill-power-v1.md)

机制105的状态处置与致死保护使用`combat_rules.status_policies`和`combat_rules.death_guards`。显式声明可驱散/可转移的状态类别，通过既有触发器执行`dispel_statuses`或`transfer_statuses`；转移保留实例、原来源、层数及绝对期限，失败时与条件状态消费保持原子性。每个实体代可配置一次致死保护，正常伤害经减伤和盾后才判断，HP费用与强制终局不触发。具体类型、边界和公开事实见[通用战斗接口](docs/interfaces/combat-rules-v1.md)。

机制105触发器还支持`counter_scope: "target"`隔离目标计数/冷却，及`counting: "action_total"`在最后一个原生计划结束后按总实际量结算一次自身收益。后者支持直接伤害/承伤/盾吸收，排除DOT及派生；按创建者实体代次和原动作ID归属，汇总与目标计数预算显式可配，失败不会对旧动作迟到重奖。详见通用战斗接口的动作汇总契约。

机制105提供`combat_rules.status_windows`，把持有者实际伤害/承伤/盾吸收累计及结束HP/灵力恢复配置化。它复用数值/复合状态或HOT的身份和期限，支持reset/preserve刷新、snapshot/live最大属性、独立到期/消费开关；退场只清理，原来源归属与持有者收益分开，结束最多一次。Node/H5可读取状态`window`及私有`window_record_ended`，详细公式、载体限制和预算见[接入契约](docs/interfaces/combat-rules-v1.md#状态窗口与结束恢复)。

机制105（SDK 0.182.0 起）允许 `projectile` 与 `piercing_projectile` 技能配置 `end_on_source_invalid`（默认 false）。开启时成员退场（settling）不再以 SourceInvalid 终止其在途弹道，推进循环继续放行；0.183.0 起命中经 DetachedShot 投递结算冻结载荷——跳过"来源必须为有效状态实体"一条校验，护盾、减伤、免疫、暴击与归功链照常执行，命中后弹道按原规则消耗。默认关闭时弹道随来源清理，行为与历史一致。该字段与已释放区域（detached enter area）的同名字段语义一致但配置域不同：区域字段在区域释放配置上，本字段在技能 wire 上。见 [裁定记录](docs/decisions/SDK-5-95-divergence-arbitration.md)。

机制105的`timed_compound_modifier.flags`支持控制和免疫与数值修正共用一个状态身份：最多3个唯一的敌对控制标记或免疫标记，配合原有最多8项数值。免控、首次免控、打断、缩时、净化和退出清理均按完整实例处理；窗口等未验收组合仍拒绝，转移按后续控制/免疫契约执行，旧复合数值状态保持原行为。见[配置与接入契约](docs/interfaces/combat-rules-v1.md#复合控制与免疫标记)。

机制105提供`combat_rules.status_cleanses`：按HOT原生跳数或状态自然到期，清理持有者已登记的可驱散状态。配置类别、数量及全部/载体施加以来的时间筛选，保留原来源归属；保护、退场和预算失败不补发，驱散载体不执行到期动作。与窗口恢复、复合标记、过量转盾共用既有生命周期，Node/H5读取cleanse能力及仅含数量的私有结果。见[接入契约](docs/interfaces/combat-rules-v1.md#周期与到期净化)。

机制105提供`combat_rules.periodic_recovery`，在同一HOT周期内配置基础回灵、满血或过量治疗转灵力。基础回灵先执行，转换只消费实际回灵所需的治疗预算，余量再转盾，随后净化；来源与持有者分离、上限/封锁/保护和原调度保持。周期快照发布可选raw_spirit，私有periodic_spirit_tick报告真实收益及预算分配，见[接入契约](docs/interfaces/combat-rules-v1.md#hot双资源恢复与治疗余量分配)。


机制105的恢复余量可直接生成通用记录盾：即时自身/友方治疗、HOT及恢复道具原有的`overheal_shield.status_id`允许引用`combat_rules.shield_records`。只有真实增加盾量才按原来源有效威力采样记录上限；reset/preserve、统一盾池和一次结束释放沿原实例执行。满池不刷新或清记录，道具预检失败不扣库存/版本/冷却。字段及顺序见[过量恢复生成记录盾](docs/interfaces/combat-rules-v1.md#过量恢复生成记录盾)，公共示例仍是机制验证配置。


机制105的通用状态转移已覆盖原生吸收盾：普通盾、过量恢复生成的盾和记录盾均可显式配置`status_policies.transferable:true`。完整实例、原来源、余额/记录及期限保持；条件消费和混合数值转移后的最终maxHP决定目的统一池容量，失败整批不动、不消费条件或启动ICD。转移不重授、不刷新或触发结束，后续由新持有者原生吸收/结束。详见[护盾转移契约](docs/interfaces/combat-rules-v1.md#原生护盾转移的预算与记录)。


机制105的显式状态转移现覆盖DOT/HOT：原实例/来源、期限、跳数/下一跳和采样策略保持；DOT新持有者仍须相对原来源敌对，HOT可在transferable政策明确授权下跨阵营搬移收益。快照保持原值，live按原来源或新持有者在下一原生跳读取；旧生成盾不随HOT隐式转移，来源保护/退出和重连仍用原生命周期。详见[原生DOT/HOT转移](docs/interfaces/combat-rules-v1.md#原生dothot转移)。


机制105的原生状态转移现包括标量/复合控制、免疫与成员灵力恢复封锁。硬控保持原来源敌对、原来源可行动及最终预览的免疫门禁，失败不消费/启ICD/提前打断；成功后复用正常施加的原生打断，剩余期限不重算。免疫类可按显式政策搬移，后续首次免控只消费完整新持有实例一次。详见[控制与免疫转移](docs/interfaces/combat-rules-v1.md#原生控制与免疫转移)。公开PVE通过下述友方提交入口验证NPC来源硬控承担；Boss控制替换仍未签收。

机制105提供`ally_skill_committed`通用事实：指定友方的即时治疗、HOT或护盾成功提交并执行主效果后，以施法者/冻结友方身份驱动既有触发器。通过配置可承担或清理友方状态；计数、条件、ICD、概率与预算复用。附带效果失败保留主技能已支付费用及收益，自身条件消费仍原子。详见[指定友方技能提交契约](docs/interfaces/combat-rules-v1.md#指定友方技能提交事实)。

机制105允许显式转移合法窗口/净化载体：保留原来源、累计/最大属性快照、施加与到期时间以及HOT原调度；后续记录、恢复与净化只作用当前持有者。since_application沿原施加时间筛选，结束仍先净化后恢复一次，重连/退场和预算沿原机制。详见[生命周期转移契约](docs/interfaces/combat-rules-v1.md#窗口与净化载体转移)。

机制105支持按战斗profile配置control_policies，通用控制期限帽与完整数值替换复用原状态事务。basic_interval_delay以bps增加普攻间隔，可与复合movement_slow共用来源/身份/期限，并与attack_speed组合推进成员普攻及NPC近战时钟；1000ms基础间隔加15%为1150ms。配置限制、转移重验及取整见[接入契约](docs/interfaces/combat-rules-v1.md#按目标配置组处理控制)。

机制105的direct_hit在主伤害及命中附带状态后投放，包括全盾吸收；apply_other_status可把本人来源的目标层数按条件原子消费并转为有害数值/硬控状态，复用目标profile控制策略与既有触发预算。标量movement_slow支持1～32层，按单实例层数乘算后取最强减速；旧机制及复合减速限制保持。见[命中条件与目标状态转化](docs/interfaces/combat-rules-v1.md#命中条件与目标状态转化)。

机制105的攻击规则支持contact_status：命中判定后消费本人来源的有限状态层数，将配置的逐层增伤/破甲只用于本次统一伤害链。armor_penetration_cap_bps可显式约束静态、标记及实时属性的破甲总和。标记在实际接触时取样，在途预算保持冻结；不新增伤害事件或权威前端计算。见[消费状态增强本次命中](docs/interfaces/combat-rules-v1.md#消费状态增强本次命中)。

机制105提供`chain_damage`触发效果：条件消费后冻结固定量/来源ATK/事实量的组合预算，在同一原生时刻向最近且视线可达的未访问敌人执行最多8跳，可配置衰减、指定末跳倍率、伤害种类与有害附带状态。免伤尝试、全局命中预算与来源失效均有明确终止规则，派生不再触发或累计记录；Node/H5读取权威私有跳序，重连只恢复原生状态。参数、准入和取整见[有界链式伤害](docs/interfaces/combat-rules-v1.md#有界链式伤害)，完整原内容组合继续单独验收。

机制105支持`contact_status_consumed`及其`contact_consumption`筛选：只在攻击规则实际消费指定层数后，提供增强后的防御前预算给通用触发器。chain_damage可配置forward选择，reduce_skill_cooldowns可按当前剩余比例和固定毫秒返还本人指定技能。联动独立预检，私有摘要与权威冷却截止分开消费，重连不重放；见[命中消费联动与技能冷却返还](docs/interfaces/combat-rules-v1.md#命中消费联动与技能冷却返还)。

机制105提供`combat_rules.contact_reactions`：捕获主伤害前已有的有限有害状态，在合格接触或实际扣血后整份消费，按原施加者实时有效威力追加一次有界伤害。支持本人或第三方引发、目标profile控制替代、独立预算及原生伤害/转移/清理；新附带状态不被同一次命中立即消费，派生不递归。Node/H5通过原状态和私有来源摘要消费，详见[接触状态反应](docs/interfaces/combat-rules-v1.md#接触状态反应)。

机制105支持`finish_other`：按精确生命阈值选择原生终结尝试，或按目标combat profile改为有来源威力上限的真伤。条件消费后的生命上限、盾余额、威力和免伤共同预检；不直接修改HP，沿原盾/致死保护/死亡归属结算，派生不递归，重连不重放。字段及隐私边界见[生命阈值终结与配置组替代](docs/interfaces/combat-rules-v1.md#生命阈值终结与配置组替代)。


机制105支持终结成功事实`finisher_kill`：按提供者规则和execute/power_damage/any筛选真实致死结果，驱动既有自身状态、治疗、回灵、冷却返还或有限追击。主规则完成后同步执行，原动作身份/代次及预算保持，保命或非致死无成功事实，派生不再触发，重连不重奖。restore_spirit可配置`max_spirit_bps`，固定量加当前有效maxSpirit比例沿原资源缺口和封锁处理；见[终结联动接入契约](docs/interfaces/combat-rules-v1.md#终结成功后的有界联动)和两份followup示例。原完整内容、专用阶段及功法威力绑定继续单独验收。


机制105支持`contact_status.sampling:"action"`：第一次合格接触冻结本人状态的实际消费层数，同动作多目标/区域/引导/重复命中只消费一次，后获得的状态留给新动作。`finish_other.rule.consumed_status_bonus`按冻结层数提高终结阈值，`finisher.enhanced`筛选真实增强结果，可配置下一次增益且防止同次施法自刷新。快照预算有界，原生计划/源退出清理，在途重连保持；参见[接入契约](docs/interfaces/combat-rules-v1.md#按动作消费增强与终结防自刷新)与enhancement两配置。原完整阶段/功法威力及所有投放继续开放。

机制105的[通用施法阶段组合](docs/interfaces/cast-lifecycle-v1.md)补充：准备/活跃阶段有效新移动打断、移动引导、提前松手排队及独立松手前摇、绝对期限的明确跳数恢复引导、全技能后摇权限与原子接管，以及显式资源准入配置。通过配置声明策略与数值；Rust统一执行，Node/H5消费权威时钟与私有结果。原始技能全量绑定和当前模块整体验收仍按SDK-5-88总账推进。

成员技能可通过可信建局资料与宿主控制独立授予、装备和撤销；未装备技能不能由玩家输入施放，重新装备保持冷却债务。参见[成员技能所有权与槽位控制](docs/interfaces/skill-loadouts-v1.md)。

指定单位即时效果与定向引导：[配置、输入、预算及中断契约](docs/interfaces/unit-effects-v1.md)。

资源预留与提交：[配置和私有准备投影](docs/interfaces/resource-commit-v1.md)。


Current-resource transfer: [resource-steal-v1](docs/interfaces/resource-steal-v1.md) defines explicit HP policies, frozen target-current budgets, private actual-return results, and live `life_steal` status aggregation. Original content policy remains pending; no game IDs are embedded in the mechanism.


Trusted raw growth inputs and native constant-point/selected-skill power derivation: [character-build-v1](docs/interfaces/character-build-v1.md). Node passes immutable raw records through the trusted Create seat; it does not calculate battle attributes or final power. Only the recipient sees its effective slot power values. Full original equipment progression remains under R5.1.

Finite status-end leaves and single delayed missing-HP damage: [status-ends-v1](docs/interfaces/status-ends-v1.md). Private source lineage survives ticks and transfer, blocks recursive rewards, and uses native status clocks and damage settlement. Two isolated catalogs preserve the existing version capacity bound.

Native action selection, current equipped-provider/mastery-row eligibility and bounded per-action attempts: [source-gates-v1](docs/interfaces/source-gates-v1.md). Committed plans retain their skill classification; derived effects remain non-recursive, while qualification and source ownership are checked in Rust.


Fixed and per-layer status values, real zero counters, finite threshold leaves and same-instance damage windows: [status-counters-v1](docs/interfaces/status-counters-v1.md). Rust preserves identity, source, deadline and record through adjustment and consumption; public replicas carry the actual counter value.

有条件的严格单位提交、一次消费的双方状态组合、消费前冻结的按层资源倍率及仅延长既有数值状态期限：[conditional-activation-v1](docs/interfaces/conditional-activation-v1.md)。支持可信阶段上下界、实时装备资格及失败原子预检；原内容的HP夺取管线、乘区和完整成长投放各自签收。

初次提交实际已付灵力的有界比例返还、显式计数器耗尽移除及既有结束叶子：[paid-resource-effects-v1](docs/interfaces/paid-resource-effects-v1.md)。先付清费用，后执行恢复；重连不重放，原内容普通功法清单及完整境界技能仍逐项签收。

[最终施法拦截](docs/interfaces/cast-interception-v1.md)：可信配置控制普通施法的最终提交拦截和目标 profile 分支，沿用现有 Node 输入与发布协议。

即时自身技能可显式配置 `commit_on_accept`，在无前摇、无引导时使用同一个提交与去重入口。已有动作汇总也支持在整次直接伤害动作结束后给本人已有计数器加层，见[施法生命周期](docs/interfaces/cast-lifecycle-v1.md)与[战斗规则](docs/interfaces/combat-rules-v1.md)。

机制 105 的协议 6 宿主可以通过 `combat_rules.member_summons` 绑定自身技能与单位模板。召唤体独立参战和回收，公开帧通过 `monsters[].summon` 标识归属和到期。 [有界召唤配置与发布](docs/interfaces/combat-rules-v1.md#有界成员召唤)

协议 6 的多人场景可提供世界凸边界和定时阻挡。SDK 0.139.0 支持配置方向性高差、格内坡道和固定步地形伤害，公开 `terrain_damage` 供宿主消费。 [世界几何与地形接入](docs/interfaces/world-geometry-v1.md)

SDK 0.140.0 的多人场景可启用 `pve.npc_ai`，配置仇恨切换阈值、强制目标标记、同群协助、远程距离和有序技能槽。协议 6 公开怪物的目标、归位和实际技能结算。 [怪物战斗 AI 接入](docs/interfaces/npc-combat-ai-v1.md)

SDK 0.141.0 提供配置化 Boss 阶段和自动参战席位，复用既有技能、库存及结算入口。 [Boss 阶段](docs/interfaces/npc-combat-ai-v1.md#boss-阶段) · [参战 NPC](docs/interfaces/participant-ai-v1.md)

多人 World 支持统一奖池、环境补充、共享与私有容器、部分拾取、带贡献优先权的怪物掉落、独立实例物品和死亡遗物。库存修订与恢复账本覆盖跨席位转移。 [资源、容器与遗物](docs/interfaces/world-loot-v1.md)

协议 6 可为多种奖励资产配置独立击杀积分、结果倍率和余额上限，与库存和实例一并原子结算。 [多资产结算奖励](docs/interfaces/settlement-rewards-v1.md)

协议 6 可按结果声明核心结算后的附属事实，支持独立的提交、查询、重试和恢复。 [结算附属事实](docs/interfaces/settlement-facts-v1.md)

协议 6 可选的成长结算支持实际扣血计分、入场双余额、阈值转换、仓库实存与溢出，以及按结果更新区域版本。 [成长结算接入](docs/interfaces/settlement-progression-v1.md)

SDK 0.156.0 可声明堆叠物品和实例共用的仓库格。Node SQLite 参考宿主可按区域版本和写入序号保存位置，迁移时原子清除旧坐标。 [共享仓库格与位置写入](docs/interfaces/settlement-progression-v1.md#共享格)

SDK 0.157.0 可为普通撤离局开启成员互为敌对，也支持零费用准入。 [撤离局对抗接入](docs/interfaces/publications-v6.md#撤离局成员对抗) · [准入与结算](docs/interfaces/settlement-progression-v1.md)

SDK 0.158.0 支持限时存活和共用截止时间的分波清敌，并发布波次进度。 [顺序模式接入](docs/interfaces/publications-v6.md#限时存活与分波清敌)

SDK 0.159.0 支持按席位配置和切换训练目标组，清除时仅终止该席的战斗运行时。 [训练目标接入](docs/interfaces/publications-v6.md#每席训练目标)

SDK 0.160.0 增加可配置的训练目标往返、目标重建、训练复活和私有输出统计。 [训练运行时接入](docs/interfaces/publications-v6.md#目标行为复活与个人统计)

SDK 0.161.0 增加共享击杀计数、强度与掉落档位、一次性限时强化、击杀进度阶段和固定截止。 [共享击杀计数接入](docs/interfaces/publications-v6.md#共享击杀计数与强度档位)

SDK 0.162.0 增加撤离预警、终局宽限伤害、硬截止与可选公共读条。 [撤离时限接入](docs/interfaces/publications-v6.md#撤离时限与读条广播)

协议 6 支持局内加持槽、带限额和读条的共享机会，以及原生检查点领取资格。加持可按等级启用战斗触发、灵力减耗与来源专属护盾，复用原有发布和伤害结算。召唤绑定可在存活期间给本体提供直接伤害增益和减伤。 [局内加持](docs/interfaces/world-boons-v1.md) · [召唤配置](docs/interfaces/combat-rules-v1.md#有界成员召唤)

检查点可绑定专属反制伤害。宿主选择到点即结算或发放一次限时反制，可配置加持等级的窗口延长与伤害倍率。 [检查点反制](docs/interfaces/publications-v6.md#检查点反制)

SDK 0.163.0 支持局内装备竞夺、追猎、死亡转手及撤离实例入账。 [装备竞夺接入](docs/interfaces/publications-v6.md#局内装备竞夺与追猎)

SDK 0.164.0 支持按区带停留累积攻防削弱，以及共享交互点的减层和限时增益。 [区域停留接入](docs/interfaces/publications-v6.md#区域停留与共享交互)

SDK 0.165.0 支持按奖励通道配置逐次伤害计分取整，并在恢复时校验冻结积分。 [成长结算接入](docs/interfaces/settlement-progression-v1.md)

SDK 0.166.0 支持可信装备实例的独立奖励余额，并在核心结算中冻结当前阶段进度。 [实例奖励](docs/interfaces/settlement-rewards-v1.md#可信装备实例归属) · [阶段进度](docs/interfaces/settlement-progression-v1.md#当前阶段进度)

SDK 0.167.0 支持宿主按生效档案提供事件计分倍率。击杀和实际扣血各自按发生时的档案计分，耗尽后的待处理状态随核心结算提交。 [事件计分倍率](docs/interfaces/settlement-rewards-v1.md#事件计分倍率) · [耗尽交接](docs/interfaces/settlement-progression-v1.md#余额耗尽后的交接)

SDK 0.168.0 支持可选的堆叠物资流向。宿主可从同一核心结算读取拾取总量、消耗、主动丢弃和遗物转出，与携入、终局保留和仓库实存分别使用。 [物资流向契约](docs/interfaces/settlement-progression-v1.md#堆叠物资流向)

SDK 0.169.0 支持按配置进行限次奖励调整，冻结缩放、追加、暂存返还和剩余次数，与奖励同笔提交。 [限次奖励契约](docs/interfaces/settlement-rewards-v1.md#限次奖励调整)

SDK 0.170.0 支持可选的末次部分收费和顺序转换。入场与补偿共用实际扣费，奖励通道可选择标准费或实扣量，终局按配置执行扣减、耗尽检查和进度计算。 [入场与转换契约](docs/interfaces/settlement-progression-v1.md#末次入场与顺序转换)

SDK 0.171.0 支持协议 6 的权威等距格网。地形、寻路和碰撞共用菱形格网，动作和快照沿用世界坐标。 [等距格网契约](docs/interfaces/world-geometry-v1.md#权威等距格网)
