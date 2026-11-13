# 成员技能所有权与可信槽位控制

机制105、配置Schema7、发布协议6支持独立的成员技能集合。`skill` 与 `additional_skills` 仍提供旧默认槽位；`skill_library` 声明额外定义，三者合计最多64项。库中的定义不自动授予。每个定义沿用现有技能机制、参数与验证规则；定义和数值随World配置冻结。

可信建局请求的 `participants[].skills` 可声明：

```json
{"owned_skill_ids":["old-bolt","learned-bolt"],"equipped_skill_ids":["old-bolt"]}
```

拥有集合最多64项，装备槽最多8项，可以为空；ID必须唯一、存在于当前配置，装备必须是拥有集合的子集。装备顺序就是自身快照的技能槽顺序。省略 `skills` 沿用旧默认，初始版本视为0；显式声明会在自身快照暴露 `skill_loadout_revision:0`。

游戏宿主负责确认授予资格，SDK执行可信变更。Rust入口是 `Client::set_skill_loadout`，JSONL入口如下；这条命令不属于玩家 `Action`：

```json
{"type":"set_skill_loadout","request":{"key":{"id":"match","incarnation":1},"participant_id":"P00","entity_generation":1,"operation_id":"loadout-change-1","expected_revision":0,"loadout":{"owned_skill_ids":["old-bolt","learned-bolt"],"equipped_skill_ids":["learned-bolt"]}}}
```

成功返回 `loadout:{operation_id,revision,equipped_skill_ids}`。Node 接入使用：

```js
await host.setSkillLoadout('P00', {
  operation_id: 'loadout-change-1', expected_revision: 0,
  loadout: {owned_skill_ids: ['old-bolt', 'learned-bolt'], equipped_skill_ids: ['learned-bolt']},
});
```

变更立即在当前已完成的逻辑时刻原子生效，普通输入与控制命令经同一有界邮箱串行处理；技能提交仍遵守固定步进。不同请求的先后决定同刻资格和准备取消，不会追溯撤销已经完成的提交。

期望版本不符返回 `REVISION_CONFLICT`，同一操作ID换内容返回 `CONFLICT`。操作ID在整个World内唯一，128条控制记录不淘汰，满后新操作返回 `CAPACITY`；精确重试仍返回原结果。输出背压返回 `QUEUE_FULL`，无任何变更。超时或发送结果未知时保留原操作ID与原内容重试，不新建操作ID。无效ID、非子集、重复、超限、成员已死亡/退出或World已关闭都不能授予新技能。

移除装备或所有权会取消对应的未提交准备、引导未来跳、二段机会、未提交连招及尚未使用的后摇取消窗口。已有后摇时钟与技能冷却债务保持；重新装备、遗忘后重学和重新排序都不能刷新它们。已支付费用不返还，在途弹道、区域和已投放状态按原身份、来源与生命周期结束。已经提交的实际接触与动作汇总沿原结算规则处理。

自身完整/增量发布包含当前槽位、冷却与版本，并只向自身发送 `skill_loadout_changed`。拥有集合和控制审计仅供可信宿主；不要把责任检查点转发给客户端。解码器拒绝版本回退、丢失版本及不升版本就改槽位，失败保留原基线。连接重连保持实际World状态；进程故障恢复沿既有关局补偿协议，审计按初始授予与控制记录逐项重验，精确旧请求不重放授予，也不恢复战斗。

H5示例用自身权威槽位生成菜单，空槽位仍可正常操作基础功能。示例服务默认不提供授予接口；验收程序可显式开启 `--trusted-control-stdin`，由本地可信父进程写入控制命令，HTTP玩家输入仍拒绝授予。示例：[四人配置](../guides/config-cookbook.md#combat-rules)loadouts-a.json)、[八人配置](../guides/config-cookbook.md#combat-rules)loadouts-b.json)。

[Schema](skill-loadouts-v1.schema.json)描述集合、请求、回执、私有发布投影与审计结构。技能定义本体仍由现有技能规范和SDK冷装载器完整校验；库与默认定义的总数、集合子集、版本与审计链也由SDK执行。
