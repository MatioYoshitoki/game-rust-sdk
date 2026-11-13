# SDK-5-10 严格敌方单位目标执行记录

2026-11-04；PRD v1.47；crate0.13.0 / 机制28 / schema4 / 协议3。

先冻结[需求与UT01～UT10](../batches/SDK-5-10-unit-targets.md)，再实现并受控验收。[逐项映射](../checklists/SDK-5-10-acceptance.json)与[机器汇总](../../artifacts/sdk-5-10/acceptance.json)绑定同一最终候选。

## 已交付机制

unit_projectile在主/副槽复用已有弹道字段/合法状态绑定，最多两个技能。CastUnit必须使用当前World发布的完整ID/代次；接受/提交两次检查敌方身份、存活/启用/非归途/非复位、中心距离与视线。不自动追击或换人，提交前失效清准备且免扣成本/CD/self状态。

成功提交沿原整组容量/灵力预检、独立CD与共享后摇执行，按提交时合法位置冻结方向及有效ATK。发出的严格弹道不追踪、不重算攻击，不伤害挡在前面的非目标单位；原方向弹道的首接触伤害保持。目标死亡/换代/归途使严格弹道失效，不退款或回滚已生效状态。新unit_target用于当前身份、准备和提交事件；旧1～27省略，公共Action/Error/视图源码变更需适配，六类Client/协议1～3/资产/恢复schema保持。

H5提供目标按钮和等距点选/触摸、高亮及身份展示；失效清选择，不自动绑定同名新代。Node只按发布身份提交意图，全部数值和绑定来自配置，Rust裁决命中、状态和金额。重启不恢复目标/准备/弹道/World，仅履行原冻结责任。

## 实际验收

| 检查 | 最终结果 |
|---|---|
| Rust默认/全特性 | 同集合939项，保留原903、新增36；Clippy -D warnings通过 |
| Node存储/流控/投影/资产 | 22项通过 |
| 两套包外普通Node/H5 | 主副互换、ID/参数/绑定变化，各撤离/死亡，共4条完整链 |
| 严格选目标 | 显式选第二敌人，准备中UI改选第一敌人，冻结动作仍只伤第二敌人并施加正确状态；首敌HP保持180 |
| 战斗闭环 | 实际护盾/HOT、自身与单位技能独立CD/共享约束，击杀两名敌人、拾取、一次结算，目标死亡清选择 |
| 纯触屏 | 目标按钮、双技能、拾取及撤离/放弃完成双终局，HTTP只读观察 |
| 拒绝与重连 | 旧代INVALID_TARGET、超距/视线OUT_OF_RANGE、资源不足NO_RESOURCE无费用/CD副作用；重连身份/CD/后摇保持，旧连接拒绝 |
| 强杀恢复 | 活跃双CD记录后准备新单位动作（含冻结目标）、冻结终局后、资产提交后三点各连续两次；换外部参数/绑定不改原Create/hash/正文/金额/operation ID，不恢复临时目标/战斗或重复扣奖 |
| 旧版浏览器 | 机制19～27各普通双终局，18条链 |
| 独立交付 | 移除包内回归/样例后包外普通Node20条链（单位样例实际命中两种目标身份），Rust协议1六模式、协议2/3十四场景/十四次重启及两次核对 |
| 解压源码 | 空Cargo缓存、vendor、完全离线，同939项默认/全特性及Clippy通过 |
| 基线保护 | 85项旧JSON/历史归档摘要保持；来源包manifest及全部登记文件校验 |

所有正常链一次入场/结算，页面异常0，最终World/未决责任0。活跃局强杀补偿一次，其余两处保持一次结算。原20场景回归保留；完整48场景/498项内容及生产环境未据此签收。

## 交付与复核

- [交付归档](../../artifacts/sdk-5-10/delivery/match-kernel-0.13.0-aarch64-apple-darwin.tar.gz)，4,617,070 bytes，SHA-256 `ca045ca943274854cf7cc0a4187a13083cd4a25f0cd2c6a75cfc4b6dfa4308a9`。
- 源crate `c6ab3b0264b5c5a16b5aa90ae0c874fe3fdf8b1514ba230d0f030b21b871722f`；普通二进制 `fdfda0e3c5f807aff2e665bb9ff31f75eb1ba5ff404413df232f0385d675c03d`。
- [本地验收](../../artifacts/sdk-5-10/verification.json)、[原生检查](../../artifacts/sdk-5-10/native-verification.json)、[保护基线](../../artifacts/sdk-5-10/baseline-verification.json)、[独立消费](../../artifacts/sdk-5-10/delivery/independent-verification.json)、[离线源包](../../artifacts/sdk-5-10/delivery/source-verification.json)。
- [接入说明](../interfaces/unit-targets-v1.md)；[4183预览](http://127.0.0.1:4183/)，独立`.local/sdk-5-10-demo.sqlite`，预览未创建对局。原4173～4182及其账本未操作。

```sh
python3 scripts/verify_sdk_5_10.py --native
python3 scripts/package_sdk.py --out artifacts/sdk-5-10/delivery
python3 scripts/verify_sdk_5_10_delivery.py
python3 scripts/verify_sdk_5_10_source.py
python3 scripts/verify_sdk_5_10_acceptance.py
```

宿主验收脚本只对明确STALE_SEQUENCE的未接受输入有限重试，成功后重复原载荷验证幂等，不重试结果不明的动作或资产写入。

本批仅严格敌方单位弹道。友军/单位瞬时治疗、追踪、穿透/多目标、地面目标、更多槽位、引导/蓄力/再施放、HP成本、多技能AI、多人及完整技能目录仍按[剩余矩阵](../checklists/sdk-requirements-gap.md)推进。两名敌人和效果组合是配置数据，没有新增地图专属机制。
