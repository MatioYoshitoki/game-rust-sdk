# SDK-5-98 训练模式房主有限管理机制（已交付，0.186.0）

2026-11-13。来源：宿主训练房需求（training-sanctum.DESIGN v2.3 §4 转发件），宿主已确认提交通道。机制版本 **106** / schema 7 / 协议 6，交付版本 **0.186.0**。实现：受信宿主 RPC `reset_training_cooldowns` / `restore_training_member`（kernel-host Command + runtime/session/coordinator 管道）；`members.training_tools{enabled, restore_clears}` 配置；私有事件 `TrainingCooldownsReset` / `TrainingMemberRestored{participant_id, hp, spirit, cleansed[]}`；净化语义 = 内核 `harmful` 分类（clears 为空清除全部有害状态，非空为显式 id 过滤）。功能验收（真实协议栈驱动）：开关拒绝（TT01）、CD 重置与席位隔离（TT02/03）、恢复满血满灵（TT05）、位置保留（TT06）、死亡恢复 INVALID_STATE（TT07）、request_id 幂等（TT08）；Rust 单元 5 项（门禁/重置/恢复净化/正面保留/死亡拒绝）。

## 0. 结论速览（对照宿主三动作请求）

| 宿主请求 | 内核现状 | 本批动作 |
|---|---|---|
| set/clear_training_targets | **已存在**：注册成员动作 `SetOwnedTargets{preset_id}` / `ClearOwnedTargets{}`，每席预设白名单（1~8 组 × 1~16 目标）+ 落点足迹校验 + 木桩/巡动行为 | 零新机制即可上线（预设分组方案）；"运行时按模板新增实例到宿主候选点"为可选扩展，延后 |
| reset_cooldowns | **不存在**（`ResetTrainingMetrics{}` 只清统计窗口） | 新增受信宿主 RPC |
| restore_member_state | **不存在**（复活恢复 `restore_member_life` 含实体换代+换位，不满足"位置不动/在途保留"） | 新增受信宿主 RPC |
| training_tools 开关 | 部分存在（`training_targets` 本身即训练隔离面） | 新增 `members.training_tools{enabled}` 配置 + 动作门禁 |

## 1. 既有训练机制面（机制 105 已交付，SDK-5-91 批）

配置入口 `members.training_targets[]`（每席一份）+ `members.training_runtime{respawn,metrics}`。训练校验强制隔离面：protocol 6 / 机制 105 / 必有 combat_rules / **禁** `world_loot`、`world_boons`、`settlement_rewards`、`settlement_facts`、`settlement_progression`、checkpoint、participant_ai / `entry_years=0`、`carried_quantity=0`、`cultivation_coefficient=0` —— 与宿主"无结算、无掉落、无资产路径"完全同构（[training_targets.rs](../../src/config/training_targets.rs)）。

已有注册成员动作（协议 6 Action）：

- `SetOwnedTargets{preset_id}` / `ClearOwnedTargets{}`：从本席预设组启用/停用目标；变更时对落点做足迹+遮挡校验（`grid.clear`），失败整组拒绝
- `ResetTrainingMetrics{}`：清统计窗口（`training_runtime.metrics{window_ms,publish_interval_ms}`，窗口内 DPS 事实由内核发布 `TrainingView`）
- 预设：每席 1~8 组 × 每组 1~16 个**已在目录注册**的目标 npc id（`monster_positions` + `extra_spawns` 实例），目标跨席唯一归属

## 2. 附带问题答复（代码事实）

### Q1 成员重入语义

- 成员名单在 create 冻结（协议 6 必需名单 1~8）；**不存在离场再入场**。
- `Action::Leave`（激活后，训练模式）→ `withdraw_member` → **`terminal_intent(Outcome::Death)`**：判负式终局，成员进入 settling → 发出零金额 `CommitSettlement` → 本局对该成员结束；terminal 后 `Join`/`Foreground` 一律 `InvalidState`。
- **战斗死亡**在配置 `training_runtime.respawn{delay_ms,points[]}` 时**不终局**：`delay_ms` 后在对应点位 `restore_member_life`（满血满灵、实体换代使旧代在途效果失效、普攻时钟与后摇清零、事件 `ModeRespawned`、`TrainingView.respawn_at_ms` 对外发布）。复活时技能 CD 存储不被触碰、状态按实体代键随换代失效——**"复活后 CD/增益口径"正是本批要在规格中显式钉死的边界**。
- 结论：宿主"重入已可覆盖单人恢复"的假设**不成立**——Leave 即终局，重入唯一形式是重开对局；**动作 2 必要**。
- 统计：成员终局后其事件流停止；宿主侧事件流聚合不受内核影响 ✓。

### Q2 木桩表达

**已是一等机制，无需新做**：`behaviors` 的 `Dummy{reset_on_defeat, motion{end,speed}}`——`reset_on_defeat: true` 即"死亡原位恢复"（`replace_training_npc` 同位重建、代次递增、旧代战斗事实链关闭）；`motion` 为可配置沿线巡动（路径连通性在 prepare 校验）。死亡事实抑制 = 重建走代次换代而非死亡终局链，天然不触发击杀结算。推荐：**用现有编排表达**，本批不新增专门机制。

