//! 训练模式房主工具扩展（SDK-5-99，机制 107）：运行时按注册模板新增/移除目标实例。
//!
//! 受信宿主 RPC；归属 = 请求中的席位；作用域按该席 `owner_scope`（共享时全体可攻击）。
//! 白名单 = 目录注册模板（`additional_templates` + `template_id`）；落点需足迹可清。
use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnTrainingTargets {
    pub key: MatchKey,
    pub request_id: String,
    pub participant_id: String,
    pub template_id: String,
    pub positions: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DespawnTrainingTargets {
    pub key: MatchKey,
    pub request_id: String,
    pub participant_id: String,
    pub ids: Vec<String>,
}
