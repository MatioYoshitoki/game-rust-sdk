# SDK-5-7 受治疗修正验收（2026-11-04）

先冻结[HM01～HM10](../batches/SDK-5-7-healing-received.md)，再实现0.10.0/机制25。有限范围全部通过；[逐项映射](../checklists/SDK-5-7-acceptance.json)和[机器汇总](../../artifacts/sdk-5-7/acceptance.json)绑定源码、原生输入、版本、报告及交付包。

## 已交付行为

定时状态新增healing_received选择器，复用来源分组、refresh/ignore、槽位/期限和清理。目标有效modifier_bps相加后以0为倍率下限，无额外正向玩法帽。倍率、时长、重复策略和绑定全部通过配置初始化。

HOT、怪物预算自疗、支援共用resolve_heal：保留原基础量/预算份额，执行时一次倍率/取整后按当前HP缺口裁剪。原自然恢复、回复光球、归位等恢复保持原规则。新增倍率不永久改写HP/ATK/DEF，不复活/转盾/回灵，不额外消费随机数或触发主动命中。

机制25快照提供healing_received_bps，heal_resolved提供基础量、倍率、调整量和实回，原三条治疗路径的事件实际值一致。旧1～24不增字段/事件。重连保持状态原期限，失效/复位/关闭清理；重启只履行冻结资产责任，移除临时状态/派生字段，不恢复World或重算旧金额。

[配置与接入](../interfaces/healing-received-v1.md) · [完整样例](../../examples/heal-modifier-host/catalog.json)

## 执行证据

| 门禁 | 结果 |
|---|---|
| 原生/配置 | 默认/全特性相同835项Rust（保留800、新增35）、Clippy -D warnings及22项Node通过。覆盖20种非法配置、数值边界、self/hit引用、规范化重载及旧19～24形状。[默认日志](../../artifacts/sdk-5-7/default.log) · [原生摘要](../../artifacts/sdk-5-7/native-verification.json) |
| 金额与聚合 | raw100在−50%/+50%/−25%下为50/150/75；多定义/多来源累加、总削弱归零、无ATK/DEF帽、refresh/ignore/最新来源、容量原子性、零量/满血/超量/死亡及u64调整量通过。[HM02/HM03映射](../checklists/SDK-5-7-acceptance.json) |
| 三条真实路径 | HOT在修正到期后保持原raw/节拍并恢复正常；真实怪物自疗及支援将预算100分34/33/33，再按−50%得到17/16/16，成本/CD与原事件实际量一致。修正中途到期后后续份额33/33，不回算预算、不重复乘；支援读取目标而非施法者倍率。同上HM04/HM05 |
| 生命周期/其它恢复 | 原自然恢复和光球回复不受新乘区影响；不打断撤离/支援、不回退Boss阶段或引入随机数。后台期间修正正常到期且HOT不补跳，死亡/换代/整组复位、100次关闭及合法容量下输出峰值封存/World回收通过。同上HM05/HM06 |
| 普通Node/H5 | 两套包外配置4条链：基础HOT20，A削弱至50%实回10，B强化至150%实回30；修正到期后同一HOT均恢复100%实回20，随后战斗→拾取→撤离/死亡结算。包含刷新/忽略、重连与旧连接拒绝，页面异常0、一次入场/结算、World/未决=0。[报告cases](../../artifacts/sdk-5-7/verification.json) |
| 纯触屏 | 390×844只用触屏控件完成真实受伤、施加/重施、回复变化/到期恢复、战斗拾取、双终局；HTTP仅只读观察。页面异常0且回收完成。同上touchCases；[手机截图](../../artifacts/sdk-5-7/outside-b-received-0-received.png) |
| 强杀/恢复 | 活跃修正已影响回复、冻结终局后、资产提交后三处强杀，各连续两次恢复。修改外部倍率不重算原Create/hash/金额/结果/operation ID，不重复扣费/发奖，不恢复临时派生或World。同上crashes |
| 旧版本普通链 | 旧19～24各双终局，共12条；原属性、护盾、DOT/HOT及JSON保持，不出现heal_resolved。完整浏览器链一次执行通过。同上legacy |
| 独立交付 | Cargo源包、21项锁定第三方源及普通release二进制；Rust源与候选一致、包内无工作区绝对路径。删除回归目录和samples后，包外25～19配置各Node双终局，共14条。空缓存离线Rust协议1六模式、协议2/3十四场景与各自重启、两次核对通过。[报告](../../artifacts/sdk-5-7/delivery/independent-verification.json) |
| 解压源码/保护 | 空Cargo缓存、离线执行解压源码的相同835项默认/全特性及Clippy通过，不把重复运行计作新增。原20场景、旧样例、v1.1来源包及历史0.3.1～0.9.0交付73项原文/hash保持。[源包报告](../../artifacts/sdk-5-7/delivery/source-verification.json) · [保护清单](../../artifacts/sdk-5-7/baseline-verification.json) |

旧协议2包外副本的撤离解锁/读条仍为3000/500ms，以适配参考玩家期限；原数据保持。原生集合保留全部旧测试；本批没有重跑无关具名地图的全部浏览器专项，也未签收来源50向量/48场景的全量精确映射。

## 交付和范围

[0.10.0本地交付包](../../artifacts/sdk-5-7/delivery/match-kernel-0.10.0-aarch64-apple-darwin.tar.gz)，4,540,524字节；SHA256为`3d2b5d631f5650807a46b80fe3a22945fb297277b452fc8ef41e984d2aebfa8f`。预编译平台Apple Silicon macOS，replay=false；其它平台及生产发布分别验收。[交付清单](../../artifacts/sdk-5-7/delivery/package.json)

试玩：[127.0.0.1:4180](http://127.0.0.1:4180/)，独立账本`.local/sdk-5-7-demo.sqlite`。受伤后施放技能并停手，观察受治疗50%时每跳10，到期后恢复每跳20。启动检查未创建对局、页面异常0，旧4173～4179预览/账本保持。[预览检查](../../artifacts/sdk-5-7/local-preview.json)

本批只签收HOT/怪物自疗/支援的普通受治疗修正；恢复强化、其它治疗公式/复杂叠层、友军目标、过量转盾、完整状态目录及生产接入继续按[剩余矩阵](../checklists/sdk-requirements-gap.md)推进。D-03/D-05不变，不扩新地图或第二款游戏内容。
