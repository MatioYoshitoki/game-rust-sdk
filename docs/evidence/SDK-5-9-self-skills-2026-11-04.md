# SDK-5-9 自身施法执行记录

2026-11-04；PRD v1.46；crate0.12.0 / 机制27 / schema4 / 协议3。

先冻结[需求与SS01～SS10](../batches/SDK-5-9-self-skills.md)，再实现并受控验收。[逐项映射](../checklists/SDK-5-9-acceptance.json)与[机器汇总](../../artifacts/sdk-5-9/acceptance.json)绑定同一最终候选。

## 已交付机制

主/副槽均可配置self_status，与原projectile组合或交换，总数仍1～2。新自身形状只含ID、成本、前后摇/CD及1～2个合法self状态绑定，禁止无用弹道参数和hit绑定；CastSelf没有方向/目标或客户端金额。固定步提交时原子预检容量/灵力，成功才施加整组状态、扣费、设置自身CD及共享后摇。不创建弹道/直接攻击/命中收益，也不消费攻击随机数；0前摇在下一固定步执行。护盾、HOT、属性/叠层等沿已有合法绑定执行。

取消、后台、死亡、终局、关闭及实际遭遇换轮释放准备不扣费；提交后取消不退款或撤销状态，移动/导航/切技能/前后台/重连不能缩短后摇。ignore保持实例但仍正常消费成本/CD。新配置和意图严格分型，旧机制1～26形状和语义保持；公共Action新增枚举分支，Rust穷尽match需适配，六类Client/协议1～3/资产/恢复schema保持。

## 实际验收

| 检查 | 最终结果 |
|---|---|
| Rust默认/全特性 | 相同903项，保留原867、新增36；Clippy -D warnings通过 |
| Node存储/流控/投影/资产 | 22项通过 |
| 两套包外普通Node/H5 | 交换主副槽、修改ID/成本/CD/弹道参数/自身绑定，各撤离/死亡，共4条完整链 |
| 真实自身效果 | 施法不降低敌人HP，HOT实际回复缺口，护盾实际吸收；弹道继续战斗、拾取及结算 |
| 纯触屏 | 两个技能按钮与拾取/撤离/放弃完成双终局；HTTP只读观察 |
| 拒绝与重连 | 灵力3时自身施法NO_RESOURCE无副作用；共享恢复/独立CD拒绝、重复输入去重、状态身份/两项CD/后摇重连及旧连接拒绝 |
| 强杀恢复 | 活跃双CD/准备、冻结终局后、资产提交后三点各连续两次；改变外部自身绑定/参数不重算原Create/hash/事务正文/金额，不恢复临时World/状态/CD或重复扣奖 |
| 旧版浏览器 | 机制19～26各普通双终局，16条链 |
| 独立交付 | 包外移除回归/样例后，普通Node18条链、Rust协议1六模式、协议2/3十四场景及十四次重启、两次核对 |
| 解压源码 | 空Cargo缓存、vendor、完全离线，同903项默认/全特性及Clippy通过 |
| 基线保护 | 81项旧JSON/历史归档摘要保持；来源包manifest及全部登记文件校验 |

所有正常链一次入场/结算、页面异常0，最终World/未决责任均0。活跃局强杀补偿一次，其余两处保持原一次结算。原20场景回归保留；完整48场景/498项内容及生产环境未据此签收。

## 交付与复核

- [交付归档](../../artifacts/sdk-5-9/delivery/match-kernel-0.12.0-aarch64-apple-darwin.tar.gz)，4,574,523 bytes，SHA-256 `597c3f0002a88ca7e5144d1c42de43f6407dc82e7f033c1531abd576fe52432a`。
- 源crate `5e50758824fea6e06333fab30cc28015a5c7dc2f7dd7601de066b1ba27ac8376`；普通二进制 `da2e764041b9cf327696ea9ff29066b7a09d23c9752f8049d06a6ea87e5962cd`。
- [本地验收](../../artifacts/sdk-5-9/verification.json)、[原生检查](../../artifacts/sdk-5-9/native-verification.json)、[保护基线](../../artifacts/sdk-5-9/baseline-verification.json)、[独立消费](../../artifacts/sdk-5-9/delivery/independent-verification.json)、[离线源包](../../artifacts/sdk-5-9/delivery/source-verification.json)。
- [接入说明](../interfaces/self-skills-v1.md)；[4182预览](http://127.0.0.1:4182/)，独立`.local/sdk-5-9-demo.sqlite`且预览未创建对局；原4173～4181及其账本未操作。

```sh
python3 scripts/verify_sdk_5_9.py --native
python3 scripts/package_sdk.py --out artifacts/sdk-5-9/delivery
python3 scripts/verify_sdk_5_9_delivery.py
python3 scripts/verify_sdk_5_9_source.py
python3 scripts/verify_sdk_5_9_acceptance.py
```

浏览器验收的宿主脚本只对明确STALE_SEQUENCE的未接受请求有限重试，成功后重复完全相同的载荷验证幂等；不会重试结果不明的动作或资产写入。

本批只关闭自身施法与已有self效果组合。严格单位/地面目标、更多技能数、引导/蓄力/再施放、多技能AI、新状态族、HP成本、多人及完整目录仍按[剩余矩阵](../checklists/sdk-requirements-gap.md)推进。示例护盾/HOT属于配置内容，没有新增地图专属机制。
