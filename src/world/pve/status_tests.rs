use super::*;
use crate::config::{ConfigCatalog, DuplicatePolicy};

fn config() -> Config {
    let mut c = ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/status-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone();
    c.pve_mut();
    c
}
pub(super) fn world(c: &Config) -> World {
    let mut w = World::new(
        c,
        ParticipantFacts {
            principal: "principal-A".into(),
            revision: 1,
            hp_points: 1000,
            spirit_points: 200,
            atk_points: 100,
            def_points: 100,
            spd_points: 8,
            warehouse_quantity: 95,
        },
        7,
    );
    w.phase = Phase::Active;
    w.joined = true;
    w.ready = true;
    w.protected = false;
    w.active_at = Some(0);
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.next_decision = u64::MAX;
        m.next_attack = u64::MAX;
    }
    w
}
pub(super) fn cast(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1., 0.],
        },
        &format!("cast-{now}"),
        c,
        now,
    )
    .unwrap()
}
pub(super) fn advance(w: &mut World, c: &Config, from: u64, to: u64) -> Vec<Event> {
    (from + 1..=to)
        .filter(|t| t % c.step_ms == 0)
        .flat_map(|t| w.tick(c, t))
        .collect()
}
pub(super) fn damage(e: &[Event]) -> Vec<u32> {
    e.iter()
        .filter_map(|e| match e {
            Event::Damage { source, amount, .. } if source == "participant" => Some(*amount),
            _ => None,
        })
        .collect()
}
pub(super) fn bind(
    w: &mut World,
    id: &str,
    source: StatusEntity,
    target: StatusEntity,
    now: u64,
) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hit_statuses(w, &[id.into()], &source, &target, now, &mut e);
    w.pve = Some(a);
    e
}

