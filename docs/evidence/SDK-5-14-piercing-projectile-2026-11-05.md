# SDK-5-14 方向型直线穿透执行记录

2026-11-05；PRD v1.51；crate0.17.0 / 机制32 / schema4 / 协议3。

先冻结[需求与PP01～PP09](../batches/SDK-5-14-piercing-projectile.md)，再实现并验收。[逐项映射](../checklists/SDK-5-14-acceptance.json)和[机器汇总](../../artifacts/sdk-5-14/acceptance.json)绑定同源最终候选。

## 交付与边界

方向型piercing_projectile复用ContactBudget的有界身份历史/总预算及原直接伤害/状态/击杀/掉落链。连续扫掠而非只查终点，按首次接触t/ID/代次排序，墙体与边界使用扫掠圆连续交点（含圆角），墙同距优先。每身份每发弹成功接触一次，闪避/失效不耗预算、0HP损失/全盾吸收耗一次；预算0立即结束，不伤后方目标。配置初始化所有成本/时间/伤害/射程/速度/半径/TTL/预算/绑定。最多两发在途，准备方向/参数冻结，提交当前位置/self后ATK冻结、原子门禁与自己的CD。射程/TTL及死亡/终局/关闭/实际换轮回收，不退款/跨轮伤害。

PveView.piercing_projectiles及Started/Hit/Ended输出Rust事实。H5显式点选/触摸设方向、只画Rust弹道/余量/历史；Node只发意图。移动/取消输入/后台保留已提交弹道，同World重连保留实际飞行进度及历史；重启仅恢复原Create/hash/金额/operation ID责任，不恢复战斗飞行。公开DTO/事件枚举增加分支，源码消费需适配，六类Client/协议1～3/资产恢复schema保持。旧1～31省略新字段，原弹道/NPC/指定单位/区域顺序保持。

## 验收

- 1084项Rust，默认/全特性同集合，保留1049、新增35；Clippy全目标/全特性-D warnings、22项Node通过。覆盖高速/同距/圆边/墙/射程端点/TTL、零损失/闪避、冻结/实时属性、独立历史/代次、失败免扣、100次关闭、128历史及峰值责任回收。
- 两套包外H5配置主副互换：A预算2只中前两敌、第三无误伤，B预算3顺序命中三敌；状态/实际回复→三敌击杀/拾取→撤离/死亡4链，纯触屏双终局通过。页面异常0，入场/结算各一次，World/未决=0。
- 真实历史/飞行重连、精确输入重复与旧连接隔离、资源/非法意图及两发在途容量拒绝通过（第三发免扣，自身技能仍可用）。活跃准备+已命中在途弹、冻结终局后、资产提交后三处强杀各两次恢复，更改外部数值不改变冻结责任/重复扣奖或恢复临时弹道。
- 旧19～31普通H5双终局26链通过。独立普通Node32链，含新增包外穿透A/B双终局；移除SDK配置/样例后消费成功。Rust协议1六模式、协议2/3十四场景/十四重启/两次核对保持。
- 解压源crate空CARGO_HOME、只用交付vendor离线默认/全特性同1084项及Clippy通过。101项历史JSON/归档、来源v1.1 manifest及16个登记文件逐字节保持。旧4173～4186预览/账本保持，4187独立账本且未代用户开局。

验收脚本对连接换代明确区分尚未接受的拒绝与已接受请求的精确重复：仅前者重读连接后重试，后者若以STALE_CONNECTION隔离则验证代次且不换号重发。同一binary/Rust/Web/配置输入下已通过的片段保留原脚本SHA，记录于verification.json的harnessProvenance，原脚本同时归档供复核。

## 交付包

[0.17.0独立包](../../artifacts/sdk-5-14/delivery/match-kernel-0.17.0-aarch64-apple-darwin.tar.gz)，4,749,193字节，1355个文件。

- archive SHA-256 `25907eed729249f639e12c2d4ea29df628acfb03745447e95874f980f8f4f2db`
- source crate SHA-256 `02cf0182a5331114c6a4191311a4cfd000dbba6726fd89347b533519aff98147`
- ordinary binary SHA-256 `e396844f207a5720a553b07a07966da442b103204beab6aec776109cf3007770`

[包外消费](../../artifacts/sdk-5-14/delivery/independent-verification.json)、[离线源码](../../artifacts/sdk-5-14/delivery/source-verification.json)、[浏览器/恢复](../../artifacts/sdk-5-14/verification.json)、[保留基线](../../artifacts/sdk-5-14/baseline-verification.json)。[配置/接入](../interfaces/piercing-projectiles-v1.md)，[演示](http://127.0.0.1:4187/)。

本批只关闭M05方向型直线穿透子集；追踪/回旋/反弹/分裂、单位型穿透/其它几何/完整武器家族、周期成本/引导、其它来源存续策略、多人/生产接入及原精确场景仍保留在[缺口表](../checklists/sdk-requirements-gap.md)。