### Q3 无结算终局形态

`result_contract` + `asset_contract` 在 schema 7 为**必填**（训练配置不能删除结算契约），但训练校验强制零入场/零成长/无奖励表。因此成员死亡或 Leave 的输出形态 = **正常 Defeat 流水**：伤害/死亡事件 → 终局意图 → `CommitSettlement`（借贷全零）→ 宿主回执 → Draining/Closed。**最小默认语义已存在**：宿主照常回执即完成闭环，"副作用隔离由宿主保证"成立。配置 respawn 时战斗死亡改走复活不终局；`Leave` 仍终局（见 Q1）。

## 3. 新增机制规格（本批实施项）

### 3.1 受信宿主 RPC（非成员 Action）

沿用 `set_skill_loadout` / `set_equipped_weapon` 先例：以 kernel-host `Command` 注册，成员输入通道**不注册**这两个动作（成员客户端无法伪造；宿主侧无需过滤）。内核不引入"房主"概念。

#### `reset_training_cooldowns`

```json
{"type":"reset_training_cooldowns","request":{"key":…,"targets":"all"|"participant_id":"p0",
 "request_id":…}}
```

- 效果：仅清除目标成员的战斗技能与携带神通 CD 存储（含 `resource_commit` 预留状态的释放语义按"重置"处理：预留作废、灵力不返还——预留是已提交成本，重置 CD ≠ 退款）
- 保留：普攻时钟、后摇、读条/施法中的 pending、在途投射物/区域/锚点、灵力当前值、位置、状态
- 之后施放仍校验全部正式条件（灵力/目标/禁制/预留）
- 幂等：`request_id` 幂等；重复提交无副作用
- 门禁：仅 `training_targets` 配置 + `members.training_tools.enabled=true`；否则 `UNSUPPORTED`

#### `restore_training_member`

```json
{"type":"restore_training_member","request":{"key":…,"targets":"all_alive"|"participant_id":"p0",
 "request_id":…}}
```

- 效果：hp/spirit 补至当前最大值；清除**有害状态**（内核 `StatusDefinition.harmful` 分类：控制/减益/periodic 伤害等自动归类，正面增益永不触碰）；可选 `restore_clears` 显式过滤清单（机制 87 语义）；私有事件 `TrainingMemberRestored{participant_id, hp, spirit, cleansed[]}`
- 保留：正面增益、CD（与 reset 正交）、位置、普攻时钟/后摇、在途技能实体、统计
- 死亡成员：`INVALID_STATE`（死亡恢复由 respawn 或重开覆盖；不复活）
- 门禁/幂等：同上

### 3.2 配置

`members.training_tools{enabled: bool}`（可选，默认缺省=关闭）。校验：仅训练配置（`training_targets` 存在）允许该字段；非训练配置出现即 `CONFIG_TRAINING_TOOLS`。

### 3.3 明确不做（与本批冻结）

宿主任意写 World、通用 method/args、复活死亡成员、清除正面增益、修改统计、运行时按模板新增实例（可选扩展另批，先以预设分组覆盖）、`max_alive`（预设 16/组 + 每席预设数已构成上限；运行时生成方案引入时再配）。

## 4. 验收范围（TT01～TT12）

| # | 验收项 |
|---|---|
| TT01 | 非训练配置提交两 RPC → `UNSUPPORTED`；`training_tools` 缺省关闭 → 同 |
| TT02 | 开启后：单席/全场重置 CD，快照 `player_skills` 就绪时刻归零 |
| TT03 | 重置后施放仍校验灵力不足/禁制 → `NO_RESOURCE`/`ACTION_REJECTED` |
| TT04 | 重置不动普攻时钟（连发间隔不变）、不动后摇、不动在途弹道（弹道照常命中） |
| TT05 | 恢复：残血/空灵 → 满；负面/控制清除；正面增益保留（数值不变） |
| TT06 | 恢复不动 CD 与位置；在途技能实体保留并照常结算 |
| TT07 | 死亡成员恢复 → `INVALID_STATE`；respawn 配置下死亡成员按 respawn 语义处理 |
| TT08 | `request_id` 幂等：重复提交零新增事件 |
| TT09 | 目标预设：`SetOwnedTargets`/`ClearOwnedTargets` 切换含落点校验拒绝与木桩重建 |
| TT10 | 零结算闭环：成员 Leave/死亡 → 零金额 `CommitSettlement` → 回执 → `Closed` |
| TT11 | respawn：死亡 → `TrainingView.respawn_at_ms` → 满状态复活于点位 |
| TT12 | metrics：窗口发布 + `ResetTrainingMetrics` 清窗 |

## 5. 交付形态

机制 106 / schema 7 / 协议 6；`sdk_info` 新增 `trainingTools` 能力键；版本 0.186.0；Node 参考入口 + H5 训练房页签（工具条三按钮）；验收记录 + gap 矩阵/roadmap 更新；新 release tag（遵守发布纪律）。
