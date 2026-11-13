//! Counter mutations never create a carrier or a second lifetime.
use super::*;
impl Statuses {
    pub(crate) fn adjust_counter(
        &mut self,
        id: &str,
        amount: u32,
        now: u64,
        refresh_duration: Option<u64>,
        events: &mut Vec<Event>,
    ) -> Option<(StatusView, u32)> {
        let state = self.entries.iter_mut().find(|s| {
            s.instance_id == id && s.stack_counter == Some(true) && now < s.expires_at_ms
        })?;
        let previous = state.stacks?;
        let maximum = state.max_stacks?;
        state.stacks = Some(previous.saturating_add(amount).min(maximum));
        if let Some(duration) = refresh_duration {
            state.applied_at_ms = now;
            state.expires_at_ms = now + duration;
        }
        events.push(Event::StatusStacked {
            status: state.clone(),
            added_stacks: state.stacks.unwrap() - previous,
        });
        Some((state.clone(), previous))
    }
}
#[cfg(all(test, match_sdk_test_data))]
mod paid_resource_policy_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn paid_resource_private_zero_policy_lifetime_is_bounded_by_real_instances() {
        let mut statuses = Statuses::default();
        let d:StatusDefinition=serde_json::from_value(json!({"id":"charges","mechanism":"timed_compound_modifier","base_modifiers":[{"attribute":"def","modifier_bps":0}],"stack_counter":{"initial":3,"maximum":3,"zero_policy":"remove"},"duration_ms":8000,"duplicate_policy":"refresh","source_scoped":true})).unwrap();
        let x: serde_json::Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/counter-a.json"
        ))
        .unwrap();
        let rules: StatusRules = serde_json::from_value(x["pve"]["status_rules"].clone()).unwrap();
        let actor: StatusEntity =
            serde_json::from_value(json!({"kind":"participant","id":"source","generation":1}))
                .unwrap();
        for n in 0..500 {
            statuses.apply(
                std::slice::from_ref(&d),
                (&actor, 100),
                (&actor, 1000),
                n * 10000,
                &rules,
                &mut vec![],
            );
            assert_eq!(statuses.remove_zero_counters.len(), 1);
            let id = statuses.entries()[0].instance_id.clone();
            if n % 2 == 0 {
                assert!(statuses.consume(&id, 3, &mut vec![]));
            } else {
                statuses.expire(n * 10000 + 8000, &mut vec![]);
            }
            assert!(statuses.remove_zero_counters.is_empty());
            assert!(statuses.entries().is_empty());
        }
    }
}
