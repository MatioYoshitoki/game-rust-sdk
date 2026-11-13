# SDK-5-73 NPC 弹道减速验收

2026-11-07。SDK 0.75.0、机制 90、schema 7、协议 6。

SL01～SL05 通过。Rust 默认和全特性各通过 1688 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建，以及 Node 和 H5 的四人、八人验收和机制 89 眩晕复验。 [批次范围](../batches/SDK-5-73-member-npc-movement-slow.md) · [机器验收](../../artifacts/sdk-5-73/acceptance.json)

普通时钟 Node 与 H5 在四人和八人房分别实测 135、108 世界单位/秒，减速到期后以同一移动意图恢复至 180。准备中的自身技能在减速期间正常提交。所有成员各完成一次准入和结算。 [Node](../../artifacts/sdk-5-73/public/report.json) · [H5](../../artifacts/sdk-5-73/browser/report.json) · [旧机制 H5](../../artifacts/sdk-5-73/legacy89-browser/report.json)

Rust 专项覆盖绑定、强度边界、方向移动、导航、追击、地形、施法阶段、最强值、精确到期、目标私有、免疫、缩时、刷新和净化。Rust 两种构建配置各验证 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-73/native-verification.json)

归档 SHA-256 为 `613f0ef5d602dbc06e2e34a0333478c3782ab78c9db2a48b3efc7b2975bc561a`。包外复验使用 `python3 scripts/verify_sdk_5_45_delivery.py --npc-movement-slow`。 [交付包](../../artifacts/sdk-5-73/delivery/match-kernel-0.75.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-73/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4330/)