#[test]
fn status_real_projectile_uses_old_defense_then_new_defense_and_expires() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 570);
    assert_eq!(damage(&e), vec![66]);
    assert_eq!(w.snapshot(&c, 570).effective_def, Some(200));
    assert_eq!(
        w.snapshot(&c, 570).pve.unwrap().monsters[0].effective_def,
        Some(50)
    );
    cast(&mut w, &c, 720);
    let e = advance(&mut w, &c, 570, 1200);
    assert_eq!(damage(&e), vec![80]);
    advance(&mut w, &c, 1200, 3000);
    let s = w.snapshot(&c, 3000);
    assert!(s.statuses.is_empty());
    assert_eq!(s.effective_def, Some(100));
    assert!(s.pve.unwrap().monsters[0].statuses.is_empty());
}
#[test]
fn status_cancel_before_commit_has_no_cost_cooldown_projectile_or_instance() {
    let c = config();
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    w.action(&Action::CancelAction {}, "cancel", &c, 30)
        .unwrap();
    advance(&mut w, &c, 0, 600);
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready_at, 0);
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    assert!(w.pve.as_ref().unwrap().statuses.is_empty());
}
#[test]
fn status_invalid_or_unaffordable_input_never_applies() {
    let c = config();
    let mut w = world(&c);
    {
        let direction = [0., 0.];
        assert!(
            w.action(
                &Action::Cast {
                    skill_id: c.skill.id.clone(),
                    direction
                },
                "bad",
                &c,
                0
            )
            .is_err()
        );
    }
    w.spirit = 0;
    assert_eq!(
        w.action(
            &Action::Cast {
                skill_id: c.skill.id.clone(),
                direction: [1., 0.]
            },
            "poor",
            &c,
            0
        ),
        Err(Error::NoResource)
    );
    assert!(w.pve.as_ref().unwrap().statuses.is_empty());
}
#[test]
fn status_dodge_or_lethal_contact_does_not_apply_to_target() {
    for lethal in [false, true] {
        let mut c = config();
        if lethal {
            c.pve_mut().stats.insert("hp".into(), 1);
        } else {
            c.pve_mut().stats.insert("dodge".into(), 100);
        }
        let mut w = world(&c);
        cast(&mut w, &c, 0);
        let e = advance(&mut w, &c, 0, 600);
        assert!(
            w.snapshot(&c, 600).pve.unwrap().monsters[0]
                .statuses
                .is_empty()
        );
        assert!(
            !e.iter().any(
                |e| matches!(e,Event::StatusApplied{status} if status.definition_id=="exposed")
            )
        );
    }
}
#[test]
fn status_legal_zero_damage_contact_applies_but_wall_does_not() {
    let mut c = config();
    c.params.minimum_damage = 0;
    c.skill.damage_bps = 0;
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    assert_eq!(damage(&advance(&mut w, &c, 0, 600)), vec![0]);
    assert_eq!(
        w.snapshot(&c, 600).pve.unwrap().monsters[0].statuses.len(),
        1
    );
    let mut c = config();
    let p = c.pve_mut();
    let mut tile = p.palette[0].clone();
    tile.blocks_projectiles = true;
    p.palette.push(tile);
    p.cells_rle = vec![(0, 332), (1, 1), (0, 691)];
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    assert!(damage(&advance(&mut w, &c, 0, 600)).is_empty());
    assert!(
        w.snapshot(&c, 600).pve.unwrap().monsters[0]
            .statuses
            .is_empty()
    );
}
#[test]
fn status_capacity_preflight_is_atomic_for_self_and_hit_groups() {
    let mut c = config();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    c.skill.on_commit_self_statuses = vec!["guard".into(), "exposed".into()];
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 300);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready_at, 0);
    assert!(w.pve.as_ref().unwrap().shots.is_empty());
    assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    c.skill.on_commit_self_statuses.clear();
    c.skill.on_hit_target_statuses = vec!["guard".into(), "exposed".into()];
    let mut w = world(&c);
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 600);
    assert_eq!(damage(&e), vec![66]);
    assert_eq!(w.spirit, 190);
    assert!(e.iter().any(|e| matches!(e, Event::StatusRejected { .. })));
    assert!(w.pve.as_ref().unwrap().statuses.is_empty());
}
#[test]
fn status_full_slot_still_refreshes_or_ignores_existing_instance() {
    for policy in [DuplicatePolicy::Refresh, DuplicatePolicy::Ignore] {
        let mut c = config();
        let p = c.pve_mut();
        p.status_rules.as_mut().unwrap().max_active_per_entity = 1;
        p.status_definitions[0].duplicate_policy = policy;
        let mut w = world(&c);
        let who = status::participant();
        bind(&mut w, "guard", who.clone(), who.clone(), 0);
        let first = w.snapshot(&c, 1).statuses[0].clone();
        let e = bind(&mut w, "guard", who.clone(), who, 300);
        let next = &w.snapshot(&c, 301).statuses[0];
        assert_eq!(first.instance_id, next.instance_id);
        match policy {
            DuplicatePolicy::Refresh => {
                assert_eq!(next.expires_at_ms, 1500);
                assert!(matches!(e[0], Event::StatusRefreshed { .. }));
            }
            DuplicatePolicy::Ignore => {
                assert_eq!(next, &first);
                assert!(matches!(e[0], Event::StatusIgnored { .. }));
            }
            DuplicatePolicy::Stack => unreachable!("legacy policies only"),
        }
    }
}
#[test]
fn status_source_scoping_separates_monster_and_participant_and_refresh_updates_owner() {
    for scoped in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[0].source_scoped = scoped;
        let mut w = world(&c);
        let player = status::participant();
        let monster = w.pve.as_ref().unwrap().status_entity(0);
        bind(&mut w, "guard", player.clone(), player.clone(), 0);
        bind(&mut w, "guard", monster.clone(), player.clone(), 300);
        let s = w.snapshot(&c, 301);
        assert_eq!(s.statuses.len(), if scoped { 2 } else { 1 });
        if !scoped {
            assert_eq!(s.statuses[0].source, monster);
        }
        let mut a = w.pve.take().unwrap();
        let mut e = vec![];
        a.clear_status_entity(&monster, StatusRemovalReason::SourceInvalid, &mut e);
        w.pve = Some(a);
        assert_eq!(w.snapshot(&c, 301).statuses.len(), usize::from(scoped));
    }
}
#[test]
fn status_half_open_expiry_read_only_views_and_reapplication_identity() {
    let mut c = config();
    c.pve_mut().status_definitions[0].duration_ms = 101;
    let mut w = world(&c);
    let who = status::participant();
    bind(&mut w, "guard", who.clone(), who.clone(), 0);
    let id = w.snapshot(&c, 100).statuses[0].instance_id.clone();
    assert_eq!(w.snapshot(&c, 100).effective_def, Some(200));
    assert_eq!(w.snapshot(&c, 101).effective_def, Some(100));
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.prepare_statuses(&w, 120, &mut e);
    a.prepare_statuses(&w, 120, &mut e);
    w.pve = Some(a);
    assert_eq!(e.len(), 1);
    bind(&mut w, "guard", who.clone(), who, 120);
    assert_ne!(w.snapshot(&c, 121).statuses[0].instance_id, id);
}
#[test]
fn status_defense_clamps_aggregates_and_uses_wide_integer_math() {
    let mut c = config();
    let p = c.pve_mut();
    p.status_definitions[0].modifier_bps = 100000;
    p.status_definitions[1].modifier_bps = -10000;
    let mut w = world(&c);
    let who = status::participant();
    bind(&mut w, "guard", who.clone(), who.clone(), 0);
    assert_eq!(w.snapshot(&c, 1).effective_def, Some(200));
    bind(&mut w, "exposed", who.clone(), who.clone(), 0);
    assert_eq!(w.snapshot(&c, 1).effective_def, Some(200));
    let mut c = config();
    c.pve_mut().status_definitions[0].modifier_bps = 5000;
    c.pve_mut().status_definitions[1].modifier_bps = -5000;
    let mut w = world(&c);
    bind(&mut w, "guard", who.clone(), who.clone(), 0);
    bind(&mut w, "exposed", who.clone(), who.clone(), 0);
    assert_eq!(w.snapshot(&c, 1).effective_def, Some(100));
    w.facts.def_points = 0;
    assert_eq!(w.snapshot(&c, 1).effective_def, Some(0));
}
#[test]
fn status_monster_projectile_commit_and_player_hit_share_the_same_store() {
    let mut c = config();
    let a = &mut c.pve_mut().combat.projectile;
    a.on_commit_self_statuses = vec!["guard".into()];
    a.on_hit_target_statuses = vec!["exposed".into()];
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[0].casting = Some((0, 30));
    let e = advance(&mut w, &c, 0, 600);
    assert!(e.iter().any(|e|matches!(e,Event::StatusApplied{status} if status.source.kind==StatusEntityKind::Monster && status.target.kind==StatusEntityKind::Participant)));
    assert_eq!(w.snapshot(&c, 600).effective_def, Some(50));
    assert_eq!(
        w.snapshot(&c, 600).pve.unwrap().monsters[0].effective_def,
        Some(200)
    );
}
#[test]
fn status_monster_capacity_rejection_does_not_commit_cost_or_cooldown() {
    let mut c = config();
    let p = c.pve_mut();
    p.status_rules.as_mut().unwrap().max_active_per_entity = 1;
    p.combat.projectile.on_commit_self_statuses = vec!["guard".into(), "exposed".into()];
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[0].casting = Some((0, 30));
    advance(&mut w, &c, 0, 60);
    let a = w.pve.as_ref().unwrap();
    assert_eq!(a.monsters[0].spirit, 200);
    assert_eq!(a.monsters[0].cooldown[0], 0);
    assert!(a.shots.is_empty());
    assert!(a.statuses.is_empty());
}
#[test]
fn status_background_and_foreground_keep_original_deadlines() {
    let c = config();
    let mut w = world(&c);
    let who = status::participant();
    bind(&mut w, "guard", who.clone(), who, 0);
    w.action(&Action::Background {}, "bg", &c, 30).unwrap();
    advance(&mut w, &c, 0, 600);
    assert_eq!(w.snapshot(&c, 600).statuses[0].expires_at_ms, 1200);
    w.action(&Action::Foreground {}, "fg", &c, 600).unwrap();
    advance(&mut w, &c, 600, 1230);
    assert!(w.snapshot(&c, 1230).statuses.is_empty());
}
#[test]
fn status_true_return_reset_invalidates_sources_and_old_projectile_binding() {
    let mut c = config();
    c.pve_mut().combat.projectile.on_hit_target_statuses = vec!["exposed".into()];
    let mut w = world(&c);
    let source = w.pve.as_ref().unwrap().status_entity(0);
    let player = status::participant();
    bind(&mut w, "guard", source.clone(), player.clone(), 0);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.reset_boss(0, 30, &mut e);
    a.hit_statuses(&w, &["exposed".into()], &source, &player, 30, &mut e);
    w.pve = Some(a);
    assert!(w.snapshot(&c, 30).statuses.is_empty());
    assert!(!e.iter().any(|e| matches!(e, Event::StatusApplied { .. })));
}
#[test]
fn status_target_death_clears_its_instances_and_owned_effects() {
    let c = config();
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    let player = status::participant();
    bind(&mut w, "guard", m.clone(), player, 0);
    bind(&mut w, "exposed", m.clone(), m, 0);
    let mut a = w.pve.take().unwrap();
    a.hurt_monster(&mut w, &c, 0, 1000000, false, &mut vec![]);
    assert!(a.statuses.is_empty());
    w.pve = Some(a);
}
#[test]
fn status_leave_extraction_and_close_release_instances_including_foreign_sources() {
    for mode in [0, 1, 2] {
        let c = config();
        let mut w = world(&c);
        let m = w.pve.as_ref().unwrap().status_entity(0);
        bind(&mut w, "guard", m.clone(), m, 0);
        let events = match mode {
            0 => w.action(&Action::Leave {}, "leave", &c, 0).unwrap(),
            1 => {
                w.extraction_end = Some(30);
                w.tick(&c, 30)
            }
            _ => w.close_container_interaction(0),
        };
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::StatusRemoved { .. }))
        );
    }
}
#[test]
fn status_hundred_lifetimes_clear_without_retaining_timers_or_ids() {
    let c = config();
    for _ in 0..100 {
        let mut w = world(&c);
        cast(&mut w, &c, 0);
        advance(&mut w, &c, 0, 600);
        assert!(!w.pve.as_ref().unwrap().statuses.is_empty());
        w.close_container_interaction(600);
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
    }
}

