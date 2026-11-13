# SDK-5-5 周期物理DOT验收（2026-11-04）

先冻结[PD01～PD10](../batches/SDK-5-5-periodic-damage.md)，再实现0.8.0/机制23。有限范围全部通过；[逐项映射](../checklists/SDK-5-5-acceptance.json)和[机器汇总](../../artifacts/sdk-5-5/acceptance.json)绑定源码、原生输入、版本、报告与交付包。

## 已交付行为

- periodic_damage复用状态实例、来源分组、槽位/容量、期限和清理。配置给出间隔、时长、ATK万分比、refresh/ignore与命中绑定，只允许玩家与怪物之间的敌对target施加。没有逐图专用规则或新宿主tick入口。
- 施加组开始时冻结来源有效ATK比例，首跳等待间隔，每跳读取目标当前DEF/盾。refresh保留实例ID并重取来源、归零进度/重置计划；ignore保持原来源和节拍。到期/身份失效先于周期执行，随后进入原移动/AI/弹道流程；同刻多个计划按时刻、实例ID排序。
- 每跳不重判闪避/暴击、不消费伤害随机数，也不重复触发命中状态、回灵/剑印/吸血、反击/主动命中窗口或支援打断。实际HP损伤仍触发原HP阶段、交互/撤离中断、统一死亡/击杀/一次掉落链。
- 保护/后台/未就绪、归途或复位倒计时跳过到期点，不扣盾血、不延寿、不补跳。来源/目标失效、终局/关闭清理；重连继续同World节拍，重启只履行冻结资产责任，不恢复DOT/World或按墙钟补跳。
- 新实例periodic与periodic_tick给出权威进度、计划/执行时刻和实际吸收/扣血。旧19～22实例省略新字段；六类Client、协议1～3及资产/恢复schema保持。0.8.0包含公开Rust DTO字段和事件枚举的源码变更。

[配置与接入](../interfaces/periodic-damage-v1.md) · [完整验证样例](../../examples/dot-host/catalog.json)

## 执行证据

| 门禁 | 结果 |
|---|---|
| 原生/配置 | 默认和全特性相同767项Rust，保留736、新增31；Clippy -D warnings及22项Node通过。覆盖24种非法字段/计时/绑定组合、128跳上界、旧定义规范化及来源/容量原子性。[默认日志](../../artifacts/sdk-5-5/default.log) · [原生摘要](../../artifacts/sdk-5-5/native-verification.json) |
| 精确节拍/数值 | interval600/duration2200为600/1200/1800三跳；101ms非整步不漂移，末点首次执行步已到期则取消。ATK100/倍率5000冻结raw50，经DEF100/C200每跳33；盾60依次吸收33/27，HP0/6/33。覆盖零量/minimum_damage、实时DEF、来源ATK后续变化、刷新/忽略、超杀和随机数不变。[PD02～PD05映射](../checklists/SDK-5-5-acceptance.json) |
| 生命周期/故障 | 跨来源隔离/最新所有权、重复输入/重连、保护/归途跳过、死亡/换代/整组复位、100次关闭、周期输出峰值QueueFull后World回收与原冻结结算通过。同上原生集合 |
| 普通Node/H5 | 两套包外配置改变ID、倍率、间隔、时长及重复/来源策略，同一构建4条链。先真实周期扣盾/血，停手等待DOT独立击杀，再拾取及撤离/死亡结算；同World刷新重连保持原计划、旧连接拒绝。页面异常0，一次入场/结算，World/未决=0。[报告cases](../../artifacts/sdk-5-5/verification.json) |
| 纯触屏 | 390×844只用触屏控件施加/重施、等待周期击杀、拾取、撤离/放弃，HTTP仅只读观察。双终局、页面异常0、World/未决=0。同上报告touchCases；[手机截图](../../artifacts/sdk-5-5/outside-a-dot-1-dot.png) |
| 强杀/责任恢复 | 活跃已跳且有后续计划、冻结终局意图后、资产提交后三处强杀，各连续两次恢复；原Create/hash/金额/结果/operation ID保持，一次补偿或结算。外部周期参数变化不重算旧责任，不恢复临时状态或World。同上报告crashes |
| 旧版普通链 | 机制19、20、21、22各撤离/死亡，共8条普通H5链；原DEF/叠层/属性/护盾与旧JSON形状保持。同上报告legacy |
| 独立交付 | Cargo源包、21项锁定第三方源及普通release二进制；Rust源与候选一致，包内无工作区绝对路径。移除SDK回归目录与samples后，复制在包外的23/22/21/20/19配置完成普通Node十条链。空缓存离线Rust协议1六模式、协议2/3十四场景、各自重启及两次核对通过。[报告](../../artifacts/sdk-5-5/delivery/independent-verification.json) |
| 解压源码/保护 | 解压源包在空Cargo缓存、离线模式通过相同767项默认/全特性及Clippy，不把重复执行计作新增。原20场景、旧样例、v1.1来源包和历史0.3.1～0.7.0交付的65项原文/hash保持。[源包报告](../../artifacts/sdk-5-5/delivery/source-verification.json) · [保护清单](../../artifacts/sdk-5-5/baseline-verification.json) |

首轮浏览器验收已到旧机制22检查，验证脚本在护盾到期后误将可省略的statuses字段按必填读取，发生KeyError。[首轮日志](../../artifacts/sdk-5-5/first-browser-attempt.log)保留；仅修正验证脚本为检查真实施加时的视图，Rust/配置/Node/Web核心及原生输入未变。完整浏览器链重新执行并通过，原生检查按日志及输入摘要核对后复用，不重复计数。

旧协议2样例在包外副本把撤离解锁/读条调为3000/500ms，适配参考玩家25秒墙钟期限；原数据保持。本批不重跑无关具名地图的全部浏览器专项；全原生集合保留。来源50向量/48场景的完整精确映射继续在I05跟踪。

## 交付和范围

[0.8.0本地交付包](../../artifacts/sdk-5-5/delivery/match-kernel-0.8.0-aarch64-apple-darwin.tar.gz)，4,503,524字节；SHA256为`96cccc9f84d6146baf216cbbaccb65712936a140a007d0f1eda97fc608644c3a`。Apple Silicon macOS普通二进制，replay=false；其它平台和生产发布分别验收。[交付清单](../../artifacts/sdk-5-5/delivery/package.json)

试玩：[127.0.0.1:4178](http://127.0.0.1:4178/)，独立账本`.local/sdk-5-5-demo.sqlite`。命中一次后停手观察DOT，重复命中会刷新节拍。启动检查未创建对局、页面异常0，旧4173～4177预览/账本保持。[预览检查](../../artifacts/sdk-5-5/local-preview.json)

只关闭敌对命中、来源有效ATK比例、refresh/ignore的物理DOT切片；HOT、其它DOT公式/叠层/死亡来源保留、控制/驱散、地形DOT、完整状态目录及生产接入继续按[剩余矩阵](../checklists/sdk-requirements-gap.md)推进。D-03/D-05未变，第二款游戏不提前排入本批。
