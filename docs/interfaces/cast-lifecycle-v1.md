# 通用施法阶段组合（机制105）

配置在会话创建前装载、校验并冻结。Rust拥有阶段、费用、目标重验、移动、脉冲和冷却；Node只提交输入并转发权威快照/事件。以下字段均为可选扩展，schema7、协议6。旧配置的锁定引导、提前松手取消、弹道引导端点排除和资源准入上限保持原行为。

## 移动与后摇

`cast_movement.preparation/recovery`沿用free、slow、locked；引导可另配`active`，缺省locked。slow的speed_bps为1～9999。准备和活跃阶段还允许：

```json
{"kind":"interrupt_on_new_move","min_turn_degrees":15,"min_destination_distance":4}
```

转向阈值1～180度、目的地变化阈值大于0且不超过10000世界单位。相对当前阶段开始时的移动方向/导航终点检测累计变化，预持、keepalive和阈值内噪声不打断。有效新移动接管；无效导航、非有限方向和被控制拒绝的输入不取消动作。Stop或零方向只回中；回中后的下一次有效非零输入属于新移动。准备中断不扣费/CD，活跃中断保留已有结果，只停止未来跳。

可选顶层`cast_limits:{"mobile_fixed_windup_max_ms":200}`限制方向/地面技能的自由移动固定前摇；范围0～3600000ms。长准备应显式选择slow/locked/interrupt或真实蓄力。自身、单位锁定和蓄力不归此项，世界锚等空间投放另按计划契约验证。

所有既有成员技能均可配置`recovery_cancel:{"after_ms":0,"by_move":true,"by_explicit_action":true}`。after_ms沿固定步校验，须给后摇保留至少一步。by_move缺省false；by_explicit_action缺省true。显式扩展支持有效移动、显式取消、新技能与合法普攻接管；新动作先完成目标/费用/CD等准入，再结束旧后摇。失败不提前结束旧后摇。引导的取消时钟从真实终态起算；一次输入作用于到达时的阶段，取消活跃引导后进入的新后摇由后续输入取消。权限和时钟随原后摇保留到重连。旧仅after_ms的配置仍限定原普通弹道接管。

## 松手、排队与独立前摇

charge可选`early_release:{"kind":"queue_until_min"}`和正整数`release_windup_ms`。提前松手排到最短准备完成，再经历松手前摇；手动/自动松手都冻结方向与有效蓄力量。前摇不增加威力，也尚未扣费，可沿原准备中断入口取消。重复松手/重瞄准拒绝。

有独立前摇时，各准备时长分别ceil到固定步；总自动提交时长不超过5000ms。原350/1400/100ms在30ms步下对应最短释放360ms、满蓄1410ms和前摇120ms；提前松手在480ms提交，按350ms有效蓄力取75%，自动在1530ms提交，按1400ms取100%。原有效蓄力曲线仍用配置原量，取整不增加威力。

私有`pending_action.charge`可包含`released_at_ms`及实际`release_windup_ms`，`commits_at_ms`是最终承诺时刻，`auto_commit_at_ms`包含前摇。客户端不能仅凭提交时刻推断仍可编辑；已松手即锁定。Rust和共享Node/H5解码器核对各时刻、方向和原子增量。

## 引导预算与明确跳数

channel保留duration_ms、interval_ms、spirit_per_pulse，可加`pulse_count`（1～128）。明确跳数时duration必须等于interval×count；interval至少一固定步，不要求整除固定步。第k跳期限为提交起点+k×原间隔，实际执行向上落到固定步，延迟不滚入下一跳；最后期限等于持续期。旧未声明跳数的弹道引导保持端点排除规则。

self_status可声明channel.recovery，并必须声明pulse_count；不能同时再配即时self_heal。不创建HOT或弹道。示例：

```json
{
  "duration_ms":5000,"interval_ms":1000,"spirit_per_pulse":0,"pulse_count":5,
  "recovery":{"spirit":{"fixed":0,"source_attack_bps":0,"target_max_spirit_bps":1000}}
}
```

recovery.hp为fixed、source_attack_bps、target_max_hp_bps；spirit将最后一项换为target_max_spirit_bps。至少一项公式，fixed≤1000000，ATK比例≤100000，目标最大属性比例≤10000，禁止全零。提交时冻结总预算；recovery.remainder_policy可选earliest（缺省，余数依次给最早几跳）或final（整除余数全部留给最后一个计划跳）；原skill-roster的equal_budget_share使用final。提前结束不补发末跳余数。每跳重读受疗、缺口、有效上限和回灵封锁；余量不补发、不重复放大，不产生未声明的过量转盾。周期费用先原子支付，失败不执行本跳。

active_channel可带total_pulses；恢复引导另带recovery:true、direction:[0,0]，不伪造弹道方向或暴露冻结预算。私有channel_recovery_pulse报告action_id、pulse、spirit_cost、raw_hp/raw_spirit、hp_restored/spirit_restored、scheduled_at_ms/executed_at_ms。取消、控制、断线及退场只清理未执行部分，不补发历史跳或结束奖励。

## 资源准入配置

顶层可选：

```json
{"resource_limits":{"max_hp":1000000000,"max_spirit":1000000000,"max_spirit_cost":8000000}}
```

基础HP/SP上限1～10亿，单次灵力费用上限1～800万且不超过SP上限；缺省均100万。仅改变准入边界，基础ATK/DEF/SPD及资产边界保持各自原限制，不赠送资源。成员Load/Admission、初始值/NPC基础资源、主/附加/NPC技能、引导追加费用、再施放费用和固定HP费用使用对应边界；临时有效上限仍沿原聚合与钳制。

NPC 技能提交先重验当前灵力、效果容量和施法拦截。成功后从当前灵力扣除费用，再施加提交状态。如果状态降低有效灵力上限，钳制扣费后的余额。提交被拒绝时保留原灵力和冷却，也不投放提交状态。

两套演示配置为casting-four/casting-eight，配置、账户、UI和公开验证驱动均在既有combat样例中。原始生命周期/费用投影见src/verification/source_cast_lifecycle_vectors.json；绑定的是阶段和预算，不代表原技能所有空间、真伤、熟练阶段及投放已签收。片段Schema见[cast-lifecycle-v1.schema.json](cast-lifecycle-v1.schema.json)，交叉引用与时序约束仍由Rust和共享解码器完成。

SDK 0.102.0：`self_status.commit_on_accept: true` 只接受 `windup_ms: 0` 且没有蓄力或引导的自身技能。通过已有输入与资源、状态容量检查后，当刻使用同一原生提交事务。重复请求不重复扣费、不重复投放；开启前仍由固定步提交。声明仅由可信配置加载。
