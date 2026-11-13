# SDK-5-8 配置化双技能执行记录

2026-11-04；PRD v1.45；crate0.11.0 / 机制26 / schema4 / 协议3。

先更新[需求与MS01～MS10](../batches/SDK-5-8-player-skills.md)，再实现并验收。[逐项映射](../checklists/SDK-5-8-acceptance.json)与[机器汇总](../../artifacts/sdk-5-8/acceptance.json)绑定同一最终候选。

## 已交付范围

保留主skill，additional_skills允许一个已注册方向弹道技能。两个技能独立成本/CD，共享灵力、单准备动作和后摇。Rust按ID冻结技能/方向/轮次，成功提交后才扣费与设CD，状态绑定经过原容量原子预检。切技能、普攻、移动/导航、取消、前后台和重连不能绕过已提交后摇。遭遇实际换轮取消未提交动作不扣费；取消复位倒计时保留同轮准备，在途弹道按原轮次限定目标。

Snapshot提供player_skills和准备中的pending_action；提交事件提供skill_id，旧主技能CD别名不被附加技能覆盖。H5从配置展示两个按钮（Q/2），Node只提交意图并读取权威输出。重启只核对原冻结责任，清除临时技能视图。旧机制1～25保持原JSON形状和动作策略，资产/协议/恢复schema保持。

## 实际验收

| 检查 | 最终结果 |
|---|---|
| Rust默认/全特性 | 同集合867项，保留原835、新增32；Clippy -D warnings通过 |
| Node存储/流控/投影/资产 | 22项通过 |
| 普通H5 | 两套包外技能ID/成本/CD/弹道参数/绑定配置，各撤离/死亡，4条完整战斗→拾取→结算链 |
| 触屏 | 两个技能按钮与拾取/撤离/放弃按钮完成双终局；HTTP仅观察 |
| 拒绝与重连 | 后摇/CD拒绝无成本或CD副作用；灵力3不足以施放附加技能明确NO_RESOURCE；同World重连保持两项CD、后摇和灵力，旧连接拒绝 |
| 强杀恢复 | 活跃双CD并准备普攻、冻结终局后、资产提交后三点，各连续两次恢复；原Create/hash/业务正文和金额不变，不恢复动作/CD表/World |
| 旧消费者 | 机制19～25各普通双终局14条链，旧机制省略新字段 |
| 独立交付 | 移除包内回归/样例后，包外Node16条链、Rust协议1六模式、协议2/3十四场景及十四次重启、两次核对 |
| 解压源包 | 空Cargo缓存、vendored依赖、完全离线，同867项默认/全特性和Clippy通过 |
| 基线保护 | 77项原JSON/历史归档摘要一致；来源包manifest及全部登记文件校验 |

所有正常链一次入场/结算，页面异常0；最终World和未决责任均0。活跃局强杀使用一次补偿，其余两个强杀边界保持一次结算。原20场景回归保留；完整48场景/498项内容及生产门禁未据此签收。

## 交付与复核

- [交付包](../../artifacts/sdk-5-8/delivery/match-kernel-0.11.0-aarch64-apple-darwin.tar.gz)，4,558,253 bytes，SHA-256 `a333c00dea87a2f56b487567e003b6106e816d1ffa8a4bd1d0d715c0edf59ddc`。
- 源crate SHA-256 `d465e0e69528c38eeca21d49152c2c5a7a68bc478ddd2f1db613b93bcb77f00f`；普通二进制 `7572fa55f0286737ac334a16cfcfa58ef9c4e8cf101bb7d7e155d4c516a6000d`。
- [本地验收](../../artifacts/sdk-5-8/verification.json)、[原生检查](../../artifacts/sdk-5-8/native-verification.json)、[保护基线](../../artifacts/sdk-5-8/baseline-verification.json)、[独立消费](../../artifacts/sdk-5-8/delivery/independent-verification.json)、[离线源包](../../artifacts/sdk-5-8/delivery/source-verification.json)。
- [接入文档](../interfaces/player-skills-v1.md)；[预览](http://127.0.0.1:4181/)，独立`.local/sdk-5-8-demo.sqlite`，预览未创建对局。原4173～4180与其账本未操作。

复核命令：

```sh
python3 scripts/verify_sdk_5_8.py --native
python3 scripts/package_sdk.py --out artifacts/sdk-5-8/delivery
python3 scripts/verify_sdk_5_8_delivery.py
python3 scripts/verify_sdk_5_8_source.py
python3 scripts/verify_sdk_5_8_acceptance.py
```

本批仅1～2个方向弹道技能及已有状态绑定；严格单位/地面/自身目标、任意技能数量、引导/蓄力/再施放、多技能AI、完整技能目录和生产接入继续按[剩余矩阵](../checklists/sdk-requirements-gap.md)推进。例子中的护盾/HOT组合属于配置内容，没有增加地图专属逻辑。
