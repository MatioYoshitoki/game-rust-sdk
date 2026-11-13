# SDK-5-52 多人成员持续治疗倍率验收

2026-11-06。SDK 0.54.0、机制 69、schema 7、协议 6。

MH01～MH06 通过。Rust 默认和全特性各通过 1611 项，含新增 4 项。113 项 Node 测试、Clippy 和格式检查通过。交付包通过仓库外空缓存离线构建、普通时钟 Node 和 H5 的四人及八人样例，以及机制 68 周期减伤兼容回归。 [批次范围](../batches/SDK-5-52-member-periodic-healing-received.md) · [机器签收](../../artifacts/sdk-5-52/acceptance.json)

| 样例 | 倍率修正 | 每跳基础量 | 生效期实回 | 倍率到期后实回 |
|---|---:|---:|---:|---:|
| 四人房 | +50% | 51 | 76 | 51 |
| 八人房 | −50% | 51 | 25 | 51 |

两房击杀 NPC 拾取回春丸两份，受持续伤害后使用一份。即时回复均为 10，后续自身 HOT 事件只发给用药成员，退出时各有一份入账。 [Node](../../artifacts/sdk-5-52/public/report.json) · [H5](../../artifacts/sdk-5-52/browser/report.json)

Rust 专项覆盖配置边界、倍率相加、下限、向下取整、宽整数、当前缺口、满血、延后执行、保护和同刻致死 DOT。两种构建配置各运行 10 项真实时钟压力测试。 [原生验证](../../artifacts/sdk-5-52/native-verification.json)

归档 SHA-256 为 `9f6b9147b7db22773ac7d37341026153706deb66c59557c6d6b8af7f87c19c2d`。包外复验运行 `python3 scripts/verify_sdk_5_45_delivery.py --periodic-healing-received`。 [交付包](../../artifacts/sdk-5-52/delivery/match-kernel-0.54.0-aarch64-apple-darwin.tar.gz) · [包外验证](../../artifacts/sdk-5-52/delivery/independent-verification.json) · [H5 演示](http://127.0.0.1:4284/)
