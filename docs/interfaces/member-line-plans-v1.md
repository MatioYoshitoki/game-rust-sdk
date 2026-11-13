# 多人线形区域计划 v1

0.27.0，schema7/协议6/机制42，固定1～8必需成员PVE。在[圆形计划](member-circle-plans-v1.md)和[统一战斗入口](unified-member-combat-v1.md)上增加固定胶囊几何，不增加Client方法、金融字段或地图分支。完整约束见[LA01～LA12](../batches/SDK-5-25-member-line-plans.md)。

技能沿原line_field配置：id、cost、windup_ms、recovery_ms、cooldown_ms、damage_bps、length、width、warning_ms、duration_ms、tick_interval_ms、contact_policy及原self/hit绑定。length为0.1～10000，width为0.2～256，首跳/截止/128跳、每跳/总成功预算、重复间隔沿圆形持续计划。members.area_capacity共用，容量×单区最大总命中≤128，输出工程预算在任何准入I/O前检查；旧多人机制拒绝line_field。

输入是cast{skill_id,direction:[dx,dy]}；只提交方向意图，Rust检查有限非零并归一化。接受/提交检查来源起点及半宽足迹的弹道阻挡和边界。提交时按当时位置、已接受方向、配置长度生成胶囊，复用连续扫圆墙裁剪，允许零轴长；不使用地面落点/视线门槛。提交成功付费/CD一次、self绑定后冻结ATK。移动/后台/重连保留几何和绝对期限，来源settling清理，不等待金融回执。重启只恢复金融Closed责任。

公开areas中的line_field计划保留原字段，额外必须带line:{start,end,width}。start严格等于center，width严格等于2×radius，轴长≤10000。circle/field省略line；present null、错误形状、未知字段均拒绝。Rust AreaPlanViewV4字面量补line:None；严格接入方使用新随包解码器。[JSON Schema](member-line-plans-v1.schema.json)定义结构，Rust/JS解码器还检查关联身份、几何等式、时序/预算/历史与原子基线。

areas在full中为当前全列表，delta存在时完整替换，[]清空、省略不变。area_started/tick/ended复用原事件。公开计划仅含World计划ID、实际来源、技能、几何、时序、进度和预算/历史，不含输入ID、冻结攻击/绑定、灵力/CD。准备/ActionCommitted仍仅本人，线形提交的ground_position/unit_target省略。接触按轴进度、到轴距离平方、ID、代次排序，当前足迹/遮挡/DEF/盾实时读取，共用直接接触/状态/死亡归功/掉落链。

Node入口member-line-host.mjs、H5入口member-line-server.mjs；包内对应hosts/node、hosts/h5，配置samples/member-line，两套示例只初始化数值。H5选择方向点后施法，胶囊预警/活动及预算来自Rust发布；前端不判定命中。PvP/友军、穿透、移动区域及完整模式仍开放。
