# SDK-5-3 通用ATK/DEF修正验收（2026-11-04）

先冻结[AT01～AT10](../batches/SDK-5-3-attribute-modifiers.md)，再实现0.6.0/机制21。有限范围全部通过；[逐项映射](../checklists/SDK-5-3-acceptance.json)和[机器汇总](../../artifacts/sdk-5-3/acceptance.json)绑定本批源码、日志、版本与交付包。

## 已交付行为

- timed_attribute_modifier显式选择atk/def，两者共用状态实例、refresh/ignore/stack、来源分组、容量原子性、一次取整与生命周期。ATK有独立配置正帽，基础账户/模板不改写。
- 玩家和怪物弹道提交时先成功施加self，再读取一次有效ATK；准备期变化可影响提交，在途不二次乘算。DEF在命中时读取，命中减攻影响目标后续计划。近战/范围在释放、重击在预警创建、反击在窗口创建、站位/直接命中窗口在预警创建时冻结攻击量；原取消/来源死亡/旧轮规则保持。
- 通用实例输出attribute，机制21的玩家/怪物视图输出effective_atk。旧DEF实例和旧19/20快照分别省略新字段，协议/资产/恢复schema与六类Client保持。0.6.0为公开Rust字段及枚举的次版本源码变更。
- Node/H5仅展示权威属性、有效ATK/DEF、层数、来源和截止。重连保留原绝对时间；重启只履行原冻结资产责任，Closed视图移除临时属性，不重建战斗World或按新参数重算责任。

[接入说明与启动命令](../interfaces/attribute-modifiers-v1.md) · [有限样例](../../examples/attribute-host/catalog.json)。本批未新增具名地图规则或第二款游戏内容。

## 执行证据

| 门禁 | 结果与证据 |
|---|---|
| 原生/配置 | 默认与全特性为相同708项（保留686、新增22），Clippy -D warnings通过；23种非法新字段/版本/属性/帽用例、基础值不变、正负修正/抵消/独立帽、宽整数、零ATK与最小伤害、来源/容量及100次关闭覆盖。[默认日志](../../artifacts/sdk-5-3/default.log) · [集合与冻结保护](../../artifacts/sdk-5-3/baseline-verification.json) |
| 精确攻击时点 | 纯ATK三层真实伤害83→100→116→116，到期普通攻击回66；叠加DEF削弱为83→109→140→155。覆盖准备期获得/同刻到期、飞行期ATK失效/新增而攻击量保持、命中时DEF变化、玩家普攻、怪物self弹道及来源死亡、命中减攻后的近战33，以及范围/重击/反击/站位冻结边界。同一原生集合，见[AT03～AT05映射](../checklists/SDK-5-3-acceptance.json) |
| 普通Node/H5 | 两份仓库外配置改变ID、倍率、上限、时长、来源策略与资产绑定，共4条撤离/死亡链；其中正帽1500bps使两层+1000的ATK准确夹到115。包含多层刷新重连、旧连接拒绝、封顶刷新、整组到期、战斗拾取、一次扣费/结算；World/未决=0，页面异常0。[报告](../../artifacts/sdk-5-3/verification.json) |
| 纯触屏 | 手机390×844只通过触控按钮完成状态、战斗、拾取与撤离/放弃；HTTP仅只读观察。两条链页面异常0且World/未决归零。同上报告touchCases；[截图](../../artifacts/sdk-5-3/outside-a-stack-1-layers.png) |
| 强杀/责任恢复 | 活跃通用多层状态、冻结终局意图后、资产提交后三处强杀，各连续两次恢复。原Create/hash/结果/金额/operation ID保持，一次退款或结算；外部属性倍率/层数/期限改变不重算旧责任，快照无状态与effective_atk，不恢复World。同上报告crashes |
| Node及旧机制 | 22项队列/存储/投影检查通过；旧19和20各普通撤离/死亡，旧JSON无attribute/effective_atk，旧状态/叠层/伤害保持。[Node日志](../../artifacts/sdk-5-3/node-storage.log) · 同上报告legacy |
| 独立交付 | 实际Cargo源包、21项锁定第三方源及普通release二进制；所有Rust源与候选一致、整包无工作区绝对路径。删除包内回归配置/示例及samples后，包外21/20/19各普通Node双终局，共6条；空缓存离线Rust消费方协议1六模式、协议2/3共14场景及各自重启（含2次核对）通过。[报告](../../artifacts/sdk-5-3/delivery/independent-verification.json) |
| 解压源包 | 全新Cargo缓存、离线模式再次执行源包默认/全特性相同708项及Clippy，通过；不将重复运行计为新增测试。[报告](../../artifacts/sdk-5-3/delivery/source-verification.json) |
| 冻结兼容 | 原20场景、19/20样例、v1.1来源包和0.3.1/0.4.0/0.5.0历史包原文/hash保持。[57项保护清单](../../artifacts/sdk-5-3/baseline-verification.json) |

普通协议2参考样例的原撤离解锁为180秒，包外验证副本使用3000ms解锁/500ms引导以匹配参考脚本25秒墙钟期限；原样例不改。此次完整受控验收与独立交付均通过。本批不重跑无关具名地图的全部浏览器专项；旧机制的完整原生集合保留并执行，受影响攻击/生命周期通过新增测试及实时链验证。

## 交付与范围

[0.6.0本地交付包](../../artifacts/sdk-5-3/delivery/match-kernel-0.6.0-aarch64-apple-darwin.tar.gz)，4,447,161字节；SHA256为`cf03f0e1e71a5879ecd7239c37d83ae81feaca153db260c81e93b7da65303c2c`。预编译平台Apple Silicon macOS，replay=false；其它平台和生产发布另验。[交付清单](../../artifacts/sdk-5-3/delivery/package.json)

试玩页：[127.0.0.1:4176](http://127.0.0.1:4176/)，独立账本`.local/sdk-5-3-demo.sqlite`。启动冒烟未创建对局，页面异常0；旧4173/4174/4175预览及账本保持。[预览检查](../../artifacts/sdk-5-3/local-preview.json)

只关闭M03的有限ATK/DEF通用修正切片。其它属性、未注册状态策略、控制/盾/DOT/HOT、86状态完整内容、多人与特殊模式、生产容量/库存及跨游戏实接继续按[剩余矩阵](../checklists/sdk-requirements-gap.md)跟踪。D-03/D-05没有因本批实施而自动裁定。
