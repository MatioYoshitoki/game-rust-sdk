# 协议 2：配置绑定的资产事务

crate 0.3.0 继续支持本协议；schema 4 的配置结果与结算策略使用新增协议 3，见 [结果契约](results-v3.md)。

crate 0.2.0 的六类 Client 调用保持。`Config::protocol_version()` 返回所选配置支持的协议：schema 1/2 为 1，schema 3 为 2。Create 必须精确匹配，不静默切换。权威类型见 `src/protocol/assets.rs` 与 `src/protocol.rs`。

## 配置与事实

schema 3 在现有数值配置上增加：

```json
{"asset_contract":{"entry_cost_asset_id":"entry-credit","reward_asset_id":"experience"}}
```

两个资产 ID 必须为 1～64 个 UTF-8 字节，无控制字符，互不相同，且不得等于任何配置物品 ID。本批保持当前玩法及公式：携入数量来自 `carried_quantity`，成本数量来自 `params.entry_years`，收益数量来自现有积分/结果公式。新增映射把这些数量输出为指定资产。参数名称、战斗能量、Outcome 与成长预览视图的进一步解耦后续处理。

Rust 发 `Business::LoadAssets {}`。宿主返回 `ReceiptOutcome::AssetsLoaded {facts}`，包括 principal、revision、五个现有战斗属性以及 `assets:[{asset_id,quantity}]`。资产行最多 128 行，ID 严格递增且唯一；余额为 u32、可为 0。必须包含携入物品与成本资产，余额满足配置要求；身份/属性/修订仍按原准入规则校验。不能给协议 2 返回旧 Loaded，也不能给协议 1 返回 AssetsLoaded。

## 一个原子事务，三类用途

`Business::CommitAssets {transaction}` 使用有限目录。资产行严格按 ID 排序、唯一、正 u32 数量，0 数量行省略。成本和收益资产命名由接入方决定；宿主按原文执行，不计算掉落或奖励。

```json
{"kind":"commit_assets","transaction":{"kind":"admission","expected_revision":0,"debits":[{"asset_id":"entry-credit","quantity":1},{"asset_id":"xuan-tie","quantity":5}]}}
```

- **Admission**：带 expected_revision，debits 包含携入物品和成本资产，最多两行。
- **Settlement**：带 expected_revision、credits 和 summary，最多 65 行 credits。credits 包含实际保留物品和公式产生的收益。summary 包含 outcome、按 item_id 排序的 inventory 收支行，以及可选 scoring/map_progress/reset_progress，承接原机制的数量守恒、消费、击杀和结果证明。
- **Compensation**：带 original_operation_id 和 credits，精确返还原入场 debits；宿主核对原入场已提交，预期修订为原入场提交 revision。不得从其他操作取资金或重新选择退款金额。

每个请求的整批资产、revision 和稳定 commit_id 必须在存储侧同一事务提交。余额不足/版本冲突拒绝整批。业务回执仍用原 operation_id/attempt_id 和 Committed、Refused、Unknown、NotCommitted、Reconciled。部分返回不受支持。

Unknown 查询同一原操作；有 fencing 的 NotCommitted 才能有限重试，同一 operation_id 和完整 transaction 保持。已推进尝试后，旧尝试的负回执可能被拒绝；不能改报未提交或使用新 ID。人工核对沿用原责任与冻结正文。

提交证明被 Rust 接纳后，协议 2 发 `AssetSettlementApplied {credits,summary}`；协议 1 继续原 SettlementApplied。其他生命周期事件与调用不变。状态发布的成长视图仍沿用原结构，本批泛化的是业务资产边界。

## 历史责任与恢复

检查点仍用 IoState schema 1，保存实际发出的原版本 BusinessRequest 和原回执。内部玩法意图投影不序列化到检查点；恢复先严格校验规范事务与配置映射，再复用既有数量/机制证明校验。重复/乱序/0 数量行、多发资产、错误收益、伪造库存摘要和错误入场关联均拒绝，即使宿主重算摘要也不能绕过。

协议 1 的旧配置、旧请求与旧检查点继续按旧类型校验，旧资产责任不转成协议 2。升级时保留准确旧原文与 hash；配置映射改变只能影响绑定新版本的新局。Rust 不重建 World，不按新配置重算旧奖励。Closed 后仍继续处理原未决操作；只有权威提交/核对证明才能结束责任。

## 可运行的本地宿主

仓库根目录执行：

```sh
cargo build --locked --bin kernel-host
python3 adapters/python/asset_host.py \
  --binary target/debug/kernel-host \
  --configs examples/asset-host/catalog.json \
  --account examples/asset-host/account.json \
  --state /tmp/asset-host.sqlite
```

首次创建使用 JSON 账户事实，默认完成测试场战斗、拾取和死亡结算；相同 state 再运行则恢复原责任，余额不会重新初始化。显式 `--reconcile` 授权本地适配器核对/纠正原冻结事务。`--mode extraction` 可走撤离；加速验收需先构建 `--features replay` 并传 `--verification`，普通启动无需 replay。

宿主先持久接受整批检查点/请求/事件，再执行 SQLite 事务或交付本地脚本玩家；原配置原文也保存在账本中，重启按原 hash 加载。账本采用单进程独占、同步执行，仅在旧进程已退出且无迟到写入者的条件下，把缺失本地提交证明视为 NotCommitted。远程数据库、异步队列、账号鉴权和跨机接管须提供各自的事务/fencing/接管验收。主机文件权限视为可信，账本无抗特权篡改签名。

专项入口：`python3 scripts/verify_sdk_3_1.py`。[范围与 AS01～AS08](../batches/SDK-3-1-assets.md)
