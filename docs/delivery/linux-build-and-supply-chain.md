# Linux 生产构建与供应链审计（0.185.0）

生产环境（Linux，Docker/PM2）的构建与供应链材料。本仓库主开发机为 macOS aarch64（交付包内 `bin/kernel-host` 为该平台），Linux 产物按下述**可复现离线路径**由任一 Linux x86_64/aarch64 机器或容器构建。

## 工具链钉死

- [rust-toolchain.toml](../../rust-toolchain.toml)：`channel = "1.96.0"`（profile minimal）。rustup 检测到该文件自动安装并对齐；所有构建（本机、交付包、Linux 产物）共用同一版本。
- 依赖版本由 [Cargo.lock](../../Cargo.lock) 锁定；交付包经 `vendor/` + `.cargo/config.toml` 替换源 + `CARGO_NET_OFFLINE=true` 全离线构建（打包器内建门禁）。

## Linux 构建路径（x86_64 与 aarch64）

产物为 **musl 静态二进制**（`x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl`），不依赖宿主 glibc 版本，可直接放入任意 `FROM scratch`/distroless/alpine 容器或 PM2 直接执行。

```sh
# Docker（推荐）：在 x86_64 机器产 x86_64、arm64 机器产 aarch64
docker build -f scripts/Dockerfile.linux-build -t match-kernel-build .
docker run --rm match-kernel-build        # 打印 dist/*.sha256

# 或任意 Linux 机器直跑（装 rustup 即可，工具链自动对齐 1.96.0）
sh scripts/build-linux.sh
# 产出 dist/kernel-host-linux-<arch> 与 .sha256
```

脚本步骤与校验：`rustup target add <musl-target>` → `cargo build --release --locked --offline --target <target> --bin kernel-host`（依赖仅取 `vendor/`）→ sha256 落盘。构建后自检：产物 `--sdk-info` 的 `crateVersion` 必须等于 0.185.0。

**生产部署**：Docker 镜像拷入产物 + 配置清单挂载 + `PM2`/容器以 `kernel-host --configs /path/catalog.json` 启动；协议面与启动旗标见包内 `docs/guides/sdk-integration.md`。

## 依赖清单（vendor，21 项）

全量名称/版本/许可证见 [dependency-inventory-0.185.0.md](dependency-inventory-0.185.0.md)（与交付包 `vendor/` 及 `distribution.json` 一致）。许可证分布：`MIT OR Apache-2.0` ×16、`MIT` ×2、`Unlicense OR MIT` ×1、`(MIT OR Apache-2.0) AND Unicode-3.0` ×1、`MIT/Apache-2.0` ×1 —— 全部落入 [deny.toml](../../deny.toml) 许可证白名单（MIT/Apache-2.0/BSD/ISC/Unicode/Zlib/CC0）。

## cargo audit / cargo deny 结果

见 [audit-results-0.185.0.md](audit-results-0.185.0.md)：RustSec 扫描零漏洞零告警（1490 条公告，exit 0）、21 依赖零重复版本、许可证全落白名单；随发布附为 Release 资产。
