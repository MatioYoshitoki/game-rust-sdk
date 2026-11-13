# B3-8 模板击杀积分验收（2026-11-03）

对应PRD v1.19 §3.21，先明确[B3-8需求与S01～S08](../batches/B3-8-template-scoring.md)，再实施配置、Rust积分/恢复与H5。状态：S01～S08受控验收通过。新增36项Rust、21项真实浏览器/进程场景及16组完整回归全部通过，最终报告/预览绑定同一源码/配置/二进制。

## 配置与行为

独立场景 `b3-8-template-scoring-v1`，raw SHA-256 `f076b3a642adc826a4ba4190e65fd4422a4cb3aed875a2ccffef9ea6df2dfe37`。离线导出先校验不可变来源manifest，复用B3-7森林/战斗/阶段/箱机制；普通/精英验证狼/守将分值10/30/100显式配置，验证精英复用妖狼属性/AI/掉落。这些分值不代表完整来源精英/Boss类型倍率或被动已实现。

schema2/mechanism7，仅新PVE启用。每模板积分可覆盖或继承root，0合法；kill_credit=false强制0且不计旧计分击杀，仍记录实际死亡并允许掉落/推进。旧World.kills/PveView.kills保留原口径；新scoring分离实际数、计分数、积分与权威结果预览。三杀140分，按总分一次floor得到撤离127/死亡52；部分普通+精英40分，撤离52/死亡15。

Rust以模板ID记录实际死亡次数，切阶段/回血/归位/箱/拾取不计分且不额外抽RNG。新CommitSettlement与SettlementApplied冻结同一有序摘要；恢复精确核验模板/次数/上界、计数/积分/修为及实际模板掉落上界，0积分/0系数不丢真实战果。旧八raw/hash和旧责任正文保持，六类Client不增加写分能力。

## 验收映射

| 标准 | 已通过证据 |
|---|---|
| S01 | 6项配置：旧八hash、能力/类型/分值区间、继承/0/非计分、未来Boss与守护重生加权成长预算 |
| S02 | 22项World中的多模板分值、0分/非计分、首次真实死亡一次、非致死/阶段/回血/归位不计、守护新代、RNG/掉落顺序不变 |
| S03 | 实际/计分/积分分离、累计后一次floor、0系数、独立World/击杀顺序、冻结撤离127/死亡52和逐物品守恒 |
| S04 | 公开重复攻击/额外分数字段、连接接管/旧连接、刷新/保护/归位保留、关闭停止，新守护代计分一次 |
| S05 | 8项公开协议/恢复/队列测试：缺失/伪造/乱序/重复/超界摘要、矛盾修为、无真实击杀的掉落拒绝，0分/0系数真实拾取恢复，旧责任禁止新元数据 |
| S06 | 21项真实浏览器/进程场景：完整/部分撤离死亡、零积分/非计分/0系数/继承/总分取整，提交前后强杀/丢回执/换积分与系数后人工核对/重复重启/未冻结退款 |
| S07 | 地图实例分值，桌面/390px手机显示权威实际数/积分/预览，结果显示冻结积分，一键公开输入，页面异常与截图 |
| S08 | 默认/all-features相同Rust、Clippy、原全部专项与新专项，队列饱和冻结实际积分并回收World，最终源码/配置/二进制审核 |

新增36项Rust：World22、公开协议/恢复/饱和8、配置6；总353=317+36，配置专项44中的新6已计入36。不将默认/all-features或专项子集重复相加。

## 复现

```sh
python3 scripts/export_b3_8.py --check
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
```

实现期夹具按既有契约校正：守护成长预算夹具清除额外掉落/光球以单独触达成长预算；恢复先查询未知业务，不假设直接重写冻结结算；拾取等待实际到达范围，避免假定固定导航时长；饱和时未提交的致死攻击不算Boss积分，只冻结已真实获得的40分。规则/资产断言未因此放宽。

16组完整回归全部通过，执行命令、退出状态、用时和日志见[regression-run.json](../../artifacts/b3-8/regression-run.json)；日志为 `artifacts/b3-8/regression-*.log`。基础/H5/持久恢复/并发先单独执行，旧地图/连接/超时/配置/容器最多两组并行，参数、守护、单组、多波、Boss和积分专项独立执行，减少加速夹具的外部CPU争用。

[新专项机器报告](../../artifacts/b3-8/verification.json)包含36项新Rust与21项真实浏览器/进程场景，页面异常0。默认/all-features各353项相同Rust、格式、Clippy `-D warnings`、来源/导出/独立回放、12条Node链/2项协议、原全部专项及H5性能通过。旧八raw/hash保持；16份报告逐文件核对最终106个源码/配置/脚本及B1验收映射，所有带二进制哈希的报告与预览一致，见[source-audit.json](../../artifacts/b3-8/source-audit.json)。

本机实时等距采样：快照间隔中位数31.0ms/P95 44.9ms，按键到权威移动47.7ms，长任务0；这是本地受控结果，不代表生产容量。截图：[部分积分/预览](../../artifacts/b3-8/partial-score.png)、[390px手机](../../artifacts/b3-8/mobile.png)、[完整撤离](../../artifacts/b3-8/extraction.png)。

4173预览升级为9场景，选择“迷雾森林 · 模板计分”。正常停止旧宿主后备份原SQLite并校验完整性，保留原会话/账本，见[local-upgrade-backup.json](../../artifacts/b3-8/local-upgrade-backup.json)。只读冒烟检查目录、新模板积分/hash、等距页面、页面异常0且未创建新局，见[local-preview-smoke.json](../../artifacts/b3-8/local-preview-smoke.json)。

本批只签收通用模板计分和原有战斗/搜刮/资产链，精英/Boss特殊战斗能力、完整来源分类被动、多人和生产库存/容量仍另批。
