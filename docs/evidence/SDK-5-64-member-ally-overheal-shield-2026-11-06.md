# SDK-5-64 友方即时治疗溢出转护盾验收

2026-11-06。SDK 0.66.0、机制 81、schema 7、协议 6。

AO01～AO05 通过。Rust 默认和全特性各通过 1652 项，含新增 4 项。118 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 80 指定友方护盾兼容复验。 [批次范围](../batches/SDK-5-64-member-ally-overheal-shield.md) · [机器签收](../../artifacts/sdk-5-64/acceptance.json)

| 样例 | 治疗基础量 | 实回气血 | 实授护盾 | 清理原因 |
|---|---:|---:|---:|---|
| 四人满血 | 100 | 0 | 50 | 来源离场 |
| 八人缺 160 | 300 | 160 | 70 | 目标离场 |

Node 和 H5 核对施法者与目标的事件顺序、气血与护盾结果、私有实例及退场结算。 [Node](../../artifacts/sdk-5-64/public/report.json) · [H5](../../artifacts/sdk-5-64/browser/report.json)

Rust 专项验证目标倍率和取整、气血优先、单盾和总盾上限、零新增时保留期限、状态槽不足仍保留治疗，以及不同来源的退场隔离。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-64/native-verification.json)

归档 SHA-256 为 `64d444d5470129bd62ceafa976a640c61eb7b6b26703cf97ccdca4eddf0eabd8`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --ally-overheal-shield`。 [交付包](../../artifacts/sdk-5-64/delivery/match-kernel-0.66.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-64/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4312/)
