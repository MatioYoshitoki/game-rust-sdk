# SDK-5-18 同World双成员执行记录

2026-11-05；PRD v1.56；crate0.20.0 / 协议4 / schema5 / 机制35。[TM01～TM13规格](../batches/SDK-5-18-two-member-loop.md)、[逐项签收映射](../checklists/SDK-5-18-acceptance.json)、[同源机器证据](../../artifacts/sdk-5-18/acceptance.json)。

本批实现同一Runtime、同一World的两名必需成员。Rust持有双方受控状态及一份共享NPC、地形、弹道和资源；每成员的金融专用账本不创建World。双方分别准入，均有真实sent全量且ready后只共同激活一次；一人死亡、离场、撤离、离线或核心提交未知不暂停另一人。共享资源按邮箱FIFO尝试，恰一次收益；Rust逐接收者生成私有全量与明确归属的事件，Node只执行认证、传输和冻结业务I/O。

声明范围为平面等高零DOT、普通近战/直线弹道NPC、普攻/普通方向弹道/自身ATK或DEF、公共整数资源与光球、有限成长及撤离/死亡结算。协议4对白名单外配置创建前拒绝；协议1～3保留原单人契约。数值和内容由两套外部配置初始化，不按地图、成员或技能ID分支。

## 实现与迁移

新增[CreateV4及有类型DTO](../../src/protocol/v4.rs)、[版本分派](../../src/session.rs)、[双成员协调器](../../src/two_members.rs)与[唯一共享World](../../src/world/two_members.rs)，复用原伤害、状态聚合、地形导航/扫掠及资产责任状态机。每成员独立输入幂等表、连接epoch、sent基线、有界发布队列、背包revision、成本/CD和结算。PublicMemberView仅含公开身份、位置、HP与阶段；本人SP、库存/动作/CD、主体和金融事件不发给另一人。

新增[普通Node/SQLite宿主](../../adapters/node/two-member-host.mjs)与[H5薄适配器](../../adapters/h5/two-member-server.mjs)。一批输出、最新责任检查点及全部原请求先同事务持久化再执行I/O；不同成员I/O独立，未知结果只查询原operation。关闭状态之后继续drain最新责任，未排空时宿主报告pending未知，不能使用旧检查点伪报零。H5只发意图和展示Rust事实，使用2:1等距投影及缓存地形。

[接入契约](../interfaces/two-members-v4.md)及[wire schema](../interfaces/two-members-v4.schema.json)在行为实现前建立。Rust原六个Client入口继续存在，Create/业务回执接受新旧有类型载荷；OutputBody新增v4变体，穷尽匹配需适配。旧schema1成员检查点格式和摘要规则保持，聚合检查点使用排序键的紧凑JSON摘要，避免JSONL转Value重排导致跨语言误判。恢复校验成员归属、来源上限及两人聚合数量，仅建立Closed责任账，不恢复战斗。

## 实际验证

| 范围 | 结果与证据 |
|---|---|
| 默认/全特性Rust | 两组同1184项，原1146项集合保留、新增38项；[default](../../artifacts/sdk-5-18/default.log)、[all-features](../../artifacts/sdk-5-18/all-features.log)、[Clippy](../../artifacts/sdk-5-18/clippy.log)。包括双人实际状态、NPC确实伤害双方、满包拒绝、错成员/旧连接/错误回执、未知准入/补偿、成员离线期限及终局清理。 |
| Node存储/适配 | 30项全部通过，保留原22项、增加8项成员账本、批次原子性、冻结配置/原请求及恢复隔离；[日志](../../artifacts/sdk-5-18/node-tests.log)。 |
| 普通JSONL公共消费 | 16项：12类创建拒绝后无部分创建/业务请求，两套双人完整链、慢A/丢失核心回执时B继续、必需席位拒绝后一次补偿；[报告](../../artifacts/sdk-5-18/public/report.json)。第二套反转准入提交、Join、资源收益及终局顺序。 |
| 两套双页面H5 | 两个真实连接共同激活、NPC伤害双方、独立移动/自身状态、重连时另一人移动、实战击杀、争抢同一掉落、A撤离后B继续并终局，均无console/page异常；[报告与账本](../../artifacts/sdk-5-18/browser/report.json)。H5全量来自普通包内二进制，未注入World状态。 |
| 强杀恢复 | 单方准入已提交；双方终局且一方未提交；核心已提交但回执丢失。三个边界均真实SIGKILL、确认旧kernel退出，然后从同一个原冻结检查点恢复两次，共6次；修改包外费用/技能数值不能改原hash/金额/operation。均Closed/pending0，第二次不重复收益；[报告](../../artifacts/sdk-5-18/crashes/report.json)。 |
| 资源释放 | 原生验收包含100次共享World关闭及100次真实公共Runtime创建/共同激活/关闭/丢失drain处理，确认线程join、无活World和未决责任；不是仅构造两条成员记录。 |
| 独立交付 | [独立消费报告](../../artifacts/sdk-5-18/delivery/independent-verification.json)：6种协议1 Rust模式、协议2/3各7条责任链及各自再启动核对、10条原圆形/线形/重复弹道Node链、2套新双成员Node链，共32例。复制配置到账本和工作目录均在包外，运行时移走SDK配置/examples/samples；Node启动入口兼容临时目录符号链接。 |
| 离线源码 | 同源解包后使用空CARGO_HOME、包内vendor及离线模式，default/all-features同1184项和Clippy通过；[报告](../../artifacts/sdk-5-18/delivery/source-verification.json)。 |

验收过程补齐了服务时钟原点、跨语言检查点编码、关闭后的责任排空、重连必须等待新连接快照，以及包外Node入口路径判定。各修正均由相应真实失败驱动，最终候选的摘要与全部当前报告交叉核对，迭代失败日志不作为最终通过证据。

## 交付与范围

[0.20.0本地交付包](../../artifacts/sdk-5-18/delivery/match-kernel-0.20.0-aarch64-apple-darwin.tar.gz)包含aarch64-apple-darwin普通二进制、源crate/vendor、协议1 Rust消费方、协议2/3 Python宿主、协议4 Node/SQLite与双页面H5、两套配置、DTO文档和文件摘要。包摘要、source crate摘要、binary摘要及所有保留源码摘要见[签收](../../artifacts/sdk-5-18/acceptance.json)，不据此声明其它平台或生产环境已验收。

预览为[4190双成员验证台](http://127.0.0.1:4190/)，独立数据库`.local/sdk-5-18-demo.sqlite`；[只读冒烟](../../artifacts/sdk-5-18/local-preview.json)只做GET，不创建会话。旧预览、用户原数据库和原配置不迁移或覆盖。受保护的101个既存文件逐字节核对，含原来源包、旧配置/参考账户和SDK-5-17记录；允许迁移的是当前Cargo版本/锁、SDK说明、交付README和协议1消费方源码。SDK-5-17已登记的用户清理历史artifacts例外继续保留，不复原或宣称复核缺失旧档。

TM01～TM13只签收本批有限闭环。R2.1在声明的双人PVE实体范围签收；R2.2只完成双席，R2.3只完成私有全量。标准8席/模式容量、delta/旁观、完整实体关系和双人战斗组合、完整M01/M14仍开放；本批没有运行原48场景/50向量，不增加原内容覆盖通过数，也未寻找第二款游戏。
