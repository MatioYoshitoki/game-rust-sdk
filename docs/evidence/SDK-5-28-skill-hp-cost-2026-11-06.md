# SDK-5-28 技能生命成本验收（2026-11-06）

SDK-5-28按HC01～HC10限定签收，0.30.0/协议6/schema7/机制45。每个既有技能可选配置hp_cost（fixed或current_hp_ratio，value及min_hp_after_cost完整提供）；接受冻结定义，提交按当前HP先floor再最低生命截断，在目标/灵力/状态/计划全部重验后与灵力/CD/效果原子生效。声明成本的本人ActionCommitted.hp_cost发布实际整数（含0），未声明与普攻省略，公共费用不泄漏。成本不走伤害链，不消耗盾或触发归功；未提交无费用，成功后重连/取消不退款或重复收费。既有HP/CD full/delta、金融及仅Closed重启责任保持。

保留原1474项并新增20项Rust，默认/全特性同1494项；原90项加4项Node同94项，Clippy/fmt通过。2488份真实发布逐字段跨语言重建，77项公共边界、四槽/四人与八槽/八人H5、三处强杀各同检查点两次金融恢复、原机制1～44关键链、52项包外普通消费、空缓存离线源码/最小消费者全部通过。原hp-ratio/hp-min/hp-clamped三向量从原JSON精确绑定并经真实Rust提交；本批新增执行3/50向量、0/48场景，498内容项路由与14M/9I/9P完整缺口保持。3包映射完成、7包部分实现、15包待实现，仍有22包未完整关闭。

- [实现前HC01～HC10](../batches/SDK-5-28-skill-hp-cost.md)、[接口](../interfaces/skill-hp-cost-v1.md)、[机器签收](../../artifacts/sdk-5-28/acceptance.json)。
- [原生验证](../../artifacts/sdk-5-28/native-verification.json)：新增20项Rust验证配置门禁/末槽、原始三向量、floor/截断边界、七机制/1～8人、准备不收费/定义冻结/变更后当前HP、目标/灵力/状态/计划失败原子、成本不走盾/伤害、重复/重连/退出与私有事实；94项Node含4个实际费用观察/坏事实原子/旧事件/独立身份用例。原1474项Rust（含原100次Runtime释放回归）/90项Node保留，默认/全特性同集、Clippy/fmt通过。
- [原数值绑定](../checklists/SDK-5-28-source-vectors.json)：仅hp-ratio、hp-min、hp-clamped的源JSON输入/expected原样映射到真实Rust提交的私有hp_cost、实际HP差与最低生命；选中fixture内容与原JSON在最终签收再次核对。只完成3个HP成本算术向量，不代表完整技能或50向量通过。
- [跨语言发布](../../artifacts/sdk-5-28/interop-verification.json)：2488帧（73full/2415delta），8接收者、8槽真实HP/CD/准备/计划/空清，默认/全特性向量相同，Rust与JS逐字段精确重建、重复帧幂等；继续原严格HP/ID/基线检查。
- [普通Node公共边界](../../artifacts/sdk-5-28/public/report.json)：77项，含旧64项基底扩展生命成本坏配置11项、恶意费用覆盖2项；所有声明槽实际提交并有本人私有费用事实，未知结算时同伴继续，真实战斗/共享拾取/独立终局。配置及输入拒绝不生成部分责任或成本。
- [真实4页/8页H5](../../artifacts/sdk-5-28/browser/report.json)：逐槽选择器、实际成本显示、真实战斗/竞争收益/终局、同World重连、旧帧拒绝、Sent增量丢失和缓存恢复；无目标错误受控，未捕获异常0/未决0。[截图检查](../../artifacts/sdk-5-28/visual-verification.json)。
- [强杀恢复](../../artifacts/sdk-5-28/crashes/report.json)：三边界、同检查点各两次，部分准入在World前；另两处先有真实多槽/生命成本/周期/盾，再SIGKILL。责任/hash/资产冻结，Closed金融收尾；不恢复World/HP费用观察/CD/计划/基线，重复恢复不改余额。
- 同候选旧关键链[协议4](../../artifacts/sdk-5-28/legacy4/report.json)、[协议5](../../artifacts/sdk-5-28/legacy5/report.json)、[37](../../artifacts/sdk-5-28/legacy6/report.json)、[38](../../artifacts/sdk-5-28/legacy38/report.json)、[39](../../artifacts/sdk-5-28/legacy39/report.json)、[40](../../artifacts/sdk-5-28/legacy40/report.json)、[41](../../artifacts/sdk-5-28/legacy41/report.json)、[42](../../artifacts/sdk-5-28/legacy42/report.json)、[43](../../artifacts/sdk-5-28/legacy43/report.json)、[44](../../artifacts/sdk-5-28/legacy44/report.json)通过，包外保留原协议1～3。
- [52项包外普通消费](../../artifacts/sdk-5-28/delivery/independent-verification.json)、[空缓存离线源码](../../artifacts/sdk-5-28/delivery/source-verification.json)：原50项加4槽/8槽两套生命成本，隐藏SDK配置及包内samples后显式使用包外数据，普通release而非replay。相同源crate/vendor、空Cargo缓存、离线默认/全特性1494同集及Clippy、最小Rust消费方、交付H5只读零异常通过。

[交付包](../../artifacts/sdk-5-28/delivery/match-kernel-0.30.0-aarch64-apple-darwin.tar.gz) SHA-256：`4169bb4bd71580a94050af02ca9ba1b5d8b3f46570c01c469ce9c17e723bb170`；源crate：`5643f1038da745ae1d6be3b88458ae3fbc232c5864eaae4ff68ebf120e360272`；二进制：`24e3c7efafa2dd749f5a3d5e0afa3559be90d27134b931cd2bcded344ffb0bb0`。

24475份起点旧JSON/原资料/SDK-5-17～27验收证据保护；完整起点文件清单另留baseline-all-files.json；11份macOS Finder元数据.DS_Store不列为验收证据，排除清单为baseline-exclusions.json（仅artifacts/.DS_Store随系统更新，其余原资料/证据哈希一致）。Cargo接入示例的非JSON版本清单按0.30.0更新。用户此前清理的早期artifacts例外保持，不伪造历史证据。预检在preflight-public/preflight-browser，本批最终报告均绑定交付候选。包外脚本曾保留机制44的旧断言，在源码默认/全特性/Clippy及最小消费方构建通过后被机制45能力描述触发停止；修正脚本并按delivery/source-stage.json核对相同交付包/源crate及成功日志摘要后复用源码阶段，重新构建最小消费方并完成剩余消费。首次失败日志在delivery-attempt-1.log，产品代码/交付包未改变。新[4219预览](http://127.0.0.1:4219/)与.local/sdk-5-28-demo.sqlite独立，[预览只读检查](../../artifacts/sdk-5-28/local-preview.json)未建局；旧端口/账本不变。

本批只关闭配置生命成本切片；真意/多发倍率、完整施法阶段、动态不同技能表、友军/PvP、召唤、模式和正式接入仍开放。