#[test]
fn status_encounter_reset_clears_old_round_and_cannot_reapply_from_it() {
    let sample = config();
    let mut c = crate::config::default_catalog()
        .active("b3-13")
        .unwrap()
        .as_ref()
        .clone();
    c.schema_version = 4;
    c.mechanism_version = 19;
    c.asset_contract = sample.asset_contract;
    c.result_contract = sample.result_contract;
    let p = c.pve_mut();
    p.status_definitions = sample.pve.as_ref().unwrap().status_definitions.clone();
    p.status_rules = sample.pve.as_ref().unwrap().status_rules.clone();
    let center = p.encounters[0].trigger.center;
    let delay = p.encounters[0].reset.as_ref().unwrap().delay_ms;
    let mut w = world(&c);
    w.position = center;
    advance(&mut w, &c, 0, 30);
    let a = w.pve.as_ref().unwrap();
    let index = a
        .monsters
        .iter()
        .position(|m| m.enabled && m.encounter_index == Some(0))
        .unwrap();
    let old = a.status_entity(index);
    bind(&mut w, "guard", old.clone(), status::participant(), 30);
    let mut a = w.pve.take().unwrap();
    let mut events = vec![];
    w.position = [0., 0.];
    a.advance_encounter_resets(&w, 60, &mut events);
    a.advance_encounter_resets(&w, 60 + delay, &mut events);
    assert!(a.statuses.is_empty());
    assert_ne!(a.status_entity(index), old);
    a.hit_statuses(
        &w,
        &["guard".into()],
        &old,
        &status::participant(),
        60 + delay,
        &mut events,
    );
    assert!(a.statuses.is_empty());
    assert!(events.iter().any(|e| matches!(
        e,
        Event::StatusRemoved {
            reason: StatusRemovalReason::Reset,
            ..
        }
    )));
}

