# SDK-5-15 移动弹道重复命中执行记录

2026-11-05；PRD v1.52；crate0.18.0 / 机制33 / schema4 / 协议3。

先冻结[需求与RP01～RP09](../batches/SDK-5-15-projectile-repeat.md)，再实现并验收。[逐项映射](../checklists/SDK-5-15-acceptance.json)与[机器汇总](../../artifacts/sdk-5-15/acceptance.json)绑定同源候选。

## 交付内容

piercing_projectile可选repeat_interval_ms：0每身份每发一次；正数按该World/发弹/ID/代次的最新成功接触时刻+间隔给资格，等值合法。复用ContactBudget；必须真实扫掠到当前合法目标，每步每身份最多尝试一次，未接触不补跳，出入/重连不清时钟，新代及多弹独立。首次/重复共用max_hits总预算，成功含全盾0损失消耗一次并更新时间；闪避/失效/未到期/墙不耗不刷新。原冻结攻击量/实时防御盾、状态/击杀/掉落保持，成本/CD只提交一次，死亡/终局/关闭/实际换轮清理未来接触，重启仅恢复冻结责任。

Rust发布repeat_interval_ms、最新contacts.hit_at_ms、正间隔next_eligible_at_ms和Hit.hit_at_ms；H5只显示权威事实，Node只发意图。显式字段仅≥33/schema4/PVE，旧1～32及省略字段配置/输出保持形状。公开DTO/枚举源码消费需适配，六类Client/协议1～3/恢复schema保持。

## 验收

- 1115项Rust，默认/全特性同集合，保留1084、新增31；Clippy全目标/全特性-D warnings、22项Node通过。覆盖0/等值/跨步/短间隔、进出/错过不补跳/代次、闪避/盾/当前防御、冻结参数、预算终止、128次成功只占一个身份、100次关闭/峰值与实际换轮回收。
- 两套包外H5配置主副交换：A显式0命中三个不同身份、余1预算后射程结束；B正间隔同一身份连续四次并击杀、预算耗尽结束，真实伤害/状态/HOT回复→三敌清理/一次掉落/拾取→撤离/死亡4链。纯触屏双终局、资源/两发在途容量/非法意图拒绝通过，页面异常0，一次入场/结算、World/未决=0。
- B重连发生于真实重复至少两次之后，保留最新时刻/资格/余量/位置；精确重复去重与旧连接隔离，不换号重发已接受操作。活跃准备+已有重复在途弹、冻结终局后、资产提交后三处强杀各两次恢复，改变外部间隔/伤害不改原Create/hash/金额/operation ID，不恢复临时计时或重复扣奖。
- 旧19～32普通H5双终局28链通过。独立普通Node36链含新增一次/重复包外配置双终局；删除SDK配置/样例后消费成功。Rust协议1六模式、协议2/3十四场景/十四次重启/两次核对通过。
- 解压源码空CARGO_HOME、仅交付vendor离线默认/全特性同1115项及Clippy通过。105项原配置/历史归档与原来源manifest及16项登记文件逐字节保持（构建缓存不计配置）。旧4173～4187预览/账本保留，新4188独立账本、未代用户开局。

## 独立交付

[0.18.0独立包](../../artifacts/sdk-5-15/delivery/match-kernel-0.18.0-aarch64-apple-darwin.tar.gz)，4,764,037字节，1363个文件。

- archive SHA-256 `c10faa658f44a3e2201bce2fe511d02c07254442da173202f1b287766b24ffbf`
- source crate SHA-256 `567a35ab0766214b1ff110d07f2c01ef92153853a41ee930668b220a9fa4bfbb`
- ordinary binary SHA-256 `557700edf2c438f6e78fa2cabefe3739eb3a3178aef2895a6564ea0a3accb51e`

[浏览器/恢复](../../artifacts/sdk-5-15/verification.json)、[容量](../../artifacts/sdk-5-15/capacity-verification.json)、[保留基线](../../artifacts/sdk-5-15/baseline-verification.json)、[包外消费](../../artifacts/sdk-5-15/delivery/independent-verification.json)、[离线源码](../../artifacts/sdk-5-15/delivery/source-verification.json)。[配置/接入](../interfaces/projectile-repeat-v1.md)，[演示](http://127.0.0.1:4188/)。

本批只关闭M05移动直线穿透的重复接触子集，轨迹/追踪/回旋/反弹/分裂、单位型穿透/其它几何/完整家族、更多动作/多人/生产接入及原精确内容仍见[剩余缺口](../checklists/sdk-requirements-gap.md)。
