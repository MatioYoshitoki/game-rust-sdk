# 多人 World 资源与容器 v1

SDK 0.142.0 起在机制 105、schema 7 和协议 6 支持宿主配置顶层 `world_loot`。堆叠和实例物品均使用 `pve.items` 登记。奖池、资源点、容器、所有者和玩法数值由宿主提供。

| 字段 | 契约 |
|---|---|
| `environment_capacity` / `container_capacity` | 环境和容器地面堆数的独立预算，各为 0～96。连同怪物掉落和主动丢弃的预留合计最多 128，加载时也检查输出队列容量。 |
| `partial_pickup` | 为真时允许拾取当前背包可容纳的数量。 |
| `interaction_range` / `shared_clearance` | 容器交互范围和共享容器周围的环境资源净空，使用 World 坐标。 |
| `pools` | 最多 32 个奖池，按 ID 供节点、容器和怪物模板引用。 |
| `nodes` / `containers` | 最多 32 个环境节点和 16 个容器，跨来源 ID 唯一。 |
| `one_shot_capacity` | SDK 0.143.0 新增。统一奖池怪物掉落的独立堆数预算，省略时为 0，最大 96。纳入 128 堆总预算与输出容量检查。 |
| `monster_pools` | SDK 0.143.0 新增。怪物模板 ID 到 `pool`、`priority_ms`、`correction_radius` 的映射，省略时为空。 |
| `instances` | SDK 0.144.0 新增。至多 32 个实例模板，按物品 ID 声明 `quality` 和 `relic_eligible`。 |
| `relics` | SDK 0.144.0 新增。死亡遗物的可选策略，声明 `capacity` 和 `interaction_range`。 |

## 奖池和补充

奖池的 `guaranteed` 固定执行。`independent` 各自按 `chance_bps` 判断，条目在 `entry` 中声明。`groups` 先按 `chance_bps` 判断，再均匀抽取 `count_min` 到 `count_max` 个条目，组内同一条目最多取一次。每个条目使用 `item_id`、`quantity_min`、`quantity_max`，两端均包含。概率为 0 或 10000 时结果确定。一个奖池最多输出 8 堆。

环境节点声明 `id`、`pool`、`points`、`replenish_ms`、`ttl_ms`。`points` 为 1～16 个候选坐标。可选的 `hotspot` 用 `group` 和 `radius` 优先匹配本局已放置的共享容器，按代轮转锚点。普通候选始终适用共享净空、角色足迹及从至少一个出生点可导航的要求。预算不足或几何暂时阻挡时，冻结抽样并按资源扫描节奏重试。每代全部拾取或过期后开始下一次补充计时。恢复账本以配置和运行时长计算供应上界。

## 怪物一次性掉落

绑定奖池的模板应将原 `drop_table.guaranteed` 和 `drop_table.random` 留空。模板和奖池必须存在，单次奖池的最大输出必须能装入 `one_shot_capacity`。恢复光球继续使用原规则。

每次有成员归属的击败生成一次抽样。散落候选距击败点的距离由 `params.drop_min_radius` 和 `params.drop_max_radius` 决定。候选受阻时在 `correction_radius` 内有限搜索，半径范围为 0～75。每批奖励使用同一个合法落点，并检查完整成员足迹及从击败点可导航。容量或几何暂时阻挡时，保留抽样和候选位置，按 `params.runtime.resource_sweep_ms` 重试。发布时起算 `params.resource_ttl_ms`，拾尽或过期后回收预算。

启用 `priority_ms` 后，贡献按各成员累计造成的实际生命损失计算，召唤伤害归召唤者。怪物回巢并重置时清空贡献。击败时从仍在场的贡献者中选最高者，同分按参与者 ID 升序。击杀计分沿用原归属。所有成员可见掉落，通过资源 `owner` 判断拾取优先权。优先期从发布起算，最多持续到掉落过期。优先者终局或优先期结束时，同一资源及余量对在场成员开放。暂时离线保护保留优先期。设为 0 时从发布起公开领取。

## 容器

