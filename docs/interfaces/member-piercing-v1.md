# 多人穿透弹道 v1

0.28.0 / schema7 / 协议6 / 机制43，固定1～8必需成员PVE。沿既有piercing_projectile和cast方向输入，将穿透接入共享World、敌对接触、ContactBudget与[统一伤害](unified-member-combat-v1.md)；[PP01～PP12](../batches/SDK-5-26-member-piercing.md)冻结本批范围。

配置沿range、speed、radius、ttl_ms、max_hits、repeat_interval_ms及原成本/准备/后摇/CD、比例和状态绑定。repeat省略或0每代一次，正数在实际逻辑步按上次成功+间隔含等号，每步同身份一次、不补跳且必须再次实际扫过。主副技能均可，旧多人版本拒绝穿透。所有数值/绑定由配置初始化；members.projectile_capacity与普通、单位及NPC弹道共享，容量×最大max_hits≤128并校验输出工程预算，在I/O前拒绝超预算。

接受/提交重验来源起点及半径足迹。提交self绑定后冻结ATK/起点/方向，一次成本/CD；提交步不移动/接触，之后连续扫圆按speed×dt和余程推进，不追踪/反弹；目标按碰撞进度、ID、代次排序，墙同刻优先。DEF/盾与位置实时读取，闪避不耗、全盾耗且有原附带；统一DOT/死亡/归功/掉落。TTL截止点先结束，预算/墙/余程/TTL/来源失效各一次。后台/移动/同World重连保在途几何/历史/预算/期限，settling清自己的弹道且不等金融；重启只Closed金融责任。

ProjectileViewV4新增可省略非null的piercing:{skill_id,direction,speed,remaining_range,created_at_ms,expires_at_ms,max_hits,used_hits,remaining_hits,repeat_interval_ms,contacts}。只有穿透输出且不能同时unit_target；旧弹道省略。contacts每项{target:{id,generation},hit_at_ms,next_eligible_at_ms}，next字段必须存在：repeat=0时null，否则hit_at+interval；目标历史可包含NPC已复位的旧代次，未来代次、重复身份拒绝。活跃计划used<max，remaining=max-used，once时历史数=used，重复时历史数≤used；全World活跃计划max_hits之和≤128。

新增公开projectile_contact事件{projectile_id,source,target,position,hit_at_ms,remaining_hits}，沿projectile_started、projectile_ended。结束事件仅穿透增加reason=wall/range_exhausted/expired/hit_budget_exhausted/source_invalid，旧事件省略。World全局弹道ID与实际EntityRef不含私有输入ID、冻结攻击/绑定/成本。Rust字面量补piercing:None与结束reason:None；严格客户端使用新版随包解码器。projectiles full/delta仍按ID列表upsert/removed/order，移除与空清准确；坏几何/身份/时序/预算/历史/基线原子拒绝。[Schema](member-piercing-v1.schema.json)定义结构，跨字段等式和基线由Rust/JS解码器检查。六类Client、协议、金融/恢复DTO不变。

Node入口member-piercing-host.mjs，H5入口member-piercing-server.mjs；两套配置examples/member-piercing-host，交付对应hosts/node、hosts/h5、samples/member-piercing。前端只提交方向、显示Rust弹道位置/余程/预算和实际接触，不做战斗模拟。PvP/友军、反弹/追踪、更多槽位与完整模式仍开放。

SDK 0.105.0 允许在机制 105 的 `basic` 中声明 `piercing: {plan_id, max_hits}`。普攻按已有距离、速度、半径和寿命发射，每个目标代次最多成功接触一次。`plan_id` 应与技能 ID 区分，在现有弹道视图中通过 `piercing.skill_id` 传出。客户端沿用 `attack` 或 `attack_unit`。`basic.settle_on_end` 可单独启用结束回灵和冷却，具体时序见普攻跟进接口。[普攻跟进](basic-followup-v1.md)
