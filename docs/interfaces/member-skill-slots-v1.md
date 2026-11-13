# 配置化多技能槽 v1

0.29.0 / schema7 / 协议6 / 机制44，固定1～8必需成员PVE。沿skill＋additional_skills数组初始化1～8个技能，顺序和全部数值由配置决定。对应[MS01～MS12](../batches/SDK-5-27-member-skill-slots.md)，只扩展静态技能表，复用原[统一施法与战斗](unified-member-combat-v1.md)、[区域](member-line-plans-v1.md)和[穿透](member-piercing-v1.md)。

主技能必配，附加数组可省略/空或0～7个完整有类型项。所有ID唯一，任何末项非法机制/数值/绑定或最坏计划预算都在I/O前拒绝，不裁掉末项。8为SDK工程上限，实际数量由数组长度初始化，不重复维护容量字段。旧1～43保持最多两槽及原字段存在规则。公开sdk_info.playerSkills.maxSkills仍为旧两槽声明，memberSkills提供机制44的1～8槽、配置顺序、共享准备/后摇与独立CD能力。

每成员按技能ID独立成本/CD，共用灵力、一个准备动作和共享后摇；切槽不替换准备、不绕过后摇或重置已提交CD。取消/后台/保护/重连中断未提交动作无扣费/CD/自授状态；移动沿原规则保留准备，提交按实际位置/目标重验。费用、整组状态、区域/弹道容量在提交前原子检查，增加技能数不增加并发容量；主副或任意槽都走同一机制实现。

沿PrivateMemberView.player_skills:[{skill_id,ready_at_ms}]按配置顺序输出，未提交为0、已过期CD仍可保留原绝对时刻。pending_action.skill_id仅指配置技能或null（普攻）；recovery_until_ms保持原共享后摇。公共成员不带这些私有字段。full/delta仍完整替换own私有视图，无新协议字段/Action/Client方法；Rust和JS随包解码器检查≤8、ID唯一/合法、时间范围及准备关联，坏帧原子拒绝。旧空列表形状保留，带技能准备必须能在实际私有列表找到ID。旧合法输出保持。

同World重连保全部已提交CD/后摇与计划预算/期限、旧连接拒绝。来源settling即时清本人动作/计划/状态，不等金融、不删同伴；退出冻结视图与Close契约保持。重启只金融Closed责任，不热恢复World、CD、计划、临时状态或基线。

Node入口member-skills-host.mjs，参考skill-slot-probe.mjs实际遍历配置槽；H5入口member-skills-server.mjs由配置生成选择器，web/skill-slots.mjs只生成现有有类型意图并观察本人真实提交。四槽/四人、八槽/八人的参考配置位于examples/member-skills-host，交付对应hosts与samples/member-skills。[Schema](member-skill-slots-v1.schema.json)约束列表结构，唯一ID/准备关联和时序由解码器追加验证。接入方使用自己的业务/数值配置；页面捕获缺少有效目标等意图错误并显示INVALID_TARGET，不产生未捕获异常。动态换装/重排、成员不同技能表、友军/PvP及完整施法阶段不在本批。
