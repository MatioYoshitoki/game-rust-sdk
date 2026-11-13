# SDK-5-23 统一多人战斗与单位弹道执行记录

2026-11-05，0.25.0/协议6/schema7/机制40。先冻结[UC01～UC12](../batches/SDK-5-23-unified-member-combat.md)，[逐项测试映射](../checklists/SDK-5-23-acceptance.json)与[最终机器签收](../../artifacts/sdk-5-23/acceptance.json)绑定同一交付候选。

统一入口见[combat.rs](../../src/world/two_members/combat.rs)：实际EntityRef解析、PVE关系策略、敌对接触筛选、直接/DOT伤害、HOT回复及状态基础值。伤害只在一个入口依DEF/取整/minimum_damage→盾→HP执行，再处理目标撤离/死亡/归功/掉落；随机、附带、基本回灵留在直接接触层。当前PVE的成员与NPC各自互为友方，跨类敌对，不声明任意阵营/PvP。旧单成员协议适配保持，未将全部历史运行时重写成一个类型。

现有unit_projectile/cast_unit多人接入：接受/提交校验已发布NPC身份、生命代次/资格、射程/视线；准备期失败无成本/CD/自身状态/弹道。提交按目标当前合法位置冻结方向/攻击，不跟踪或转绑新代。其它合格敌对单位先挡住无伤害/附带/RNG/回灵，墙同时接触优先。真实命中共用盾/DOT/伤害链；死亡/退出/复位/TTL/Close清理，同World重连保已提交计划，旧连接拒绝。私有准备/提交只给本人，公共弹道只新增可选unit_target，普通/NPC/旧机制省略，null拒绝。数据与绑定均由配置初始化。

- [原生/Node](../../artifacts/sdk-5-23/native-verification.json)：默认/全特性同1344项Rust，保留1311、新增33；60项Node保留54，Clippy/fmt通过。包括1/2/4/8成员关系、准备/提交/飞行身份、目标死亡/归途/移动/复位、阻挡/墙、全吸收附带、一次成本/CD及技能槽、重复/取消/保护/重连、私有视图与清理；100次真实带单位计划公共Runtime终局送达、责任归零及线程退出。
- [跨语言重建](../../artifacts/sdk-5-23/interop-verification.json)：2008真实Rust帧（65 full/1943 delta），8接收者逐字段一致/重复幂等，默认/全特性向量摘要相同。覆盖单位准备/计划身份及结束，既有周期/盾与过期/清空，坏身份/null/基线原子拒绝。
- [43项公开边界](../../artifacts/sdk-5-23/public/report.json)：13项非法Create不部分I/O、18项配置/绑定/预算拒绝、10项实际JSONL单位身份/输入拒绝无成本/CD/提交、两套4/8人普通Node闭环。真实单位提交/弹道/目标身份、DOT/HOT/盾、来源退出、重连/缓存、共享拾取、独立结算；金融未知不影响同伴，未决0。原始Rust输出重建核验计划/准备/事件及私有状态/吸收切片。
- [四人/八人真实页面](../../artifacts/sdk-5-23/browser-final/report.json)：页面实际单位技能按钮与API输入，目标来自发布身份；观察单位计划/周期/盾，P00退出、P01重连、旧帧拒绝、丢Sent delta及清缓存后request_full恢复，再完成实际战斗/共享拾取/独立终局。异常0、宿主释放。联调发现并修复H5白名单未转发cast_unit；最终交付候选重新构建并完整复验。护盾在重连观察窗口消失时核验原期限或真实depleted/source_invalid等移除证明，不能把合法耗尽当重连丢状态。
- [三个SIGKILL/六次恢复](../../artifacts/sdk-5-23/crashes/report.json)：部分准入（World尚未激活）、最后一席终局未决、核心提交回执丢失；后两处此前实际发生单位计划、DOT/HOT/盾。各同一检查点恢复两次，修改外部配置仍保原成员/hash/资产；Closed未决0，单位/周期/盾/发布计数全0，不热恢复战斗。
- 旧关键链：[协议4的16项](../../artifacts/sdk-5-23/legacy4/report.json)、[协议5的20项](../../artifacts/sdk-5-23/legacy5/report.json)、[机制37的31项](../../artifacts/sdk-5-23/legacy6/report.json)、[机制38的29项](../../artifacts/sdk-5-23/legacy38/report.json)、[机制39的33项](../../artifacts/sdk-5-23/legacy39/report.json)，包外另含原协议1～3。
- [空缓存离线源码](../../artifacts/sdk-5-23/delivery/source-verification.json)、[42项包外普通消费](../../artifacts/sdk-5-23/delivery/independent-verification.json)：同一真实源crate/vendor、普通release及包外显式数据，默认/全特性同集、Clippy；保留原40消费并增加两套单位动作，消费时隐藏SDK回归/包内samples，交付H5只读启动异常0，未启用replay。

[交付包](../../artifacts/sdk-5-23/delivery/match-kernel-0.25.0-aarch64-apple-darwin.tar.gz)SHA-256：`5558d98b7fca8c0c109097ebf934c22b141f9dab1fa2b70da7ec44d7d26f7975`；源crate：`27f1197237aa8383a4ed02669431f3f14b281f42a74ac672301621f9465e899b`；普通二进制：`9a0544a23f6e7f57ae831ec29eeb46d72a099efe1d206f1149deffef2b033ff3`。新[4204预览](http://127.0.0.1:4204/)独立.local/sdk-5-23-demo.sqlite，[只读检查](../../artifacts/sdk-5-23/local-preview.json)未建局；旧4173/4190/4193/4195/4198/4201页面与账本保持。8993份起点现存旧JSON/来源与SDK-5-17～22证据/归档逐字节保护，历史用户清理例外保留。

R3.1增加有限单位动作实现，当前3包完成映射、6包部分实现、16包待实现，22包未完整关闭；更多动作、友军/PvP、完整状态/公式、模式/观战/正式接入继续开放，14M/9I/9P未自动关闭。原48场景/50数值向量新增执行数为0；不寻找第二款游戏。
