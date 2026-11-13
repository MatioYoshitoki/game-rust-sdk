# NPC 命中属性

机制 99 / schema 7 / 协议 6。在 `pve.stats` 或 `pve.additional_templates.*.stats` 配置 `hit` 命中点。

```json
{
  "hp": 1000,
  "spirit": 200,
  "atk": 100,
  "def": 100,
  "spd": 0,
  "crit": 0,
  "dodge": 0,
  "hit": 100
}
```

`hit` 为 0～1000000 的整数，省略等价于 0。NPC 弹道和近战对合法成员尝试命中时，Rust 读取当前来源的命中点与成员闪避状态，计算 `min(0.5, dodgeBps / (10000 + hit × 100))`。例如 5000 bps 闪避遇到 100 点命中时，实际闪避率为 25%。闪避成功会结束本次接触，近战使用原攻击间隔。

宿主通过 `kernel-host --sdk-info` 查询 `memberNpcHit`。客户端观察防守成员的私有闪避状态、弹道和现有 `damage` 事实。 [范围与验收](../batches/SDK-5-82-member-npc-hit.md)
