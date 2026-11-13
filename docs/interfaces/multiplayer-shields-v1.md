# 多人吸收盾（机制39）

0.24.0/schema7/协议6；[MS01～MS12](../batches/SDK-5-22-multiplayer-shields.md)。沿[多人周期](multiplayer-periodic-v1.md)和[原盾公式](absorption-shields-v1.md)，六类Client及金融/恢复DTO保持。

玩家/NPC可通过提交自身或普通方向弹道敌对命中目标授予absorption_shield；不含友军/PvP/其它动作。shield_bps、目标合计shield_cap_bps、时长、来源分组、refresh/ignore及绑定均由配置初始化。所有直接伤害和DOT先DEF与原取整，再按期限/施加时刻/实例ID扣盾，最后裁剪HP。DOT真实吸收只记一次，HOT不耗盾或转盾。

机制39的snapshot.self和monsters新增可省略shield_total整数，含0、拒绝null；旧机制省略。本人合计等于全部本人目标实例之和，NPC合计包含隐藏的其他来源，其可见statuses之和可以更小。members不增加私有盾量。NPC实例及private吸收consumptions只给本人来源和NPC自身可见者；私有amount是可见切片，不能把不同接收者相加作为游戏总吸收。Damage.amount仍为实际HP损失；PeriodicTick.absorbed为真实总吸收。ShieldAbsorbed.source沿原来源字符串标签，权威身份仍为完整EntityRef调用链。

来源退出从settling立即清理，refresh转移/ignore保留按实际来源处理；旧来源不误删新来源。保护/归途不耗盾，期限继续。重连不重授/续期，NPC真实复位换代拒绝旧弹道，重启仅Closed金融责任。公开Rust结构体字面量需补Option字段，严格JS客户端使用0.24.0解码器；原1～38输出保持。
