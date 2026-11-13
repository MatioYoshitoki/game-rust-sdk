# 规则门槛、差异和准入条件

[机器台账](rule-gates.json)保留每条影响ID和工作包。D-03/D-05待产品裁定；草案不冒充已确认。OBS表示本轮观察，不是新增产品决定或新的M大类。

|ID / 状态|问题 / 进入实现前条件|影响条目数 / 工作包|
|---|---|---|
|D-01 / source_baseline|生命周期优先旧castTime投影：结构化castLifecycle/targeting优先；客户端不能按旧投影决定提交。|86 / R3.1|
|D-02 / draft_requires_confirmation|防御基准/帽/取整：来源草案采用守方境界ATK×2、70%帽；执行原向量前评审签收，不默改expected。|177 / R3.3,R6.2|
|D-03 / open_product_decision|属性魔数冲突：四个原状态的attrType=3是maxHP，文字是移速。产品明确采用行为/原生属性和边界后才实现受影响效果。|9 / R3.2|
|D-04 / source_baseline|恢复以光球为准：不叠加旧击杀立即恢复；原向量及拾取链绑定。|45 / R3.3,R6.2|
|D-05 / open_product_decision|itemsGained经手或净额：产品明确统计口径；保留carried/looted/consumed/kept/dropped事实，不让不明字段影响金额。|52 / R5.1|
|D-06 / draft_requires_confirmation|核心与附属提交：核心失败不得成功；未知先查原操作，附属不得反转核心；完整服务契约冻结后签收。|52 / R5.1,R6.1|
|D-07 / source_baseline|竞技断线30s/连接60s：二者并存；竞技判负后重连仅观礼。|3 / R5.3|
|D-08 / source_baseline|模式席位与地图playerLimit：采用resolved-matchmaking/各模式容量；下一批两席是显式受控子集，不能闭合标准8席和模式容量。|5 / R2.2,R5.3|
|D-09 / draft_requires_confirmation|概率0/1：数学0必不发、1必发；不继承旧RNG消费轨迹，奖池验收前签收。|45 / R4.3|
|D-10 / source_baseline|elemRes缺省转化1：完整属性派生明确缺省及版本；不是丢弃抗性。|52 / R5.1|
|D-11 / source_baseline|重复策略枚举：仅refresh/stack/ignore；sourceScoped分组，不造independent。|86 / R3.2|
|G-01 / open_spec|special/器型/状态语义：每个未知有效绑定给输入、状态、时机、取值对象、排序、上限、正常/拒绝/边界精确预期；不能只补函数名。|396 / R3.1,R3.2,R3.3,R3.4,R3.5,R5.1,R5.2,R5.3|
|G-02 / open_spec|生产碰撞场：冻结装饰类型/程序化/动态阵法/高度/地形DOT及坐标规则，与正式地图代表可达性对照。|21 / R4.1|
|G-03 / open_spec|完整成长公式/事务：补正常撤离、死亡、寿尽转世、满仓/冲突的金额/前后快照；先冻结再由Rust实现。|52 / R5.1|
|G-04 / open_spec|ModeAction与客户端映射：有类型schema、逐字段表和真实解码；SDK-5-18只冻结新的双人有限契约，不宣称旧客户端全部适配。|89 / R2.3,R3.1,R5.3,R6.1|
|G-05 / open_evidence|正式验收环境：冻结真实客户端/DB/Socket、负载机器/队列/阈值/平台并运行；本批映射不是行为证据。|0 / R6.1,R6.2,R6.3,R6.4,R6.5,R6.6|
|OBS-01 / observed_implementation_gap|移动/显式动作取消后摇：原技能recovery.cancelByMove/ExplicitAction=true；现有SDK后摇不可缩短。R3.1补可取消阶段及成本/CD/共享动作边界。|176 / R3.1|
|OBS-02 / observed_implementation_gap|来源死亡后世界效果继续：原runtime.endOnSourceInvalid=false；现有持续场/穿透计划随来源退场。R3.4补独立存续与冻结归属，不得复活来源。|80 / R3.4|
|OBS-03 / open_capacity_contract|原数值与现有限额不兼容：原spiritCost最高8000000、maxTargets64/total1024超现1m/32/128；S44容量1低于当前输入队列16。先冻结合法预算/profile、拒绝和资源界限，不直接放大常量。|93 / R3.1,R3.4,R6.2,R6.3|
|OBS-04 / observed_implementation_gap|一个原状态的复合效果身份：effect_berserk同时ATK+30%/DEF-20%，要保持一个状态槽位/期限/来源；拆成两条不能当作等价。effect_shield则明确为5层DEF增益，不是吸收盾。|36 / R3.2|
|OBS-05 / open_spec|说明与效果结构不完全一致：effect_zhenlong_suppress说明攻速而fix_attribute[1,-30,1]指ATK；effect_leidi_field/execute说明需追踪父技能后再归属，不能凭说明新增状态效果。|5 / R3.2,R3.3|
|OBS-06 / source_precedence|VFX不决定运动：liuyun-huifeng VFX orbit_then_pierce但实际runtime linear+first；采用实际计划，不据美术补回旋穿透。|2 / R3.4|
