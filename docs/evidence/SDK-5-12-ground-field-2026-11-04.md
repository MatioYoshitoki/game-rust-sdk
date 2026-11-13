# SDK-5-12 固定圆形持续区域执行记录

2026-11-04；PRD v1.49；crate0.15.0 / 机制30 / schema4 / 协议3。

先冻结[需求与GF01～GF09](../batches/SDK-5-12-ground-field.md)，再实现、验收。[逐项映射](../checklists/SDK-5-12-acceptance.json)与[机器汇总](../../artifacts/sdk-5-12/acceptance.json)绑定同一最终候选。

## 已交付机制

ground_field固定圆形复用严格CastGround双重校验和既有直接命中链；所有半径/预警/寿命/周期/倍率/成本/CD及合法状态绑定由配置初始化。提交原子检查灵力/self状态及与单次圆形共享的2计划总容量，成功才冻结中心/直接攻击量/时间表/绑定/来源/轮次，成本/CD/后摇仅一次。容量满不会误拦普攻/自身技能。首跳等待间隔、结束点排除、非整步/有界补跑不漂移、最多128跳；每周期当前位置、离开再入/空区/遮挡/圆边/闪避/活体资格和稳定创建/目标顺序确定，多区域独立，每敌每周期最多一次。

每跳读取当前DEF/盾，合法hit状态沿原施加边界，死目标不重复命中/掉落，不递归创建区域。来源固定当前participant，失效策略固定end；死亡/终局/关闭/实际换轮结束未来周期且原因明确，正常到期/取消只结束一次。移动/取消输入/后台/同World重连保留提交区域/CD/后摇，不退款/回滚已发生伤害。重启仅履行原Create/hash/金额/operation ID责任，不恢复临时区域/准备/状态/World或重算旧金额。Rust发布player_fields/Started/Tick/Ended，旧1～29省略新字段/事件，原单次圆形/NPC时序及六类Client/协议1～3/资产恢复schema保持；公共视图/事件枚举源码消费需适配。

## 验收

- 默认/全特性同1010项Rust，保留973、新增37；Clippy全目标/全特性-D warnings通过，22项Node存储/流控/等距/账本检查通过。
- 两套包外H5配置交换主副、改ID/时长/间隔/倍率/绑定，撤离/死亡共4条完整链。真实点选/技能按钮、准备中改选保持冻结中心；空区三周期全0接触、活动区域三周期各命中双敌，实际命中状态/自身回复→两敌击杀/拾取→结算。纯触屏双终局，HTTP仅只读观察。
- 灵力不足/超距/视线拒绝；活动周期进度重连、重复输入/旧连接隔离；准备+活跃区域、冻结终局后、资产提交后三点强杀，各连续两次恢复。修改外部时长/间隔/成本/倍率/绑定不改原Create/hash/金额/operation ID，不重复扣奖或恢复战斗。
- 100次关闭，合法状态输出峰值与直接区域伤害峰值均可回收World、保留并完成冻结资产责任。旧19～29普通H5双终局22条链通过；页面异常0、入场/结算各一次、World/未决=0。
- 独立普通Node24链、Rust协议1六模式，协议2/3十四场景/十四次重启/两次核对通过；移除包内配置/样例后包外消费，解压源crate空CARGO_HOME且只用交付vendor离线同1010项默认/全特性及Clippy通过。
- 93项原JSON/历史归档逐字节保持，原v1.1来源manifest及全部16项登记文件校验通过。旧预览4173～4184和各自账本保持，本批新开4185，未代用户创建对局。

## 交付

[0.15.0独立包](../../artifacts/sdk-5-12/delivery/match-kernel-0.15.0-aarch64-apple-darwin.tar.gz)，4,669,314字节。

- archive SHA-256 `1c895d62122dcfaeb4f00b15950a730580301067f5db829d2b334beb1cc92bb6`
- source crate SHA-256 `802a8bd3a775a9a7cd89bc17495a1ca0a0a0c0eba2f46498ac2a23491e133895`
- ordinary binary SHA-256 `c84aa64d0be54c14350910ca5d7386e8ed176c5fddd2a086180b33453972151c`

[普通H5/触屏/恢复](../../artifacts/sdk-5-12/verification.json)、[基线保留](../../artifacts/sdk-5-12/baseline-verification.json)、[独立消费](../../artifacts/sdk-5-12/delivery/independent-verification.json)、[离线源码](../../artifacts/sdk-5-12/delivery/source-verification.json)、[本地预览](../../artifacts/sdk-5-12/local-preview.json)。接入见[persistent-fields-v1](../interfaces/persistent-fields-v1.md)。

[4185预览](http://127.0.0.1:4185/)：点选/触摸地图后Q创建持续区域，2为自身护盾/HOT；预警/剩余时间/周期进度均来自Rust。独立状态`.local/sdk-5-12-demo.sqlite`，未创建对局。

仅固定圆形持续敌对接触、提交一次成本及来源失效结束策略。其它区域/治疗光环/移动附着/陷阱/形状、独立重复接触间隔和总命中预算、引导/周期费用、来源保留策略、多人与完整M05/技能目录继续保留；生产接入另验。
