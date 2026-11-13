# SDK-5-39 自动选敌短追击验收

2026-11-06。SDK 0.41.0，协议 6，schema 7，机制 56。

在对局配置中为普通普攻声明自动选敌范围。Rust 优先处理射程内候选，射程外按距离与 ID 选择同一目标并复用短追击。

AP01～AP06 通过。默认和全特性各通过 1566 项 Rust 测试，含新增 5 项。112 项 Node 测试、Clippy 和格式检查也已通过。交付包通过空缓存离线构建、包外 Node、真实四页和八页 H5，以及机制 55 包外回归。 [批次范围](../batches/SDK-5-39-auto-basic-pursuit.md) · [机器签收](../../artifacts/sdk-5-39/acceptance.json)

| 样例 | Node / H5 移动距离 | Node / H5 普攻提交数 | Node / H5 结算人数 |
|---|---:|---:|---:|
| 四人自动追上并提交 | 165.0 / 165.0 | 1 / 1 | 4 / 4 |
| 八人自动拒绝后锁定并取消 | 5.4 / 48.6 | 0 / 0 | 8 / 8 |

四人房使用普通普攻输入，自动选中最近可见目标后以原动作发射一枚弹道。八人房的目标超出自动范围，普通普攻被拒。显式锁定后可追击，移动中取消后零次提交。两组同伴均可移动，成员离场后各结算一次。 [普通宿主](../../artifacts/sdk-5-39/public/report.json) · [浏览器](../../artifacts/sdk-5-39/browser/report.json)

Rust 专项核对配置门禁、射程内优先与冷却队列、等距选敌、独立范围、实际命中及冷却，也核对阻挡、目标换代、时限、幂等、重连与成员隔离。默认和全特性各有 10 项真实时钟压力用例分组运行。 [原生测试](../../artifacts/sdk-5-39/native-verification.json)

包外验证使用外置配置并隐藏交付样例与源码配置。机制 55 的显式追击和中途取消样例通过。12855 份已有文件通过哈希核对。 [包外验证](../../artifacts/sdk-5-39/delivery/independent-verification.json) · [机制 55 回归](../../artifacts/sdk-5-39/legacy55/report.json)

归档 SHA-256 为 `1c2ada0a43290492e3ccbf5e465abe01f8fc4c32decc36a8c6aebc6b79eb8aaa`。源码 crate SHA-256 为 `7713b3d5206ac04135c7c27217dbd4c980ea4e7b80c4c59dbb2db904caec7296`。二进制 SHA-256 为 `026cbddc7e1783a943b521a7182d965f0953f574b951d599ecaf316ccd0a2ac7`。 [交付包](../../artifacts/sdk-5-39/delivery/match-kernel-0.41.0-aarch64-apple-darwin.tar.gz)

演示使用本批独立账本。四人房保留「自动选择」后点击普攻。八人房可改为显式锁定后点击普攻，再取消追击。 [打开演示](http://127.0.0.1:4252/)
