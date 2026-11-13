# 严格地面目标与单次圆形范围接入（SDK-5-11）

crate0.14.0 / 机制29 / schema4 / 协议3 / 单人PVE，[GC01～GC09范围](../batches/SDK-5-11-ground-circle.md)。主/副槽均支持，最多两个技能。所有数值及合法self/hit状态绑定由外部配置初始化，无地图/技能ID分支。

```json
{"mechanism":"ground_circle","id":"ground-burst","cost":12,"windup_ms":120,"recovery_ms":300,"cooldown_ms":1200,"damage_bps":10000,"range":500,"radius":48,"warning_ms":900,"on_hit_target_statuses":["exposed"]}
```

不允许speed等无效字段。range0.1～10000，radius0.1～128，warning_ms0～60000；成本/前后摇/CD/倍率及状态引用沿原两槽约束。旧机制配置新类型启动拒绝，未知字段（包括null）也拒绝。

```json
{"kind":"cast_ground","skill_id":"ground-burst","position":[450,400]}
```

position为两个有限世界坐标，不是屏幕坐标或方向。落点必须在地图[0,width×cell_size)、[0,height×cell_size)且所在格可行走；允许脚下点，不要求整个圆在地图内。接受/提交两次检查同一冻结落点，距施法者当前中心≤range且Grid视线无遮挡；不自动裁剪、吸附或导航。结构/非有限值INVALID_MESSAGE、越界/不可行走INVALID_TARGET、超距/视线OUT_OF_RANGE、未知技能/旧机制UNSUPPORTED、意图类型不匹配INVALID_MESSAGE，原ready/保护/准备/后摇/CD/资源门禁保持。

接受冻结技能/点和遭遇轮次，不扣费。准备期间移动保留准备；提交再次校验当前位置，失效发ActionRejected且免扣。当前灵力、整组self状态及每World最多2个未结算计划原子预检，失败不施加self状态或扣费。成功施加self状态后冻结有效ATK比例量，开启独立CD、共享后摇，发布固定中心预警。0前摇下一固定步提交；0预警再下一固定步才结算。

机制29的pending_action和地面ActionCommitted带可省略ground_position。PveView.player_area_warnings为有界数组，项目含action_id/skill_id/center/radius/started_at_ms/resolves_at_ms。PlayerAreaStarted{warning}、PlayerAreaResolved{action_id,skill_id,hits}和PlayerAreaCancelled{action_ids,reason}可观察计划；hits为实际直接接触数（闪避不计，护盾全吸收的0HP伤害接触仍计）。到期沿怪物稳定索引，以当前存活/启用/非归途/非复位且原轮次敌人为候选，中心距离≤技能半径+单位半径且弹道无遮挡才进入既有命中链。每敌每计划最多一次，空圆也只结算一次，移动进出可躲避。冻结攻击量、当前DEF/盾、原闪避/取整/击杀/掉落与合法hit状态沿原规则。

CancelAction/后台/死亡/终局/关闭/实际换轮可取消未提交准备且免扣。提交后移动/取消输入/后台/同World重连保留预警及CD/后摇、不退款；死亡/终局/关闭/实际换轮清空未结算计划并聚合通知。复位倒计时的敌人暂不可命中，实际换轮才清计划。进程重启只履行原冻结资产责任，清除临时点/预警/准备/状态，不重算旧金额或恢复World。旧1～28省略新字段，原NPC圆形预警的成本与攻击量时序保持。公开Action/事件/视图为Rust源码变更；Client六类、协议1～3、资产和恢复schema保持。

```sh
cargo build --locked --features replay --bin kernel-host
node adapters/h5/server.mjs --port 4184 --state .local/sdk-5-11-demo.sqlite --configs examples/ground-skill-host/catalog.json --account examples/ground-skill-host/account.json
```

[完整配置](../../examples/ground-skill-host/arena.json)默认Q为地面圆形，2为自身护盾/HOT。H5先点选或触摸地图选择落点，再按技能；准备中换选只影响下一次。描边为当前选择，实心预警以Rust视图为准。Node参考只提交选定坐标，不自行判伤。独立交付samples/ground-targets可以复制到包外接入。

本批不含持续区域、陷阱、追踪、落点裁剪、友军效果、更多槽、引导/蓄力/再施放、多人或完整技能目录。
