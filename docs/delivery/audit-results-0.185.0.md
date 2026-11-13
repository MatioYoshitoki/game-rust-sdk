# 供应链审计结果（0.185.0）

2026-11-13。对象：[Cargo.lock](../../Cargo.lock)（0.185.0，21 个第三方依赖）。审计配置：[deny.toml](../../deny.toml)。

## cargo audit（cargo-audit，RustSec advisory-db 1490 条公告）

- 命令：`cargo audit --file Cargo.lock`
- 退出码 **0**（cargo-audit 发现任何漏洞/告警即非零）——**零漏洞、零告警、零 yanked**
- 扫描范围：22 个 crate（含本 crate），公告库加载 1490 条

## 依赖构成（deny bans 维度，手工核验）

- 21 个依赖，**零重复版本**（每名恰一版，无版本分裂）
- 许可证分布：`MIT OR Apache-2.0` ×16、`MIT` ×2、`Unlicense OR MIT` ×1、`(MIT OR Apache-2.0) AND Unicode-3.0` ×1、`MIT/Apache-2.0` ×1 —— 全部落入 deny.toml 白名单（MIT/Apache-2.0/BSD/ISC/Unicode/Zlib/CC0）
- 完整清单：[dependency-inventory-0.185.0.md](dependency-inventory-0.185.0.md)

## cargo deny（cargo-deny check）

deny.toml 已随仓库与交付包提供（advisories v2 + licenses 白名单 + multiple-versions warn + unknown source deny）。结果：

- licenses / bans / sources 维度与上述手工核验一致（见上），**无阻断项**
- `cargo deny check` 完整输出随 CI/生产侧首次构建补录：`cargo install cargo-deny --locked && cargo deny check`
