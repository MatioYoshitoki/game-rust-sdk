# 技能生命成本接入（机制45）

0.30.0；协议6/schema7、固定1～8成员PVE与静态1～8槽。沿[多技能槽](member-skill-slots-v1.md)和[多人发布](publications-v6.md)，范围见[HC01～HC10](../batches/SDK-5-28-skill-hp-cost.md)。

每个完整技能对象可增加可省略但非null的hp_cost：

```json
{"mode":"current_hp_ratio","value":0.05,"min_hp_after_cost":1}
```

mode只允许fixed/current_hp_ratio，value为有限非负数（fixed≤1000000、比例≤1），最低保留值为1～1000000整数。固定成本先floor(value)，比例先floor(提交时HP×value)，再夹到[0,HP.saturating_sub(min_hp_after_cost)]。生命已低于保留值时成本为0，不能由成本治疗。所有旧机制拒绝该字段；机制45可省略，继续旧零生命成本。

准备只冻结定义、不收费。Rust提交先校验身份/目标/起点/落点、灵力、状态及计划容量，再原子扣当前HP/灵力、启动CD/后摇并创建效果。生命成本不属于伤害，不消耗护盾、不触发受击或归功，后续弹道/区域/DOT/HOT无二次成本。接受前或提交前拒绝/取消/保护/重连/退出无收费；成功后不退款。最低生命截断不是免伤或致死保护。

本人EventV4.Private中的ActionCommitted增加可选hp_cost:u32，声明生命成本的成功技能包含实际0；其它技能和普攻省略。示例：

```json
{"kind":"private","event":{"kind":"action_committed","action_id":"host-generated-input","skill":true,"skill_id":"configured-id","spirit_cost":2,"hp_cost":4}}
```

保持原Action/Client方法/快照字段/金融与恢复DTO。HP/CD按原有full/delta发布，公共成员HP仍可见；私有费用不向同伴发布。Rust构造Event::ActionCommitted字面量需hp_cost:None，原JSON省略兼容。显示端读取实际事件和权威HP，不按HP差反算费用（同时可能有伤害/HOT），也不能回传成本值。web/hp-costs.mjs只观察私有真实费用，拒绝坏事实且不改原统计。

Node入口skill-hp-cost-host.mjs，H5入口skill-hp-cost-server.mjs；开发数据examples/skill-hp-cost-host，交付samples/skill-hp-cost，四槽/四人与八槽/八人两套。H5按配置选择技能并显示累计实际生命成本，地图继续等距投影。参考端口4219，测试4220，包外只读4221，使用独立账本。

同World重连保当前HP、已提交CD/后摇/计划，未提交动作中断；旧连接拒绝。退出清本人计划/状态；重启只Closed金融收尾，不恢复战斗或费用观察统计。完整倍率/真意/多发公式、动态技能与其它施法阶段另行交付。