fn active_engine() -> crate::engine::Engine {
    active_engine_with_config(config())
}
pub(super) fn active_engine_with_config(c: Config) -> crate::engine::Engine {
    use crate::asset_contract::tests as s;
    let mut e = s::admitted_with_config(c.clone());
    s::input(&mut e, 1, Action::Join {});
    let p = e
        .drain()
        .into_iter()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap();
    let create = s::create(&c);
    e.transport(TransportReceipt {
        key: create.key,
        recipient_id: create.participant_id,
        connection: create.connection,
        output_id: p.id.clone(),
        outcome: TransportOutcome::Sent,
    })
    .unwrap();
    s::input(
        &mut e,
        2,
        Action::CombatReady {
            publication_id: p.id,
        },
    );
    e.tick();
    e.drain();
    e
}
#[test]
fn status_public_engine_deduplicates_input_and_rejects_nonfinite_payloads() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine();
    let c = s::create(&e.config);
    let input = Input {
        key: c.key,
        participant_id: c.participant_id,
        entity_generation: 1,
        connection: c.connection,
        request_id: "status-cast".into(),
        sequence: 3,
        action: Action::Cast {
            skill_id: e.config.skill.id.clone(),
            direction: [1., 0.],
        },
    };
    assert_eq!(e.input(input.clone()), e.input(input.clone()));
    let mut events = vec![];
    for _ in 0..20 {
        e.tick();
        events.extend(e.drain());
    }
    assert_eq!(events.iter().filter(|o|matches!(&o.body,OutputBody::Event{event:Event::StatusApplied{status}} if status.definition_id=="guard")).count(),1);
    let mut bad = input;
    bad.request_id = "nan".into();
    bad.sequence = 4;
    bad.action = Action::Cast {
        skill_id: e.config.skill.id.clone(),
        direction: [f64::NAN, 0.],
    };
    assert_eq!(e.input(bad), Err(Error::InvalidMessage));
}
#[test]
fn status_new_connection_preserves_instance_and_rejects_old_generation() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine();
    let c = s::create(&e.config);
    let skill_id = e.config.skill.id.clone();
    s::input(
        &mut e,
        3,
        Action::Cast {
            skill_id,
            direction: [1., 0.],
        },
    );
    for _ in 0..5 {
        e.tick();
        e.drain();
    }
    let before = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms)
        .statuses;
    assert_eq!(before.len(), 1);
    let reconnect = Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: Connection {
            id: "replacement".into(),
            generation: 2,
        },
        request_id: "reconnect".into(),
        sequence: 4,
        action: Action::Reconnect {
            previous_connection: c.connection.clone(),
            authenticated_principal: c.authenticated_principal,
        },
    };
    e.input(reconnect).unwrap();
    assert_eq!(
        e.world
            .as_ref()
            .unwrap()
            .snapshot(&e.config, e.status().logical_ms)
            .statuses,
        before
    );
    assert_eq!(
        e.input(Input {
            key: c.key,
            participant_id: c.participant_id,
            entity_generation: 1,
            connection: c.connection,
            request_id: "old-bg".into(),
            sequence: 4,
            action: Action::Background {}
        }),
        Err(Error::StaleConnection)
    );
}

