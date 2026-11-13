#!/usr/bin/env sh
# Linux 生产产物离线构建：x86_64 + aarch64 musl 静态二进制（不依赖宿主 glibc 版本）。
#
# 用法（任选其一）：
#   1) 在任意 Linux x86_64/aarch64 机器上：安装 rustup(1.96.0 由 rust-toolchain.toml 自动对齐)
#      与 musl 工具链后直接运行本脚本；
#   2) Docker 内（推荐，工具链完全固定）：
#        docker run --rm -v "$PWD:/repo" -w /repo rust:1.96.0 sh scripts/build-linux.sh
#      （arm64 宿主机跑 arm64 镜像、x86_64 宿主机跑 amd64 镜像，各得对应架构产物；
#        需要双架构时分别在两类机器/容器上各跑一次，或用 buildx + QEMU。）
#
# 输入：仓库检出 + vendor/（`cargo vendor --offline --locked --versioned-dirs vendor` 已生成）
# 输出：dist/kernel-host-linux-<arch> 与 .sha256
set -eu
cd "$(dirname "$0")/.."

TARGET_ARCH="$(uname -m)"
case "$TARGET_ARCH" in
  x86_64) TARGET="x86_64-unknown-linux-musl" ;;
  aarch64|arm64) TARGET="aarch64-unknown-linux-musl" ;;
  *) echo "unsupported arch: $TARGET_ARCH" >&2; exit 2 ;;
esac

# musl 工具链（Debian/Ubuntu 基镜像）；非 root 时加 sudo。
if command -v apt-get >/dev/null 2>&1 && ! command -v musl-gcc >/dev/null 2>&1; then
  apt-get update >/dev/null && apt-get install -y musl-tools >/dev/null
fi
if command -v rustup >/dev/null 2>&1; then
  rustup target add "$TARGET"
fi

# 离线 + 锁定版本：依赖只来自 vendor/，Cargo.lock 与工具链由 rust-toolchain.toml 钉死。
export CARGO_NET_OFFLINE=true
cargo build --release --locked --offline --target "$TARGET" --bin kernel-host

mkdir -p dist
cp "target/$TARGET/release/kernel-host" "dist/kernel-host-linux-${TARGET_ARCH}"
shasum -a 256 "dist/kernel-host-linux-${TARGET_ARCH}" > "dist/kernel-host-linux-${TARGET_ARCH}.sha256"
"dist/kernel-host-linux-${TARGET_ARCH}" --sdk-info | head -c 200; echo
echo "built: dist/kernel-host-linux-${TARGET_ARCH}"
