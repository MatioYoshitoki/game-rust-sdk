# 双成员契约 v4（SDK-5-18）

本契约在实现前冻结。协议4/schema5/机制35对应同一World的两名必需成员，具体白名单和TM01～TM13见[批次规格](../batches/SDK-5-18-two-member-loop.md)。协议1～3的载荷保持原样。配置必须声明`two_members`，缺字段、未知字段、非法容量/出生点及白名单外玩法拒绝；不得降级成两份单人世界。

六个Client入口保持名称：create接受Create或CreateV4；submit_business_receipt接受旧回执或带participant_id的MemberBusinessReceipt；其余沿Input/TransportReceipt/Close。JSONL仍使用原type标签，Rust通过Into类型兼容旧调用。OutputBody新增有类型v4变体，Rust下游穷尽匹配须加入新变体；旧协议输出不出现它们。

CreateV4为`{protocol_version:4,key,request_id,config_id,config_hash,seed,participants:[{participant_id,authenticated_principal,connection},…],publication_mode?,recovery?}`。participants恰两项且身份/主体/连接唯一，按participant_id排序的有界成员表确定模拟遍历顺序；同刻已接纳命令按同一有界邮箱的FIFO顺序尝试，先接纳的资源领取先裁决。创建幂等比较原载荷，不把重排当作同一请求。配置`two_members.spawns`按稳定成员顺序分配。版本/schema不匹配在任何读取/扣费之前拒绝。

MemberBusinessReceipt为`{key,participant_id,operation_id,attempt_id,outcome}`，结果复用原AssetsLoaded/Committed/NotCommitted/Unknown/Reconciled契约。每成员金融账本使用按MatchKey与成员身份哈希派生的稳定内部Key，公开请求仍携带原MatchKey与明确participant_id/principal；该哈希是无歧义身份编码，不是认证。费用、物品、结果和修为全部由Rust冻结；Node只执行事务。

新增OutputBody：

- `business_request_v4 {participant_id,principal,request}`：原BusinessRequest及其原子资产事务，只有可信宿主消费。
- `publication_v4 {recipient_id,connection,snapshot}`：snapshot包括`phase/logical_ms/active_at_ms/self/members/monsters/resources/projectiles/extraction_point/extraction_open`。self包含本人SP、动作/CD、库存revision、状态、结算状态；members只有EntityRef、公开位置/HP/阶段/保护，不含私有字段。全部是全量快照。
- `event_v4 {recipient_id,event}`：recipient_id有值的事件只交给本人。封装原成员事件（包括资产金额/operation身份）始终有recipient；公共战斗事件不含这些字段。宿主按Rust指定接收者发送，不遍历World筛字段。
- `responsibility_v4 {checkpoint}`：仅可信宿主存储的聚合冻结责任，严禁发浏览器。一个输出批次必须先把全部输出和最新checkpoint同事务持久化，再执行其中的外部业务和传输。

EntityRef为`{kind:participant|monster,id,generation}`，ParticipantRef为`{entity,owner_id}`；归属不由连接猜测。Input按成员分别校验连接/代次、序号和request_id；相同request_id/sequence跨成员合法。重连撤销本人的旧epoch和ready，须本人新连接全量sent后重新ready，不改变其他成员。

发布采用每接收者独立有界队列（`publication_queue_capacity`）和有界回执历史。未排空的旧全量可以被同接收者最新全量替代，不丢给另一人；传输回执引用已实际drain的output_id、recipient、当前连接和epoch。enqueued不推进sent基线；失败只为本人生成替代。可用于ready的是当前epoch已sent且仍在有界历史中的全量；共同激活要求双方均ready，不因更晚的在途发布使已sent资格失效。

恢复载荷为`{generation,checkpoint:{sha256,state}}`；state包含整体创建身份摘要、冻结逻辑/输出序号、恢复代和恰两份带participant_id的原金融检查点，不含位置/临时战斗状态。校验整体/成员摘要、所有者、配置、原操作、金额、来源上限和两人聚合数量后，仅建立Closed责任账。无持久结算的已扣费中断成员按原单人责任策略补偿入场；已有冻结结算必须保留原金额并查询/收尾。恢复两次不得重扣/重奖；摘要用于损坏检测，不授权特权宿主改账。

World激活只有一次；成员settling与terminal分开，核心committed前不发布撤离成功。另一人的IO/结算不暂停模拟。两人均退出可操作集合或可信close/绝对终局时，冻结所有责任并释放唯一World；金融账本保留至外部责任明确收尾。重启不恢复战斗现场。

协议4聚合检查点的sha256使用递归按键序排序后的紧凑JSON UTF-8编码，数组保持顺序；Node可以独立核对。内部各成员schema1检查点仍由Rust按原有类型序列化规则核对，宿主原样保存/转交，不能用其JSONL字段顺序重新计算。
