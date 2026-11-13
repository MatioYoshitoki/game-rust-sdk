# SDK-5-93 来源算术向量矩阵

DELIVERY-06 首批，覆盖 R6.2 的 I05 算术向量执行映射。v1.1 资料 `numeric-vectors.json` 冻结 50 条算子向量（tolerance 1e-9）。已执行的 7 条（`session_cultivation`×4、`hp_cost`×3）沿用原验收；本批把其余 46 条中可在当前机制上表达的 44 条经真实内核路径绑定，`pvp_multicast`×2 归论剑模式结算（M12/R6.2 具名编排）继续跟踪。

| 验收 | 可判定结果 |
|---|---|
| 伤害乘区 | `defense_damage`×7 经真实命中链断言（scale=2、cap=0.7、minimum=1 与行为文档 §3.6 算例一致）：基础 66、破甲 80、封顶 30、负甲 100、不可减免 100、暴击 100、下限 1。`realm_modifier`×3 经境界差乘区表断言 1/1.2/0.25。`dodge_percent`×3 断言 `min(cap, max(0,dodge−penalty)/(1+hit))` 的权威概率 30%/50%/10%。 |
| 移动与步进 | `movement_speed`×6 以 {base 180, bonus 120, reference 8, log_base 100} 配置驱动真实移动，断言 0/180/180/240/300 与 1.1 倍率 330；rank 倍率经内核倍率通道表达，单位类型常数属宿主配置。`move_step`×2 断言真实单步位移 5.4 与对角归一 [3.24,4.32]。 |
| 周期与恢复 | `duration_ticks`×5 中 150/30 经真实周期状态断言 5 跳；100/100 与 1/100 在内核域外（时长必须严格大于跳间隔），断言配置拒绝；101/100 与 151/30 命中 SDK-5-6 冻结的 `[start,end)` 截断与来源 ceil 的分歧，按冻结值断言并登记待裁定。`heal`×4 经真实治疗链断言 `adjusted_heal`/`hp_restored`：封顶 50、常规 120、零倍率 0、允许溢出 120（应用封顶与溢出转盾为声明的策略通道）。`orb`×2 断言按 maxHp/maxSpirit 比例的 30/30 与 150/120。 |
| 死亡与护盾 | `death_slots`×4 与 `death_quantity`×3 经真实死亡掉落断言格 ceil（0.5→1、1.5→2、4）与数量 floor（0.5→1、2.5→2）。`shield_grant`×2 经共享上限断言余量 50 与 0。 |
| 账本与门禁 | `docs/checklists/SDK-5-93-source-vectors.json` 逐条登记 50 条向量的绑定、断言与未执行原因。Rust 合成套件、格式、Clippy 全目标全特性通过；带 `MATCH_SDK_TEST_DATA_DIR` 的完整编译与新增测试通过。历史夹具文件缺失不因本批关闭。 |

`pvp_multicast` 的分摊规则未在行为文档成文，随 S32～S38 论剑具名编排复验。向量为算术 oracle：绑定声明内核权威算术与事件，完整玩法语义仍按原 M 类工作包验收。
