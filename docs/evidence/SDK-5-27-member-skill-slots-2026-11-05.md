# SDK-5-27 配置化多技能槽验收（2026-11-05）

SDK-5-27按MS01～MS12限定签收，0.29.0/协议6/schema7/机制44。skill＋additional_skills数组初始化1～8个静态技能，顺序和全部数值由配置决定；复用现有按ID施法、每成员/每技能独立CD与费用、共享准备/后摇及区域/弹道/状态/伤害流程。末槽的类型/数值/绑定/最坏预算完整校验，不裁掉末槽；旧1～43保留两槽与原存在规则。私有有序列表、准备/CD与重连/退出沿原投影和事务，无新Action/Client方法或金融/恢复DTO。移动保留准备并提交重验，取消/保护中断未提交无成本；重连保已提交CD/后摇/计划，重启仅金融责任。

默认/全特性同1474项Rust（保留1442、新增32）、90项Node（保留82），2488份真实帧跨语言逐字段精确重建；64项公共边界、四槽/四人与八槽/八人Node/H5实际逐槽提交及战斗/共享拾取/独立终局、三处强杀各同检查点两次金融恢复、100次多槽Runtime释放、50项包外普通消费与空缓存离线源码全部通过。原48场景/50数值向量新增执行数0；3包映射完成、7包部分实现、15包待实现，22包尚未完整关闭。

- [实现前MS01～MS12](../batches/SDK-5-27-member-skill-slots.md)、[严格接口](../interfaces/member-skill-slots-v1.md)、[机器签收](../../artifacts/sdk-5-27/acceptance.json)。
- [原生验证](../../artifacts/sdk-5-27/native-verification.json)：32项新增Rust覆盖1/2/4/8槽与成员、首中末槽有类型施法、独立费用/CD、共享准备/后摇、取消/移动/保护、提交资源/状态/目标/容量重验、实际末槽效果/同World计划、私有投影/重连/退出。8项新增Node覆盖严格列表/准备关联及纯意图/提交观察。原1442项Rust/82项Node保留，默认/全特性同集，Clippy/fmt通过。
- [跨语言真实发布](../../artifacts/sdk-5-27/interop-verification.json)：2488帧（73full/2415delta），8接收者、8槽完整CD/准备/不同机制计划/重连/空清；默认/全特性向量相同，逐字段精确重建、重复幂等。坏列表/ID/时间/准备关联及基线原子拒绝。
- [普通Node公共边界](../../artifacts/sdk-5-27/public/report.json)：13个非法创建无部分I/O、37个非法配置（含末槽与整表预算）、12个实际非法输入无成本/CD/计划、两套四槽/四人和八槽/八人全旅程。每个槽实际提交，Rust私有列表始终按配置顺序；未知金融时其他成员继续，公开成员无CD/准备等私有字段。
- [真实H5](../../artifacts/sdk-5-27/browser/report.json)：4页/8页、配置生成选择器，page0通过实际UI施放每槽，其余通过原公共有类型输入；真实战斗/共享拾取/独立终局，重连、旧帧拒绝、丢Sent增量和缓存恢复；敌人清空后单位技能显示INVALID_TARGET，提交计数不增加、无浏览器未捕获异常。各槽提交、发布CD及计划可见，[截图检查](../../artifacts/sdk-5-27/visual-verification.json)。
- [强杀与恢复](../../artifacts/sdk-5-27/crashes/report.json)：部分准入在World前，另两处已实际提交不同槽并有周期/盾/战斗；三边界各同检查点两次恢复。原成员/hash/资产保持，Closed金融收尾，无World/CD/计划/状态/基线恢复。100次公共Runtime在八槽配置下由各成员实际提交两个不同槽后完成责任并释放。
- 同候选旧关键链：[协议4](../../artifacts/sdk-5-27/legacy4/report.json)、[协议5](../../artifacts/sdk-5-27/legacy5/report.json)、[37](../../artifacts/sdk-5-27/legacy6/report.json)、[38](../../artifacts/sdk-5-27/legacy38/report.json)、[39](../../artifacts/sdk-5-27/legacy39/report.json)、[40](../../artifacts/sdk-5-27/legacy40/report.json)、[41](../../artifacts/sdk-5-27/legacy41/report.json)、[42](../../artifacts/sdk-5-27/legacy42/report.json)、[43](../../artifacts/sdk-5-27/legacy43/report.json)通过，包外另有原协议1～3。
- [50项包外消费](../../artifacts/sdk-5-27/delivery/independent-verification.json)、[空缓存离线源码](../../artifacts/sdk-5-27/delivery/source-verification.json)：原48项保留，增加四槽/八槽两套配置；同一源crate/vendor和普通release，默认/全特性同集及Clippy、最小Rust消费者；隐藏SDK回归配置/包内samples后显式使用包外数据，交付H5只读启动异常0，无replay。

[交付包](../../artifacts/sdk-5-27/delivery/match-kernel-0.29.0-aarch64-apple-darwin.tar.gz) SHA-256：`5d92505bf7f29604af20ff0964e89154fab264e825033c5edb60032d3dfdb37c`；源crate：`d798545af030a5ecd2bc5ec1bbb733ef50d0daf300d19e6320a5d8498cb03601`；普通二进制：`72189743db75a29d20e10e9e171635ee85cc294373a98ed25322e8201c46324f`。

过程记录保留：移动测试按既有“保留准备”规则修正，旧单位解码测试补齐真实Rust本就发布的技能列表；H5测试改用页面选择器提交初始自身槽，并补齐无目标错误处理，最终重制交付候选再验收。初始候选在本批candidate-attempt-1，预检及修复日志在本批preflight目录；不引用初始候选作为最终签收。19822份起点旧JSON/原资料/SDK-5-17～26证据按哈希保护，历史用户清理例外保持。新[4216预览](http://127.0.0.1:4216/)使用独立.local/sdk-5-27-demo.sqlite，[只读检查](../../artifacts/sdk-5-27/local-preview.json)未建局；既有预览/账本保持。

当前仅固定1～8必需成员PVE和静态同表1～8槽。动态换装/解锁/重排、成员不同技能表、完整施法阶段、友军/PvP、召唤与模式/正式接入仍开放。
