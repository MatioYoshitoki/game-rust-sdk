use super::status_tests::{active_engine_with_config, advance, world};
use super::*;
pub(super) fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/dual-skill-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone()
}
fn cast(w: &mut World, c: &Config, index: usize, now: u64) -> Result<Vec<Event>> {
    let id = c.player_skills().nth(index).unwrap().id.clone();
    w.action(
        &Action::Cast {
            skill_id: id,
            direction: [1., 0.],
        },
        &format!("skill-{index}-{now}"),
        c,
        now,
    )
}
fn committed(e: &[Event]) -> Vec<(Option<String>, u32)> {
    e.iter()
        .filter_map(|e| {
            if let Event::ActionCommitted {
                skill_id,
                spirit_cost,
                ..
            } = e
            {
                Some((skill_id.clone(), *spirit_cost))
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn multi_independent_costs_cooldowns_and_primary_alias() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0, 0).unwrap();
    let e = advance(&mut w, &c, 0, 120);
    assert_eq!(committed(&e), vec![(Some("ward-bolt".into()), 12)]);
    assert_eq!(w.spirit, 188);
    assert_eq!(w.skill_ready_at, 1920);
    assert_eq!(w.skill_ready(&c, "renewal-bolt"), 0);
    cast(&mut w, &c, 1, 420).unwrap();
    let e = advance(&mut w, &c, 420, 600);
    assert_eq!(committed(&e), vec![(Some("renewal-bolt".into()), 8)]);
    assert_eq!(w.spirit, 180);
    assert_eq!(w.skill_ready_at, 1920);
    assert_eq!(w.skill_ready(&c, "renewal-bolt"), 3300);
    let s = w.snapshot(&c, 600);
    let rows = s.player_skills.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].ready_at_ms, s.skill_ready_at_ms);
    assert_eq!(rows[1].ready_at_ms, 3300);
}
#[test]
fn multi_pending_blocks_both_skills_and_basic_without_side_effects() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0, 0).unwrap();
    for i in 0..2 {
        assert_eq!(cast(&mut w, &c, i, 30), Err(Error::InvalidState));
    }
    assert_eq!(
        w.action(&Action::Attack {}, "basic", &c, 30),
        Err(Error::InvalidState)
    );
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready_at, 0);
    assert_eq!(w.pending.as_ref().unwrap().due_ms, 120);
}
#[test]
fn multi_recovery_cannot_be_bypassed_by_any_attack() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    for i in 0..2 {
        assert_eq!(cast(&mut w, &c, i, 419), Err(Error::InvalidState));
    }
    assert_eq!(
        w.action(&Action::Attack {}, "basic", &c, 419),
        Err(Error::InvalidState)
    );
    assert!(cast(&mut w, &c, 1, 420).is_ok());
}
#[test]
fn multi_movement_navigation_stop_cancel_preserve_pending_and_recovery() {
    let c = config();
    for action in [
        Action::Move {
            direction: [0., 1.],
        },
        Action::Stop {},
        Action::Navigate {
            destination: [400., 500.],
        },
    ] {
        let mut w = world(&c);
        cast(&mut w, &c, 0, 0).unwrap();
        w.action(&action, "move", &c, 30).unwrap();
        assert!(w.pending.is_some());
        advance(&mut w, &c, 0, 120);
        let deadline = w.recovery_until;
        w.action(&action, "move-again", &c, 150).unwrap();
        w.action(&Action::CancelAction {}, "cancel", &c, 180)
            .unwrap();
        assert_eq!(w.recovery_until, deadline);
        assert_eq!(cast(&mut w, &c, 1, 200), Err(Error::InvalidState));
        assert_eq!(w.spirit, 188);
    }
}
#[test]
fn multi_cancel_before_commit_is_free_for_each_skill() {
    let c = config();
    for i in 0..2 {
        let mut w = world(&c);
        cast(&mut w, &c, i, 0).unwrap();
        w.action(&Action::CancelAction {}, "cancel", &c, 30)
            .unwrap();
        assert!(committed(&advance(&mut w, &c, 0, 600)).is_empty());
        assert_eq!(w.spirit, 200);
        assert!(
            w.snapshot(&c, 600)
                .player_skills
                .unwrap()
                .iter()
                .all(|s| s.ready_at_ms == 0)
        );
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    }
}
#[test]
fn multi_background_cancels_pending_but_preserves_committed_recovery() {
    let c = config();
    for committed_first in [false, true] {
        let mut w = world(&c);
        cast(&mut w, &c, 0, 0).unwrap();
        if committed_first {
            advance(&mut w, &c, 0, 120);
        }
        w.action(&Action::Background {}, "bg", &c, 150).unwrap();
        w.action(&Action::Foreground {}, "fg", &c, 180).unwrap();
        w.ready = true;
        w.protected = false;
        assert!(w.pending.is_none());
        assert_eq!(w.skill_ready_at, if committed_first { 1920 } else { 0 });
        assert_eq!(w.spirit, if committed_first { 188 } else { 200 });
        if committed_first {
            assert_eq!(cast(&mut w, &c, 1, 200), Err(Error::InvalidState));
        }
    }
}
#[test]
fn multi_same_skill_cooldown_rejects_at_minus_one_accepts_at_deadline() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 1, 0).unwrap();
    advance(&mut w, &c, 0, 180);
    let deadline = w.skill_ready(&c, &c.additional_skills[0].id);
    assert_eq!(cast(&mut w, &c, 1, deadline - 1), Err(Error::InvalidState));
    assert!(cast(&mut w, &c, 1, deadline).is_ok());
}
#[test]
fn multi_unknown_and_invalid_direction_have_no_effect() {
    let c = config();
    let mut w = world(&c);
    assert_eq!(
        w.action(
            &Action::Cast {
                skill_id: "missing".into(),
                direction: [1., 0.]
            },
            "unknown",
            &c,
            0
        ),
        Err(Error::Unsupported)
    );
    for direction in [[0., 0.], [f64::NAN, 0.], [f64::INFINITY, 1.]] {
        assert!(
            w.action(
                &Action::Cast {
                    skill_id: c.additional_skills[0].id.clone(),
                    direction
                },
                "bad",
                &c,
                0
            )
            .is_err()
        );
    }
    assert!(w.pending.is_none());
    assert_eq!(w.spirit, 200);
}
#[test]
fn multi_no_resource_at_acceptance_and_commit_is_atomic() {
    let c = config();
    for at_commit in [false, true] {
        let mut w = world(&c);
        if at_commit {
            cast(&mut w, &c, 1, 0).unwrap();
            w.spirit = 7;
            let e = advance(&mut w, &c, 0, 180);
            assert!(e.iter().any(|e| matches!(
                e,
                Event::ActionRejected {
                    reason: Error::NoResource,
                    ..
                }
            )));
        } else {
            w.spirit = 7;
            assert_eq!(cast(&mut w, &c, 1, 0), Err(Error::NoResource));
        }
        assert!(w.pending.is_none());
        assert_eq!(w.spirit, 7);
        assert_eq!(w.skill_ready(&c, &c.additional_skills[0].id), 0);
        assert!(w.pve.as_ref().unwrap().shots.is_empty());
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    }
}
#[test]
fn multi_each_skill_uses_its_own_bindings() {
    let c = config();
    for (i, want) in [(0, "ward"), (1, "renewal")] {
        let mut w = world(&c);
        cast(&mut w, &c, i, 0).unwrap();
        advance(&mut w, &c, 0, 180);
        let s = w.snapshot(&c, 180);
        assert_eq!(s.statuses.len(), 1);
        assert_eq!(s.statuses[0].definition_id, want);
    }
}
#[test]
fn multi_secondary_capacity_failure_preserves_cost_and_cooldown() {
    let mut c = config();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut w = world(&c);
    cast(&mut w, &c, 0, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    cast(&mut w, &c, 1, 420).unwrap();
    let e = advance(&mut w, &c, 420, 600);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(w.spirit, 188);
    assert_eq!(w.skill_ready(&c, "renewal-bolt"), 0);
    assert_eq!(w.snapshot(&c, 600).statuses[0].definition_id, "ward");
}
#[test]
fn multi_selected_definition_is_frozen_until_commit() {
    let mut c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 1, 0).unwrap();
    c.additional_skills[0].cost = 99;
    c.additional_skills[0].damage_bps = 1;
    c.additional_skills[0].speed = 2.;
    c.additional_skills[0].cooldown_ms = 1;
    c.additional_skills[0].on_commit_self_statuses.clear();
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(committed(&e), vec![(Some("renewal-bolt".into()), 8)]);
    assert_eq!(w.skill_ready(&c, "renewal-bolt"), 2880);
    let shot = &w.pve.as_ref().unwrap().shots[0];
    assert_eq!(shot.raw, 120);
    assert_eq!(shot.speed, 1000.);
    assert_eq!(w.snapshot(&c, 180).statuses[0].definition_id, "renewal");
}
#[test]
fn multi_in_flight_projectile_keeps_parameters_across_selection() {
    let mut c = config();
    c.skill.speed = 10.;
    c.additional_skills[0].speed = 20.;
    let mut w = world(&c);
    cast(&mut w, &c, 0, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    cast(&mut w, &c, 1, 420).unwrap();
    advance(&mut w, &c, 420, 600);
    let a = w.pve.as_ref().unwrap();
    assert_eq!(a.shots.len(), 2);
    assert_eq!((a.shots[0].speed, a.shots[0].raw), (10., 100));
    assert_eq!((a.shots[1].speed, a.shots[1].raw), (20., 120));
}
#[test]
fn multi_post_commit_cancel_does_not_refund_or_remove_projectile() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 1, 0).unwrap();
    advance(&mut w, &c, 0, 180);
    w.action(&Action::CancelAction {}, "late", &c, 181).unwrap();
    assert_eq!(w.spirit, 192);
    assert_eq!(w.skill_ready(&c, "renewal-bolt"), 2880);
    assert_eq!(w.pve.as_ref().unwrap().shots.len(), 1);
    assert_eq!(w.recovery_until, 420);
}
#[test]
fn multi_zero_timings_still_allow_only_one_pending() {
    let mut c = config();
    let s = &mut c.additional_skills[0];
    s.windup_ms = 0;
    s.recovery_ms = 0;
    s.cooldown_ms = 0;
    s.cost = 0;
    let mut w = world(&c);
    cast(&mut w, &c, 1, 0).unwrap();
    assert_eq!(cast(&mut w, &c, 0, 0), Err(Error::InvalidState));
    assert_eq!(committed(&advance(&mut w, &c, 0, 30)).len(), 1);
    assert_eq!(w.spirit, 200);
    assert!(cast(&mut w, &c, 1, 30).is_ok());
}
#[test]
fn multi_pending_view_and_event_ids_follow_selected_skill() {
    let c = config();
    for i in 0..2 {
        let mut w = world(&c);
        cast(&mut w, &c, i, 0).unwrap();
        let s = w.snapshot(&c, 0);
        assert_eq!(
            s.pending_action.unwrap().skill_id,
            Some(c.player_skills().nth(i).unwrap().id.clone())
        );
        advance(&mut w, &c, 0, 180);
        assert!(
            serde_json::to_value(w.snapshot(&c, 180))
                .unwrap()
                .get("pending_action")
                .is_none()
        );
    }
}
#[test]
fn multi_basic_pending_view_has_null_skill_id() {
    let c = config();
    let mut w = world(&c);
    w.action(&Action::Attack {}, "basic", &c, 0).unwrap();
    let v = serde_json::to_value(w.snapshot(&c, 0)).unwrap();
    assert!(v["pending_action"]["skill_id"].is_null());
    assert_eq!(v["pending_action"]["commits_at_ms"], 200);
    let e = advance(&mut w, &c, 0, 210);
    assert_eq!(committed(&e), vec![(None, 0)]);
}
#[test]
fn multi_single_skill_26_keeps_primary_alias() {
    let mut c = config();
    c.additional_skills.clear();
    let mut w = world(&c);
    cast(&mut w, &c, 0, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    let s = w.snapshot(&c, 120);
    assert_eq!(s.player_skills.as_ref().unwrap().len(), 1);
    assert_eq!(s.player_skills.unwrap()[0].ready_at_ms, s.skill_ready_at_ms);
}
#[test]
fn multi_old_19_to_25_events_views_and_recovery_policy_unchanged() {
    for name in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
        "hot-host",
        "heal-modifier-host",
    ] {
        let c = crate::ConfigCatalog::load(
            std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                .join(format!("examples/{name}/catalog.json")),
        )
        .unwrap()
        .default_config()
        .as_ref()
        .clone();
        let mut w = world(&c);
        cast(&mut w, &c, 0, 0).unwrap();
        let e = advance(&mut w, &c, 0, 300);
        let v = serde_json::to_value(w.snapshot(&c, 300)).unwrap();
        assert!(v.get("player_skills").is_none() && v.get("pending_action").is_none());
        assert!(committed(&e).iter().all(|(id, _)| id.is_none()));
        w.recovery_until = 9999;
        w.action(
            &Action::Move {
                direction: [0., 1.],
            },
            "move",
            &c,
            300,
        )
        .unwrap();
        assert_eq!(w.recovery_until, 300);
    }
}
#[test]
fn multi_public_retry_and_reconnect_preserve_both_deadlines() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine_with_config(config());
    let c = s::create(&e.config);
    let input = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: "secondary".into(),
        sequence: 3,
        action: Action::Cast {
            skill_id: e.config.additional_skills[0].id.clone(),
            direction: [1., 0.],
        },
    };
    assert_eq!(e.input(input.clone()), e.input(input.clone()));
    for _ in 0..7 {
        e.tick();
        e.drain();
    }
    let before = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    e.input(input.clone()).unwrap();
    e.input(Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "resume".into(),
            generation: 2,
        },
        request_id: "resume".into(),
        sequence: 4,
        action: Action::Reconnect {
            previous_connection: c.connection,
            authenticated_principal: c.authenticated_principal,
        },
    })
    .unwrap();
    let after = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    assert_eq!(after.player_skills, before.player_skills);
    assert_eq!(after.recovery_until_ms, before.recovery_until_ms);
    assert_eq!(after.spirit, before.spirit);
    assert_eq!(e.input(input), Err(Error::StaleConnection));
}
#[test]
fn multi_hundred_closes_cancel_preparation_and_release_world() {
    use crate::asset_contract::tests as s;
    for _ in 0..100 {
        let mut e = active_engine_with_config(config());
        let key = s::create(&e.config).key;
        let now = e.status().logical_ms;
        let c = e.config.clone();
        cast(e.world.as_mut().unwrap(), &c, 1, now).unwrap();
        e.close(Close {
            key,
            request_id: "close".into(),
        })
        .unwrap();
        assert_eq!(e.live_worlds(), 0);
        let r = s::requests(&e.drain()).remove(0);
        let receipt = s::receipt(
            &e.config,
            &r,
            ReceiptOutcome::Committed {
                revision: 2,
                commit_id: "close-assets".into(),
            },
        );
        e.business(receipt).unwrap();
        assert_eq!(e.pending_count(), 0);
    }
}
#[test]
fn multi_terminal_cancels_secondary_before_commit_without_cost() {
    let mut c = config();
    c.terminal_ms = 60;
    let mut w = world(&c);
    cast(&mut w, &c, 1, 0).unwrap();
    let e = advance(&mut w, &c, 0, 90);
    assert!(committed(&e).is_empty());
    assert_eq!(w.spirit, 200);
    w.settlement(&c);
    assert!(w.pending.is_none());
}
#[test]
fn multi_precommit_attack_capture_uses_current_effective_atk() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 1, 0).unwrap();
    w.facts.atk_points = 200;
    advance(&mut w, &c, 0, 180);
    assert_eq!(w.pve.as_ref().unwrap().shots[0].raw, 240);
    w.facts.atk_points = 1;
    assert_eq!(w.pve.as_ref().unwrap().shots[0].raw, 240);
}
#[test]
fn multi_legal_output_peak_preserves_frozen_asset_responsibility() {
    use crate::asset_contract::tests as s;
    let mut c = config();
    c.output_capacity = 32;
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 3;
    c.pve_mut().status_definitions[0].tick_interval_ms = Some(30);
    c.pve_mut().monster_positions = (0..32)
        .map(|i| {
            [
                100. + f64::from(i % 16) * 40.,
                600. + f64::from(i / 16) * 80.,
            ]
        })
        .collect();
    let mut e = active_engine_with_config(c);
    let now = e.status().logical_ms;
    let w = e.world.as_mut().unwrap();
    let mut a = w.pve.take().unwrap();
    let mut events = vec![];
    for i in 0..32 {
        let m = a.status_entity(i);
        a.commit_statuses(w, &["renewal".into(), "ward".into()], &m, now, &mut events);
    }
    w.pve = Some(a);
    e.tick();
    let o = e.drain();
    assert_eq!(e.live_worlds(), 0);
    assert!(o.len() <= 32);
    assert!(o.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::Fault {
                code: Error::QueueFull
            }
        }
    )));
    let r = s::requests(&o).remove(0);
    let receipt = s::receipt(
        &e.config,
        &r,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "peak-assets".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
}
