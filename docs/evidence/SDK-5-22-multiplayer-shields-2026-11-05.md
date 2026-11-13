# SDK-5-22 多人吸收盾执行记录

2026-11-05，0.24.0/协议6/schema7/机制39。实现前冻结[MS01～MS12](../batches/SDK-5-22-multiplayer-shields.md)，[逐项映射](../checklists/SDK-5-22-acceptance.json)及[机器签收](../../artifacts/sdk-5-22/acceptance.json)绑定同一候选。

复用Statuses吸收盾与多人周期/伤害链，不按地图或成员复制机制。数值、绑定、重复策略及预算由配置初始化。授予以目标基础maxHp计量、共享目标总帽；refresh保ID并更新实际来源，ignore保原来源/期限。直接/DOT均先DEF/取整/minimum_damage，再盾，最后HP裁剪；超杀不得先按HP裁剪。DOT.absorbed记真实总吸收，HOT不耗盾或转盾。直接接触全吸收仍执行命中附带/基础回灵与撤离中断，DOT只在HP损失时打断。

本人盾总量和实例一致；NPC总量是共享战斗事实，可见实例/Granted/Absorbed/Removed按本人来源/NPC自身权限切片，私有amount不能跨接收者相加。解码先校验余额/身份/基线后原子替换；null或坏帧保持缓存。来源进入settling立即清理，不等金融回执；NPC复位清旧代。重连不补发/续期，重启只Closed金融责任。

- [原生回归](../../artifacts/sdk-5-22/native-verification.json)：默认/全特性同1311项Rust，保留1275、新增36；54项Node保留48，Clippy/fmt通过。包括1/2/4/8成员共享帽、来源转移/忽略、期限/顺序、超杀/最小伤害、全吸收撤离差异、私有明细、退出/复位与100次真实带盾公共Runtime释放。
- [真实Rust→JS](../../artifacts/sdk-5-22/interop-verification.json)：1520帧，57 full/1463 delta，8接收者逐字段精确一致、重复幂等，默认/全特性向量摘要相同。涵盖本人/NPC盾量与部分消耗，公开消费另验授予、刷新、耗尽、到期和清空。
- [33项公开边界](../../artifacts/sdk-5-22/public/report.json)：13项非法Create无部分I/O、18项配置/绑定/预算拒绝、两套四人/八人真实Node闭环。观察自身/NPC授盾、直接/DOT吸收、来源退出、重连与缓存恢复，独立结算/未决0。持久化原始Rust事件核验consumptions原实例权限与切片amount；单席金融未知不延迟同伴。
- [四人/八人H5](../../artifacts/sdk-5-22/browser-recheck/report.json)：独立页面实际观察盾/周期及吸收，P00退出、P01重连、旧帧拒绝、丢Sent delta和清缓存经request_full恢复，再完成战斗、共享拾取和各自终局。console/page异常0，宿主释放。首次八页验收的同时持有短时HOT假设在负载下失效（真实效果已发生）；修正为逐页记录已发生事件，重连分别验证保留或按原期限自然到期，未修改机制/数值以迁就脚本。
- [三个SIGKILL/六次恢复](../../artifacts/sdk-5-22/crashes/report.json)：部分准入（World未激活）、最后一席终局未决、提交成功回执丢失；后两处此前实际带盾/DOT/HOT。各同一检查点恢复两次，改变外部配置仍保原成员/hash/资产，Closed未决0，盾/周期/full/delta计数均0。
- 旧关键链：[协议4的16项](../../artifacts/sdk-5-22/legacy4/report.json)、[协议5的20项](../../artifacts/sdk-5-22/legacy5/report.json)、[原机制37的31项](../../artifacts/sdk-5-22/legacy6/report.json)、[原机制38的29项](../../artifacts/sdk-5-22/legacy38/report.json)，包外另含协议1～3。
- [空缓存离线源码](../../artifacts/sdk-5-22/delivery/source-verification.json)、[40项包外普通消费](../../artifacts/sdk-5-22/delivery/independent-verification.json)：同一真实源crate/vendor和普通release，默认/全特性同集及Clippy；保留原38消费并加两套盾配置。普通消费时隐藏SDK回归与包内samples，显式读包外配置；交付H5只读启动、异常0，未启用replay。

[交付包](../../artifacts/sdk-5-22/delivery/match-kernel-0.24.0-aarch64-apple-darwin.tar.gz)SHA-256：`60c970255d92e7a6230013da017a15b64037da07f8e14e4b8d92e52bf5a17888`；源crate：`6f0ac6123aa65dade296c33570528dfd8c759af3010b7a5016396dc54a8038e3`；普通二进制：`4cdf0430d4fd3413cc8181042da068ec9acc36fd0a205a4bd9695767d6259016`。新[4201预览](http://127.0.0.1:4201/)使用独立.local/sdk-5-22-demo.sqlite，[只读检查](../../artifacts/sdk-5-22/local-preview.json)未建局；旧4173/4190/4193/4195/4198预览与账本保持。7144份起点现存旧配置/JSON示例、原来源及SDK-5-17～21证据/归档逐字节保护；用户历史清理例外保留。

R2三包与R3.2/R3.3共5包仍部分实现、17包待实现，22包未完整关闭。友军/PvP、受治疗修正/控制及其他动作/完整伤害链、模式/观战/正式接入继续开放；14M/9I/9P与原48场景/50向量不因本批自动关闭，原场景/向量新增执行数为0。
