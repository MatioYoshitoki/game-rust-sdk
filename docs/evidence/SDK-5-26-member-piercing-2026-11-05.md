# SDK-5-26 穿透弹道多人适配验收（2026-11-05）

SDK-5-26按PP01～PP12限定签收，0.28.0/协议6/schema7/机制43。多人piercing_projectile沿cast方向输入，复用同一World弹道容器、连续扫圆与敌对目标迭代器、ContactBudget、直接伤害/盾/DOT/死亡归功链。所有速度/半径/射程/TTL、预算/重复间隔、成本/CD/绑定由配置初始化；普通/单位/NPC/穿透共享容量及128份历史工程预算。每代一次或按实际逻辑步含等号重复，一步同身份一次且不补跳，墙同刻优先、TTL截止先结束。重连保在途余程/预算/历史/绝对期限，来源settling立即清自己的穿透，重启仅金融责任。

默认/全特性同1442项Rust（保留1408、新增34）、82项Node（保留74），3496份真实帧精确跨语言重建；64项公共边界、4页/8页H5及Node实际施法/战斗/共享拾取/独立终局、三处强杀各两次同检查点恢复、100次带穿透Runtime释放、48项包外普通消费及空缓存离线源码全部通过。原48场景/50数值向量新增执行数0；3包映射完成、7包部分实现、15包待实现，22包尚未完整关闭。

- [实现前规格PP01～PP12](../batches/SDK-5-26-member-piercing.md)、[接口](../interfaces/member-piercing-v1.md)、[机器签收](../../artifacts/sdk-5-26/acceptance.json)。
- [原生回归](../../artifacts/sdk-5-26/native-verification.json)：保留1408项Rust/74项Node，新增34项Rust/8项Node；主副技能、1/2/4/8席、起点接受/提交原子重验、共享容量、连续扫圆/墙与单位同刻、顺序/足迹/代次、一次/重复预算、盾/附带/死亡与退出/重连。默认/全特性同集，Clippy/fmt通过。
- [真实帧互操作](../../artifacts/sdk-5-26/interop-verification.json)：3496帧（89full/3407delta）、8接收者，默认/全特性向量相同；普通与穿透、一次/重复、预算/历史/创建/更新/删除/清空和重连逐字段精确重建，重复幂等；坏几何/null/代次/历史/时序/互斥目标/预算原子拒绝。
- [公共边界](../../artifacts/sdk-5-26/public/report.json)：13个非法创建无部分I/O、37个配置拒绝、12个实际非法输入无成本/CD/弹道、四人/八人两套普通Node全旅程。未知金融期间同伴继续；公开计划/接触/结束无私有输入ID、冻结攻击/绑定/成本。
- [真实H5页面](../../artifacts/sdk-5-26/browser/report.json)：4页/8页实际选择方向点并cast，显示Rust穿透位置/余程/预算；DOT/HOT/盾、源退出、重连保留或真实合法结束、旧帧拒绝、丢Sent增量及缓存恢复、共享掉落争抢、撤离/离场独立结算。异常0、未决0，宿主释放；[截图检查](../../artifacts/sdk-5-26/visual-verification.json)。
- [强杀/恢复](../../artifacts/sdk-5-26/crashes/report.json)：部分准入在World前，另两边界实际穿透接触/周期/盾后SIGKILL；各从同一检查点两次恢复，原成员/hash/资产保持、金融Closed，无World/弹道/状态/基线热恢复。100次带真实穿透Runtime完成责任并释放。
- 同候选旧机制：[协议4](../../artifacts/sdk-5-26/legacy4/report.json)、[协议5](../../artifacts/sdk-5-26/legacy5/report.json)、[37](../../artifacts/sdk-5-26/legacy6/report.json)、[38](../../artifacts/sdk-5-26/legacy38/report.json)、[39](../../artifacts/sdk-5-26/legacy39/report.json)、[40](../../artifacts/sdk-5-26/legacy40/report.json)、[41](../../artifacts/sdk-5-26/legacy41/report.json)、[42](../../artifacts/sdk-5-26/legacy42/report.json)通过，包外另保留协议1～3。
- [48项包外消费](../../artifacts/sdk-5-26/delivery/independent-verification.json)与[空缓存离线源码](../../artifacts/sdk-5-26/delivery/source-verification.json)：旧46项保留、新增一次/重复两套穿透配置；同一源crate/vendor和普通release，默认/全特性同集及Clippy、最小Rust宿主。隐藏SDK回归配置/包内samples后使用外部数据消费，交付H5只读启动异常0，无replay。

[交付包](../../artifacts/sdk-5-26/delivery/match-kernel-0.28.0-aarch64-apple-darwin.tar.gz) SHA-256：`51cd483231549bc3468a6979f6984786091289113aa2daa5eb150116722ccd08`；源crate：`2b9d87b2624ca3a7e1f2cdc678aee5bfe3db90609c89d5cb14ecb925fb90a0f4`；普通二进制：`d4c0e04a9a06b70f1ef13f80f72566e1122434a0d612802359fce5eafd52c07d`。

首次并发回归发现100次释放测试误将终局冻结快照中的普通NPC弹道视为活跃穿透；修正测试为所有穿透为空，并保留既有普通NPC冻结事实，重新制作候选并跑同源全套验收。初始候选/日志保存在本批candidate-attempt-1与native-attempt-1中。15485份起点旧JSON/原资料/SDK-5-17～25证据按哈希保护，历史用户清理例外保留。新[4213预览](http://127.0.0.1:4213/)独立.local/sdk-5-26-demo.sqlite，[只读检查](../../artifacts/sdk-5-26/local-preview.json)未建局；既有预览/账本保持。

当前仍限固定1～8必需成员PVE。友军/PvP、追踪/反弹、移动区域、更多槽位、完整模式/观战和生产接入仍待补齐，本批不宣称完整原需求完成。
