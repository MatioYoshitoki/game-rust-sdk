# DELIVERY-03 世界几何、导航与对局 AI 签收

2026-11-10，SDK 0.141.0，机制 105，schema 7，协议 6。GN01～GN06 的 SDK 机制与公开接入使用独立合成输入通过。 [模块范围](../batches/SDK-5-90-geometry-navigation-and-ai.md) · [Boss 阶段契约](../interfaces/npc-combat-ai-v1.md#boss-阶段) · [参战 NPC 契约](../interfaces/participant-ai-v1.md)

| 验收 | 结果 |
|---|---|
| GN01～GN04 几何、动态阻挡和地形 | 复验边界、障碍、阶段性阻挡、方向性高差和地形伤害。组合对局中，真实 Node 客户端观察到定时挡板开启和关闭。成员直行停在崖前 x≈30.8，随后沿坡道导航至高地。 |
| GN05 怪物与 Boss | 复验独立仇恨、强制目标、同群协助、支援和远程行为。单次伤害使 Boss 依次从 1 进入 2、3，保留已经准备的技能。治疗后阶段保持，下一次提交使用第三阶段槽位。Node 重连全量恢复第 3 阶段，实际归位后实体升到代次 2 并重回第 1 阶段。 |
| GN05 参战 NPC | 保护中保持静止，恢复就绪后走原生动作。Rust 组合验证跳过他人专属和满包物品、共享资源只移除一次，以及低血撤离。Node 对局的自动席位独立搜索、通过指定单位技能击杀、拾取并撤离。物品余额由 4 变为 6，人工席位保持 4，公开投影沿用普通成员身份。 |
| GN06 公开宿主 | 两组双成员对局在工作区和离线包内各复跑一次。每席位入场和结算各一次，未决责任为零。外部玩法指令无法接管自动席位。协议 Schema 验证工作区联调的 213 个公开发布封包，其中 24 个携带 Boss 阶段。 |

Rust 默认和全特性各 36 项，Node/H5 自动化 217 项，格式、Clippy 和最小 Rust 宿主离线检查通过。`aarch64-apple-darwin` 离线包校验 1802 个文件，包内 release JSONL 与 Node 适配器通过同一组合。归档 SHA-256 为 `8600f4e6fb6d3a6d8a1e40cf704f59dc0ab89c03de060b1a74db06e0b1e06d7e`，源码 crate 为 `5d40da89269df069a671b07fac5dabe6d1a598a783b97ed9f024bacabfbfb70a`，release JSONL 为 `37b2d94ee2725ae33614af3dbab8c60a39324df3c1ed2b9af01e43bce1481b37`。 [组合回归](../../src/world/two_members/module03_contract_tests.rs) · [平面几何](SDK-5-90-world-geometry-2026-11-10.md) · [地形验收](SDK-5-90-terrain-2026-11-10.md) · [怪物战斗 AI](SDK-5-90-npc-combat-ai-2026-11-10.md)

正式地图的装饰与动态阵法几何按 G-02 由宿主提供并核验。参战 NPC 的容器和实例物品组合归模块 04，生产策略和容量证据随实际宿主接入补齐。
