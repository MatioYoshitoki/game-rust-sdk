use super::status_tests::{active_engine_with_config, advance, bind, world};
use super::*;
pub(super) fn config() -> Config {
    crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/unit-skill-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone()
}
fn make(c: &Config) -> World {
    let mut w = world(c);
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.aggro = true;
    }
    w
}
fn target(w: &World, i: usize) -> UnitTarget {
    w.pve.as_ref().unwrap().unit_reference(i).unwrap()
}
fn cast(w: &mut World, c: &Config, t: &UnitTarget, now: u64) -> Result<Vec<Event>> {
    let s = c.player_skills().find(|s| s.is_unit()).unwrap();
    w.action(
        &Action::CastUnit {
            skill_id: s.id.clone(),
            target: t.clone(),
        },
        &format!("unit-{now}"),
        c,
        now,
    )
}
fn count(e: &[Event]) -> usize {
    e.iter()
        .filter(|e| matches!(e, Event::ActionCommitted { .. }))
        .count()
}
fn vision(w: &mut World, blocked: bool) {
    let a = w.pve.as_mut().unwrap();
    Arc::make_mut(&mut a.grid.config).palette[0].blocks_vision = blocked;
}
#[test]
fn unit_skill_real_second_target_damage_and_status_never_auto_select_first() {
    let c = config();
    let mut w = make(&c);
    let t = target(&w, 1);
    cast(&mut w, &c, &t, 0).unwrap();
    let e = advance(&mut w, &c, 0, 300);
    assert_eq!(count(&e), 1);
    let a = w.pve.as_ref().unwrap();
    assert_eq!(a.monsters[0].hp, 180);
    assert_eq!(a.monsters[1].hp, 114);
    let s = w.snapshot(&c, 300);
    assert!(s.pve.as_ref().unwrap().monsters[0].statuses.is_empty());
    assert_eq!(
        s.pve.unwrap().monsters[1].statuses[0].definition_id,
        "exposed"
    );
    assert_eq!(w.spirit, 188);
}
#[test]
fn unit_skill_views_expose_only_live_enabled_nonreturning_references() {
    let c = config();
    for case in 0..3 {
        let mut w = make(&c);
        match case {
            0 => w.pve.as_mut().unwrap().monsters[0].hp = 0,
            1 => w.pve.as_mut().unwrap().monsters[0].returning = true,
            _ => w.pve.as_mut().unwrap().monsters[0].enabled = false,
        };
        let s = w.snapshot(&c, 0);
        assert!(
            s.pve
                .as_ref()
                .unwrap()
                .monsters
                .iter()
                .filter(|m| m.id == "status-target-1")
                .all(|m| m.unit_target.is_none())
        );
        assert!(
            s.pve
                .unwrap()
                .monsters
                .iter()
                .any(|m| m.unit_target.is_some())
        );
    }
}
#[test]
fn unit_skill_accept_rejects_missing_dead_disabled_returning_old_generation_and_self() {
    let c = config();
    for case in 0..7 {
        let mut w = make(&c);
        let mut t = target(&w, 0);
        match case {
            0 => t.id = "missing".into(),
            1 => w.pve.as_mut().unwrap().monsters[0].hp = 0,
            2 => w.pve.as_mut().unwrap().monsters[0].enabled = false,
            3 => w.pve.as_mut().unwrap().monsters[0].returning = true,
            4 => t.generation += 1,
            5 => t.id = "participant".into(),
            _ => t.generation = 0,
        };
        assert_eq!(cast(&mut w, &c, &t, 0), Err(Error::InvalidTarget));
        assert_eq!(w.spirit, 200);
        assert!(w.pending.is_none());
        assert!(w.pve.as_ref().unwrap().shots.is_empty());
    }
}
#[test]
fn unit_skill_range_boundary_is_inclusive_and_never_starts_navigation() {
    let mut c = config();
    c.skill.range = 100.;
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[0].position = [500., 400.];
    let t = target(&w, 0);
    assert!(cast(&mut w, &c, &t, 0).is_ok());
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[0].position = [500.01, 400.];
    let t = target(&w, 0);
    assert_eq!(cast(&mut w, &c, &t, 0), Err(Error::OutOfRange));
    assert!(w.pve.as_ref().unwrap().player_path.is_empty());
    assert_eq!(w.movement, [0., 0.]);
}
#[test]
fn unit_skill_accept_checks_vision_without_granting_self_status() {
    let mut c = config();
    c.skill.on_commit_self_statuses = vec!["ward".into()];
    let mut w = make(&c);
    let t = target(&w, 0);
    vision(&mut w, true);
    assert_eq!(cast(&mut w, &c, &t, 0), Err(Error::OutOfRange));
    assert!(w.snapshot(&c, 0).statuses.is_empty());
    assert_eq!(w.spirit, 200);
}
#[test]
fn unit_skill_commit_rechecks_death_disable_returning_generation_range_and_vision_atomically() {
    let mut c = config();
    c.skill.on_commit_self_statuses = vec!["ward".into()];
    for case in 0..6 {
        let mut w = make(&c);
        let t = target(&w, 1);
        cast(&mut w, &c, &t, 0).unwrap();
        match case {
            0 => w.pve.as_mut().unwrap().monsters[1].hp = 0,
            1 => w.pve.as_mut().unwrap().monsters[1].enabled = false,
            2 => w.pve.as_mut().unwrap().monsters[1].returning = true,
            3 => w.pve.as_mut().unwrap().monsters[1].status_generation += 1,
            4 => w.pve.as_mut().unwrap().monsters[1].position = [1000., 1000.],
            _ => vision(&mut w, true),
        }
        let e = advance(&mut w, &c, 0, 120);
        assert_eq!(count(&e), 0, "{case}");
        assert!(e.iter().any(|e|matches!(e,Event::ActionRejected{reason,..} if *reason==if case>=4{Error::OutOfRange}else{Error::InvalidTarget})),"{case}: {e:?}");
        assert_eq!(w.spirit, 200);
        assert_eq!(w.skill_ready_at, 0);
        assert!(w.snapshot(&c, 120).statuses.is_empty());
        assert!(w.pve.as_ref().unwrap().shots.is_empty());
        assert!(w.pending.is_none());
    }
}
#[test]
fn unit_skill_commit_aims_at_current_positions_but_preserves_selected_identity() {
    let c = config();
    let mut w = make(&c);
    let t = target(&w, 1);
    cast(&mut w, &c, &t, 0).unwrap();
    w.pve.as_mut().unwrap().monsters[1].position = [500., 500.];
    w.position = [420., 400.];
    advance(&mut w, &c, 0, 120);
    let s = &w.pve.as_ref().unwrap().shots[0];
    assert_eq!(s.strict_target, Some(t));
    assert_eq!(s.position, [420., 400.]);
    assert_eq!(s.direction, normalize([80., 100.]));
}
#[test]
fn unit_skill_pending_target_is_not_changed_by_later_selection_or_rejected_cast() {
    let c = config();
    let mut w = make(&c);
    let a = target(&w, 0);
    let b = target(&w, 1);
    cast(&mut w, &c, &a, 0).unwrap();
    assert_eq!(cast(&mut w, &c, &b, 30), Err(Error::InvalidState));
    assert_eq!(
        w.snapshot(&c, 30).pending_action.unwrap().unit_target,
        Some(a.clone())
    );
    advance(&mut w, &c, 0, 120);
    assert_eq!(w.pve.as_ref().unwrap().shots[0].strict_target, Some(a));
}
#[test]
fn unit_skill_intervening_unit_blocks_without_damage_or_hit_status() {
    let c = config();
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[1].position = [600., 400.];
    w.pve.as_mut().unwrap().monsters[0].position = [500., 400.];
    let t = target(&w, 1);
    cast(&mut w, &c, &t, 0).unwrap();
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!(count(&e), 1);
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| m.hp == 180));
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    assert!(
        w.snapshot(&c, 600)
            .pve
            .unwrap()
            .monsters
            .iter()
            .all(|m| m.statuses.is_empty())
    );
    assert_eq!(w.spirit, 188);
    assert!(
        !e.iter()
            .any(|e| matches!(e,Event::Damage{source,..} if source=="participant"))
    );
}
#[test]
fn unit_skill_direction_projectile_keeps_original_first_contact_behavior() {
    let mut c = config();
    c.skill.mechanism = super::self_skill_tests::config().skill.mechanism;
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[0].position = [500., 400.];
    w.pve.as_mut().unwrap().monsters[1].position = [600., 400.];
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        "free",
        &c,
        0,
    )
    .unwrap();
    advance(&mut w, &c, 0, 600);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 114);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 180);
}
#[test]
fn unit_skill_committed_projectile_does_not_home_after_target_moves() {
    let c = config();
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[0].position = [600., 400.];
    let t = target(&w, 0);
    cast(&mut w, &c, &t, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    w.pve.as_mut().unwrap().monsters[0].position = [600., 600.];
    advance(&mut w, &c, 120, 900);
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| m.hp == 180));
    assert_eq!(w.spirit, 188);
}
#[test]
fn unit_skill_committed_target_loss_never_rebinds_or_refunds() {
    let c = config();
    for case in 0..3 {
        let mut w = make(&c);
        let t = target(&w, 1);
        cast(&mut w, &c, &t, 0).unwrap();
        advance(&mut w, &c, 0, 120);
        match case {
            0 => w.pve.as_mut().unwrap().monsters[1].hp = 0,
            1 => w.pve.as_mut().unwrap().monsters[1].status_generation += 1,
            _ => w.pve.as_mut().unwrap().monsters[1].returning = true,
        }
        advance(&mut w, &c, 120, 150);
        assert!(w.pve.as_ref().unwrap().shots.is_empty());
        assert_eq!(w.spirit, 188);
        assert_eq!(w.skill_ready_at, 1320);
        assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
    }
}
#[test]
fn unit_skill_wall_wins_equal_contact_and_hit_order_is_stable() {
    let c = config();
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[0].position = [450., 400.];
    let t = target(&w, 0);
    cast(&mut w, &c, &t, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    w.pve.as_mut().unwrap().monsters[0].position = [400., 400.];
    Arc::make_mut(&mut w.pve.as_mut().unwrap().grid.config).palette[0].blocks_projectiles = true;
    advance(&mut w, &c, 120, 150);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 180);
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    let mut w = make(&c);
    w.pve.as_mut().unwrap().monsters[1].position = [450., 400.];
    let t = target(&w, 1);
    cast(&mut w, &c, &t, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    w.pve.as_mut().unwrap().monsters[0].position = [450., 384.];
    w.pve.as_mut().unwrap().monsters[1].position = [450., 416.];
    advance(&mut w, &c, 120, 300);
    assert!(w.pve.as_ref().unwrap().monsters.iter().all(|m| m.hp == 180));
}
#[test]
fn unit_skill_freezes_parameters_and_attack_then_uses_existing_live_defense() {
    let mut c = config();
    let mut w = make(&c);
    let t = target(&w, 0);
    cast(&mut w, &c, &t, 0).unwrap();
    c.skill.cost = 99;
    c.skill.cooldown_ms = 1;
    c.skill.damage_bps = 1;
    advance(&mut w, &c, 0, 120);
    let shot = &w.pve.as_ref().unwrap().shots[0];
    assert_eq!(shot.raw, 100);
    assert_eq!(w.spirit, 188);
    assert_eq!(w.skill_ready_at, 1320);
    w.facts.atk_points = 10000;
    let e = advance(&mut w, &c, 120, 300);
    assert!(
        e.iter()
            .any(|e| matches!(e,Event::Damage{source,amount:66,..} if source=="participant"))
    );
}
#[test]
fn unit_skill_commit_resource_decline_and_self_capacity_failure_are_atomic() {
    for poor in [false, true] {
        let mut c = config();
        c.skill.on_commit_self_statuses = vec!["ward".into(), "renewal".into()];
        if !poor {
            c.pve_mut()
                .status_rules
                .as_mut()
                .unwrap()
                .max_active_per_entity = 1;
        }
        let mut w = make(&c);
        if !poor {
            bind(
                &mut w,
                "ward",
                status::participant(),
                status::participant(),
                0,
            );
        }
        let states = w.snapshot(&c, 0).statuses;
        let t = target(&w, 0);
        cast(&mut w, &c, &t, 0).unwrap();
        if poor {
            w.spirit = 11;
        }
        let e = advance(&mut w, &c, 0, 120);
        assert_eq!(count(&e), 0);
        assert!(e.iter().any(|e|matches!(e,Event::ActionRejected{reason,..} if *reason==if poor{Error::NoResource}else{Error::Capacity})));
        assert_eq!(w.snapshot(&c, 120).statuses, states);
        assert_eq!(w.spirit, if poor { 11 } else { 200 });
        assert_eq!(w.skill_ready_at, 0);
        assert!(w.pve.as_ref().unwrap().shots.is_empty());
    }
}
#[test]
fn unit_skill_zero_windup_and_cost_still_use_one_fixed_step_pending() {
    let mut c = config();
    c.skill.windup_ms = 0;
    c.skill.cooldown_ms = 0;
    c.skill.recovery_ms = 0;
    c.skill.cost = 0;
    let mut w = make(&c);
    let t = target(&w, 0);
    cast(&mut w, &c, &t, 0).unwrap();
    assert_eq!(cast(&mut w, &c, &t, 0), Err(Error::InvalidState));
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    assert_eq!(count(&advance(&mut w, &c, 0, 30)), 1);
    assert_eq!(w.spirit, 200);
    assert!(cast(&mut w, &c, &t, 30).is_ok());
}
#[test]
fn unit_skill_shared_pending_recovery_and_move_cancel_cannot_bypass() {
    let c = config();
    let mut w = make(&c);
    let t = target(&w, 0);
    cast(&mut w, &c, &t, 0).unwrap();
    let own = Action::CastSelf {
        skill_id: c.additional_skills[0].id.clone(),
    };
    assert_eq!(w.action(&own, "blocked", &c, 30), Err(Error::InvalidState));
    advance(&mut w, &c, 0, 120);
    for a in [
        Action::Move {
            direction: [0., 1.],
        },
        Action::Stop {},
        Action::Navigate {
            destination: [400., 500.],
        },
        Action::CancelAction {},
    ] {
        w.action(&a, "move", &c, 150).unwrap();
        assert_eq!(w.recovery_until, 420);
    }
    assert_eq!(w.action(&own, "blocked", &c, 419), Err(Error::InvalidState));
    assert!(w.action(&own, "allowed", &c, 420).is_ok());
}
#[test]
fn unit_skill_cancel_background_terminal_and_death_before_due_do_not_spend() {
    for case in 0..4 {
        let mut c = config();
        if case == 2 {
            c.terminal_ms = 60;
        }
        let mut w = make(&c);
        let t = target(&w, 0);
        cast(&mut w, &c, &t, 0).unwrap();
        match case {
            0 => {
                w.action(&Action::CancelAction {}, "cancel", &c, 30)
                    .unwrap();
            }
            1 => {
                w.action(&Action::Background {}, "background", &c, 30)
                    .unwrap();
            }
            2 => {}
            _ => {
                w.hp = 0;
                w.desired_outcome = Some(Outcome::Death);
            }
        }
        let e = advance(&mut w, &c, 0, 180);
        assert_eq!(count(&e), 0);
        assert_eq!(w.spirit, 200);
        assert_eq!(w.skill_ready_at, 0);
        assert!(w.pve.as_ref().unwrap().shots.is_empty());
    }
}
#[test]
fn unit_skill_primary_and_secondary_cd_aliases_are_independent() {
    for swap in [false, true] {
        let mut c = config();
        if swap {
            std::mem::swap(&mut c.skill, &mut c.additional_skills[0]);
        }
        let mut w = make(&c);
        let t = target(&w, 1);
        cast(&mut w, &c, &t, 0).unwrap();
        advance(&mut w, &c, 0, 120);
        let s = w.snapshot(&c, 120);
        assert_eq!(
            s.player_skills.as_ref().unwrap()[0].ready_at_ms,
            s.skill_ready_at_ms
        );
        assert_eq!(s.skill_ready_at_ms, if swap { 0 } else { 1320 });
        assert_eq!(w.spirit, 188);
    }
}
#[test]
fn unit_skill_mismatched_intents_unknown_skill_and_old_versions_are_rejected() {
    let c = config();
    let mut w = make(&c);
    let t = target(&w, 0);
    for a in [
        Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        Action::CastSelf {
            skill_id: c.skill.id.clone(),
        },
        Action::CastUnit {
            skill_id: c.additional_skills[0].id.clone(),
            target: t.clone(),
        },
    ] {
        assert_eq!(w.action(&a, "wrong", &c, 0), Err(Error::InvalidMessage));
    }
    assert_eq!(
        w.action(
            &Action::CastUnit {
                skill_id: "missing".into(),
                target: t.clone()
            },
            "wrong",
            &c,
            0
        ),
        Err(Error::Unsupported)
    );
    for version in 1..28 {
        let mut c = c.clone();
        c.mechanism_version = version;
        let mut w = make(&c);
        assert_eq!(cast(&mut w, &c, &t, 0), Err(Error::Unsupported));
    }
}
#[test]
fn unit_skill_pending_and_committed_views_keep_frozen_target() {
    let c = config();
    let mut w = make(&c);
    let t = target(&w, 1);
    cast(&mut w, &c, &t, 0).unwrap();
    assert_eq!(
        w.snapshot(&c, 0).pending_action.unwrap().unit_target,
        Some(t.clone())
    );
    let e = advance(&mut w, &c, 0, 120);
    assert!(
        e.iter()
            .any(|e| matches!(e,Event::ActionCommitted{unit_target:Some(v),..} if v==&t))
    );
    assert!(w.snapshot(&c, 120).pending_action.is_none());
}
#[test]
fn unit_skill_old_snapshots_and_events_omit_target_fields() {
    for sample in [
        "status-host",
        "stack-host",
        "attribute-host",
        "shield-host",
        "dot-host",
        "hot-host",
        "heal-modifier-host",
        "dual-skill-host",
        "self-skill-host",
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
        w.action(
            &Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.],
            },
            "legacy",
            &c,
            0,
        )
        .unwrap();
        let pending = serde_json::to_value(w.snapshot(&c, 0)).unwrap();
        assert!(
            pending
                .get("pending_action")
                .is_none_or(|v| v.get("unit_target").is_none())
        );
        let e = advance(&mut w, &c, 0, 300);
        let view = serde_json::to_value(w.snapshot(&c, 300)).unwrap();
        assert!(
            view["pve"]["monsters"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m.get("unit_target").is_none())
        );
        assert!(e.iter().all(|e| {
            serde_json::to_value(e)
                .unwrap()
                .get("unit_target")
                .is_none()
        }));
    }
}
#[test]
fn unit_skill_public_identity_validation_and_reconnect_do_not_duplicate() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine_with_config(config());
    let c = s::create(&e.config);
    let t = target(e.world.as_ref().unwrap(), 1);
    let input = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: "unit-cast".into(),
        sequence: 3,
        action: Action::CastUnit {
            skill_id: e.config.skill.id.clone(),
            target: t.clone(),
        },
    };
    let mut bad = input.clone();
    bad.request_id = "bad-zero".into();
    bad.action = Action::CastUnit {
        skill_id: e.config.skill.id.clone(),
        target: UnitTarget {
            id: t.id.clone(),
            generation: 0,
        },
    };
    assert_eq!(e.input(bad), Err(Error::InvalidMessage));
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
    assert_eq!(before.spirit, 88);
    e.input(input.clone()).unwrap();
    e.input(Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: Connection {
            id: "unit-resume".into(),
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
    assert_eq!(after.pve.unwrap().monsters[1].unit_target, Some(t));
    assert_eq!(after.spirit, before.spirit);
    assert_eq!(e.input(input), Err(Error::StaleConnection));
}
#[test]
fn unit_skill_hundred_closes_release_target_preparations_and_frozen_assets() {
    use crate::asset_contract::tests as s;
    for commit in [false, true] {
        for _ in 0..50 {
            let mut e = active_engine_with_config(config());
            let t = target(e.world.as_ref().unwrap(), 1);
            let now = e.status().logical_ms;
            let c = e.config.clone();
            cast(e.world.as_mut().unwrap(), &c, &t, now).unwrap();
            if commit {
                for _ in 0..5 {
                    e.tick();
                    e.drain();
                }
            }
            let key = s::create(&e.config).key;
            e.close(Close {
                key,
                request_id: "unit-close".into(),
            })
            .unwrap();
            assert_eq!(e.live_worlds(), 0);
            let r = s::requests(&e.drain()).remove(0);
            let receipt = s::receipt(
                &e.config,
                &r,
                ReceiptOutcome::Committed {
                    revision: 2,
                    commit_id: "unit-close-assets".into(),
                },
            );
            e.business(receipt).unwrap();
            assert_eq!(e.pending_count(), 0);
        }
    }
}
#[test]
fn unit_skill_no_resource_readiness_and_protection_cannot_prepare() {
    let c = config();
    for case in 0..4 {
        let mut w = make(&c);
        let t = target(&w, 0);
        match case {
            0 => w.spirit = 11,
            1 => w.ready = false,
            2 => w.protected = true,
            _ => w.hp = 0,
        };
        assert_eq!(
            cast(&mut w, &c, &t, 0),
            Err(if case == 0 {
                Error::NoResource
            } else {
                Error::NotReady
            })
        );
        assert!(w.pending.is_none());
        assert_eq!(w.skill_ready_at, 0);
    }
}
#[test]
fn unit_skill_background_after_commit_preserves_cd_recovery_and_projectile() {
    let c = config();
    let mut w = make(&c);
    let t = target(&w, 0);
    cast(&mut w, &c, &t, 0).unwrap();
    advance(&mut w, &c, 0, 120);
    w.action(&Action::Background {}, "background", &c, 150)
        .unwrap();
    assert_eq!(w.skill_ready_at, 1320);
    assert_eq!(w.recovery_until, 420);
    assert_eq!(w.spirit, 188);
    assert_eq!(w.pve.as_ref().unwrap().shots.len(), 1);
}
#[test]
fn unit_skill_return_to_home_changes_identity_without_rebinding() {
    let c = config();
    let mut w = make(&c);
    let old = target(&w, 0);
    let mut a = w.pve.take().unwrap();
    a.reset_boss(0, 30, &mut vec![]);
    w.pve = Some(a);
    let new = target(&w, 0);
    assert_eq!(old.id, new.id);
    assert_eq!(new.generation, old.generation + 1);
    assert_eq!(cast(&mut w, &c, &old, 30), Err(Error::InvalidTarget));
    assert!(cast(&mut w, &c, &new, 30).is_ok());
}
#[test]
fn unit_skill_legal_output_peak_keeps_target_views_and_frozen_asset_responsibility_bounded() {
    use crate::asset_contract::tests as s;
    let mut c = config();
    c.output_capacity = 32;
    c.skill.windup_ms = 0;
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
    let t = target(e.world.as_ref().unwrap(), 0);
    let w = e.world.as_mut().unwrap();
    let mut a = w.pve.take().unwrap();
    for i in 0..32 {
        let m = a.status_entity(i);
        a.commit_statuses(w, &["renewal".into(), "ward".into()], &m, now, &mut vec![]);
    }
    w.pve = Some(a);
    let c = e.config.clone();
    cast(e.world.as_mut().unwrap(), &c, &t, now).unwrap();
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
            commit_id: "unit-peak-assets".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
}
