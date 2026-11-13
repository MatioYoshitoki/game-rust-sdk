# SDK-5-16 通用线形持续区域执行记录

2026-11-05；PRD v1.53；crate0.19.0 / 机制34 / schema4 / 协议3。

先冻结[LF01～LF09](../batches/SDK-5-16-line-field.md)，再实现并验收。[逐项映射](../checklists/SDK-5-16-acceptance.json)、[机器汇总](../../artifacts/sdk-5-16/acceptance.json)绑定同源候选。

## 实现与边界

line_field由length/完整width/预警/时长/周期/contact_policy及绑定初始化，接受冻结方向与参数，提交取当前位置并重验width/2弹道足迹，沿连续圆盘扫掠在首墙/地图边界截断，实际胶囊固定世界坐标。通过线段距离与目标半径、最近点弹道视线检查接触，按路径进度/路径距离/ID/代次排序，复用PlayerField调度、ContactBudget和原直接命中/状态/击杀/掉落。

首次/重复共用总预算，0每身份一次、正数按成功计划时刻给资格；闪避/失效/墙不耗，全盾0HP损失耗预算。ATK和几何/策略冻结，DEF/盾实时，成本/CD只提交一次，与两种圆形共用2份区域容量，失败无部分效果。死亡/终局/关闭/实际换轮清理未来接触和收益；重连保留，重启只恢复冻结业务责任。

PlayerFieldView/Started新增可省略line:{start,end,width}，旧圆形输出省略；Rust发布实际线段、周期与预算，H5只做等距绘制，Node只发意图。六类Client、协议1～3、责任schema保持，公开DTO源码消费需适配新增字段。

## 验收结果

- 默认/全特性同1146项Rust，保留1115、新增31；Clippy全目标/全特性-D warnings及22项Node通过。覆盖水平/斜线/端帽/侧边/刚越界/目标足迹、墙与边界截断/侧墙遮挡、接受/提交重验、冻结参数、周期/重复/排序/预算、128次成功只留一份身份历史、共享容量/原子拒绝、100次关闭/峰值与实际复位。
- 两套包外H5主副交换、一次/重复配置均完成撤离/死亡4链；A按路径接触三身份各一次，B同身份四次并击杀且预算结束；self HOT真实回复，随后普攻清理三敌、拾取、一次结算。纯触屏双终局通过；资源/两份共享计划容量/非法方向与输入拒绝、真实重复历史后重连/旧连接隔离通过，页面异常0、World/未决=0。
- 容量/非法输入验收脚本统一输入序号所有者并等待页面前台就绪后重验，原脚本摘要与已完成同源链保存在浏览器报告harnessProvenance中；Rust/配置/H5实现未变化，原生检查保持同源。
- 活跃准备+已有重复区域历史、冻结终局后、资产提交后三处强杀各两次恢复。改变外部几何/伤害/成本不改原Create/hash/请求金额/operation ID，不恢复临时区域/时钟或重复扣奖。
- 旧机制31圆形预算区域与机制33重复弹道普通H5双终局4链回归；独立普通Node40链（新增两套线形配置双终局，保留原36链），删除SDK回归配置和样例后包外消费通过。协议1 Rust六模式、协议2/3十四场景/十四重启/两核对通过。
- 解压源码空CARGO_HOME、仅vendor离线默认/全特性同1146项与Clippy通过。109项原配置/历史归档、SDK-5-15原证据及来源manifest/登记文件逐字节保持。旧4173～4188预览/账本保留，新4189独立账本且未代用户开局。

## 独立交付

[0.19.0独立包](../../artifacts/sdk-5-16/delivery/match-kernel-0.19.0-aarch64-apple-darwin.tar.gz)，4,792,074字节、1372个文件。

- archive SHA-256 `e8169f281c7ac0e80c9e46d7650fc713377deb58482c19442ee785a3c692aa45`
- source crate SHA-256 `a12dc9625ff74142a204fe2fbe956e84071598f7c8ee03126d1e3485e15a309c`
- ordinary binary SHA-256 `814f0aa90311810826cf86d76905d1c6ef418857a379e1b209e17590b33754db`

[浏览器/恢复](../../artifacts/sdk-5-16/verification.json)、[原生测试](../../artifacts/sdk-5-16/native-verification.json)、[保留基线](../../artifacts/sdk-5-16/baseline-verification.json)、[包外接入](../../artifacts/sdk-5-16/delivery/independent-verification.json)、[离线源码](../../artifacts/sdk-5-16/delivery/source-verification.json)、[配置接入](../interfaces/line-fields-v1.md)、[演示](http://127.0.0.1:4189/)。

[一次接触的等距区域截图](../../artifacts/sdk-5-16/outside-line-a-line.png)。

本批仅固定世界线段胶囊区域；完整射线引导、更多轨迹/几何/器型、多人与生产接入仍见[剩余缺口](../checklists/sdk-requirements-gap.md)。
