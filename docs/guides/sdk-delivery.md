# SDK 本地交付与独立接入

当前交付版本见 `Cargo.toml` 和 `kernel-host --sdk-info`。SDK 交付机制、契约和参考宿主，游戏配置与账户由接入方持有。

## 构建与校验

```sh
python3 -B scripts/package_sdk.py --out /absolute/path/to/delivery
```

构建器生成 Cargo 源包，从源包在离线环境构建普通 JSONL 二进制，并校验交付包摘要。详见 [包内接入说明](../../delivery/README.md)。

解压后用 `python3 -B verify_bundle.py` 校验文件，再用 `bin/kernel-host --sdk-info` 查看支持信息。使用宿主清单启动 `bin/kernel-host --configs /absolute/path/to/game/catalog.json`。用包外数据和账本验收准入、回执、结算和恢复。

升级时保存接入方配置原文、id/hash、原 Create 身份和可信检查点。旧配置 schema 1 依赖历史版本的内置参数，先用原版本处理未决责任，再由宿主迁移到 schema 2。

## 历史机制记录

以下版本、批次证据和样例路径为历史记录。

机制26在schema4/协议3 PVE中保留skill主技能，并允许additional_skills配置一个附加弹道技能。两项独立成本/CD，共用灵力、单个准备动作与后摇；移动、取消、切技能、前后台或重连不能缩短已提交后摇。Snapshot.player_skills提供按配置顺序的绝对CD，pending_action显示准备动作；ActionCommitted.skill_id标识已提交技能。旧skill_ready_at_ms仍仅表示主技能，机制1～25省略新字段并保持原动作策略。重启恢复仅核对冻结资产，清除临时技能视图。公开DTO增加可选字段，Rust结构体字面量需适配。

机制27新增玩家self_status技能和CastSelf输入（kind=cast_self、skill_id），主/副槽均可使用。配置只含ID/成本/前后摇/CD及1～2项on_commit_self_statuses；严格拒绝弹道几何/伤害和hit绑定字段。固定步提交时沿原整组容量/灵力预检施加合法自身状态，不创建弹道或直接攻击；与弹道技能共用准备/后摇和灵力、各自CD。0前摇仍在下一固定步提交。机制1～26的配置和输出保持，旧Cast必须有方向，新CastSelf禁止方向/目标等多余字段。Action新增枚举分支，Rust穷尽match需适配；六类Client/协议1～3/资产及恢复schema保持。

机制28新增unit_projectile与CastUnit（kind=cast_unit、skill_id、target{id,generation}）。只支持当前World已发布的存活敌人；接受/提交重验身份代次、资格、中心射程及视线，提交前失败免扣。提交时按合法当前位置冻结方向，沿原弹道/伤害/合法状态执行，不追踪或换人。非目标先接触只阻挡不受伤，目标失效/换代中止在途弹道，不退款或回滚CD。MonsterView、准备视图与单位提交事件增加可省略unit_target；旧1～27不输出该字段，旧配置/责任保持。Action/Error枚举与视图字段为源码变更，Rust消费方需适配；六类Client/协议1～3/资产及恢复schema保持。

机制29增加ground_circle / CastGround，明确世界坐标position；接受/提交双重校验地图半开边界、可行走落点、射程与视线，严格拒绝而不裁剪。提交前原子预检灵力/self状态/两计划容量；成功冻结中心/攻击量、扣费/CD/后摇并发布预警，至少下一固定步才按当前敌人位置结算一次圆形伤害，沿原直接命中/DEF/盾/状态/掉落链。PveView.player_area_warnings和pending_action/ActionCommitted.ground_position仅机制29输出；新增PlayerAreaStarted/Resolved/Cancelled。死亡/终局/关闭/实际换轮清计划，移动/后台/同World重连保留已提交计划且不退款，重启不恢复临时战斗。公开Action/事件/视图源码消费需适配，协议和资产责任保持。

机制30新增ground_field，复用CastGround严格选点及接受/提交重验，配置必填duration_ms/tick_interval_ms。首跳在预警结束后等待一个间隔，按计划时刻周期结算且截止点排除，最多128跳；当前目标位置/DEF/盾/闪避、原伤害/命中状态/击杀掉落链保持，直接攻击量提交冻结。只提交扣费/CD一次，与单次圆形共用每World两个地面计划槽。PveView.player_fields和PlayerFieldStarted/Tick/Ended仅机制30输出；来源失效固定结束，死亡/终局/关闭/实际换轮清理，后台/移动/同World重连保留已提交区域，重启不恢复临时战斗。旧1～29保持，公开视图/事件枚举源码消费需适配，协议和冻结资产责任保持。

机制31允许ground_field配置可选contact_policy（max_targets_per_tick 1～32、max_total_hits 1～128、repeat_interval_ms 0～60000）。0为同一ID/代次每区域一次，正数按上次成功接触的计划时刻计算，等值合法，进出/重连不清历史。每周期按距离平方/ID/代次排序；真实直接接触（含0HP损失/全盾吸收）消耗名额和预算，闪避/阻挡不消耗，额度满停止后续目标/状态。总预算0以hit_budget_exhausted立即只结束一次；成本/CD仍只在提交时处理。通用身份/时间/计数组件首批仅接入持续区域，历史≤128；配置时FieldView.contact_policy和Tick.hit_targets/remaining_hits输出权威事实，省略策略及旧1～30保持原形状/顺序/语义。公开DTO/枚举源码消费需适配，协议与冻结资产责任保持。

