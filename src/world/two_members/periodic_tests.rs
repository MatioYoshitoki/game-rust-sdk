use super::*;
use crate::config::DuplicatePolicy;
pub(super) fn setup(n: usize) -> (Config, World) {
    let catalog = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/multiplayer-periodic-host/catalog.json"),
    )
    .unwrap();
    let mut c = catalog.default_config().as_ref().clone();
    let base = c.spawn;
    c.members.as_mut().unwrap().capacity = n;
    c.members.as_mut().unwrap().spawns = (0..n)
        .map(|i| {
            [
                base[0] - (i / 2) as f64 * 64.,
                base[1] + (i % 2) as f64 * 80.,
            ]
        })
        .collect();
    let seats: Vec<_> = (0..n)
        .map(|i| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            combat_build: None,
            skills: None,
            participant_id: format!("P{i:02}"),
            authenticated_principal: format!("principal-P{i:02}"),
            connection: Connection {
                id: format!("c{i}"),
                generation: 1,
            },
        })
        .collect();
    let facts = seats
        .iter()
        .enumerate()
        .map(|(i, s)| ParticipantFacts {
            principal: s.authenticated_principal.clone(),
            revision: 0,
            hp_points: 1000 + i as u32 * 100,
            spirit_points: 300,
            atk_points: 100 + i as u32 * 100,
            def_points: 100,
            spd_points: 8,
            warehouse_quantity: 100,
        })
        .collect();
    let mut w = World::new(&c, &seats, facts, 77);
    for m in &mut w.members {
        m.joined = true;
        m.ready = true;
    }
    w.activate(&c, 0, &mut vec![]);
    (c, w)
}
pub(super) fn bind(
    w: &mut World,
    c: &Config,
    source: Owner,
    target: EntityRef,
    id: &str,
    now: u64,
) -> Events {
    let max = match target.kind {
        StatusEntityKind::Participant => {
            w.members
                .iter()
                .find(|m| m.reference.entity == target)
                .unwrap()
                .facts
                .hp_points
        }
        StatusEntityKind::Monster => w
            .npcs
            .iter()
            .find(|n| n.entity == target)
            .map_or(0, |n| n.profile.stats["hp"]),
    };
    let mut e = vec![];
    w.apply_owned_statuses(source, (target, max), &[id.into()], c, now, &mut e);
    e
}
fn dot(w: &mut World, c: &Config, source: usize, npc: usize) -> Events {
    bind(
        w,
        c,
        Owner::Member(source),
        w.npcs[npc].entity.clone(),
        "wound",
        0,
    )
}
fn incoming(w: &mut World, c: &Config, npc: usize, target: usize) -> Events {
    bind(
        w,
        c,
        Owner::Npc(npc),
        w.members[target].reference.entity.clone(),
        "wound",
        0,
    )
}
fn hot(w: &mut World, c: &Config, target: usize) -> Events {
    bind(
        w,
        c,
        Owner::Member(target),
        w.members[target].reference.entity.clone(),
        "renewal",
        0,
    )
}
pub(super) fn pulse(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.prepare_periodic(now, &mut e);
    w.advance_periodic(c, now, &mut e);
    e
}
fn ticks(e: &Events) -> Vec<(&Option<usize>, &Event)> {
    e.iter()
        .filter_map(|(p, e)| {
            if let EventV4::Private { event } = e {
                if matches!(
                    event,
                    Event::PeriodicTick { .. } | Event::PeriodicHealTick { .. }
                ) {
                    Some((p, event))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn mp_periodic_real_identity_and_independent_sources_for_one_two_four_eight() {
    for n in [1, 2, 4, 8] {
        let (c, mut w) = setup(n);
        for i in 0..n {
            dot(&mut w, &c, i, 0);
            hot(&mut w, &c, i);
        }
        let e = pulse(&mut w, &c, 600);
        assert_eq!(
            ticks(&e)
                .iter()
                .filter(|(_, e)| matches!(e, Event::PeriodicTick { .. }))
                .count(),
            n
        );
        for i in 0..n {
            let s = w.snapshot(i, &c, Phase::Active, 600);
            assert!(
                s.monsters[0]
                    .statuses
                    .as_ref()
                    .unwrap()
                    .iter()
                    .all(|v| v.source.id == format!("P{i:02}"))
            );
            assert_eq!(s.own.statuses[0].target.id, format!("P{i:02}"));
        }
    }
}
#[test]
fn mp_periodic_refresh_transfers_owner_and_old_source_exit_keeps_new_instance() {
    let (mut c, mut w) = setup(2);
    c.pve_mut().status_definitions[0].source_scoped = false;
    dot(&mut w, &c, 0, 0);
    let id = w.statuses.entries()[0].instance_id.clone();
    dot(&mut w, &c, 1, 0);
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(w.statuses.entries()[0].instance_id, id);
    assert_eq!(w.statuses.entries()[0].source.id, "P01");
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    assert!(
        ticks(&pulse(&mut w, &c, 600))
            .iter()
            .any(|(p, e)| **p == Some(1)
                && matches!(e,Event::PeriodicTick{source,..}if source.id=="P01"))
    );
}
#[test]
fn mp_periodic_ignore_keeps_actual_owner_and_hides_its_instance_from_attempting_member() {
    let (mut c, mut w) = setup(2);
    c.pve_mut().status_definitions[0].source_scoped = false;
    c.pve_mut().status_definitions[0].duplicate_policy = DuplicatePolicy::Ignore;
    dot(&mut w, &c, 0, 0);
    let e = dot(&mut w, &c, 1, 0);
    assert_eq!(w.statuses.entries()[0].source.id, "P00");
    assert!(e.iter().all(|(p, _)| *p == Some(0)));
    w.terminal_intent(1, Outcome::Death, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert!(w.statuses.is_empty());
}
#[test]
fn mp_periodic_dot_captures_each_effective_source_and_reads_current_defense() {
    let (c, mut w) = setup(2);
    dot(&mut w, &c, 0, 0);
    dot(&mut w, &c, 1, 0);
    let raw: Vec<_> = w
        .statuses
        .entries()
        .iter()
        .map(|s| s.periodic.as_ref().unwrap().raw_damage)
        .collect();
    assert_eq!(raw, vec![60, 120]);
    w.members[0].facts.atk_points = 999;
    let e = pulse(&mut w, &c, 600);
    assert!(ticks(&e).iter().any(|(_, e)| matches!(
        e,
        Event::PeriodicTick {
            raw_damage: 60,
            hp_damage: 40,
            ..
        }
    )));
    let def = &mut Arc::make_mut(&mut w.npcs[0].profile).stats;
    def.insert("def".into(), 0);
    let e = pulse(&mut w, &c, 1200);
    assert!(ticks(&e).iter().any(|(_, e)| matches!(
        e,
        Event::PeriodicTick {
            raw_damage: 60,
            hp_damage: 60,
            ..
        }
    )));
}
#[test]
fn mp_periodic_hot_freezes_individual_max_hp_and_clips_missing_hp() {
    let (c, mut w) = setup(2);
    w.members[0].hp = 980;
    w.members[1].hp = 1000;
    hot(&mut w, &c, 0);
    hot(&mut w, &c, 1);
    let e = pulse(&mut w, &c, 450);
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.members[1].hp, 1055);
    assert!(ticks(&e).iter().any(|(_, e)| matches!(
        e,
        Event::PeriodicHealTick {
            raw_heal: 50,
            hp_restored: 20,
            ..
        }
    )));
    let e = pulse(&mut w, &c, 900);
    assert!(ticks(&e).iter().any(|(_, e)| matches!(
        e,
        Event::PeriodicHealTick {
            raw_heal: 50,
            hp_restored: 0,
            ..
        }
    )));
}
#[test]
fn mp_periodic_first_last_half_open_and_non_aligned_schedule() {
    let (mut c, mut w) = setup(1);
    let d = &mut c.pve_mut().status_definitions[0];
    d.tick_interval_ms = Some(601);
    d.duration_ms = 1400;
    dot(&mut w, &c, 0, 0);
    assert!(ticks(&pulse(&mut w, &c, 600)).is_empty());
    let e = pulse(&mut w, &c, 630);
    assert!(matches!(
        ticks(&e)[0].1,
        Event::PeriodicTick {
            scheduled_at_ms: 601,
            executed_at_ms: 630,
            ..
        }
    ));
    let e = pulse(&mut w, &c, 1230);
    assert!(matches!(
        ticks(&e)[0].1,
        Event::PeriodicTick {
            scheduled_at_ms: 1202,
            executed_at_ms: 1230,
            ..
        }
    ));
    assert!(ticks(&pulse(&mut w, &c, 1400)).is_empty());
    assert!(w.statuses.is_empty());
}
#[test]
fn mp_periodic_expiry_precedes_late_jobs_and_refresh_resets_cadence() {
    let (c, mut w) = setup(1);
    dot(&mut w, &c, 0, 0);
    pulse(&mut w, &c, 600);
    let id = w.statuses.entries()[0].instance_id.clone();
    let target = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "wound", 900);
    assert_eq!(w.statuses.entries()[0].instance_id, id);
    assert_eq!(
        w.statuses.entries()[0]
            .periodic
            .as_ref()
            .unwrap()
            .next_tick_at_ms,
        Some(1500)
    );
    assert!(ticks(&pulse(&mut w, &c, 1200)).is_empty());
    assert!(ticks(&pulse(&mut w, &c, 5400)).is_empty());
}
#[test]
fn mp_periodic_dot_before_hot_cannot_revive_dead_target_and_other_hot_continues() {
    let (mut c, mut w) = setup(2);
    c.pve_mut().status_definitions[1].tick_interval_ms = Some(600);
    w.members[0].hp = 1;
    w.members[1].hp = 500;
    hot(&mut w, &c, 0);
    incoming(&mut w, &c, 0, 0);
    hot(&mut w, &c, 1);
    let e = pulse(&mut w, &c, 600);
    assert_eq!(w.members[0].phase, MemberPhase::Settling);
    assert_eq!(w.members[0].hp, 0);
    assert_eq!(w.members[1].hp, 555);
    assert!(
        !ticks(&e)
            .iter()
            .any(|(p, e)| **p == Some(0) && matches!(e, Event::PeriodicHealTick { .. }))
    );
    assert_eq!(w.statuses.entries().len(), 1);
}
#[test]
fn mp_periodic_member_protection_consumes_only_related_plans_without_replay() {
    let (c, mut w) = setup(8);
    dot(&mut w, &c, 0, 0);
    incoming(&mut w, &c, 0, 0);
    incoming(&mut w, &c, 1, 7);
    w.protect(0, 0, &mut vec![]);
    let before = w.members[7].hp;
    let e = pulse(&mut w, &c, 600);
    assert!(w.members[7].hp < before);
    assert!(
        ticks(&e)
            .iter()
            .filter(|(p, _)| **p == Some(0))
            .all(|(_, e)| matches!(
                e,
                Event::PeriodicTick {
                    outcome: PeriodicTickOutcome::Protected,
                    hp_damage: 0,
                    ..
                }
            ))
    );
    w.members[0].foreground = true;
    w.members[0].ready = true;
    w.members[0].protected = false;
    assert!(ticks(&pulse(&mut w, &c, 900)).is_empty());
    let e = pulse(&mut w, &c, 1200);
    assert_eq!(ticks(&e).iter().filter(|(p, _)| **p == Some(0)).count(), 2);
}
#[test]
fn mp_periodic_returning_skips_source_and_target_without_delaying_other_npc() {
    let (c, mut w) = setup(2);
    dot(&mut w, &c, 0, 0);
    incoming(&mut w, &c, 0, 0);
    incoming(&mut w, &c, 1, 1);
    w.npcs[0].returning = true;
    let e = pulse(&mut w, &c, 600);
    assert!(
        ticks(&e)
            .iter()
            .filter(|(p, _)| **p == Some(0))
            .all(|(_, e)| matches!(
                e,
                Event::PeriodicTick {
                    outcome: PeriodicTickOutcome::Returning,
                    hp_damage: 0,
                    ..
                }
            ))
    );
    assert!(ticks(&e).iter().any(|(p, e)| **p == Some(1)
        && matches!(
            e,
            Event::PeriodicTick {
                outcome: PeriodicTickOutcome::Applied,
                ..
            }
        )));
}
#[test]
fn mp_periodic_illegal_friendly_or_stale_target_does_not_apply() {
    let (c, mut w) = setup(2);
    let target = w.members[1].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "wound", 0);
    let target = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "renewal", 0);
    let mut stale = target;
    stale.generation += 1;
    bind(&mut w, &c, Owner::Member(0), stale, "wound", 0);
    assert!(w.statuses.is_empty());
}
#[test]
fn mp_periodic_whole_group_capacity_rejection_does_not_refresh_or_charge() {
    let (mut c, mut w) = setup(1);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    hot(&mut w, &c, 0);
    let before = w.statuses.entries().to_vec();
    let mut duplicate = c.pve.as_ref().unwrap().status_definitions[1].clone();
    duplicate.id = "renewal-2".into();
    c.pve_mut().status_definitions.push(duplicate);
    let self_ref = w.members[0].reference.entity.clone();
    let mut e = vec![];
    w.apply_owned_statuses(
        Owner::Member(0),
        (self_ref, 1000),
        &["renewal".into(), "renewal-2".into()],
        &c,
        300,
        &mut e,
    );
    assert_eq!(w.statuses.entries(), before);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusRejected {
                reason: Error::Capacity,
                ..
            }
        }
    )));
}
#[test]
fn mp_periodic_npc_commit_self_hot_and_hit_dot_use_npc_identity() {
    let (c, mut w) = setup(2);
    w.npcs[0].casting = Some(super::npc_skills::NpcCast::legacy(30, [-1., 0.]));
    w.npcs[0].hp = 1000;
    let mut e = vec![];
    w.advance_npcs(&c, 30, 0.03, &mut e);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.source == s.target && s.source.kind == StatusEntityKind::Monster)
    );
    let e = pulse(&mut w, &c, 480);
    assert!(ticks(&e).iter().any(|(_,e)|matches!(e,Event::PeriodicHealTick{source,hp_restored,..}if source.kind==StatusEntityKind::Monster&&*hp_restored>0)));
    for now in [60, 90, 120, 150, 180] {
        w.advance_shots(&c, now, 0.03, &mut vec![]);
    }
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.source.kind == StatusEntityKind::Monster
                && s.target.kind == StatusEntityKind::Participant)
    );
}
#[test]
fn mp_periodic_npc_self_capacity_fails_before_spirit_cooldown_and_projectile() {
    let (mut c, mut w) = setup(1);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    dot(&mut w, &c, 0, 0);
    w.npcs[0].casting = Some(super::npc_skills::NpcCast::legacy(30, [-1., 0.]));
    let before = (w.npcs[0].spirit, w.npcs[0].next_cast);
    w.advance_npcs(&c, 30, 0.03, &mut vec![]);
    assert_eq!((w.npcs[0].spirit, w.npcs[0].next_cast), before);
    assert!(w.shots.is_empty());
}
#[test]
fn mp_periodic_dot_does_not_consume_rng_or_repeat_direct_contact_rewards() {
    let (c, mut w) = setup(1);
    dot(&mut w, &c, 0, 0);
    let before = (w.rng, w.members[0].spirit, w.shots.len());
    pulse(&mut w, &c, 600);
    assert_eq!((w.rng, w.members[0].spirit, w.shots.len()), before);
    assert_eq!(w.statuses.entries().len(), 1);
}
#[test]
fn mp_periodic_dot_kill_credit_and_drop_are_once_for_actual_source() {
    let (c, mut w) = setup(2);
    w.npcs[0].hp = 1;
    dot(&mut w, &c, 1, 0);
    let e = pulse(&mut w, &c, 600);
    assert!(e.iter().any(
        |(_, e)| matches!(e,EventV4::MonsterDefeated{credited_to,..}if credited_to.id=="P01")
    ));
    assert!(w.members[0].tally.is_empty());
    assert_eq!(w.members[1].tally.values().sum::<u32>(), 1);
    let drops = w.resources.len();
    assert!(drops > 0);
    pulse(&mut w, &c, 1200);
    assert_eq!(w.resources.len(), drops);
}
#[test]
fn mp_periodic_simultaneous_sources_cannot_double_kill_or_drop() {
    let (c, mut w) = setup(2);
    w.npcs[0].hp = 1;
    dot(&mut w, &c, 0, 0);
    dot(&mut w, &c, 1, 0);
    let e = pulse(&mut w, &c, 600);
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(e, EventV4::MonsterDefeated { .. }))
            .count(),
        1
    );
    assert_eq!(
        w.members.iter().flat_map(|m| m.tally.values()).sum::<u32>(),
        1
    );
    assert!(w.statuses.is_empty());
}
#[test]
fn mp_periodic_exit_clears_only_actual_source_and_target_with_private_removal_ids() {
    let (c, mut w) = setup(2);
    dot(&mut w, &c, 0, 0);
    dot(&mut w, &c, 1, 0);
    hot(&mut w, &c, 0);
    let kept = w.statuses.entries()[1].instance_id.clone();
    let mut e = vec![];
    w.terminal_intent(0, Outcome::Death, &mut e);
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(w.statuses.entries()[0].instance_id, kept);
    assert!(
        e.iter()
            .filter(|(_, e)| matches!(
                e,
                EventV4::Private {
                    event: Event::StatusRemoved { .. }
                }
            ))
            .all(|(i, _)| *i == Some(0))
    );
    assert!(
        ticks(&pulse(&mut w, &c, 600))
            .iter()
            .any(|(p, _)| **p == Some(1))
    );
}
#[test]
fn mp_periodic_npc_death_removes_its_foreign_dot_and_leaves_other_sources() {
    let (c, mut w) = setup(2);
    incoming(&mut w, &c, 0, 0);
    incoming(&mut w, &c, 1, 1);
    w.hurt_npc(0, 0, u32::MAX, &c, 30, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(w.statuses.entries()[0].source.id, w.npcs[1].entity.id);
}
#[test]
fn mp_periodic_true_npc_reset_fences_old_generation_statuses_and_shots() {
    let (c, mut w) = setup(2);
    w.npcs[0].casting = Some(super::npc_skills::NpcCast::legacy(30, [-1., 0.]));
    w.advance_npcs(&c, 30, 0.03, &mut vec![]);
    incoming(&mut w, &c, 0, 0);
    dot(&mut w, &c, 1, 0);
    let old = w.npcs[0].entity.clone();
    w.npcs[0].returning = true;
    w.npcs[0].position = w.npcs[0].home;
    w.advance_npcs(&c, 60, 0.03, &mut vec![]);
    assert_eq!(w.npcs[0].entity.generation, old.generation + 1);
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.source != status_entity(&old) && s.target != status_entity(&old))
    );
    assert!(w.shots.iter().all(|s| s.view.source != old));
    assert!(!w.valid_status_entity(&status_entity(&old)));
}
#[test]
fn mp_periodic_npc_views_are_projected_per_source_and_self_hot_is_shared() {
    let (c, mut w) = setup(3);
    dot(&mut w, &c, 0, 0);
    dot(&mut w, &c, 1, 0);
    let target = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target, "renewal", 0);
    for i in 0..3 {
        let s = w.snapshot(i, &c, Phase::Active, 30);
        let v = s.monsters[0].statuses.as_ref().unwrap();
        assert_eq!(v.len(), if i < 2 { 2 } else { 1 });
        assert!(
            v.iter()
                .all(|s| s.source == s.target || s.source.id == format!("P{i:02}"))
        );
        let json = serde_json::to_value(s).unwrap();
        assert!(
            json["members"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m.get("statuses").is_none())
        );
    }
}
#[test]
fn mp_periodic_zero_hot_and_zero_dot_use_existing_formula_boundaries() {
    let (mut c, mut w) = setup(1);
    c.pve_mut().status_definitions[0].damage_bps = Some(0);
    c.pve_mut().status_definitions[1].heal_bps = Some(0);
    w.members[0].hp = 500;
    dot(&mut w, &c, 0, 0);
    hot(&mut w, &c, 0);
    let e = pulse(&mut w, &c, 600);
    assert!(ticks(&e).iter().any(|(_, e)| matches!(
        e,
        Event::PeriodicHealTick {
            raw_heal: 0,
            hp_restored: 0,
            ..
        }
    )));
    assert!(ticks(&e).iter().any(|(_,e)|matches!(e,Event::PeriodicTick{raw_damage:0,hp_damage,..}if *hp_damage==c.params.minimum_damage)));
}
#[test]
fn mp_periodic_absolute_terminal_and_repeated_exit_never_run_late_ticks() {
    let (c, mut w) = setup(2);
    incoming(&mut w, &c, 0, 0);
    hot(&mut w, &c, 1);
    let e = w.tick(&c, c.terminal_ms);
    assert!(ticks(&e).is_empty());
    assert!(w.statuses.is_empty());
    let mut e = vec![];
    w.terminal_intent(0, Outcome::Death, &mut e);
    assert!(e.is_empty());
}
#[test]
fn mp_periodic_actual_hp_loss_interrupts_extraction_before_completion() {
    let (c, mut w) = setup(2);
    incoming(&mut w, &c, 0, 0);
    w.members[0].extract_end = Some(600);
    let e = w.tick(&c, 600);
    assert_eq!(w.members[0].extract_end, None);
    assert_eq!(w.members[0].desired, None);
    assert!(e.iter().any(|(p, e)| *p == Some(0)
        && matches!(
            e,
            EventV4::Private {
                event: Event::ExtractionInterrupted
            }
        )));
}
#[test]
fn mp_periodic_application_freezes_effective_atk_after_existing_attribute_buff() {
    let (mut c, mut w) = setup(1);
    let d:crate::config::StatusDefinition=serde_json::from_value(serde_json::json!({"id":"strong","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":10000,"duration_ms":4500,"duplicate_policy":"refresh","source_scoped":true})).unwrap();
    c.pve_mut().status_definitions.push(d);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target, "strong", 0);
    dot(&mut w, &c, 0, 0);
    assert_eq!(
        w.statuses.entries()[1]
            .periodic
            .as_ref()
            .unwrap()
            .raw_damage,
        120
    );
    let entity = w.members[0].reference.entity.clone();
    w.statuses.remove(
        StatusRemovalReason::Expired,
        |s| s.target == status_entity(&entity),
        &mut vec![],
    );
    let e = pulse(&mut w, &c, 600);
    assert!(ticks(&e).iter().any(|(_, e)| matches!(
        e,
        Event::PeriodicTick {
            raw_damage: 120,
            hp_damage: 80,
            ..
        }
    )));
}
