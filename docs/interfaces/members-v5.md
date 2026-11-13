# 配置化成员契约 v5

SDK-5-19先冻结的有限契约。0.21.0/协议5/schema6/机制36；[规格MC01～MC10](../batches/SDK-5-19-member-capacity.md)。所有人数来自配置，固定完整名单，无动态入席或自动机器人。

schema6必须有`members:{capacity,spawns,deployment_timeout_ms,publication_queue_capacity,projectile_capacity}`且不能有two_members。capacity为1～8，spawns恰capacity个，按成员ID稳定排序分配；全部合法且任意两足迹不相交。旧schema5/two_members/协议4仍严格两名。

CreateV5与CreateV4字段相同，protocol_version必须5，participants长度恰所选配置capacity。六个Client入口不增加；Rust通过CreateV5调用create，旧Create和CreateV4保持。业务回执、Input、TransportReceipt、EntityRef/ParticipantRef复用原类型。协议5有意沿用`business_request_v4/publication_v4/event_v4/responsibility_v4`输出及SnapshotV4/IoCheckpointV4结构，宿主按实际participants/recipient处理，不能把后缀v4解释为固定两名。Rust CreateRequest新增Members枚举分支，穷尽匹配需适配。

每个席位都按相同通用流程读取事实、提交原子成本和准入；全部入场成功、全量确实sent且ready后共同激活。失败、超时、关闭逐席位按原金融状态机收尾；输入身份/序列、私有发布/epoch、离线窗口、终局独立。公开成员/NPC/资源仍来自一个World；匹配参战机器人尚未实现，普通PVE NPC不占或替代成员席位。

恢复使用原CreateV5和原配置原文/hash，沿用schema1聚合/成员责任，成员数必须恰配置数且全部按稳定ID映射；聚合来源不能重复计分或造物。仅Closed责任账，无临时World。宿主保存整批输出和最新检查点后才执行I/O，未知结果查询原操作；关闭后未排空新输出不能宣称pending0。[原共用DTO和责任规范](two-members-v4.md)。

游戏能力白名单与SDK-5-18相同；本批只扩容量，不放行PvP、fields/piercing/unit/ground、shield/DOT/HOT、复杂NPC/遭遇/节点/容器、delta和观战。其它模式容量与负载另行验收。

协议5资产事实每成员最多32行，资产ID仍最多64 UTF-8字节；超限回执在应用前拒绝，检查点恢复同样验证。协议4的128行上限不变。声明上限内的最大责任使用普通256KiB JSONL入口恢复。