机制32新增piercing_projectile（方向Cast，必填ttl_ms与max_hits），按连续扫掠顺序接触敌方身份、墙同距优先截断，每身份每发弹一次、总成功接触耗尽结束。复用ContactBudget，闪避不耗、全盾0损失耗预算。最多2发在途，提交原子扣费/开启独立CD，死亡/终局/关闭/实际换轮结束。同World重连保留位置/期限/行程/历史/预算，重启仅恢复冻结责任。所有数值/绑定由配置初始化，旧1～31省略新增视图/事件；公开DTO/枚举源码消费需适配。本批仅方向型直线穿透，不代表完整弹道家族或M05完成。

机制33允许piercing_projectile显式配置repeat_interval_ms（0～60000整数）：0每身份一次，正数按该发弹最新成功接触时刻+间隔给资格；仍需真实扫掠，未接触不补跳，每步每身份最多尝试一次。每次成功含全盾0损失消耗原max_hits总预算并刷新最新时刻，闪避/失效不耗不刷新，成本/CD只提交一次。配置随接受冻结，同World重连保留历史/计时/预算，死亡/终局/关闭/实际换轮回收；重启仅恢复冻结责任。显式配置才输出弹道repeat_interval_ms、正间隔contacts.next_eligible_at_ms和Hit.hit_at_ms，旧1～32及省略配置的形状保持。公开DTO/事件源码消费需适配；本批只扩展移动接触频率，不代表完整M05。

SDK-5-16 / 0.19.0 / 机制34：提供[固定线形区域](../interfaces/line-fields-v1.md)，复用原周期/身份预算和资产契约。

SDK-5-18 / 0.20.0 / 协议4/schema5/机制35：包内`hosts/node/two-member-host.mjs`、`hosts/h5/two-member-server.mjs`及`web/two-member.*`提供普通Node/SQLite和双页面参考接入。复制`samples/two-members`到包外，显式提供清单、各自账户和独立数据库；参见[契约](../interfaces/two-members-v4.md)。重启仅恢复原责任，不能恢复战斗。

SDK-5-19：`samples/members`为四人/八人外部参考数据，`hosts/node/member-host.mjs`与`hosts/h5/member-server.mjs`显式传配置/账户或host配置/数据库。普通H5页面为`web/member.*`；[契约](../interfaces/members-v5.md)。旧SDK-5-18归档和报告保持，协议4双人消费链重新验证。


SDK-5-20新增`samples/delta`、`hosts/node/delta-host.mjs`、`hosts/h5/delta-server.mjs`、`web/delta.*`及共享纯解码器`web/replication.mjs`。[协议6](../interfaces/publications-v6.md)明确流epoch/基线和有限恢复；普通Node消费包外自有配置，H5显式传`--configs`与`--host-config`。源包测试数据在普通消费阶段隐藏，空缓存离线默认/全特性测试和Clippy另行验证；同源release摘要绑定本批所有接入报告。旧协议1～5仍有独立消费链，旧归档和配置不改写。


SDK-5-21交付`samples/multiplayer-periodic`、`hosts/node/multiplayer-periodic-host.mjs`、`hosts/h5/multiplayer-periodic-server.mjs`、`web/multiplayer-periodic.*`及纯观测器`web/periodic.mjs`。普通消费只提交意图/存储事实，Rust决定周期与金额；详细规则见[契约](../interfaces/multiplayer-periodic-v1.md)。空缓存离线源码与包外38项普通消费，包含旧协议1～6和两套多人周期配置；原SDK-5-20归档与证据保持。


SDK-5-22交付samples/multiplayer-shield、hosts/node/multiplayer-shield-host.mjs、hosts/h5/multiplayer-shield-server.mjs、web/multiplayer-shield.*与纯观测器web/shields.mjs。详见[多人盾接口](../interfaces/multiplayer-shields-v1.md)和[执行证据](../evidence/SDK-5-22-multiplayer-shields-2026-11-05.md)。空缓存离线源码与包外40项普通消费通过，保留原38项并加入两套盾配置；旧协议1～6关键链及机制38单独回归，原归档保护。新预览4201独立账本，旧4198等不替换。


SDK-5-23交付samples/multiplayer-combat、hosts/node/multiplayer-combat-host.mjs、hosts/h5/multiplayer-combat-server.mjs、web/multiplayer-combat.*及纯观测器web/combat.mjs；使用[接口](../interfaces/unified-member-combat-v1.md)中的NPC单位身份提交，不在宿主裁决玩法。42项包外普通消费保留原40项，增加两套单位动作配置；空缓存离线源码默认/全特性同集及Clippy通过。新4204预览独立账本，旧验证台保持。见[证据](../evidence/SDK-5-23-unified-member-combat-2026-11-05.md)。


SDK-5-24交付samples/member-area、hosts/node/member-area-host.mjs、hosts/h5/member-area-server.mjs、web/member-area.*及纯观测器web/areas.mjs。四人单次、八人持续两套配表均通过真实Node/H5闭环；额外保留原42项，包外普通消费共44项，空缓存离线源码默认/全特性同集及Clippy通过。新4207预览独立账本，旧页面保持。[接口](../interfaces/member-circle-plans-v1.md)，[证据](../evidence/SDK-5-24-member-circle-plans-2026-11-05.md)。


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
