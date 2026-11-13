//! Native-only classification. It is never accepted from a client or projected on a plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ActionOrigin {
    Basic,
    Skill(String),
}
