# 固定线形持续区域接入

crate0.19.0 / 机制34 / schema4 / 单人PVE。[需求与LF01～LF09](../batches/SDK-5-16-line-field.md)。

```json
{"mechanism":"line_field","id":"lasting-line","cost":12,"windup_ms":120,"recovery_ms":300,"cooldown_ms":1200,"damage_bps":7500,"length":260,"width":48,"warning_ms":300,"duration_ms":2400,"tick_interval_ms":600,"contact_policy":{"max_targets_per_tick":2,"max_total_hits":6,"repeat_interval_ms":600},"on_hit_target_statuses":["exposed"]}
```

两槽均可配置。必填字段如上，on_hit_target_statuses及on_commit_self_statuses可省略，引用/绑定沿已有规则。length为中心线请求长度0.1～10000，width为完整宽度0.2～256，几何必须有限；range/radius/speed、直接repeat_interval_ms和额外字段拒绝。周期/策略使用[已有区域调度](persistent-fields-v1.md)与[命中预算](contact-policy-v1.md)的界限，contact_policy在本机制必填且不能null。旧机制1～33拒绝line_field。

Node发送既有方向意图：

```json
{"kind":"cast","skill_id":"lasting-line","direction":[1,0]}
```

只接受有限非零方向。接受时归一化冻结方向/技能，提交取玩家当前位置并重验width/2的弹道通行足迹。实际中心线按连续圆盘扫掠在首个弹道墙或地图边界截断；起点非法免扣。胶囊在提交后固定，不跟随玩家或外部配置。与ground_circle/ground_field共享最多2份区域计划；灵力/self整组容量/区域容量原子检查，成本/CD仅提交一次。

每周期以目标当前圆形足迹检查距离线段≤width/2+目标半径、最近点至目标中心弹道视线。端帽/侧边等值合法，按路径投影进度、距离平方、ID/代次稳定排序。成功直接命中才消费额度并写最新成功计划时刻；0每身份一次，正间隔独立于检测周期；离开再入不清历史、错过不补伤害，新代独立。ATK提交冻结，DEF/盾/闪避逐次读取，状态/击杀/掉落沿原链。

首跳在提交+warning+tick，计划时刻严格小于截止；到期/预算耗尽/来源死亡/终局/关闭/实际换轮结束一次。移动/取消/后台及同World重连保留已提交区域和历史；重启仅恢复冻结业务责任。

沿PlayerFieldStarted/Tick/Ended、PveView.player_fields。线形视图新增line:{start,end,width}，是Rust冻结的实际截断范围；center=start、radius=width/2。客户端使用line绘制胶囊，不能仍按center/radius画完整圆形或自行判断伤害。contact_policy提供总余量/历史/下一资格，旧1～33及圆形字段省略line，公开DTO源码消费需适配新增可省略字段。

```sh
node adapters/h5/server.mjs --port 4189 --configs examples/line-field-host/catalog.json --account examples/line-field-host/account.json --state .local/sdk-5-16-demo.sqlite
node adapters/node/asset-host.mjs --binary target/debug/kernel-host --configs examples/line-field-host/catalog.json --account examples/line-field-host/account.json --extraction
```

H5点选/触摸方向，Q或主技能按钮施放，2或副技能按钮自身状态；页面只绘制Rust范围与发布事实。Node参考宿主不驱动tick或计算伤害。样例是机制验收数据，接入方维护自己的配置与资产适配。
