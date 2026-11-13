# SDK-5-25 多人线形区域验收（2026-11-05）

SDK-5-25按LA01～LA12限定签收，0.27.0/协议6/schema7/机制42。在同一World区域计划上增加line_field固定胶囊，复用连续扫圆墙裁剪、统一调度/ContactBudget及直接接触/伤害/状态链。长度/宽度、预警/周期、重复间隔/预算、绑定和容量均从配置初始化。公开line几何随areas完整替换增量发布，旧circle/field省略该字段；重连保持几何/期限/预算，来源settling立即清理，重启仅金融责任。

默认/全特性同1408项Rust（保留1376、新增32）、74项Node（保留68），3416份真实帧精确跨语言重建；64项公共边界、4页/8页H5及Node实际施法/战斗/共享拾取/独立终局、三处强杀各两次同检查点恢复、100次带线形Runtime释放、46项包外普通消费及空缓存离线源码全部通过。原48场景/50数值向量新增执行数0；3包映射完成、7包部分实现、15包待实现，22包尚未完整关闭。

- [规格LA01～LA12](../batches/SDK-5-25-member-line-plans.md)、[严格接口](../interfaces/member-line-plans-v1.md)、[机器签收](../../artifacts/sdk-5-25/acceptance.json)。
- [原生回归](../../artifacts/sdk-5-25/native-verification.json)：主/副技能、1/2/4/8席、接受/提交原子校验、连续裁剪/圆帽/零轴长、稳定目标排序、计时与预算/代次、伤害/盾/附带/归功、保护/重连/退出；全部旧1376项Rust/68项Node保持。Clippy/fmt通过。
- [真实帧互操作](../../artifacts/sdk-5-25/interop-verification.json)：3416帧（89full/3327delta），8接收者，默认/全特性向量相同，逐字段精确重建和重复幂等；线形及圆形同World、创建/历史/进度/结束/空清与重连，坏几何/null/时序/身份/预算原子拒绝。
- [公共边界](../../artifacts/sdk-5-25/public/report.json)：13个无部分I/O的非法创建、37个配置拒绝、12个实际非法输入免成本/CD/计划、2套四人/八人普通Node全旅程。未知金融期间其他成员继续，公开几何无私有输入ID/攻击/绑定。
- [真实H5页面](../../artifacts/sdk-5-25/browser/report.json)：四页/八页；实际选择方向点并cast，发布胶囊预警/活动显示、DOT/HOT/盾、同World重连保持或合法到期耗尽、旧帧拒绝、丢Sent增量及缓存重建、共享拾取争抢与撤离/离场独立结算。浏览器异常0、未决0，宿主释放。
- [强杀/恢复](../../artifacts/sdk-5-25/crashes/report.json)：部分准入例外在World之前，另两处实际线形接触、周期/盾后强杀；三个边界各从同一检查点恢复两次。冻结成员/hash/资产保持，金融Closed；无World/区域/状态/发布基线恢复。100次真实带计划Runtime完成责任并释放。
- 旧候选消费者：[协议4](../../artifacts/sdk-5-25/legacy4/report.json)、[协议5](../../artifacts/sdk-5-25/legacy5/report.json)、[机制37](../../artifacts/sdk-5-25/legacy6/report.json)、[机制38](../../artifacts/sdk-5-25/legacy38/report.json)、[机制39](../../artifacts/sdk-5-25/legacy39/report.json)、[机制40](../../artifacts/sdk-5-25/legacy40/report.json)、[机制41](../../artifacts/sdk-5-25/legacy41/report.json)全部通过，包外另保留原协议1～3。
- [空缓存离线源码](../../artifacts/sdk-5-25/delivery/source-verification.json)、[46项包外普通消费](../../artifacts/sdk-5-25/delivery/independent-verification.json)：保留旧44项、增加两套线形配置；同一源crate/vendor与普通release，默认/全特性同集、Clippy及最小Rust宿主。隐藏SDK回归配置/包内samples后用外部数据消费，交付H5只读启动异常0，无replay。

[交付包](../../artifacts/sdk-5-25/delivery/match-kernel-0.27.0-aarch64-apple-darwin.tar.gz) SHA-256：`ebfbbae364e0e44eae9152301d2856015f3acdf03ad603dd9919272db238e7dd`；源crate：`665c61742e05c67a267ae16ca2ceb14cbe9833519d4244d3ae4e254d0a45cee7`；普通二进制：`7dfa5c47851c4016a0505ce9c92128573adb6c61c19b7600d1750278c83613fb`。

两套新参考配表只初始化数值；四人示例按有限战斗验收调整HP/伤害比例，既有JSON不修改。初次接入暴露示例战斗过慢和测试护盾额度不足，修正记录保留在本批preflight/native-attempt目录，最终签收均绑定同一候选。13216份起点旧JSON/原资料及SDK-5-17～24证据按哈希保护，历史用户清理例外保持。新[4210预览](http://127.0.0.1:4210/)使用独立.local/sdk-5-25-demo.sqlite，[只读检查](../../artifacts/sdk-5-25/local-preview.json)未建局；既有预览/账本保持。

当前仍是固定1～8必需成员PVE。穿透、移动区域、友军/PvP、完整模式/观战及生产接入后续；本批不宣称完整原需求完成。
