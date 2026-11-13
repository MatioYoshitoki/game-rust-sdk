# B3-9 圆形范围预警技能验收（2026-11-03）

对应PRD v1.20 §3.22，先明确[B3-9需求与A01～A08](../batches/B3-9-area-telegraph.md)，再实施配置/Rust/H5。状态：A01～A08受控验收通过。新增42项Rust、25项真实浏览器/进程场景和17组完整回归通过，最终报告与预览绑定同一源码/配置/二进制。

## 配置与行为

独立场景 `b3-9-area-telegraph-v1`，raw SHA-256 `fb4ccfb30f48cb3dd87feebaf6a3527011246fe793a03944fc4bb5c565be0306`。离线导出先验证不可变来源manifest，复用森林、狼/精英/阶段Boss、积分与封印箱。原守将点位在阻视线的树林；仅新验证场景改选附近原地图可视空地[3376,1072]，保留完整足迹与视线门禁，原九raw/地形不变。

撼地击range550/radius96/windup1800/cooldown6000/recovery900/cost12/damage20000bps/chance1/min_phase1均显式配置。新能力为通用locked_circle，不冒充完整原Boss特殊技能/被动。开始锁点并扣MP/排CD，结束按当前足迹/效果通路最多判定一次；角色整体离圈可miss，原护甲/暴击/最小伤害/吸血复用、不加剑痕。沿原命中语义，0伤害仍打断撤离，封印仅正伤害打断。

归位/死亡/保护/接管/终局/代替换取消，不返MP/CD；普通伤害和阶段切换保留预警。32圈取消聚合一条有界事件，关闭预留圈和封印各一事件；队列饱和按实际事件峰值冻结资产并回收。预警不进入重启责任，不重放伤害；积分仍140分，完整撤离127/死亡52修为。

## 验收映射

| 标准 | 已通过证据 |
|---|---|
| A01 | 6项配置测试：旧九hash、能力/结构/未知字段、ID/名称/冲突、数值/整数/阶段条件、合法边界与非Boss模板 |
| A02 | 28项World中的门禁/概率/阶段/资源/CD/视线、不额外RNG、锁点/一次付费、蓄力停移动/旧技能/近战 |
| A03 | 足迹圈内/相切/圈外、先移动后判定、视线与效果遮挡独立、hit/miss一次、护甲暴击吸血与0伤害、无剑痕 |
| A04 | 非致死伤害/切阶段保留、后摇/CD/新序号、多个独立圈、同刻伤害先于封印/撤离完成 |
| A05 | 归位/到家、背景/前台、死亡/离开/终局、守护代替换、32圈取消与关闭额外事件预留；旧连接/恢复隔离 |
| A06 | 8项公开协议/恢复/峰值；25项浏览器：完整撤离127/死亡52、站圈/走出/重入/遮挡/变参/保护/刷新/击杀/归位/多圈/致死；提交前后强杀/丢回执/换版人工责任/重复重启/未冻结退款 |
| A07 | 桌面/390px手机等距圈/权威倒计时、公开移动躲避、页面按钮暂停取消、源场景一键闭环，页面异常与截图 |
| A08 | 默认/all-features相同Rust、Clippy、协议/性能、原全部专项及新专项，最终源码/配置/二进制一致 |

新增42项Rust：World28、公开协议/恢复/取消与峰值8、配置6；总395=353+42。配置专项50中的新6已计入42，不将默认/all-features或专项子集相加。

## 复现

```sh
python3 scripts/export_b3_9.py --check
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
python3 scripts/verify_b3_7.py
python3 scripts/verify_b3_8.py
python3 scripts/verify_b3_9.py
```

开发期按规则校正夹具：源Boss点选可视空地；移动边界夹具避开施法者实体足迹；暂停的纯页面验收先导航进入既有仇恨范围（停在300距离时尚未进入原仇恨范围，技能range不是自动仇恨范围）。预验收调整暂停夹具时，源码绑定检查中止首轮回归；随后界面明确区分原技能/范围技能独立概率，重新冻结。开发期记录保存在implementation-*，最终版本完整重跑全部17专项。

最终冻结源码后的17组回归全部通过，命令、退出状态、用时和日志见[regression-run.json](../../artifacts/b3-9/regression-run.json)，日志为 `artifacts/b3-9/regression-*.log`。基础/H5/持久恢复/并发先独立运行，旧地图/连接/超时/配置/容器最多两组并行，参数、守护、单组、多波、Boss、积分和范围专项独立运行，减少加速夹具的外部CPU争用。

[新专项机器报告](../../artifacts/b3-9/verification.json)记录42项新Rust和25项真实浏览器/进程场景，页面异常0。默认/all-features各395项相同Rust、格式、Clippy `-D warnings`、来源/导出/独立回放、12条Node链/2项协议检查、原全部专项与H5性能通过。原九raw/hash保持；17份报告逐文件核对最终113个源码/配置/脚本及B1验收映射，所有带二进制哈希的报告与预览一致，见[source-audit.json](../../artifacts/b3-9/source-audit.json)。

本机实时等距采样：快照间隔中位数30.9ms/P95 33.5ms，按键到权威移动19.0ms，长任务0；该结果仅为本地受控采样，未签收生产容量。截图：[预警](../../artifacts/b3-9/warning.png)、[390px手机预警与躲避](../../artifacts/b3-9/mobile.png)、[源森林完整撤离](../../artifacts/b3-9/extraction.png)。边界夹具使用60秒逻辑蓄力/加速时钟，默认场景仍为1.8秒。

4173预览升级10场景，选择“迷雾森林 · 范围预警”。正常停止旧宿主后备份原SQLite并校验完整性，保留原会话/账本，见[local-upgrade-backup.json](../../artifacts/b3-9/local-upgrade-backup.json)。只读冒烟验证目录、新技能参数/raw hash、等距页面和页面异常0，未创建新局，见[local-preview-smoke.json](../../artifacts/b3-9/local-preview-smoke.json)。

本批只签收一次圆形范围预警/判定及原战斗/搜刮/资产链；持续危险区、多形状/多段、召唤/眩晕、完整原Boss/多人和生产库存/容量另批。
