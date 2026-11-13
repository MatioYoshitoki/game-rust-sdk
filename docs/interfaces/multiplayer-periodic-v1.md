# 多成员周期效果（机制38）

0.23.0，schema7/协议6/CreateV6；[范围与MP01～MP12](../batches/SDK-5-21-multiplayer-periodic.md)。复用[DOT](periodic-damage-v1.md)、[HOT](periodic-healing-v1.md)与[full/delta](publications-v6.md)；旧机制1～37不扩大。

所有定义、绑定、比例、时长、节拍、来源分组与重复策略由配置初始化。普通玩家和NPC方向弹道通过on_hit_target_statuses施加敌对DOT；玩家技能和NPC弹道通过on_commit_self_statuses给自己HOT。NPC本批只开放这两类周期绑定。HOT不支持友军目标；成员间不互伤。金融DTO和六类Client方法保持。

实体key是kind/id/generation，连接换代保持同一实体；真实NPC归位复位提高generation并清旧代状态/弹道。DOT施加时冻结来源有效ATK，HOT提交时冻结自身基础maxHp。来源与目标的保护/归途只消耗本实例计划并跳过效果；到期、死亡、退出或失效清理。来源退出从settling开始生效，无需等待结算；refresh转移来源后按新来源清理，ignore保留原来源。不存在按成员复制World、在Node计算跳点或修改HP的接口。

MonsterViewV4新增可省略statuses（存在时为数组，null拒绝）：从机制38起存在，内容为本人来源在该NPC上的状态及NPC自身状态。其他成员来源的NPC状态不出现；members不增加私有状态；本人受击/HOT仍使用snapshot.self.statuses。NPC状态字段及private事件在Rust投影后进入full/delta，JS和Rust纯解码器重建同一视图。公开Rust结构体字面量需补新Option字段，严格JS客户端使用0.23.0解码器；旧机制省略字段。

周期和状态事件继续使用event_v4/private，按实际来源/目标路由，移除ID按原实例权限拆分。SourceEntity不会使用participant常量，也不会把connection.generation当entity.generation。精确调度/金额/清理与预算见批次规格。重连保留World内周期，重启只核对原冻结金融责任，不恢复World、周期或发布基线。
