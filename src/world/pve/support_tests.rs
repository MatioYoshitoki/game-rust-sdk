use super::*;
use crate::config::{EncounterConfig, EncounterTrigger, MonsterSpawn};
pub(super) fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-11")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = c.spawn;
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    let p = c.pve_mut();
    p.width = 64;
    p.height = 64;
    p.cells_rle = vec![(0, 4096)];
    p.stats.insert("hp".into(), 300);
    p.stats.insert("spirit".into(), 100);
    p.combat.cast_chance = 0.0;
    p.combat.heal.hp_threshold = 0.0;
    for key in ["atk", "spd", "crit", "dodge", "def"] {
        p.stats.insert(key.into(), 0);
    }
    for t in p.additional_templates.values_mut() {
        t.stats = p.stats.clone();
        t.combat.cast_chance = 0.0;
        t.combat.heal.hp_threshold = 0.0;
        t.area = None;
        if let Some(b) = &mut t.boss {
            for phase in &mut b.phases {
                phase.cast_chance = 0.0;
            }
        }
    }
    let s = p
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap();
    s.include_self = false;
    s.hp_threshold = 0.85;
    s.cast_chance = 1.0;
    s.range = 300.0;
    s.windup_ms = 300;
    s.ticks = 3;
    s.tick_interval_ms = 100;
    s.heal_bps = 1000;
    s.cooldown_ms = 2000;
    s.recovery_ms = 100;
    s.interrupt_window_ms = 500;
    s.interrupt_cooldown_ms = 1000;
    p.chest_spawns[0].position = [1000.0, 500.0];
    p.encounters = vec![EncounterConfig {
        reset: None,
        id: "team".into(),
        name: "team".into(),
        requires: vec![],
        trigger: EncounterTrigger {
            center: [500.0, 500.0],
            radius: 200.0,
        },
        monsters: vec![
            MonsterSpawn {
                id: "target-b".into(),
                template_id: "verification-phase-boss".into(),
                position: [600.0, 500.0],
            },
            MonsterSpawn {
                id: "support-a".into(),
                template_id: "verification-forest-support".into(),
                position: [500.0, 500.0],
            },
            MonsterSpawn {
                id: "target-a".into(),
                template_id: p.template_id.clone(),
                position: [700.0, 500.0],
            },
        ],
        required_monster_ids: vec!["target-b".into(), "target-a".into(), "support-a".into()],
        waves: vec![],
        reward_chest_ids: vec!["sanctum-cache".into()],
    }];
    p.map_objective.as_mut().unwrap().required_encounter_ids = vec!["team".into()];
    c
}
pub(super) fn world(c: &Config) -> World {
    let mut w = World::new(
        c,
        ParticipantFacts {
            principal: "principal-A".into(),
            revision: 1,
            hp_points: 1000,
            spirit_points: 100,
            atk_points: 100,
            def_points: 0,
            spd_points: 8,
            warehouse_quantity: 95,
        },
        7,
    );
    w.phase = Phase::Active;
    w.ready = true;
    w.joined = true;
    w.foreground = true;
    w.protected = false;
    w.active_at = Some(0);
    let mut a = w.pve.take().unwrap();
    a.prepare_encounters(&w, 30, &mut vec![]);
    assert!(a.monsters.iter().all(|m| m.enabled));
    a.monsters[0].hp = 120;
    a.monsters[1].aggro = true;
    w.pve = Some(a);
    w
}
pub(super) fn start(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut events = vec![];
    a.try_start_support(w, c, 1, now, &mut events);
    w.pve = Some(a);
    events
}
fn advance(w: &mut World, c: &Config, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.advance_supports(w, c, now, &mut e);
    w.pve = Some(a);
    e
}
fn hit(w: &mut World, c: &Config, raw: u32, accepted: Option<u64>, now: u64) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    a.now = now;
    let mut e = vec![];
    a.hurt_monster_contact(w, c, 1, (raw, false, accepted), &mut e);
    w.pve = Some(a);
    e
}
fn view(w: &World) -> SupportView {
    let arena = w.pve.as_ref().unwrap();
    arena.support_view(&arena.monsters[1]).unwrap()
}

