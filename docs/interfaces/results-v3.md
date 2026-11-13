# 协议 3：结果 ID 与结算策略

crate 0.3.0 新增协议 3，保留协议 1、2 原正文及历史责任。六类 Client 方法保持。`Config::protocol_version()` 和启动目录版本元数据的 `protocolVersion` 返回所选版本要求；schema 1/2、3、4 分别对应协议 1、2、3。

## 配置绑定

schema 4 必须同时声明原 asset_contract 与：

```json
{
  "result_contract": {
    "extraction": {"result_id": "secured", "policy": "preserve"},
    "death": {"result_id": "defeated", "policy": "attrition"}
  }
}
```

extraction/death 是当前已注册的 Rust 终局触发，结果 ID 是接入方定义的业务身份。ID 为 1～64 个 UTF-8 字节、无控制字符且互不相同。两条触发可以独立选择同一策略或不同策略；未知触发/策略、缺失/额外字段、schema 与协议错配拒绝，不回退默认配置。

| 策略 | 物品规则 | 收益规则 |
|---|---|---|
| preserve | 保留当前全部物品，已合法消费的物品仍按消费摘要扣除 | `(实际积分 + extraction_score) × cultivation_coefficient` 向下取整 |
| attrition | 按 params.death 的格子比例、数量比例及下限选取损耗；其余物品保留 | `实际积分 × cultivation_coefficient × death_multiplier` 向下取整 |

这是两个已注册的规则组合。全部数值继续由配置提供；策略的未来扩展须实现并验收新能力。终局由 Rust 的撤离完成、死亡/离开/激活后关局等当前机制判定，宿主或客户端没有设置结果的动作。

默认绑定与原收益一致。将 death 绑定 preserve 时，死亡触发仍按原时点发生，但使用完整保留策略；将 extraction 绑定 attrition 时，撤离仍需满足原资格与读条，但按配置损耗结算。策略选择不绕过终局门禁，也不改变原触发机制。

## 冻结消息

LoadAssets、AssetsLoaded、Admission、Compensation 及原业务/传输回执继续沿用协议 2。结算为：

```json
{
  "kind": "commit_assets",
  "transaction": {
    "kind": "result_settlement",
    "expected_revision": 1,
    "credits": [{"asset_id": "xuan-tie", "quantity": 3}],
    "summary": {
      "result": {"result_id": "defeated", "policy": "attrition"},
      "inventory": [{"item_id": "xuan-tie", "carried": 5, "looted": 0, "kept": 3, "dropped": 2}]
    }
  }
}
```

summary 的 inventory 与可选 scoring/map_progress/reset_progress 承接原收支与机制证明；result 中的 ID 与策略共同冻结。summary 没有 Outcome 字段。正数量、唯一、按资产 ID 排序、事务原子性、revision/commit_id 与原责任身份要求见 [资产契约](assets-v2.md)。宿主不依据结果名或策略重算金额。

**`itemsGained` 口径（D-05，2026-11-13 已裁定）**：`summary.inventory` 各行的 `carried/looted/kept/dropped` 是权威拆分事实。宿主如需旧 `itemsGained` 展示字段，按经手总量语义以 `kept + dropped` 合并生成（含战备携带物），文案表述"本局结算涉及物品（含战备携带）"，不得宣称"净获得"，也不引入入局基线差分。分字段保留为可选增强，不得用不明字段影响奖励金额；`enterInventory` 为兼容字段，结算不读取。详见 [D-05 裁定](../decisions/D-05-items-gained-semantics.md)。

Rust 接纳提交证明后发 `ResultSettlementApplied {credits,summary}`，与原冻结请求一致。协议 2 继续 AssetSettlementApplied，协议 1 继续 SettlementApplied。事件必须与实际已验证的业务提交区分于 Closed；World 关闭仍可能留下待核对资产责任。

## 恢复与兼容

IoState 仍为 schema 1，保存实际原版正文。协议 3 的结果 ID 必须在原配置中注册、策略必须匹配，库存/奖励/消费及提交证明再通过共享恢复校验。旧/新结算正文不能混用。Unknown 查询、带 fencing 的重试、关闭后收尾、人工核对和多次重启使用同一个原 operation_id 与完整结果/资产。

配置绑定随精确 id/hash 固定。更改外部结果 ID 或策略只影响绑定新版本的新局，旧责任按保存的配置原文恢复；不重建 World，不重新选择终局或结算金额。检查点摘要检测损坏，语义校验限制非法状态，不构成抗特权主机伪造的签名。

旧协议 1/2 的消息、配置、结果语义和责任继续保留。普通状态发布的当前成长预览字段及游戏数值参数名称沿用；本批泛化业务结果边界，新增胜负等触发机制另批验收。

## 运行示例

```sh
cargo build --locked --bin kernel-host
python3 adapters/python/asset_host.py \
  --binary target/debug/kernel-host \
  --configs examples/result-host/catalog.json \
  --account examples/asset-host/account.json \
  --state /tmp/result-host.sqlite
```

示例为现有测试场的配置变体，撤离开放/读条时间由 JSON 缩短以便验收。默认脚本完成战斗、拾取、离开与结果结算；`--mode extraction` 走撤离。相同 state 再次运行恢复原责任；`--reconcile` 授权本地核对原事务。宿主从目录元数据选择协议，支持协议 2/3，沿用独占同步 SQLite 参考实现的范围。

验收入口：`python3 scripts/verify_sdk_3_2.py`。[RS01～RS08](../batches/SDK-3-2-results.md)
