# SDK-5-11 严格地面目标与单次圆形执行记录

2026-11-04；PRD v1.48；crate0.14.0 / 机制29 / schema4 / 协议3。

先冻结[需求与GC01～GC09](../batches/SDK-5-11-ground-circle.md)，再实现并受控验收。[逐项映射](../checklists/SDK-5-11-acceptance.json)和[机器汇总](../../artifacts/sdk-5-11/acceptance.json)绑定同一最终候选。

实现ground_circle严格配置与CastGround世界坐标输入。接受/提交重验半开地图边界、可行走中心、射程/视线；提交前免扣失败与灵力/self状态/两计划容量原子检查。成功冻结中心、有效攻击量、轮次和合法绑定，发布Rust预警后按当前敌人位置一次多目标结算。0预警仍跨步，圆边等值合法，遮挡/闪避/DEF/盾/命中状态/击杀/掉落沿原链。两计划占满不阻止普攻。NPC圆形共用几何，保留原NPC成本/攻击量时序。

移动保持准备并在提交重验；未提交CancelAction/后台/终局/死亡/关闭/实际换轮免扣。提交后移动/取消/后台/同World重连保留计划/CD/后摇，不退款；死亡/终局/关闭/实际换轮清空计划，聚合取消计入关闭输出预算。重启仅履行原冻结资产责任，不恢复选点/预警/准备/状态/World或重算旧金额。公开枚举/DTO按源码变更适配；旧1～28无地面字段，六类Client/协议1～3/资产及恢复schema保持。

## 执行范围

- 973项Rust，默认/全特性相同集合，保留939、新增34；Clippy全目标/全特性-D warnings通过。22项Node存储/流控/等距/账本检查通过。
- 两套包外H5配置，主副交换和ID/数值/状态变更，各撤离/死亡共4条链。真实地图点选及技能按钮，准备中UI换点不改冻结中心；空圆0命中、双敌各一次、合法状态与实际自身回复、击杀两敌/拾取/结算。纯触屏双终局，HTTP仅只读观察。
- 灵力不足/超距/视线拒绝；同World预警/CD/后摇重连，旧连接拒绝；活跃准备+预警、终局责任冻结后、资产提交后三点强杀，各连续两次恢复。修改外部参数不改原Create/hash/金额/operation ID，无重复扣奖。
- 旧19～28各普通H5双终局20条链；页面异常0，入场/结算各一次，关闭后World/未决责任0。
- 独立普通Node22条链；Rust协议1六模式，协议2/3十四场景/十四次重启/两次核对；移除包内配置/样例后包外消费通过。解压源码空CARGO_HOME、只用交付vendor离线同973项默认/全特性及Clippy通过。
- 89项原JSON/历史归档逐字节保持，原v1.1来源manifest及全部16项登记文件校验通过。旧预览4173～4183和各自账本保持，本批新开4184且未代用户创建对局。

## 交付

[0.14.0独立包](../../artifacts/sdk-5-11/delivery/match-kernel-0.14.0-aarch64-apple-darwin.tar.gz)，4,650,186字节。

- archive SHA-256 `106328a47bdc5f64b575dd605e3426f8fed6639b905e44b74dbf51fd950b5944`
- source crate SHA-256 `4e5607fcd57027f283c7ed8798c6e56286722505d6b799f4f3ecf3aa9e802314`
- ordinary binary SHA-256 `0d7ad1108a1855587e733180d4547ed76b0c8c53b519e1653adbb42b5e8ad4d8`

[普通H5/触屏/恢复证据](../../artifacts/sdk-5-11/verification.json)、[保留基线](../../artifacts/sdk-5-11/baseline-verification.json)、[独立消费](../../artifacts/sdk-5-11/delivery/independent-verification.json)、[离线源码验证](../../artifacts/sdk-5-11/delivery/source-verification.json)、[本地预览](../../artifacts/sdk-5-11/local-preview.json)。接入见[ground-targets-v1](../interfaces/ground-targets-v1.md)。

[4184预览](http://127.0.0.1:4184/)：点选或触摸地面，Q施放；2为自身护盾/HOT。描边为当前选择，实心圈为Rust预警。独立状态文件`.local/sdk-5-11-demo.sqlite`，未创建对局。

本批仅严格地面点与单次圆形。持续区域/陷阱、其它形状/裁剪策略、友军、更多槽位、引导/蓄力/再施放、多人与完整技能目录仍在M02/M05，生产接入门禁单独验收。