#[test]
fn support_variant_applies_to_new_casts_after_real_completion() {
    let mut c = config();
    c.mechanism_version = 13;
    let p = c.pve_mut();
    p.encounters.push(EncounterConfig {
        id: "optional-node".into(),
        name: "Optional node".into(),
        requires: vec![],
        trigger: EncounterTrigger {
            center: [400.0, 350.0],
            radius: 100.0,
        },
        monsters: vec![MonsterSpawn {
            id: "optional-guard".into(),
            template_id: p.template_id.clone(),
            position: [400.0, 320.0],
        }],
        required_monster_ids: vec!["optional-guard".into()],
        waves: vec![],
        reward_chest_ids: vec![],
        reset: None,
    });
    let skill = p
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap();
    skill.on_encounter_completed = Some(crate::config::CompletedEncounterSupport {
        encounter_id: "optional-node".into(),
        id: "changed-heal".into(),
        name: "Changed heal".into(),
        heal_bps: 400,
    });
    let ordinary_id = skill.id.clone();

    for completion_time in [200, 450] {
        let mut w = world(&c);
        assert_eq!(view(&w).skill_id, ordinary_id);
        start(&mut w, &c, 100);
        if completion_time > 400 {
            advance(&mut w, &c, 400);
        }
        let mut arena = w.pve.take().unwrap();
        arena.now = completion_time;
        let mut events = vec![];
        arena.hurt_monster_contact(&mut w, &c, 3, (500, false, None), &mut events);
        arena.advance_encounters(&w, &mut events);
        assert!(arena.encounter_completed("optional-node"));
        w.pve = Some(arena);

        let snapshot = w.pve_snapshot(&c, completion_time);
        let caster = &snapshot.pve.as_ref().unwrap().monsters[1];
        assert_eq!(caster.cast.as_deref(), Some(ordinary_id.as_str()));
        assert_eq!(caster.support.as_ref().unwrap().skill_id, ordinary_id);
        if completion_time < 400 {
            advance(&mut w, &c, 400);
        }
        advance(&mut w, &c, 500);
        advance(&mut w, &c, 600);
        let arena = w.pve.as_ref().unwrap();
        assert_eq!(arena.monsters[0].hp, 150);
        assert_eq!(arena.monsters[1].spirit, 80);
        let selected = view(&w);
        assert_eq!(selected.skill_id, "changed-heal");
        assert_eq!(selected.name, "Changed heal");
        assert_eq!(selected.remaining_casts, 3);
        assert_eq!(selected.ready_at_ms, 2400);
        assert!(start(&mut w, &c, 2399).is_empty());
        assert!(!start(&mut w, &c, 2400).is_empty());
        assert!(matches!(
            advance(&mut w, &c, 2700).as_slice(),
            [Event::SupportPulse {
                cast_id: 2,
                amount: 4,
                ..
            }]
        ));
        assert_eq!(view(&w).remaining_casts, 2);
        assert_eq!(w.pve.as_ref().unwrap().monsters[1].spirit, 60);
    }
}

