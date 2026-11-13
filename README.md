# game-rust-sdk / match-kernel

面向多款游戏的权威对局 SDK。Rust 提供对局时钟、战斗、状态、地图交互、资产责任和状态发布机制。接入方维护地图、技能、道具、数值、账户及业务适配，并在启动时提供配置清单。

## 接入

- Rust 加载宿主清单 `ConfigCatalog::load(manifest_path)`，把加载结果传给 `Runtime::with_catalog`。公开 API、版本与回执契约见 [SDK 说明](SDK.md) 和 [接入指南](docs/guides/sdk-integration.md)。
- JSONL 宿主用 `kernel-host --configs /absolute/path/to/game/catalog.json` 启动。也可通过 `MATCH_CONFIG_MANIFEST` 指定清单。查询编译能力使用 `kernel-host --sdk-info`。
- Node 宿主的十分钟最小接入路径（成员模式完整对局：建账本 → 就绪 → 战斗拾取 → 撤离结算）见 [Node 快速上手](docs/guides/node-quickstart.md)。
- 技能与机制配置速查（需求 → 机制 → 字段 → 实测示例 → 版本门槛）见 [配置手册](docs/guides/config-cookbook.md)。
- 配置文档使用 schema 2～7，清单使用 schema 1。单场景清单可省略 `default_scenario`，多场景清单须指定默认场景。
- [接口文档](docs/interfaces/) 描述配置字段、协议及机制限制。剩余范围和验收门槛见 [当前执行计划](docs/plans/sdk-delivery-roadmap.md)，已完成切片见 [验收记录](docs/evidence/)。

## 本地 H5

单成员预览需要宿主的清单和账户文件。账户由宿主提供 `principal`、`revision` 和 `stats`，协议 1 另提供 `warehouse_quantity`，协议 2/3 另提供 `assets`。

```sh
npm run h5 -- --configs /absolute/path/to/game/catalog.json \
  --account /absolute/path/to/game/account.json --state /absolute/path/to/game/h5.sqlite
```

多人预览接收宿主清单和 `--host-config`。宿主配置的 `scenarios` 以场景别名为键，每个场景的 `accounts` 指向相对于宿主配置文件的账户文件。

```sh
cargo build --locked --bin kernel-host
node adapters/h5/two-member-server.mjs \
  --configs /absolute/path/to/game/catalog.json \
  --host-config /absolute/path/to/game/host.json --database /absolute/path/to/game/members.sqlite
```

## 测试与打包

```sh
cargo test --locked --all-features
node --test adapters/node/*.test.mjs adapters/h5/*.test.mjs
python3 -B scripts/package_sdk.py --out /absolute/path/to/delivery
```

默认测试使用合成输入。旧版对局回归可用 `MATCH_SDK_TEST_DATA_DIR=/absolute/path/to/external-fixtures cargo test --locked --all-features` 运行。外部资料须沿用历史的 `config/`、`examples/`、`src/config/legacy-v1.json` 和 `src/verification/` 结构。历史批次的命令和配置路径以当时的资料为准。

开发和验收产物遵循 [项目开发规范](AGENTS.md)。
