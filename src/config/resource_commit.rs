//! Explicit resource reservation and one commit boundary; no early debit.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceCommitPolicy {
    pub reserve_at: ReserveAt,
    pub consume_at: Commit,
    pub cooldown_starts_at: Commit,
    pub refund_before_commit: Refund,
}
impl ResourceCommitPolicy {
    pub(crate) fn reserves(self) -> bool {
        self.reserve_at == ReserveAt::Accept
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Refund {
    pub kind: Full,
}
macro_rules! string_enum {
    ($name:ident, $($variant:ident => $text:literal),+) => {
        #[derive(Clone,Copy,Debug,PartialEq,Eq,Deserialize,Serialize)]
        #[serde(try_from="String",into="String")]
        pub(crate) enum $name {$($variant),+}
        impl TryFrom<String> for $name {type Error=&'static str;fn try_from(value:String)->Result<Self,Self::Error>{match value.as_str(){$($text=>Ok(Self::$variant),)+_=>Err("CONFIG_RESOURCE_COMMIT_ENUM")}}}
        impl From<$name> for String {fn from(value:$name)->Self{match value {$($name::$variant=>$text.into()),+}}}
    };
}
string_enum!(ReserveAt, Accept=>"accept", None=>"none");
string_enum!(Commit, Commit=>"commit");
string_enum!(Full, Full=>"full");
pub(super) fn fields(v: &serde_json::Value) -> Result<(), &'static str> {
    for s in super::player_skills::raw(v) {
        if let Some(policy) = s.get("resource_commit") {
            if v["schema_version"] != 7
                || v["mechanism_version"] != 105
                || !v["members"].is_object()
            {
                return Err("CONFIG_RESOURCE_COMMIT_CAPABILITY");
            }
            if !policy.is_object() {
                return Err("CONFIG_RESOURCE_COMMIT_SHAPE");
            }
        }
    }
    Ok(())
}
