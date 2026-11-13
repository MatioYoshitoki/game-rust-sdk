# SDK-5-19 配置化成员容量执行记录

2026-11-05；PRD v1.57；0.21.0 / 协议5 / schema6 / 机制36。[MC01～MC10需求](../batches/SDK-5-19-member-capacity.md)、[逐项映射](../checklists/SDK-5-19-acceptance.json)、[同源机器签收](../../artifacts/sdk-5-19/acceptance.json)。

本批将原同World双成员调度推广为配置指定的1～8名必需成员。每个成员的事实、背包、成本、动作/CD、连接/序列、发布和终局独立，NPC/地形/弹道/资源仍只有一份。全部入场提交成功、各自全量确实sent且ready后共同激活一次；一席失败或缺席不自动缩容，按原冻结责任逐席查询/补偿。一人终局或金融未知不暂停生存者，最后一人退出可操作集合后释放World。

来源`resolved-matchmaking`标准maxPlayers=8/minPlayers=1用于本批容量上限的依据。本批只接已经提供的固定完整名单，没有实现匹配队列或自动参战机器人；普通PVE NPC不能计作匹配机器人。标准完整匹配、16/24模式、PvP、delta/旁观、全部单人战斗组合与生产负载仍待做，完整R2/M01/M14继续开放。

## 契约与实现

[成员配置](../../src/config/two_members.rs)新增严格members块：capacity为1～8整数，spawns恰capacity个、任意两足迹不重叠且各自合法可达；部署/每成员发布/共享弹道预算仍由配置初始化，输出预算不足拒绝。原schema5/two_members/协议4仍严格两人，原单人协议1～3保持。

[CreateV5](../../src/protocol/v4.rs)通过原create入口使用，CreateRequest增加Members分支。[共用协调器](../../src/two_members.rs)与[唯一World](../../src/world/two_members.rs)推广成员表和身份检查，不按人数、地图、成员名编写不同玩法。协议5有意复用既有MemberBusinessReceipt、SnapshotV4及*_v4输出标签/金融检查点结构；[协议文档](../interfaces/members-v5.md)和[wire schema](../interfaces/members-v5.schema.json)明确迁移与拒绝规则。

协议5每成员可信资产事实最多32行，沿原资产ID64 UTF-8字节上限，超限在应用前拒绝；恢复同样核对。协议4保留原128行。有限事实预算用于保留256KiB普通JSONL恢复边界，不静默截掉资产；原生测试使用八份上限事实和需要JSON转义的最大长度ID，真实关闭/冻结后核对完整CreateV5恢复载荷大小。

[普通Node消费方](../../adapters/node/member-host.mjs)复用原按成员落库/发送的薄宿主，实际执行冻结业务，不计算收益或维护第二份World。[H5入口](../../adapters/h5/member-server.mjs)复用同一薄服务，选择四人/八人外部数据和多页面展示；等距投影及缓存地形保持。原源码/二进制SDK-5-18归档和现有预览均保留。

## 实际验收

| 范围 | 结果与证据 |
|---|---|
| 原生集合 | 默认和全特性同1209项，原1184项全部保留，新增25项；[原生报告](../../artifacts/sdk-5-19/native-verification.json)及[Clippy](../../artifacts/sdk-5-19/clippy.log)。新增测试覆盖1/2/4/8实际成员、末席sent/ready、逆序回执、第一/中间/末席事实与准入拒绝、未知与补偿、真实NPC伤害/死亡、资源竞争/满包、私有发布、重连/离线到期、未知核心、绝对终局、输出背压及责任恢复。 |
| 100次生命周期 | 使用公共Client创建8名不同主体，逐人真实事实/入场、共同激活、关闭与责任提交，包含丢失drain后找回输出；每次确认全部责任终态、每人一次入场/结算，drop Runtime后Client关闭，线程join。 |
| Node存储 | 原30项全部保留，通过[本批Node日志](../../artifacts/sdk-5-19/node-tests.log)复核；多成员新行为由真实公共入口/SQL/浏览器链验证。 |
| 普通协议5入口 | [20项报告](../../artifacts/sdk-5-19/public/report.json)：13类无部分创建的严格拒绝、5类创建Runtime前的错误/不支持配置拒绝、8人中一席慢核/丢失回执时同伴真实移动、必需中间席位拒绝后其它7人一次补偿。 |
| 四人/八人H5 | [报告](../../artifacts/sdk-5-19/browser/report.json)：两套外部配置各有4/8个独立页面/连接；共同激活、NPC伤害不同成员、独立移动/自身状态、重连时同伴继续移动、实际施法击杀/争抢公共掉落、第一人撤离后其余人独立退出，最终全部账户一次结算、Root宿主释放，无console/page异常。前端未注入World状态。 |
| 八人真实强杀 | [3处边界/6次恢复](../../artifacts/sdk-5-19/crashes/report.json)：3席已准入而5席未提交；7人已结算而最后一席未提交；中间席位核心已提交但回执丢失。均SIGKILL并确认旧kernel退出，每处从完全相同原检查点恢复两次；改外部配置费用/携带/技能数值不改变原责任，最终Closed/pending0且再次恢复账户不变。 |
| 旧协议4 | [16项公共回归](../../artifacts/sdk-5-19/legacy-public/report.json)保留原双成员创建拒绝、两套双人实际收益链、慢核心与缺席补偿；旧协议4仍拒绝3/8人。 |
| 同源独立交付 | [包外34例](../../artifacts/sdk-5-19/delivery/independent-verification.json)：协议1 Rust六模式；协议2/3各七条责任链和逐条再启动核对；旧圆形/线形/重复弹道Node十链；旧双人Node两链；新四人/八人Node两链。八人反转全部准入提交、Join与终局顺序。运行时移走包内SDK回归配置/examples/samples，配置、账户、数据库和工作目录均在包外。 |
| 离线源码 | [源包报告](../../artifacts/sdk-5-19/delivery/source-verification.json)：解包后仅依包内vendor，全新CARGO_HOME和离线模式，默认/全特性同1209项及Clippy通过。 |

边界测试中修正的是验收数据的旧物品字段名/容量，以及NPC受击的观察时点和未sent时的实际错误码；最终报告以实际行为断言签收，不以构造8条记录代替成员闭环。

## 交付与保留范围

[0.21.0 aarch64-apple-darwin本地包](../../artifacts/sdk-5-19/delivery/match-kernel-0.21.0-aarch64-apple-darwin.tar.gz)包含普通二进制、源crate/vendor、旧版消费方、新Node/多页面H5与四人/八人数据、协议文档和摘要。源码、二进制和报告的同源绑定以[签收JSON](../../artifacts/sdk-5-19/acceptance.json)为准，不据此宣称其它平台或生产环境已验收。

新预览为[4193多成员验证台](http://127.0.0.1:4193/)，使用独立`.local/sdk-5-19-demo.sqlite`；[只读冒烟](../../artifacts/sdk-5-19/local-preview.json)只做GET，不创建会话。旧配置/来源包及SDK-5-17/18仍存在的归档、日志、报告与解包内容按实现起点清单逐字节保护。SDK-5-17已记录的用户清理例外不撤销，不恢复或伪报复核缺失旧档。

本批没有运行原48场景/50数值向量，没有关闭完整32类缺口或寻找第二款游戏。R2.2新增固定完整名单1～8人PVE容量切片，完整匹配/机器人与模式仍开放；R2.3仍只提供私有全量，增量和旁观另批。
