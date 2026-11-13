# SDK-5-6 通用周期调度与HOT最小闭环

2026-11-04；**先冻结规格后实现，HT01～HT10受控验收全部通过**。crate **0.9.0**、机制 **24**；schema4/协议3、单人PVE。复用SDK-5-5周期游标与状态生命周期，新增有限自身HOT，不签收全部治疗公式/来源状态或新目标系统。

## 配置与施加

1. 新定义`mechanism:"periodic_heal"`，必填`tick_interval_ms`与`heal_bps`（整数0～10000），基础量`floor(自身基础maxHp×heal_bps/10000)`在施加组开始时冻结。不依赖ATK/DEF/minimum_damage，不转换灵力/护盾，不自动增加其它治疗乘区。
2. 沿用id/duration_ms/source_scoped/duplicate_policy，只接受refresh/ignore；禁止damage_bps、modifier_bps、attribute、shield_bps、max_stacks（含null）和stack/未知字段。旧机制/其它定义禁止heal_bps（含null）。DOT的damage_bps及旧JSON保持原形。
3. 仅允许玩家主技能与怪物弹道的on_commit_self_statuses；禁止on_hit_target_statuses及任何非自身目标，不新增单位/友军选择入口。提交前取消不施加，重复输入/回执不重复施加；沿原成本/CD和整组容量原子预检。怪物与玩家共用一种机制。
4. interval≥step，duration严格大于interval，计划数`floor((duration-1)/interval)`为1～128；原16定义、每绑定2引用、每实体32槽、全局128实例和发布预算保持。

## 计时、排序与回复

- 首跳start+interval，之后按原计划间隔累计，[start,end)；duration2200/interval600只有600/1200/1800三次。非整步按首次达到的固定步执行，下一计划不漂移；若该步已到期则取消末点，total_ticks为计划上限。仍有效的逾期点有限补齐，到期/关闭不补发历史跳。
- refresh保留ID、重取基础maxHp回复量、重置开始/截止与次数/节拍；ignore保持原全部参数。读取视图或重复命令不推进游标。每跳先推进游标，再执行效果。
- 统一队列按`计划时刻→效果优先级(DOT先HOT)→实例ID字典序`执行。不同计划时刻按时间先后；相同计划时刻的DOT致死先清理状态，HOT不能挽救已经产生的死亡。旧23只有DOT，排序不变。绝对终局/主动放弃优先于周期；周期先于原AI/动作/弹道与本步撤离完成。
- 存活目标实际回复=min(冻结量,max(0,当前maxHp−当前HP))。maxHp1000、heal_bps1000→每跳请求100，HP850依次回复100/50/0，最多到1000；满血和零量照常消耗节拍并记录0。死亡者不回复，当前HP已高于上限时也不倒扣HP。不产生过量盾/MP、伤害/命中状态、回灵/剑印/吸血/反击，不消费随机数；不打断交互/撤离/支援，不回退Boss已完成HP阶段，不改基础事实。
- 保护/后台/断线未就绪、怪物归途/复位倒计时按DOT策略消耗计划但跳过回复，恢复不补跳/延寿。来源/目标死亡、移除、换代/归位/整组复位、终局/关闭共用状态清理；无死亡来源保留或复活。

## 输出与宿主

HOT使用同一个periodic视图：`{kind:"max_hp_heal",interval_ms,next_tick_at_ms,ticks_done,total_ticks,raw_heal}`；不输出无意义的raw_damage。旧DOT保留kind/字段，包括raw_damage为0的情况；旧19～22实例继续省略periodic。HOT modifier_bps为0，不贡献派生属性。

新增periodic_heal_tick，带实例/定义/source/target、tick_index、scheduled_at_ms、executed_at_ms、raw_heal、hp_restored、outcome(applied/protected/returning)。满血applied/0，跳过明确原因/0，不冒充受伤或主动命中事件。

Node/H5仅转交意图/处理原存储证明/展示进度和实际回复，不推进周期或补算HP。重连保持原实例/期限/节拍，旧连接拒绝；重启只恢复冻结资产责任，清除HOT/DOT，不恢复World或按新配置/墙钟补跳。0.9.0公开Rust DTO/枚举有源码变更，六类Client/协议1～3/资产及恢复schema保持。

来源behavior §4.3/4.4/5.3支持最大HP比例与不足量裁剪等规则；这里只注册基础maxHp比例的有限自身HOT。治疗强化/衰减、其它标尺/固定量/复杂叠层、友军目标、过量转盾、驱散/控制、地形DOT、多人/新地图/第二款游戏及D-03/D-05继续保留。

## 验收标准

| ID | 通过条件 |
|---|---|
| HT01 | 机制24/schema4/PVE、字段presence/null、比例/时间/预算、self绑定及禁止hit严格校验；旧1～23配置及规范化JSON保持。 |
| HT02 | maxHp1000/bps1000冻结100；后续ATK/DEF变化不影响，refresh重取/重置、ignore原样，零量/取整/宽整数/容量整组失败无部分刷新或扣费；基础事实不永久改写。 |
| HT03 | 首末跳/半开区间、非整步/逾期/到期、相同计划DOT先HOT（颠倒ID也成立）、同族稳定排序、重复读取/命令/回执不重复回复。 |
| HT04 | HP850→950→1000→1000，实回100/50/0；零量不套minimum_damage，不超量/转盾/回灵、不消费随机数/伤害触发；死亡/终局不能复活。 |
| HT05 | 玩家与怪物self提交真实施加，取消/重复/满槽沿原语义；HOT不打断交互/撤离/支援或回退Boss阶段；受伤→HOT真实回HP→战斗/拾取→撤离/死亡结算闭环。 |
| HT06 | 后台/保护/未就绪、归途/复位跳过不延寿或补跳；重连保留节拍、旧连接拒绝；死亡/换代/归位/复位/关闭清理，100次关闭和合法配置周期输出峰值受控回收。 |
| HT07 | 两套包外配置改ID/比例/间隔/时长/重复策略，同一普通Node/H5构建各双终局；纯触屏双终局，权威进度/实际回复可见，页面异常0、一次入场/结算、World/未决=0。 |
| HT08 | 活跃已回复且有下一跳、冻结终局意图后、资产提交后三处强杀，各连续两次恢复；外部HOT参数变化不重算原Create/hash/金额/结果/operation ID，不恢复临时状态/World、不重复扣费发奖。 |
| HT09 | 默认/全特性保留原767项相同集合并通过新增检查、Clippy、Node；旧19～23普通双终局，协议1～3正常/未知/重试/拒绝/补偿/恢复边界通过。 |
| HT10 | 独立源包/vendor/普通二进制支持信息正确；解压源包空缓存离线完整测试/Clippy，移除回归数据后的包外消费通过；原20场景、旧样例、来源包和历史0.3.1～0.8.0包保持，逐项证据绑定同一候选。 |

HT01～HT10全部通过；见[逐项映射](../checklists/SDK-5-6-acceptance.json)、[验收记录](../evidence/SDK-5-6-periodic-healing-2026-11-04.md)及[机器汇总](../../artifacts/sdk-5-6/acceptance.json)。
