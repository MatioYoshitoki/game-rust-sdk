# 供应链审计结果（0.185.0）

2026-11-13。对象：[Cargo.lock](../../Cargo.lock)（0.185.0，21 个第三方依赖）。审计配置：[deny.toml](../../deny.toml)。

## cargo deny（v0.20.2，`cargo deny check`）——四维全过，exit 0

| 维度 | 结果 | 说明 |
|---|---|---|
| advisories | **ok** | RustSec 告警零、yanked 零 |
| bans | **ok** | 21 依赖零重复版本（每名恰一版） |
| licenses | **ok** | 全部落入白名单（见下）；根 crate 补 `license = "MIT OR Apache-2.0"` 后通过 |
| sources | **ok** | 仅 crates.io 注册表，无未知来源 |

许可证分布：`MIT OR Apache-2.0` ×16、`MIT` ×2、`Unlicense OR MIT` ×1、`(MIT OR Apache-2.0) AND Unicode-3.0` ×1（unicode-ident，AND 表达式要求同时接受 Unicode-3.0）、`MIT/Apache-2.0` ×1。完整清单：[dependency-inventory-0.185.0.md](dependency-inventory-0.185.0.md)。

## cargo audit（cargo-audit，独立复核）——exit 0

命令 `cargo audit --file Cargo.lock`：RustSec advisory-db 1490 条公告加载，扫描 22 个 crate，**零漏洞、零告警、零 yanked**（cargo-audit 有告警即非零退出）。

## 复检方式

```sh
cargo install cargo-deny --locked && cargo deny check   # deny.toml 随仓库与交付包提供
cargo install cargo-audit --locked && cargo audit
```
