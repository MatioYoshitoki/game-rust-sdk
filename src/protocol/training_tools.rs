//! 训练模式房主有限管理工具（SDK-5-98，机制 106）。
//!
//! 受信宿主 RPC：成员输入通道不注册这两个动作，成员客户端无法伪造；
//! 内核不引入"房主"概念，只校验训练配置开关与训练上下文。
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetTrainingCooldowns {
    pub key: MatchKey,
    pub request_id: String,
    pub targets: TrainingToolTargets,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreTrainingMember {
    pub key: MatchKey,
    pub request_id: String,
    pub targets: TrainingToolTargets,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TrainingToolTargets {
    All,
    Participant { participant_id: String },
}