#[test]
fn support_selects_lowest_ratio_then_stable_id() {
    let c = config();
    let mut w = world(&c);
    {
        let a = w.pve.as_mut().unwrap();
        a.monsters[2].hp = 40;
        Arc::make_mut(&mut a.monsters[2].profile)
            .stats
            .insert("hp".into(), 100);
    }
    start(&mut w, &c, 100);
    assert_eq!(view(&w).cast.unwrap().target_id, "target-a");
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[2].hp = 120;
    start(&mut w, &c, 100);
    assert_eq!(view(&w).cast.unwrap().target_id, "target-a");
}
#[test]
fn support_cannot_cross_encounter_or_heal_dead_disabled_returning() {
    for mode in 0..4 {
        let c = config();
        let mut w = world(&c);
        let a = w.pve.as_mut().unwrap();
        match mode {
            0 => a.monsters[0].encounter_id = Some("other".into()),
            1 => a.monsters[0].hp = 0,
            2 => a.monsters[0].enabled = false,
            _ => a.monsters[0].returning = true,
        };
        assert!(start(&mut w, &c, 100).is_empty());
    }
}
#[test]
fn support_self_selection_is_explicit_and_static_has_no_spatial_group() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap()
        .include_self = true;
    let mut w = world(&c);
    let a = w.pve.as_mut().unwrap();
    a.monsters[1].hp = 30;
    for m in &mut a.monsters {
        m.encounter_id = None;
    }
    start(&mut w, &c, 100);
    assert_eq!(view(&w).cast.unwrap().target_id, "support-a");
    let c = config();
    let mut w = world(&c);
    for m in &mut w.pve.as_mut().unwrap().monsters {
        m.encounter_id = None;
    }
    assert!(start(&mut w, &c, 100).is_empty());
}
#[test]
fn support_guardians_require_same_chest_and_generation() {
    for same in [false, true] {
        let c = config();
        let mut w = world(&c);
        let a = w.pve.as_mut().unwrap();
        for m in &mut a.monsters {
            m.encounter_id = None;
        }
        a.monsters[1].binding = Some(GuardianBinding {
            chest_id: "box".into(),
            generation: 1,
        });
        a.monsters[0].binding = Some(GuardianBinding {
            chest_id: "box".into(),
            generation: if same { 1 } else { 2 },
        });
        assert_eq!(!start(&mut w, &c, 100).is_empty(), same);
    }
}
#[test]
fn support_range_includes_boundary_and_excludes_outside() {
    for (x, allowed) in [(800.0, true), (800.01, false)] {
        let c = config();
        let mut w = world(&c);
        w.pve.as_mut().unwrap().monsters[0].position = [x, 500.0];
        assert_eq!(!start(&mut w, &c, 100).is_empty(), allowed);
    }
}
#[test]
fn support_vision_is_independent_of_walkability() {
    let mut c = config();
    let p = c.pve_mut();
    let mut tile = p.palette[0].clone();
    tile.blocks_vision = true;
    tile.walkable = true;
    let index = p.palette.len();
    p.palette.push(tile);
    let mut cells = vec![0; 4096];
    cells[15 * 64 + 17] = index;
    p.cells_rle = cells.into_iter().map(|v| (v, 1)).collect();
    let mut w = world(&c);
    assert!(start(&mut w, &c, 100).is_empty());
}
#[test]
fn support_gates_do_not_draw_random_when_no_eligible_start() {
    for mode in 0..11 {
        let mut c = config();
        if mode == 0 {
            c.pve_mut()
                .additional_templates
                .get_mut("verification-forest-support")
                .unwrap()
                .support
                .as_mut()
                .unwrap()
                .cast_chance = 0.0;
        }
        let mut w = world(&c);
        let a = w.pve.as_mut().unwrap();
        match mode {
            1 => a.monsters[1].spirit = 19,
            2 => a.monsters[1].support_used = 4,
            3 => a.monsters[1].support_ready = 101,
            4 => a.monsters[1].recovery = 101,
            5 => a.monsters[1].aggro = false,
            6 => a.monsters[1].returning = true,
            7 => a.monsters[1].hp = 0,
            8 => w.protected = true,
            9 => w.ready = false,
            10 => a.monsters[0].hp = 300,
            _ => {}
        }
        let rng = w.rng;
        assert!(start(&mut w, &c, 100).is_empty());
        assert_eq!(w.rng, rng);
    }
}
#[test]
fn support_preparation_reserves_without_cost_or_count_and_commits_once() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    assert!(advance(&mut w, &c, 399).is_empty());
    assert_eq!(view(&w).remaining_casts, 4);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].spirit, 100);
    let e = advance(&mut w, &c, 400);
    assert!(matches!(
        &e[..],
        [Event::SupportPulse {
            tick: 1,
            amount: 10,
            ..
        }]
    ));
    assert_eq!(view(&w).remaining_casts, 3);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].spirit, 80);
    assert_eq!(view(&w).ready_at_ms, 2400);
    advance(&mut w, &c, 500);
    let e = advance(&mut w, &c, 600);
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::SupportCompleted { .. }))
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 150);
    assert_eq!(view(&w).remaining_casts, 3);
    assert!(advance(&mut w, &c, 900).is_empty());
}
#[test]
fn support_budget_remainder_clamp_and_zero_heal() {
    for (bps, hp, amounts) in [
        (1034, 120, vec![11, 10, 10]),
        (1000, 298, vec![2, 0, 0]),
        (0, 120, vec![0, 0, 0]),
    ] {
        let mut c = config();
        c.pve_mut()
            .additional_templates
            .get_mut("verification-forest-support")
            .unwrap()
            .support
            .as_mut()
            .unwrap()
            .heal_bps = bps;
        let mut w = world(&c);
        start(&mut w, &c, 100);
        w.pve.as_mut().unwrap().monsters[0].hp = hp;
        let got: Vec<_> = [400, 500, 600]
            .into_iter()
            .flat_map(|now| advance(&mut w, &c, now))
            .filter_map(|e| {
                if let Event::SupportPulse { amount, .. } = e {
                    Some(amount)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(got, amounts);
    }
}
#[test]
fn support_fixed_target_does_not_follow_lower_health_ally() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    w.pve.as_mut().unwrap().monsters[2].hp = 1;
    advance(&mut w, &c, 400);
    assert_eq!(w.pve.as_ref().unwrap().monsters[2].hp, 1);
    assert_eq!(view(&w).cast.unwrap().target_id, "target-b");
}
#[test]
fn support_missing_mp_at_commit_cancels_without_partial_cost() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    w.pve.as_mut().unwrap().monsters[1].spirit = 19;
    let e = advance(&mut w, &c, 400);
    assert!(matches!(
        &e[..],
        [Event::SupportCancelled {
            reason: SupportCancelReason::Resource,
            ..
        }]
    ));
    assert_eq!(view(&w).remaining_casts, 4);
    assert_eq!(view(&w).ready_at_ms, 0);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 120);
}
#[test]
fn support_old_or_unattributed_contact_damages_without_interrupting() {
    for accepted in [None, Some(99)] {
        let c = config();
        let mut w = world(&c);
        start(&mut w, &c, 100);
        let e = hit(&mut w, &c, 20, accepted, 200);
        assert!(
            !e.iter()
                .any(|e| matches!(e, Event::SupportInterrupted { .. }))
        );
        assert!(view(&w).cast.is_some());
        assert_eq!(w.pve.as_ref().unwrap().monsters[1].hp, 280);
    }
}
#[test]
fn support_zero_damage_hit_interrupts_but_dodge_does_not() {
    for dodge in [0, 100] {
        let mut c = config();
        c.pve_mut()
            .additional_templates
            .get_mut("verification-forest-support")
            .unwrap()
            .stats
            .insert("dodge".into(), dodge);
        let mut w = world(&c);
        start(&mut w, &c, 100);
        let e = hit(&mut w, &c, 0, Some(100), 200);
        assert_eq!(
            e.iter()
                .filter(|e| matches!(e, Event::SupportInterrupted { .. }))
                .count(),
            usize::from(dodge == 0)
        );
        assert_eq!(view(&w).cast.is_none(), dodge == 0);
    }
}
#[test]
fn support_window_boundary_and_shared_cooldown() {
    let c = config();
    for now in [600, 601] {
        let mut w = world(&c);
        start(&mut w, &c, 100);
        hit(&mut w, &c, 0, Some(100), now);
        assert_eq!(view(&w).cast.is_none(), now == 600);
    }
    let mut w = world(&c);
    start(&mut w, &c, 100);
    hit(&mut w, &c, 0, Some(100), 200);
    assert_eq!(view(&w).interrupt_ready_at_ms, 1200);
    assert!(!start(&mut w, &c, 300).is_empty());
    hit(&mut w, &c, 0, Some(300), 400);
    assert!(view(&w).cast.is_some());
}
#[test]
fn support_precommit_interrupt_is_unpaid_and_partial_channel_retains_cost() {
    for partial in [false, true] {
        let c = config();
        let mut w = world(&c);
        start(&mut w, &c, 100);
        if partial {
            advance(&mut w, &c, 400);
        }
        let e = hit(&mut w, &c, 1, Some(100), if partial { 500 } else { 200 });
        assert!(
            matches!(e.last(),Some(Event::SupportInterrupted{cast}) if cast.committed==partial)
        );
        assert_eq!(view(&w).remaining_casts, if partial { 3 } else { 4 });
        assert_eq!(
            w.pve.as_ref().unwrap().monsters[1].spirit,
            if partial { 80 } else { 100 }
        );
        assert!(advance(&mut w, &c, 1000).is_empty());
        assert_eq!(
            w.pve.as_ref().unwrap().monsters[0].hp,
            if partial { 130 } else { 120 }
        );
    }
}
#[test]
fn support_single_contact_cannot_interrupt_ended_cast_twice() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    let mut e = hit(&mut w, &c, 0, Some(100), 200);
    e.extend(hit(&mut w, &c, 0, Some(100), 200));
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::SupportInterrupted { .. }))
            .count(),
        1
    );
}
#[test]
fn support_target_death_return_and_identity_change_stop_future_heals() {
    for mode in 0..5 {
        let c = config();
        let mut w = world(&c);
        start(&mut w, &c, 100);
        advance(&mut w, &c, 400);
        let a = w.pve.as_mut().unwrap();
        match mode {
            0 => a.monsters[0].hp = 0,
            1 => a.monsters[0].returning = true,
            2 => a.monsters[0].id = "new-id".into(),
            3 => a.monsters[0].boss_epoch += 1,
            _ => a.monsters[0].position = [1800.0, 1800.0],
        };
        let e = advance(&mut w, &c, 500);
        assert!(matches!(
            &e[..],
            [Event::SupportCancelled {
                reason: SupportCancelReason::TargetInvalid,
                ..
            }]
        ));
        assert_eq!(view(&w).remaining_casts, 3);
        assert!(view(&w).cast.is_none());
    }
}
#[test]
fn support_owner_death_cancels_without_counting_an_interrupt() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    let e = hit(&mut w, &c, 10000, Some(100), 200);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::SupportCancelled {
            reason: SupportCancelReason::MemberDied,
            ..
        }
    )));
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::SupportInterrupted { .. }))
    );
    assert_eq!(view(&w).remaining_casts, 4);
    assert!(advance(&mut w, &c, 400).is_empty());
}
#[test]
fn support_background_and_close_cancel_preserving_committed_budget() {
    for partial in [false, true] {
        let c = config();
        let mut w = world(&c);
        start(&mut w, &c, 100);
        if partial {
            advance(&mut w, &c, 400);
        }
        let e = w.action(&Action::Background {}, "bg", &c, 500).unwrap();
        assert!(e.iter().any(|e| matches!(
            e,
            Event::SupportCancelled {
                reason: SupportCancelReason::Background,
                ..
            }
        )));
        assert_eq!(view(&w).remaining_casts, if partial { 3 } else { 4 });
        assert!(advance(&mut w, &c, 1000).is_empty());
    }
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    assert_eq!(w.close_interaction_event_budget(), 1);
    assert!(matches!(
        &w.close_container_interaction(200)[..],
        [Event::SupportCancelled {
            reason: SupportCancelReason::Closed,
            ..
        }]
    ));
}
#[test]
fn support_max_casts_is_finite_and_true_return_does_not_refund() {
    let c = config();
    let mut w = world(&c);
    for n in 0..4 {
        let at = 100 + n * 3000;
        assert!(!start(&mut w, &c, at).is_empty());
        for t in [at + 300, at + 400, at + 500] {
            advance(&mut w, &c, t);
        }
        w.pve.as_mut().unwrap().monsters[0].hp = 120;
    }
    assert_eq!(view(&w).remaining_casts, 0);
    assert!(start(&mut w, &c, 15000).is_empty());
    w.pve.as_mut().unwrap().monsters[1].returning = true;
    advance(&mut w, &c, 16000);
    w.pve.as_mut().unwrap().monsters[1].returning = false;
    assert_eq!(view(&w).remaining_casts, 0);
}
#[test]
fn support_large_time_jump_emits_at_most_one_tick_per_step() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    let e = advance(&mut w, &c, 10000);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::SupportPulse { .. }))
            .count(),
        1
    );
    assert_eq!(view(&w).cast.unwrap().completed_ticks, 1);
}
#[test]
fn support_stage_change_keeps_locked_budget_and_target() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    w.pve.as_mut().unwrap().monsters[0].boss_phase = 1;
    advance(&mut w, &c, 400);
    assert_eq!(view(&w).cast.unwrap().target_id, "target-b");
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 130);
}
#[test]
fn support_direct_projectile_wins_same_step_as_commit() {
    for accepted in [50, 100] {
        let c = config();
        let mut w = world(&c);
        start(&mut w, &c, 100);
        let mut a = w.pve.take().unwrap();
        a.shots.push(Shot {
            strict_target: None,
            status_source: None,
            hit_statuses: vec![],
            rounds: vec![],
            manual_started: Some(accepted),
            position: [450.0, 500.0],
            direction: [1.0, 0.0],
            remaining: 300.0,
            speed: 2000.0,
            radius: 1.0,
            raw: 0,
            owner: None,
            metal: false,
            mark: false,
            expires: 1000,
            created: 300,
        });
        let mut e = vec![];
        a.step(&mut w, &c, 400, &mut e);
        assert_eq!(
            e.iter()
                .filter(|e| matches!(e, Event::SupportInterrupted { .. }))
                .count(),
            usize::from(accepted == 100)
        );
        assert_eq!(a.monsters[0].hp, if accepted == 100 { 120 } else { 130 });
        assert_eq!(a.monsters[1].spirit, if accepted == 100 { 100 } else { 80 });
    }
}
#[test]
fn support_multi_cancel_is_one_bounded_event() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap()
        .include_self = true;
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    let profile = a.monsters[1].profile.clone();
    a.monsters = (0..32)
        .map(|i| {
            let mut m = Monster::new(
                format!("support-{i}"),
                "verification-forest-support".into(),
                profile.clone(),
                [
                    200.0 + (i % 8) as f64 * 100.0,
                    200.0 + (i / 8) as f64 * 100.0,
                ],
                None,
            );
            m.hp = 100;
            m.aggro = true;
            m
        })
        .collect();
    for i in 0..32 {
        assert!(a.try_start_support(&mut w, &c, i, 100, &mut vec![]));
    }
    let mut e = vec![];
    a.cancel_all_supports(SupportCancelReason::Background, 200, &mut e);
    assert!(matches!(&e[..],[Event::SupportCancelled{casts,..}] if casts.len()==32));
    assert!(!a.has_support_cast());
}
#[test]
fn support_new_instance_has_fresh_budget_and_old_target_is_not_reused() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    advance(&mut w, &c, 400);
    let mut a = w.pve.take().unwrap();
    let profile = a.monsters[1].profile.clone();
    a.cancel_supports_for(1, SupportCancelReason::Replaced, 450, &mut vec![]);
    a.monsters[1] = Monster::new(
        "support-new".into(),
        "verification-forest-support".into(),
        profile,
        [500.0, 500.0],
        None,
    );
    assert_eq!(a.monsters[1].support_used, 0);
    assert_eq!(a.monsters[1].support_ready, 0);
    assert_eq!(a.monsters[1].support_sequence, 0);
    assert!(!a.has_support_cast());
}
#[test]
fn support_free_zero_heal_still_commits_once_and_has_no_rewards() {
    let mut c = config();
    let s = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap();
    s.cost = 0;
    s.heal_bps = 0;
    let mut w = world(&c);
    start(&mut w, &c, 100);
    for at in [400, 500, 600] {
        advance(&mut w, &c, at);
    }
    assert_eq!(view(&w).remaining_casts, 3);
    assert_eq!(w.pve.as_ref().unwrap().monsters[1].spirit, 100);
    assert_eq!(w.kills, 0);
    assert!(w.resources.is_empty());
    assert!(w.pve.as_ref().unwrap().score_tally.is_empty());
}
#[test]
fn support_cooldown_is_shared_between_different_casters() {
    let c = config();
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    let mut second = Monster::new(
        "support-second".into(),
        "verification-forest-support".into(),
        a.monsters[1].profile.clone(),
        [550.0, 600.0],
        None,
    );
    second.encounter_id = Some("team".into());
    second.aggro = true;
    a.monsters.push(second);
    assert!(a.try_start_support(&mut w, &c, 1, 100, &mut vec![]));
    assert!(a.try_start_support(&mut w, &c, 3, 100, &mut vec![]));
    let mut e = vec![];
    a.interrupt_support_on_contact(1, Some(100), 200, &mut e);
    a.interrupt_support_on_contact(3, Some(100), 200, &mut e);
    assert_eq!(e.len(), 1);
    assert!(a.monsters[3].support_cast.is_some());
}
#[test]
fn support_threshold_is_strict_and_same_encounter_can_span_waves() {
    let c = config();
    for (hp, allowed) in [(255, false), (254, true)] {
        let mut w = world(&c);
        w.pve.as_mut().unwrap().monsters[0].hp = hp;
        w.pve.as_mut().unwrap().monsters[0].wave_id = Some("earlier".into());
        w.pve.as_mut().unwrap().monsters[1].wave_id = Some("later".into());
        assert_eq!(!start(&mut w, &c, 100).is_empty(), allowed);
    }
}
#[test]
fn support_actual_return_cancels_and_keeps_cost_and_instance_budget() {
    let c = config();
    let mut w = world(&c);
    start(&mut w, &c, 100);
    advance(&mut w, &c, 400);
    w.position = [1800.0, 1800.0];
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.step(&mut w, &c, 500, &mut e);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::SupportCancelled {
            reason: SupportCancelReason::Returning,
            ..
        }
    )));
    assert_eq!(a.monsters[1].support_used, 1);
    assert_eq!(a.monsters[1].support_ready, 2400);
    assert_eq!(a.monsters[1].spirit, 80);
    assert!(!a.has_support_cast());
}
#[test]
fn support_navigation_avoids_intermediate_actors_without_changing_legacy_routes() {
    for version in [9, 10] {
        let mut c = config();
        c.mechanism_version = version;
        let mut w = world(&c);
        let goal = [750.0, 500.0];
        w.action(&Action::Navigate { destination: goal }, "navigate", &c, 30)
            .unwrap();
        for at in (60..=9030).step_by(30) {
            w.tick(&c, at);
        }
        assert_eq!(
            distance(w.position, goal) < 1.0,
            version == 10,
            "version {version}: {:?}",
            w.position
        );
    }
}
#[test]
fn support_many_invalid_targets_cancel_in_one_event_without_refund() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-forest-support")
        .unwrap()
        .support
        .as_mut()
        .unwrap()
        .include_self = true;
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    let profile = a.monsters[1].profile.clone();
    a.monsters = (0..32)
        .map(|i| {
            let mut m = Monster::new(
                format!("support-{i}"),
                "verification-forest-support".into(),
                profile.clone(),
                [
                    200.0 + (i % 8) as f64 * 100.0,
                    200.0 + (i / 8) as f64 * 100.0,
                ],
                None,
            );
            m.hp = 100;
            m.aggro = true;
            m
        })
        .collect();
    for i in 0..32 {
        assert!(a.try_start_support(&mut w, &c, i, 100, &mut vec![]));
    }
    for m in &mut a.monsters {
        m.hp = 0;
    }
    let mut e = vec![];
    a.advance_supports(&w, &c, 400, &mut e);
    assert!(
        matches!(&e[..],[Event::SupportCancelled{casts,reason:SupportCancelReason::TargetInvalid}]if casts.len()==32)
    );
    assert!(
        a.monsters
            .iter()
            .all(|m| m.support_cast.is_none() && m.support_used == 0 && m.spirit == 100)
    );
}
#[test]
fn support_narrow_corridor_keeps_approach_when_no_actor_detour_exists() {
    let mut c = config();
    c.spawn = [400.0, 512.0];
    let p = c.pve_mut();
    let mut blocked = p.palette[0].clone();
    blocked.walkable = false;
    let wall = p.palette.len();
    p.palette.push(blocked);
    let mut cells = vec![wall; 4096];
    for y in [15, 16] {
        for x in 0..64 {
            cells[y * 64 + x] = 0;
        }
    }
    p.cells_rle = cells.into_iter().map(|v| (v, 1)).collect();
    for spawn in &mut p.encounters[0].monsters {
        spawn.position[1] = 512.0;
    }
    let mut w = world(&c);
    {
        let a = w.pve.as_mut().unwrap();
        for m in &mut a.monsters[1..] {
            m.hp = 0;
            m.enabled = false;
        }
        a.monsters[0].position = [600.0, 512.0];
        a.monsters[0].home = [600.0, 512.0];
    }
    w.action(
        &Action::Navigate {
            destination: [900.0, 512.0],
        },
        "approach",
        &c,
        30,
    )
    .unwrap();
    for at in (60..=3030).step_by(30) {
        w.tick(&c, at);
    }
    assert!(w.position[0] > 400.0 && w.position[0] < 600.0);
    assert!(distance(w.position, [600.0, 512.0]) >= c.params.player_radius + 24.0);
}
