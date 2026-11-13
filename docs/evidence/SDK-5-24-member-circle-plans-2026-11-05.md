# SDK-5-24 多人圆形区域执行记录

2026-11-05，0.26.0/协议6/schema7/机制41。实现前冻结[GA01～GA12](../batches/SDK-5-24-member-circle-plans.md)；[测试映射](../checklists/SDK-5-24-acceptance.json)与[机器签收](../../artifacts/sdk-5-24/acceptance.json)绑定同一普通release和真实源crate/vendor。

[同一圆计划](../../src/world/two_members/areas.rs)承接单次预警与有限持续，复用ContactBudget和[统一接触入口](../../src/world/two_members/combat.rs)。落点/射程/视线接受与提交重验，提交前失败不扣费/CD/授状态，提交self状态后冻结ATK、固定中心，每次读当前目标/DEF/盾。持续首跳等间隔、截止排除/最多128跳；repeat=0每生命身份一次，正数按scheduled时刻含等号，闪避不耗预算、全盾合法接触耗且保留原附带。NPC死亡/归途过滤，实际复位换代使用新历史身份；归功/掉落一次。

全World区域容量与全部数值/绑定由配置初始化，输出预算及容量×单区最大接触≤128历史额度在I/O前拒绝越界。来源后台/移动/重连保已提交中心/预算/期限，准备中断；settling/死亡/退出/断线超时清自己的计划，不等待金融，终局/Close清全部。重启只Closed金融责任。

公开areas用World生成id和来源EntityRef，包含几何/计划时刻/跳数/有限预算历史，不含私有输入ID、攻击、绑定或成本。full含数组包括空，delta存在完整替换、[]清空、省略不变；旧1～40省略、null拒绝。本人准备/ActionCommitted私有，公开area_started/tick/ended仅战斗事实，Rust投影后再diff。Rust/JS原子校验来源/时序/跳数/预算/代次历史，坏帧保留缓存。

- [原生/Node回归](../../artifacts/sdk-5-24/native-verification.json)：默认/全特性同1376项Rust，保留1344、新增32；68项Node保留60，Clippy/fmt通过。含1/2/4/8成员、双形态/两槽、预警/非整步/截止、容量竞争原子、实时位置/防御/盾、重复/预算/代次、取消/保护/重连/退出、隐私/坏帧；100次真实带区域公共Runtime终局、责任归零及线程退出，不作生产负载声明。
- [真实Rust→JS](../../artifacts/sdk-5-24/interop-verification.json)：2272帧（65 full/2207 delta），8接收者逐字段一致/重复幂等，默认/全特性向量摘要一致。两种区域、进度/成功历史、到期/结束/空清、周期与盾、重同步均由Rust实际输出重建。
- [59项公开边界](../../artifacts/sdk-5-24/public/report.json)：13项非法Create无部分I/O、32项配置/绑定/预算拒绝、12项实际JSONL落点/输入拒绝无成本/CD/新区域，以及4/8人两套真实Node闭环。原始Rust帧另解码核验公共计划无私有字段、创建/准备/进度/空清和状态私有切片；来源退出、带区域重连/缓存恢复、未知金融下同伴继续，战斗/拾取/独立结算未决0。
- [四人/八人真实页面](../../artifacts/sdk-5-24/browser/report.json)：实际点选落点并施放单次/持续区域，显示Rust预警/活动圆和预算；P00带已提交区域退出，P01重连保中心/期限/预算进度或核验合法结束，旧帧拒绝、丢Sent delta/清缓存后request_full恢复，再完成真实战斗、同一掉落竞争和独立终局。浏览器异常0、宿主释放。短时HOT/盾按原期限、耗尽/来源清理事实验收，不因重连重授；不要求所有异步页面同一时刻持有效果。
- [三处SIGKILL/六次恢复](../../artifacts/sdk-5-24/crashes/report.json)：部分准入（尚未World）、最后一席终局未决、核心提交回执丢失；后两处此前真实产生区域与接触/周期/盾。各同一检查点恢复两次，改外部配置仍保原成员/hash/资产，Closed未决0，区域/周期/盾/单位/发布观察均0，无战斗热恢复。
- 旧关键链：[协议4的16项](../../artifacts/sdk-5-24/legacy4/report.json)、[协议5的20项](../../artifacts/sdk-5-24/legacy5/report.json)、[机制37的31项](../../artifacts/sdk-5-24/legacy6/report.json)、[机制38的29项](../../artifacts/sdk-5-24/legacy38/report.json)、[机制39的33项](../../artifacts/sdk-5-24/legacy39/report.json)、[机制40的43项](../../artifacts/sdk-5-24/legacy40/report.json)，包外另有原协议1～3。
- [空缓存离线源码](../../artifacts/sdk-5-24/delivery/source-verification.json)、[44项包外普通消费](../../artifacts/sdk-5-24/delivery/independent-verification.json)：同一真实源crate/vendor与普通release，默认/全特性同集及Clippy；保留原42消费并加两套区域配表，隐藏SDK回归/包内samples后显式使用包外数据，交付H5只读启动异常0。未启用replay。

两套外部参考配表：四人为refresh单次圆，八人为ignore持续圆/多来源DOT、目标/总命中与重复间隔预算；八人基础HP、状态槽等差异均在JSON初始化，Rust不含示例/地图ID判断。旧参考JSON不修改。

[交付包](../../artifacts/sdk-5-24/delivery/match-kernel-0.26.0-aarch64-apple-darwin.tar.gz)SHA-256：`8764337d0f30b4c5b6402f87d55c6d47da238ccd9fb0e2d1733e7eb81a674988`；源crate：`dc3d2f062f9dda8c8afd294bcbd2f266b6ba77d78b03a89d8363fd3c1594d176`；普通二进制：`99dfa5f5a74c58748eddab21974ad0bbf8b84d7903acc27593016ff36ae0eabd`。新[4207预览](http://127.0.0.1:4207/)独立.local/sdk-5-24-demo.sqlite，[只读检查](../../artifacts/sdk-5-24/local-preview.json)未建局；旧4173/4190/4193/4195/4198/4201/4204页面和账本保持。11152份起点旧JSON/来源及SDK-5-17～23证据归档逐字节保护，历史用户清理例外保留。

当前3包映射完成、7包部分实现（R2三包及R3.1～R3.4）、15包待实现，22包未完整关闭；14M/9I/9P与完整原需求保持开放。友军/PvP、线形/移动区域、穿透及完整模式/正式接入仍后续；原48场景/50数值向量新增执行数0，不寻找第二款游戏。
