use super::status_tests::{active_engine_with_config, advance, bind, world};
use super::*;
pub(super) fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/self-skill-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone()
}
fn intent(c: &Config) -> Action {
    Action::CastSelf {
        skill_id: c.player_skills().find(|s| s.is_self()).unwrap().id.clone(),
    }
}
fn cast(w: &mut World, c: &Config, now: u64) -> Result<Vec<Event>> {
    w.action(&intent(c), &format!("self-{now}"), c, now)
}
fn count(e: &[Event]) -> usize {
    e.iter()
        .filter(|e| matches!(e, Event::ActionCommitted { .. }))
        .count()
}
#[test]
fn self_skill_commit_applies_states_without_projectile_damage_rng_or_hit_income() {
    let c = config();
    let mut w = world(&c);
    let rng = w.rng;
    let hp = w.pve.as_ref().unwrap().monsters[0].hp;
    cast(&mut w, &c, 0).unwrap();
    assert!(w.snapshot(&c, 0).statuses.is_empty());
    assert_eq!(w.spirit, 200);
    let e = advance(&mut w, &c, 0, 120);
    assert_eq!(count(&e), 1);
    assert_eq!(w.snapshot(&c, 120).statuses.len(), 2);
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    assert_eq!(w.rng, rng);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, hp);
    assert_eq!(w.kills, 0);
    assert_eq!(w.spirit, 192);
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::Damage { .. } | Event::MonsterKilled { .. }))
    );
}
#[test]
fn self_skill_zero_windup_commits_only_on_fixed_step_and_dedicated_pending() {
    let mut c = config();
    let s = &mut c.additional_skills[0];
    s.windup_ms = 0;
    s.cost = 0;
    s.cooldown_ms = 0;
    s.recovery_ms = 0;
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    assert_eq!(w.spirit, 200);
    assert!(w.snapshot(&c, 0).statuses.is_empty());
    assert_eq!(cast(&mut w, &c, 0), Err(Error::InvalidState));
    assert_eq!(count(&advance(&mut w, &c, 0, 30)), 1);
    assert!(cast(&mut w, &c, 30).is_ok());
}
#[test]
fn self_skill_works_without_enemy_target_or_aim() {
    let mut c = config();
    c.pve_mut().monster_positions.clear();
    let mut w = world(&c);
    w.position = [1100., 1100.];
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    assert_eq!(w.snapshot(&c, 120).statuses.len(), 2);
    assert!(w.pve.as_ref().unwrap().monsters.is_empty());
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
}
#[test]
fn self_skill_primary_slot_updates_primary_alias() {
    let mut c = config();
    std::mem::swap(&mut c.skill, &mut c.additional_skills[0]);
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    assert_eq!(w.skill_ready_at, 2320);
    let s = w.snapshot(&c, 120);
    assert_eq!(s.player_skills.unwrap()[0].ready_at_ms, s.skill_ready_at_ms);
    assert_eq!(s.statuses.len(), 2);
}
#[test]
fn self_skill_mismatched_inputs_and_unknown_id_have_no_side_effects() {
    let c = config();
    let mut w = world(&c);
    assert_eq!(
        w.action(
            &Action::Cast {
                skill_id: c.additional_skills[0].id.clone(),
                direction: [1., 0.]
            },
            "bad",
            &c,
            0
        ),
        Err(Error::InvalidMessage)
    );
    assert_eq!(
        w.action(
            &Action::CastSelf {
                skill_id: c.skill.id.clone()
            },
            "bad",
            &c,
            0
        ),
        Err(Error::InvalidMessage)
    );
    assert_eq!(
        w.action(
            &Action::CastSelf {
                skill_id: "missing".into()
            },
            "bad",
            &c,
            0
        ),
        Err(Error::Unsupported)
    );
    assert_eq!(w.spirit, 200);
    assert!(w.pending.is_none());
}
#[test]
fn self_skill_old_versions_reject_new_input() {
    for sample in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
        "hot-host",
        "heal-modifier-host",
        "dual-skill-host",
    ] {
        let c = crate::ConfigCatalog::load(
            std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
                .join(format!("examples/{sample}/catalog.json")),
        )
        .unwrap()
        .default_config()
        .as_ref()
        .clone();
        let mut w = world(&c);
        assert_eq!(
            w.action(
                &Action::CastSelf {
                    skill_id: c.skill.id.clone()
                },
                "new-input",
                &c,
                0
            ),
            Err(Error::Unsupported)
        );
        assert_eq!(w.spirit, 200);
    }
}
#[test]
fn self_skill_protection_readiness_and_death_block_acceptance() {
    let c = config();
    for case in 0..3 {
        let mut w = world(&c);
        match case {
            0 => w.protected = true,
            1 => w.ready = false,
            _ => w.hp = 0,
        };
        assert_eq!(cast(&mut w, &c, 0), Err(Error::NotReady));
        assert!(w.pending.is_none());
        assert_eq!(w.spirit, 200);
    }
}
#[test]
fn self_skill_no_resource_at_acceptance_never_reserves_or_applies() {
    let c = config();
    let mut w = world(&c);
    w.spirit = 7;
    assert_eq!(cast(&mut w, &c, 0), Err(Error::NoResource));
    assert!(w.pending.is_none());
    assert!(w.snapshot(&c, 0).statuses.is_empty());
    assert_eq!(w.skill_ready(&c, "renewing-ward"), 0);
}
#[test]
fn self_skill_no_resource_at_commit_is_atomic() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    w.spirit = 7;
    let e = advance(&mut w, &c, 0, 120);
    assert_eq!(count(&e), 0);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::NoResource,
            ..
        }
    )));
    assert_eq!(w.spirit, 7);
    assert_eq!(w.skill_ready(&c, "renewing-ward"), 0);
    assert!(w.snapshot(&c, 120).statuses.is_empty());
}
#[test]
fn self_skill_capacity_failure_does_not_partially_refresh_grant_or_charge() {
    let mut c = config();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut w = world(&c);
    bind(
        &mut w,
        "ward",
        status::participant(),
        status::participant(),
        0,
    );
    let before = w.snapshot(&c, 0).statuses;
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 120);
    assert_eq!(count(&e), 0);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(w.snapshot(&c, 120).statuses, before);
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready(&c, "renewing-ward"), 0);
}
#[test]
fn self_skill_freezes_parameters_and_bindings_at_acceptance() {
    let mut c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    c.additional_skills[0].cost = 99;
    c.additional_skills[0].cooldown_ms = 1;
    c.additional_skills[0].on_commit_self_statuses.clear();
    advance(&mut w, &c, 0, 120);
    assert_eq!(w.spirit, 192);
    assert_eq!(w.skill_ready(&c, "renewing-ward"), 2320);
    assert_eq!(w.snapshot(&c, 120).statuses.len(), 2);
}
#[test]
fn self_skill_cancel_before_commit_never_charges_or_applies() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    w.action(&Action::CancelAction {}, "cancel", &c, 30)
        .unwrap();
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(count(&e), 0);
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready(&c, "renewing-ward"), 0);
    assert!(w.snapshot(&c, 180).statuses.is_empty());
}
#[test]
fn self_skill_background_cancels_preparation_and_preserves_committed_deadlines() {
    let c = config();
    for commit in [false, true] {
        let mut w = world(&c);
        cast(&mut w, &c, 0).unwrap();
        if commit {
            advance(&mut w, &c, 0, 120);
        }
        w.action(&Action::Background {}, "background", &c, 150)
            .unwrap();
        w.action(&Action::Foreground {}, "foreground", &c, 180)
            .unwrap();
        assert!(w.pending.is_none());
        assert_eq!(w.spirit, if commit { 192 } else { 200 });
        assert_eq!(w.recovery_until, if commit { 360 } else { 0 });
        assert_eq!(
            w.skill_ready(&c, "renewing-ward"),
            if commit { 2320 } else { 0 }
        );
    }
}
#[test]
fn self_skill_pending_and_recovery_block_projectile_and_basic() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    for t in [30, 359] {
        if t == 359 {
            advance(&mut w, &c, 0, 120);
        }
        for a in [
            Action::Attack {},
            Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.],
            },
        ] {
            assert_eq!(w.action(&a, "blocked", &c, t), Err(Error::InvalidState));
        }
    }
    assert!(
        w.action(
            &Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.]
            },
            "allowed",
            &c,
            360
        )
        .is_ok()
    );
}
#[test]
fn self_skill_movement_stop_navigation_and_cancel_do_not_shorten_recovery() {
    let c = config();
    for a in [
        Action::Move {
            direction: [0., 1.],
        },
        Action::Navigate {
            destination: [400., 500.],
        },
        Action::Stop {},
        Action::CancelAction {},
    ] {
        let mut w = world(&c);
        cast(&mut w, &c, 0).unwrap();
        advance(&mut w, &c, 0, 120);
        w.action(&a, "movement", &c, 150).unwrap();
        assert_eq!(w.recovery_until, 360);
        assert_eq!(w.spirit, 192);
        assert_eq!(w.snapshot(&c, 150).statuses.len(), 2);
    }
}
#[test]
fn self_skill_cd_deadline_is_inclusive_and_independent_of_projectile() {
    let c = config();
    let mut w = world(&c);
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "projectile",
        &c,
        0,
    )
    .unwrap();
    advance(&mut w, &c, 0, 120);
    let primary = w.skill_ready_at;
    cast(&mut w, &c, 420).unwrap();
    advance(&mut w, &c, 420, 540);
    assert_eq!(w.skill_ready_at, primary);
    assert_eq!(w.spirit, 180);
    assert_eq!(cast(&mut w, &c, 2739), Err(Error::InvalidState));
    assert!(cast(&mut w, &c, 2740).is_ok());
}
#[test]
fn self_skill_shield_and_hot_have_real_state_effects_without_direct_attack() {
    let c = config();
    let mut w = world(&c);
    w.hp = 500;
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    assert_eq!(w.snapshot(&c, 120).shield_total, Some(100));
    let e = advance(&mut w, &c, 120, 720);
    assert_eq!(w.hp, 600);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::PeriodicHealTick {
            hp_restored: 100,
            ..
        }
    )));
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 600);
}
#[test]
fn self_skill_full_health_hot_never_creates_shield_or_spirit() {
    let mut c = config();
    c.additional_skills[0].on_commit_self_statuses = vec!["renewal".into()];
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 720);
    assert_eq!(w.hp, 1000);
    assert_eq!(w.spirit, 192);
    assert_eq!(w.snapshot(&c, 720).shield_total, Some(0));
}
#[test]
fn self_skill_ignored_states_keep_instances_but_commit_cost_and_cd() {
    let mut c = config();
    c.additional_skills[0].cooldown_ms = 300;
    c.additional_skills[0].recovery_ms = 0;
    for d in &mut c.pve_mut().status_definitions {
        d.duplicate_policy = crate::config::DuplicatePolicy::Ignore;
        d.duration_ms = 5000;
    }
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    let old = w.snapshot(&c, 120).statuses;
    cast(&mut w, &c, 420).unwrap();
    let e = advance(&mut w, &c, 420, 540);
    assert_eq!(count(&e), 1);
    assert_eq!(w.snapshot(&c, 540).statuses, old);
    assert_eq!(w.spirit, 184);
    assert_eq!(w.skill_ready(&c, "renewing-ward"), 840);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::StatusIgnored { .. }))
            .count(),
        2
    );
}
#[test]
fn self_skill_pending_view_event_and_late_cancel_keep_authority() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    let p = w.snapshot(&c, 0).pending_action.unwrap();
    assert_eq!(p.skill_id, Some("renewing-ward".into()));
    assert_eq!(p.commits_at_ms, 120);
    let e = advance(&mut w, &c, 0, 120);
    assert!(e.iter().any(|e|matches!(e,Event::ActionCommitted{skill_id:Some(id),spirit_cost:8,..} if id=="renewing-ward")));
    let before = w.snapshot(&c, 120);
    w.action(&Action::CancelAction {}, "late-cancel", &c, 150)
        .unwrap();
    let after = w.snapshot(&c, 150);
    assert_eq!(after.statuses, before.statuses);
    assert_eq!(after.player_skills, before.player_skills);
    assert_eq!(after.spirit, before.spirit);
    assert!(after.pending_action.is_none());
}
#[test]
fn self_skill_terminal_and_death_before_commit_do_not_apply() {
    let mut c = config();
    c.terminal_ms = 60;
    let mut w = world(&c);
    cast(&mut w, &c, 0).unwrap();
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(count(&e), 0);
    assert_eq!(w.spirit, 200);
    w.settlement(&c);
    assert!(w.pending.is_none());
    assert!(w.snapshot(&c, 180).statuses.is_empty());
}
#[test]
fn self_skill_retries_and_reconnect_preserve_instances_cost_and_deadlines() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine_with_config(config());
    let c = s::create(&e.config);
    let input = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: "self-cast".into(),
        sequence: 3,
        action: intent(&e.config),
    };
    assert_eq!(e.input(input.clone()), e.input(input.clone()));
    for _ in 0..5 {
        e.tick();
        e.drain();
    }
    let before = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms);
    assert_eq!(before.spirit, 92);
    e.input(input.clone()).unwrap();
    e.input(Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "self-resume".into(),
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
    assert_eq!(after.statuses, before.statuses);
    assert_eq!(after.player_skills, before.player_skills);
    assert_eq!(after.recovery_until_ms, before.recovery_until_ms);
    assert_eq!(after.spirit, before.spirit);
    assert_eq!(e.input(input), Err(Error::StaleConnection));
}
#[test]
fn self_skill_hundred_closes_release_preparations_states_and_frozen_responsibility() {
    use crate::asset_contract::tests as s;
    for commit in [false, true] {
        for _ in 0..50 {
            let mut e = active_engine_with_config(config());
            let now = e.status().logical_ms;
            let c = e.config.clone();
            cast(e.world.as_mut().unwrap(), &c, now).unwrap();
            if commit {
                for _ in 0..5 {
                    e.tick();
                    e.drain();
                }
            }
            let key = s::create(&e.config).key;
            e.close(Close {
                key,
                request_id: "self-close".into(),
            })
            .unwrap();
            assert_eq!(e.live_worlds(), 0);
            let r = s::requests(&e.drain()).remove(0);
            let receipt = s::receipt(
                &e.config,
                &r,
                ReceiptOutcome::Committed {
                    revision: 2,
                    commit_id: "self-close-assets".into(),
                },
            );
            e.business(receipt).unwrap();
            assert_eq!(e.pending_count(), 0);
        }
    }
}
#[test]
fn self_skill_absorption_is_real_and_never_changes_enemy_hp() {
    let c = config();
    let mut w = world(&c);
    w.facts.def_points = 0;
    cast(&mut w, &c, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hurt_player(&mut w, &c, 0, (80, false), 150, &mut e);
    w.pve = Some(a);
    assert_eq!(w.hp, 1000);
    assert_eq!(w.snapshot(&c, 150).shield_total, Some(20));
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::ShieldAbsorbed { amount: 80, .. }))
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 600);
}
#[test]
fn self_skill_attribute_stack_changes_future_projectiles_without_rewriting_in_flight() {
    let mut c = config();
    let a = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/attribute-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone();
    c.pve_mut().status_definitions = a.pve.as_ref().unwrap().status_definitions.clone();
    c.additional_skills[0].on_commit_self_statuses = vec!["guard".into(), "exposed".into()];
    c.additional_skills[0].cooldown_ms = 300;
    c.additional_skills[0].recovery_ms = 0;
    c.skill.speed = 10.;
    c.skill.cooldown_ms = 0;
    c.skill.recovery_ms = 0;
    let mut w = world(&c);
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "first",
        &c,
        0,
    )
    .unwrap();
    advance(&mut w, &c, 0, 120);
    cast(&mut w, &c, 120).unwrap();
    advance(&mut w, &c, 120, 240);
    assert_eq!(w.snapshot(&c, 240).effective_atk, Some(125));
    assert_eq!(w.snapshot(&c, 240).effective_def, Some(75));
    cast(&mut w, &c, 540).unwrap();
    advance(&mut w, &c, 540, 660);
    assert_eq!(w.snapshot(&c, 660).effective_atk, Some(150));
    assert_eq!(w.snapshot(&c, 660).effective_def, Some(50));
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "later",
        &c,
        660,
    )
    .unwrap();
    advance(&mut w, &c, 660, 780);
    let shots = &w.pve.as_ref().unwrap().shots;
    assert_eq!(shots.len(), 2);
    assert_eq!(shots[0].raw, 100);
    assert_eq!(shots[1].raw, 150);
}
#[test]
fn self_skill_lethal_contact_before_due_never_applies_or_debits() {
    let c = config();
    let mut w = world(&c);
    w.hp = 1;
    cast(&mut w, &c, 0).unwrap();
    let mut a = w.pve.take().unwrap();
    a.hurt_player(&mut w, &c, 0, (1000, false), 30, &mut vec![]);
    w.pve = Some(a);
    assert_eq!(w.hp, 0);
    let e = advance(&mut w, &c, 0, 180);
    assert_eq!(count(&e), 0);
    assert_eq!(w.spirit, 200);
    w.settlement(&c);
    assert!(w.pending.is_none());
    assert!(w.snapshot(&c, 180).statuses.is_empty());
}
#[test]
fn self_skill_legal_output_peak_reclaims_world_without_losing_frozen_assets() {
    use crate::asset_contract::tests as s;
    let mut c = config();
    c.output_capacity = 32;
    c.additional_skills[0].windup_ms = 0;
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
    for i in 0..32 {
        let m = a.status_entity(i);
        a.commit_statuses(w, &["renewal".into(), "ward".into()], &m, now, &mut vec![]);
    }
    w.pve = Some(a);
    let c = e.config.clone();
    cast(e.world.as_mut().unwrap(), &c, now).unwrap();
    e.tick();
    let outputs = e.drain();
    assert_eq!(e.live_worlds(), 0);
    assert!(outputs.len() <= 32);
    assert!(outputs.iter().any(|o| matches!(
        o.body,
        OutputBody::Event {
            event: Event::Fault {
                code: Error::QueueFull
            }
        }
    )));
    let r = s::requests(&outputs).remove(0);
    let receipt = s::receipt(
        &e.config,
        &r,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "self-peak-assets".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
}
