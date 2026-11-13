# SDK-5-13 命中预算与重复接触策略执行记录

2026-11-05；PRD v1.50；crate0.16.0 / 机制31 / schema4 / 协议3。

先冻结[需求与CP01～CP09](../batches/SDK-5-13-contact-policy.md)，再实现并验收。[逐项映射](../checklists/SDK-5-13-acceptance.json)和[机器汇总](../../artifacts/sdk-5-13/acceptance.json)绑定最终同源候选。

## 交付内容

ContactBudget<K>只持冻结策略/已用计数和有界身份时间映射，不依赖地图/技能/PVE几何。ground_field可选contact_policy定义每周期成功接触上限、全区域预算及重复间隔。0同一ID/代次每区域一次，正数按计划时刻计算，等值合法。按中心距离平方/ID/代次选目标；真实直接接触含全盾吸收/0HP损失消耗一次，闪避/阻挡/不合法/重复冷却不消费，继续候选；额度满不再尝试额外目标/状态。附带状态容量失败不退已发生接触预算，击杀/附带伤害不重复扣。

历史进出/重连保持，新代独立，至多128；多区域独立、参数/直接攻击量冻结。预算0立即只结束一次，释放槽与历史，预算与到期原因优先级明确，无未来周期或额外掉落，不退款/回滚既有效果。成本/CD和原子提交/源失效/终局清理沿原机制。FieldView.contact_policy与Tick.hit_targets/remaining_hits发布Rust事实，省略策略与旧1～30保持规范化/输出/索引行为，公共DTO/枚举源码消费需适配，Client六类/协议1～3/资产恢复schema保持。

## 验收范围

- 1049项Rust，默认/全特性同集合，保留1010、新增39；Clippy全目标/全特性-D warnings及22项Node存储/流控/等距/账本检查通过。
- 两套包外H5配置交换主副/改ID/数值/绑定：A每身份一次，周期接触1/1/0且余量1后到期；B独立正间隔，接触1/1/1且余量0预算结束。普通双终局4链、纯触屏双终局含预算结束、真实选点/目标/历史/余量、状态/实际回复→两敌击杀/拾取→结算通过。
- 资源/射程/视线拒绝；带已用预算/历史的同World重连、重复输入/旧连接隔离。若已接受操作的重复请求与自动连接换代交错，仅允许原请求以旧连接被STALE_CONNECTION隔离，验证新代已成立并保留原操作一次执行，禁止换代重发已接受操作。
- 活跃准备+已有命中历史区域、冻结终局后、资产提交后三点强杀，各两次恢复。更改策略/参数不改旧Create/hash/金额/operation ID，不恢复临时区域/历史/预算/战斗或重复扣奖。
- 100次关闭、128身份历史/合法快照及直接输出峰值责任回收通过。旧19～30各普通H5双终局24链通过，页面异常0，入场/结算各一次，World/未决=0。
- 独立普通Node28链，包含从交付样例生成的A/B包外策略双终局4链；SDK配置/样例移除后消费成功。Rust协议1六模式、协议2/3十四场景/十四次重启/两次核对通过。解压源crate空CARGO_HOME、只用交付vendor离线同1049项默认/全特性及Clippy通过。
- 97项原JSON/历史归档逐字节保持，原v1.1来源manifest及16项登记文件校验通过。旧4173～4185预览/账本保持，新开4186且未代用户创建对局。

## 独立交付

[0.16.0独立包](../../artifacts/sdk-5-13/delivery/match-kernel-0.16.0-aarch64-apple-darwin.tar.gz)，4,707,034字节。

- archive SHA-256 `1fee58cd49c68dc7d8759e27da2c0e9108feb64eef405eb821ab18a7f6d3133a`
- source crate SHA-256 `2c856e72909a0d66a0d56035f76d7ec0176aecb0ddea324b25b6c172e9ff3b60`
- ordinary binary SHA-256 `4b5bbdc654414aef67c1bcff696f42d51ba83370344b68f40cfb2a5d20cf05aa`

[普通H5/触屏/恢复](../../artifacts/sdk-5-13/verification.json)、[基线保留](../../artifacts/sdk-5-13/baseline-verification.json)、[独立消费](../../artifacts/sdk-5-13/delivery/independent-verification.json)、[离线源码](../../artifacts/sdk-5-13/delivery/source-verification.json)、[预览](../../artifacts/sdk-5-13/local-preview.json)。接入见[contact-policy-v1](../interfaces/contact-policy-v1.md)。

[4186预览](http://127.0.0.1:4186/)：点选/触摸后Q建区、2自身状态，显示Rust预算和历史。独立状态`.local/sdk-5-13-demo.sqlite`，未创建对局。

仅将通用策略接入持续区域；其它几何/弹道/陷阱、来源存续策略、周期费用/引导、多人与完整M05/家族/器型/技能目录继续保留，生产门禁另验。
