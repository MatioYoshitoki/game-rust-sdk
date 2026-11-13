# match-kernel 本地交付包

交付包包含 SDK 源码、当前平台的 JSONL 二进制、离线编译依赖、参考宿主以及配置与协议契约。游戏维护自己的清单、配置、账户和存储目录。

## 校验交付

```sh
python3 -B verify_bundle.py
bin/kernel-host --sdk-info
```

[distribution.json](distribution.json) 记录版本、目标平台与源码摘要。[support.json](support.json) 记录二进制支持的协议、配置 schema 和机制版本。

## 启动

```sh
bin/kernel-host --configs /absolute/path/to/game/catalog.json
```

Rust 使用 `ConfigCatalog::load` 与 `Runtime::with_catalog`。协议 1 的参考消费方可离线运行：

```sh
cargo run --offline --locked --manifest-path consumer/Cargo.toml -- \
  /absolute/path/to/game/catalog.json /absolute/path/to/game/account.json
```

多人 H5 参考宿主使用外部清单、宿主场景与账户映射、独立账本：

```sh
node hosts/h5/two-member-server.mjs --binary "$PWD/bin/kernel-host" \
  --configs /absolute/path/to/game/catalog.json \
  --host-config /absolute/path/to/game/host.json --database /absolute/path/to/game/members.sqlite
```

查阅 [SDK 接入说明](sdk/SDK.md)、[接入指南](docs/guides/sdk-integration.md)、[Node 快速上手](docs/guides/node-quickstart.md)（十分钟跑通成员模式完整对局）和 [接口契约](docs/interfaces/)。Linux 生产部署（x86_64/aarch64 musl 静态产物）与供应链审计见 [构建与供应链](docs/delivery/linux-build-and-supply-chain.md)。部署前按宿主的存储和传输实现验收准入、发布回执、结算和恢复。
