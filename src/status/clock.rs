//! A clock-only refresh preserves the instance and every accumulated value.
use super::*;
impl Statuses {
    pub(crate) fn refresh_deadline(
        &mut self,
        id: &str,
        now: u64,
        expires: u64,
        events: &mut Vec<Event>,
    ) -> bool {
        let Some(s) = self
            .entries
            .iter_mut()
            .find(|s| s.instance_id == id && now < s.expires_at_ms && expires > now)
        else {
            return false;
        };
        s.expires_at_ms = expires;
        events.push(Event::StatusRefreshed { status: s.clone() });
        true
    }
}
