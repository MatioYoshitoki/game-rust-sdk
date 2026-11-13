# SDK-5-94 来源场景执行映射·生命周期与输入核心

DELIVERY-06 / R6.2（I05 场景侧）首批。v1.1 资料 `scenarios.json` 冻结 48 个场景（状态 specification-only-no-kernel-executed）与 4 个类型化夹具。本批把生命周期与输入核心的 12 个场景经真实协议 6 公共路径（`TwoMembers` 边界：create/input/business/transport/close/tick/drain）执行并逐条断言 `then` 子句；期望值全部从场景 JSON 读取，不落源码。

夹具映射：`plain-active`/`deploying` 的参与者属性（1000/100/100/0/8、半径 16、种子 7）、边界 [-1000,1000]²、撤离开放 180000ms、部署期限 30000ms、步长 30ms、关闭环境再生与 AI。成员事实经真实准入链（LoadAssets → AssetsLoaded）注入，宿主账本为带日志的合成 Store。

| 验收 | 可判定结果 |
|---|---|
| S01 全部就绪只激活一次 | deploying 起步（A 就绪、B 收到全量未就绪），B CombatReady 后推进 30ms：phase=active、Activated 事件恰 1 次、重放同一输入不二次激活。激活时刻为登记分歧：内核随就绪输入内联激活（0），源 oracle 期望下一固定步（30）。 |
| S02 部署扣费失败不给物品 | 挂起 admit-A 后回执 refused：A 库存为空、无 Activated。相位为登记分歧：内核同步完成中止收尾（closed），源域称 draining。 |
| S03 缺席部署超时 | deploying 推进 30000ms：无 Activated，中止收尾同 S02 的相位映射。 |
| S04 普通移动固定步 | Move [1,0] 推进 30ms：位置 [5.4,0]（容差 1e-6）。 |
| S05 非有限输入不部分写入 | NaN 方向输入：INVALID_MESSAGE，位置不变；JSON 信封层 NaN 字面量同样拒绝。 |
| S06 保护停止旧移动 | Move→30ms→Background→Foreground→90ms：protected=true、位置保持 [5.4,0]、无待决移动意图，前台不复活旧移动。 |
| S07 墙体与位移预算 | 阻挡矩形 x∈[50,80]：推进 300ms 位置 x≤34（半径 16 接触），逐 30ms 步长不超过 5.4。 |
| S39 旧连接不能操纵 | A 重连至 2 代后，旧代 Move 与旧代 Leave 均 STALE_CONNECTION，位置不变、在场保持、代次仍为 2。 |
| S45 关闭后迟到提交只收尾 | 挂起结算 close→drain：提交 committed 后世界释放（worldLiveCount 0）、关闭后无世界变更、账本 committed、重复关闭幂等。 |
| S46 故障只封存本局 | 注入故障：m1 faulted、m2 保持 active，无旧内核回退。 |
| S48 100 次创建关闭回收 | 100 轮 create→activate→move→close→drain：每轮后活动世界、未决输出、未决操作均为 0；线程/任务回收由既有 Runtime 证据覆盖（本机数据目录缺失待复跑）。 |
| S08 独立伤害与治疗算术 | 已由 SDK-5-93 向量矩阵执行（numericVectorIds 全部命中），本批评审链不再重复。 |

S44（逐对局参数化输入队列）与 S47（依赖 S18/S40 驱动器的回放对比）登记待后续：S44 的 SDK 有界入口为 Runtime 信箱与适配器在途上限（工程常量），逐对局参数不在冻结范围；S47 随资源与发布场景驱动器补齐。S09～S38 分别依赖弹道/资源/撤离/模式驱动器，按后续切片执行。账本逐条登记 48 个场景状态。
