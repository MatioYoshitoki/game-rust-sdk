# B3-6 多波次遭遇验收（2026-11-03）

对应PRD v1.17 §3.19，先明确[B3-6需求与W01～W08](../batches/B3-6-wave-encounter.md)，再实现Rust/配置/H5。状态：W01～W08受控验收通过。新增35项Rust、20项浏览器/进程场景和14组完整回归最终通过，报告与最终源码/配置/二进制一致。

## 配置、来源与运行规则

独立场景 `b3-6-wave-encounter-v1`，原文SHA-256为 `be42fef69ba8247a66f40a8688bfa715ee5b87a51c74098f7e5dcf07f6b2cb78`。导出前检查不可变来源manifest，复用原森林、三狼点位/能力/掉落、双封印箱和奖池；2只/1只与0/1500ms为显式验证布局，均写入配置，不作为Rust数值常量。当前不签收完整圣所、Boss阶段、分支/跨遭遇依赖、多人或每波额外奖励。

schema仍2，机制版本5注册显式waves；旧单组字段继续按原语义，新旧形式互斥，旧原文不转换/改hash，旧视图省略wave。波ID、顺序/数量、每波模板/实例/位置/必杀列表、生成前间隔和最终箱绑定均配置初始化；六类Client和有限Action不增加生成/推进/奖励写入口。

每个遭遇一次触发，先安排绝对ready_at，到期后整组生成；本波全部指定实例实际死亡才完成，之后安排下一波，间隔0也在后续步生成。每波一个安排/开始/完成、一次受阻提示；重试不重设时点、不抽随机数、不部分生成。不计分目标仍能推进，别波/别遭遇/全局积分和归位不能代替当前目标死亡。

所有波次独立预留槽位，与静态/守护总数≤32；换波不复用身份、不删旧敌人/尸体/掉落/已提交弹道。前波非必杀活怪可以阻挡后波，清障后恢复本波。只有最终波完成才解除父容器锁，仍显式破解、开箱、落地和拾取；箱重生不重开遭遇。资源/成长/责任上界汇总全部未来波的一次性实例，避免只按首波核验冻结资产。

Waiting/Active刷新保留波次、目标、ID和原deadline；保护中不生成或推进，逻辑钟仍走，恢复后按原时点继续。死亡/关闭/绝对终局停止未来波，可中途撤离且只结算实际战果。服务重启只恢复Closed责任/结果，无持久终局只退原入场，不重放历史波次。

## 验收映射

| 标准 | 证据 |
|---|---|
| W01 | 6项配置测试覆盖原六版本、能力/形式/结构、波/实例/目标/模板引用、数量/间隔/总时长、跨波同点与后波自身足迹、未生成实例资源/槽位预算 |
| W02 | 24项World中的首波与后波0/正间隔/到期边界、零间隔后续步、一次安排/启用/完成；配置1/2/3波均复用同一控制器 |
| W03 | 玩家和前波非必杀活怪阻挡、每波受阻去重/清障、不同模板/半径、跨波同坐标新身份、旧死亡/普通怪/别组/积分不推进、非计分目标 |
| W04 | 中间波/等待箱锁、最终显式解封/开启、旧资源保留和唯一ID、全波撤离45/死亡11、部分波撤离37、不计分0杀/22修为且晶石6入库；原满包/满地面守恒复用 |
| W05 | Waiting/Active刷新、旧连接隔离、保护跨截止时间继续、重复进入不重置、死亡/关闭/终局停刷、箱重生保留完成；波事件峰值32容量封存/64容量正常 |
| W06 | 5项公开协议/恢复/输出峰值测试；20项浏览器矩阵中的提交前后强杀、丢回执、波数/间隔/成长换版后原责任、重复重启、Waiting/第二波活跃中断退款 |
| W07 | 等距地图波次/面板当前目标/已完成数/权威间隔，桌面/390px手机与一键公开输入；最终矩阵页面异常0，源码/配置/二进制绑定通过 |
| W08 | 最终默认/all-features各282项相同Rust（原247+新增35）、Clippy和14组完整回归最终通过，报告与最终源码/配置/二进制审核一致 |

新增Rust35项：World24、公开协议/恢复/峰值5、配置6。配置专项32项中的新6项已计入35，其他专项Rust也属于总数子集；默认/all-features不重复相加。

## 复现与证据

```sh
python3 scripts/export_b3_6.py --check
python3 scripts/verify_b1.py --date 2026-11-03
python3 scripts/verify_h5.py
python3 scripts/verify_io_timeouts.py
python3 scripts/verify_durability.py
python3 scripts/verify_reconnect.py
python3 scripts/verify_flow_control.py
python3 scripts/verify_b3.py
python3 scripts/verify_config_loading.py
python3 scripts/verify_b3_2.py
python3 scripts/verify_b3_3.py
python3 scripts/verify_parameters.py
python3 scripts/verify_b3_4.py
python3 scripts/verify_b3_5.py
python3 scripts/verify_b3_6.py
```

最终日志在 `artifacts/b3-6/regression-*.log`。参数/守护/单组/多波矩阵分别独立执行，控制加速发布夹具的外部CPU争用。B3-2旧场景和B3-4直接HTTP测试增加了仅对STALE_SEQUENCE明确拒绝的有限重新取序号；不对unknown写入换ID重放，也不放宽玩法/资产断言，以处理上轮已记录的页面ready输入竞争。

首轮并行的持久恢复专项未在18秒内到达结算强杀点，截图显示加速自动流程已发生连接换代并停止；并发专项首次进入界面未激活而超时。保留首次日志 `regression-durability-initial-timeout.log` 和 `regression-flow_control-initial-timeout.log`，在不改变源码、业务期限或断言的情况下串行复核，最终结果及两次执行信息写入regression-run.json。

新专项[机器报告](../../artifacts/b3-6/verification.json)列出35项新Rust和20项真实浏览器/进程场景。14份报告声明的源码逐文件匹配当前版本，带二进制哈希的报告与本地预览均使用同一kernel-host；最终91个源码/配置/脚本文件、原六配置哈希与二进制审核见[source-audit.json](../../artifacts/b3-6/source-audit.json)，执行退出状态/用时及两次超时后的串行复核见[regression-run.json](../../artifacts/b3-6/regression-run.json)。

默认/all-features各282项相同Rust、格式、Clippy `-D warnings`、来源/导出/独立回放、12条Node链/2项协议检查、H5功能/性能及原连接/超时/持久/并发/地图/配置/普通袋/分层/参数/守护/单组专项最终通过。查看[等待下一波](../../artifacts/b3-6/waiting.png)、[移动端](../../artifacts/b3-6/mobile.png)、[完整撤离](../../artifacts/b3-6/extraction.png)。

本地4173已升级7场景。先停止旧服务，备份 `.local/h5-lab.before-b3-6-20261103-154727.sqlite` 并检查完整性ok，保留1个当前Closed会话和9份已有配置。只读冒烟确认新增连续波次、2/1目标、0/1500ms、双奖励箱，页面异常0、未创建新会话，见[预览检查](../../artifacts/b3-6/local-preview-smoke.json)和[备份记录](../../artifacts/b3-6/local-upgrade-backup.json)。当前每遭遇每局一次、单人独立测试库存；完整森林/Boss/生产库存与客户端、正式容量、跨机和战斗热恢复继续按批交付。
