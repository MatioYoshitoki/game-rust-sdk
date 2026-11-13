# 严格敌方单位目标接入（SDK-5-10）

crate0.13.0 / 机制28 / schema4 / 协议3 / 单人PVE。[UT01～UT10范围](../batches/SDK-5-10-unit-targets.md)。主/副槽均可使用，仍最多两个技能，可与方向弹道、自身技能组合。

配置mechanism=unit_projectile，使用原projectile的ID/成本/前后摇/CD、damage_bps/range/speed/radius和可选on_commit_self_statuses/on_hit_target_statuses。全部数值与已有合法状态绑定由配置初始化，沿原上限/引用/重复策略/容量严格校验；不允许配置追踪、自动换人或任意脚本。旧版本选择新机制启动拒绝。

机制28的MonsterView按存活/启用、非归途且非复位倒计时资格输出unit_target，例如：

```json
{"id":"status-target-2","hp":180,"unit_target":{"id":"status-target-2","generation":1}}
```

目标身份以当前World为作用域。所有当前发布怪物是敌方；代次沿状态实体生命周期，归位/实际复位递增，普通Boss阶段切换保持。必须使用完整身份，不能把旧代按同名自动绑定新代。

```json
{"kind":"cast_unit","skill_id":"marked-bolt","target":{"id":"status-target-2","generation":1}}
```

禁止客户端方向/位置/敌我/金额或额外字段。target的ID非空、有效且≤128字节，generation为非零u32；结构或零代次非法为INVALID_MESSAGE。在动作可接纳时，未知技能/旧机制新意图UNSUPPORTED，意图与技能机制不匹配INVALID_MESSAGE；非法目标/死亡/禁用/归途/复位中/代次失效/非敌方为INVALID_TARGET；超中心射程或视线阻挡为OUT_OF_RANGE。射程相等合法，方向重合无效按OUT_OF_RANGE。准备/后摇/CD/ready/保护/资源沿原门禁。

接受与提交两次重验同一身份和资格/射程/视线，准备中目标移动仍取提交时其合法当前位置与施法者当前位置计算方向。提交前失败发ActionRejected、清准备，不扣费/开CD/施加self状态或创建弹道。成功沿原原子灵力/整组self容量预检、成本/CD/共享后摇、有效ATK冻结及命中绑定执行；0前摇仍经下一固定步提交。

提交后方向固定，不追踪、不追击、不换人。弹速/射程预算/半径/TTL及地形弹道阻挡沿原机制；只允许原身份目标获得伤害/命中状态。别的存活单位先接触则只终止弹道，无伤害或状态；地形同距优先、单位同距沿稳定索引。目标死亡/归途/换代/实际复位会在下一步使严格弹道失效，不转移或退款。目标后续移动可躲开，视线在提交后不再重验，物理弹道阻挡仍生效。原方向弹道仍按原首接触造成伤害。

pending_action与单位ActionCommitted增加可省略unit_target，记录冻结身份；原skill布尔/成本、player_skills和主CD别名保持。UI改选不改变旧准备或在途弹道；取消/后台/死亡/终局/关闭/实际换轮、重连与冻结资产责任沿原规则。重启清除临时目标/技能/状态/World，仅履行原Create/hash/金额，不重算原责任。公开Action/Error枚举及视图可选字段是Rust源码变更，穷尽match和struct literal须适配；旧1～27省略新目标字段，六类Client/协议1～3/资产及恢复schema保持。

H5支持目标按钮和等距地图点选/触摸，高亮当前完整身份；目标失效清除选择，无选择时单位按钮禁用，必须重新显式选择。Node参考按Rust发布的身份提交意图。客户端不补算命中、成本、伤害或CD。

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4183 --state .local/sdk-5-10-demo.sqlite --configs examples/unit-skill-host/catalog.json --account examples/unit-skill-host/account.json
```

[完整配置](../guides/config-cookbook.md#player-skills)arena.json)默认Q为指定单位弹道，2为自身护盾/HOT；先选目标。两名敌人、状态ID和数值均是配置数据。独立交付的samples/unit-targets可复制到包外消费。

本批不含友军、单位瞬时治疗/效果、追踪、穿透/多目标、地面目标、更多槽位、引导/蓄力/再施放、HP成本、多技能AI、多人或完整技能目录。