#[test]
fn status_both_damage_directions_use_clamped_effective_defense_exactly() {
    for (bps, expected) in [
        (-10000, 100),
        (-5000, 80),
        (0, 66),
        (10000, 50),
        (100000, 50),
    ] {
        let mut c = config();
        c.pve_mut().status_definitions[0].modifier_bps = bps;
        let mut w = world(&c);
        let m = w.pve.as_ref().unwrap().status_entity(0);
        let player = status::participant();
        bind(&mut w, "guard", player.clone(), player.clone(), 0);
        bind(&mut w, "guard", player, m, 0);
        let mut a = w.pve.take().unwrap();
        let mut events = vec![];
        a.hurt_player(&mut w, &c, 0, (100, false), 0, &mut events);
        assert_eq!(1000 - w.hp, expected);
        assert_eq!(
            a.hurt_monster(&mut w, &c, 0, 100, false, &mut events),
            expected
        );
    }
}

#[test]
fn status_event_burst_faults_with_frozen_responsibility_and_releases_world() {
    let mut c = config();
    c.output_capacity = 32;
    let p = c.pve_mut();
    p.status_rules.as_mut().unwrap().max_active_per_entity = 3;
    p.monster_positions = (0..32)
        .map(|i| {
            [
                100. + f64::from(i % 16) * 40.,
                600. + f64::from(i / 16) * 80.,
            ]
        })
        .collect();
    p.combat.projectile.on_commit_self_statuses = vec!["guard".into(), "exposed".into()];
    let mut e = active_engine_with_config(c);
    for m in &mut e.world.as_mut().unwrap().pve.as_mut().unwrap().monsters {
        m.casting = Some((0, 60));
    }
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
    let request = crate::asset_contract::tests::requests(&outputs).remove(0);
    assert!(matches!(
        request.body,
        Business::CommitAssets {
            transaction: AssetTransaction::ResultSettlement { .. }
        }
    ));
    let receipt = crate::asset_contract::tests::receipt(
        &e.config,
        &request,
        ReceiptOutcome::Committed {
            revision: 2,
            commit_id: "burst-result".into(),
        },
    );
    e.business(receipt).unwrap();
    assert_eq!(e.pending_count(), 0);
}