每个容器声明 `id`、`position`、`pool`、`visibility`、`unlock_ms`、`replenish_ms`、`ttl_ms`。`visibility.kind` 可为 `shared`、带 `participant_id` 的 `personal` 或带 `reveal_radius` 的 `hidden`。私有容器仅向绑定席位发布。隐藏容器按接收者当前距离和视线发布，开启后的奖励仅向开启者发布。共享容器可设置 `hotspot_group`。

`open_chest` 和 `cancel_chest_unlock` 使用 `chest_id` 与 `expected_generation`。一名成员最多占用一个容器。`guardians` 可引用已配置的怪物实体 ID，存活守护者阻止开启。离开可交互范围、实际扣血、取消、保护和退场释放占用。`drop_points` 可提供候选落点，省略时使用容器位置。可选的 `cost` 使用 `item_id` 和 `quantity`，从当前已拾取库存扣除。提交时重验库存版本、成本、落点和地面预算，失败时保留库存和已经抽出的奖池。成功后进入补充冷却，刷新时增加 `generation`。

协议 6 的 `containers` 按接收者发布 `id`、`generation`、`position`、`state`。`state` 为 `ready`、`reserved` 或 `cooldown`。增量出现该字段时替换整个可见集合。容器的占用、释放与开启事件仅发给对应成员。自动参战席位通过同一动作申请开启。

## 部分拾取

`pickup` 使用原 `resource_id` 和本人 `expected_revision`。有可用叠层或空格时，可入包数量受实际容量限制。余量沿用同一个资源 ID，由 `resource_updated` 和增量发布，取尽后发布 `resource_taken`。个人拾取事件记录实际入包件数。版本冲突、满包或不可到达时，库存和资源均保持本次动作前的状态。重复请求复用原结果，结算按实际净拾取量入账。

## 实例物品与账本

在 `instances` 注册的物品必须在 `pve.items` 声明 `stackLimit: 1`。统一奖池中每条实例奖励的数量上下限均为 1。每次投放为该件生成本局唯一 `instance_id`，并从模板读取无符号 16 位 `quality`。同模板多件使用独立 ID 和独立背包格。实例模板用于统一奖池，常规入场物、旧掉落表、容器成本及堆叠道具消耗仍使用数量资产。

地面资源通过可选 `instance` 发布身份和品质。本人私有库存通过 `self.instances` 发布，领取时沿用 `inventory_revision`。`commit_assets` 的 `result_settlement.summary.instances` 按 ID 排序，包含结算保留的 `instance_id`、`item_id`、`quality`。宿主在同一资产事务中同时提交数量 `credits` 和实例，按原操作 ID 幂等。恢复验证局级实例身份、模板品质、库存容量和跨席位供应上界。

## 死亡遗物

`relics.capacity` 为独立于地面资源的 1～8 个遗物槽。`interaction_range` 为 0.1～1000 个 World 单位。可选 `excluded_items` 排除物品 ID，可选 `stack_qualities` 为堆叠物品指定摘要品质，未声明时为 0。结果策略使用撤离保留和死亡损耗。启用遗物后，死亡按 `params.death` 在本局获得的合格背包格中抽选。实例还需声明 `relic_eligible`，被抽到时整件转出。携带入场数量和外部装备由原账本保留。

遗物在结算冻结前落在死者位置，检查完整角色足迹。合法时一次提交遗物和库存转出。位置或容量失败时用原库存完成死亡结算。遗物中的物品属于已从死者净拾取转出的数量或实例，接收者领取后才进入其账本。遗物无 TTL，每位死者至多一个，清空后仍保留至关局。

协议 6 的 `relics` 按全量集合发布位置、死者实体、修订、剩余堆数和最高品质。堆数为 0 时表示空匣，品质为 `null`。合法近距内使用 `open_relic` 和 `relic_id`，通过私有 `relic_opened` 获取内容与 `entry_id`。`claim_relic` 接收 `relic_id`、`expected_revision` 和 `expected_inventory_revision`，可选 `entry_id` 用于指定条目，省略时领取全部可装入的内容。动作重验距离、通路、授权与修订，成功只转移实际入包量并私发 `relic_claimed`。短线重连后须重新开启。进程恢复继续处理原冻结结算。
