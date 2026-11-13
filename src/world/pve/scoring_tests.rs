use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-8")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = [900.0, 800.0];
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.monster_positions.clear();
    p.encounters.clear();
    p.extra_spawns = [
        ("ordinary", "yao-wolf"),
        ("elite", "verification-score-elite"),
        ("boss", "verification-phase-boss"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (id, t))| crate::config::MonsterSpawn {
        id: (*id).into(),
        template_id: (*t).into(),
        position: [500.0 + i as f64 * 100.0, 400.0],
    })
    .collect();
    p.chest_spawns[0].position = [800.0, 400.0];
    p.chest_spawns[1].position = [800.0, 650.0];
    for s in std::iter::once(&mut p.stats)
        .chain(p.additional_templates.values_mut().map(|t| &mut t.stats))
    {
        s.insert("hp".into(), 300);
        s.insert("spirit".into(), 100);
        for k in ["atk", "def", "spd", "crit", "dodge"] {
            s.insert(k.into(), 0);
        }
    }
    c
}
fn world(c: &Config) -> World {
    let mut w = World::new(
        c,
        ParticipantFacts {
            principal: "principal-A".into(),
            revision: 1,
            hp_points: 1000,
            spirit_points: 100,
            atk_points: 100,
            def_points: 10,
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
    w
}
fn hit(w: &mut World, c: &Config, i: usize, raw: u32) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hurt_monster(w, c, i, raw, false, &mut e);
    w.pve = Some(a);
    e
}
fn score(w: &World, c: &Config) -> ScoreSummary {
    c.score_summary(&w.pve.as_ref().unwrap().score_tally)
}
fn kill_all(w: &mut World, c: &Config) {
    for i in 0..3 {
        hit(w, c, i, 10000);
    }
}
#[test]
fn empty_score_is_zero_and_only_new_views_expose_authoritative_predictions() {
    let c = config();
    let w = world(&c);
    let s = w.snapshot(&c, 0).pve.unwrap().scoring.unwrap();
    assert_eq!((s.kills, s.credited_kills, s.kill_score), (0, 0, 0));
    assert_eq!((s.extraction_cultivation, s.death_cultivation), (22, 0));
    assert!(score(&w, &c).templates.is_empty());
}
#[test]
fn ordinary_elite_and_boss_scores_accumulate_on_each_actual_death() {
    let c = config();
    let mut w = world(&c);
    for (i, total) in [10, 40, 140].into_iter().enumerate() {
        let e = hit(&mut w, &c, i, 10000);
        assert_eq!(score(&w, &c).kill_score, total);
        assert_eq!(
            e.iter()
                .filter(|e| matches!(e, Event::KillScored { .. }))
                .count(),
            1
        );
    }
    assert_eq!((score(&w, &c).kills, w.kills), (3, 3));
}
#[test]
fn omitted_override_inherits_root_and_explicit_zero_does_not() {
    let mut c = config();
    c.kill_score = 7;
    c.pve_mut()
        .additional_templates
        .get_mut("verification-score-elite")
        .unwrap()
        .kill_score = None;
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .kill_score = Some(0);
    let mut w = world(&c);
    kill_all(&mut w, &c);
    assert_eq!(score(&w, &c).kill_score, 17);
}
#[test]
fn zero_score_credited_target_keeps_real_and_credited_kills() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .kill_score = Some(0);
    let mut w = world(&c);
    kill_all(&mut w, &c);
    let s = score(&w, &c);
    assert_eq!((s.kills, s.credited_kills, s.kill_score), (3, 3, 40));
}
#[test]
fn noncredit_death_is_real_but_has_zero_score_and_does_not_erase_loot() {
    let mut c = config();
    let p = c.pve_mut();
    let t = p
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    t.kill_credit = false;
    t.kill_score = Some(0);
    t.drop_table.guaranteed = vec![crate::config::LootRule {
        item_id: "xuan-tie".into(),
        chance: None,
        quantity_min: 2,
        quantity_max: 2,
    }];
    let mut w = world(&c);
    kill_all(&mut w, &c);
    assert_eq!(
        (score(&w, &c).kills, w.kills, score(&w, &c).kill_score),
        (3, 2, 40)
    );
    assert!(w.resources.iter().any(|r| r.view.quantity == 2));
}
#[test]
fn conversion_floors_once_after_sum_and_adds_extraction_bonus_once() {
    let mut c = config();
    c.extraction_score = 0;
    c.cultivation_coefficient = 0.6;
    c.pve_mut().kill_score = Some(1);
    for t in c.pve_mut().additional_templates.values_mut() {
        t.kill_score = Some(1);
    }
    let mut w = world(&c);
    kill_all(&mut w, &c);
    let s = w.snapshot(&c, 0).pve.unwrap().scoring.unwrap();
    assert_eq!(
        (s.kill_score, s.extraction_cultivation, s.death_cultivation),
        (3, 1, 0)
    );
}
#[test]
fn nonlethal_damage_miss_zero_and_boss_phase_change_never_award_score() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 2, 0);
    hit(&mut w, &c, 2, 200);
    assert_eq!(
        w.pve.as_ref().unwrap().monsters[2]
            .boss_view()
            .unwrap()
            .phase_index,
        2
    );
    assert_eq!(score(&w, &c).kill_score, 0);
    assert_eq!(score(&w, &c).kills, 0);
}
#[test]
fn natural_healing_does_not_award_or_remove_score() {
    let mut c = config();
    c.params.regen.near_ratio = 0.5;
    c.params.regen.far_ratio = 0.5;
    c.pve_mut();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 10000);
    hit(&mut w, &c, 2, 200);
    let before = score(&w, &c);
    w.tick(&c, 1000);
    assert_eq!(score(&w, &c), before);
}
#[test]
fn true_home_boss_reset_does_not_award_or_remove_score() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 10000);
    hit(&mut w, &c, 2, 200);
    let before = score(&w, &c);
    w.position = [100.0, 800.0];
    w.pve.as_mut().unwrap().monsters[2].returning = true;
    let e = w.tick(&c, 30);
    assert!(e.iter().any(|e| matches!(e, Event::BossReset { .. })));
    assert_eq!(score(&w, &c), before);
}
#[test]
fn corpse_disabled_and_repeated_hits_cannot_credit_again() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 2, 10000);
    assert!(hit(&mut w, &c, 2, 10000).is_empty());
    w.pve.as_mut().unwrap().monsters[1].enabled = false;
    assert!(hit(&mut w, &c, 1, 10000).is_empty());
    assert_eq!((score(&w, &c).kills, score(&w, &c).kill_score), (1, 100));
}
#[test]
fn lethal_hit_awards_boss_once_and_skips_new_phase() {
    let c = config();
    let mut w = world(&c);
    let e = hit(&mut w, &c, 2, 10000);
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::BossPhaseChanged { .. }))
    );
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::KillScored { score: 100, .. }))
            .count(),
        1
    );
}
#[test]
fn score_recording_does_not_consume_rng_or_change_drop_sequence() {
    let c = config();
    let mut old = c.clone();
    old.mechanism_version = 6;
    let mut a = world(&c);
    let mut b = world(&old);
    kill_all(&mut a, &c);
    kill_all(&mut b, &old);
    assert_eq!(a.rng, b.rng);
    assert_eq!(
        a.resources.iter().map(|r| &r.view).collect::<Vec<_>>(),
        b.resources.iter().map(|r| &r.view).collect::<Vec<_>>()
    );
}
#[test]
fn shared_templates_have_independent_world_ledgers() {
    let c = config();
    let mut a = world(&c);
    let mut b = world(&c);
    hit(&mut a, &c, 0, 10000);
    hit(&mut b, &c, 2, 10000);
    assert_eq!(score(&a, &c).kill_score, 10);
    assert_eq!(score(&b, &c).kill_score, 100);
}
#[test]
fn kill_order_does_not_change_sorted_summary_or_conversion() {
    let c = config();
    let mut a = world(&c);
    let mut b = world(&c);
    kill_all(&mut a, &c);
    for i in [2, 0, 1] {
        hit(&mut b, &c, i, 10000);
    }
    assert_eq!(score(&a, &c), score(&b, &c));
}
#[test]
fn display_names_and_boss_capability_do_not_choose_points() {
    let mut c = config();
    let t = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    t.name = "普通怪的名字".into();
    t.kill_score = Some(9);
    let mut w = world(&c);
    hit(&mut w, &c, 2, 10000);
    assert_eq!(score(&w, &c).kill_score, 9);
}
#[test]
fn background_foreground_preserve_current_ledger() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 1, 10000);
    let before = score(&w, &c);
    w.action(&Action::Background {}, "background", &c, 30)
        .unwrap();
    w.tick(&c, 60);
    w.action(&Action::Foreground {}, "foreground", &c, 90)
        .unwrap();
    assert_eq!(score(&w, &c), before);
}
#[test]
fn terminal_world_does_not_add_more_score() {
    let c = config();
    for phase in [Phase::Draining, Phase::Closed] {
        let mut w = world(&c);
        hit(&mut w, &c, 0, 10000);
        let before = score(&w, &c);
        w.phase = phase;
        assert!(w.tick(&c, 3000).is_empty());
        assert_eq!(score(&w, &c), before);
    }
}
#[test]
fn frozen_extraction_and_death_use_score_summary_and_conserve_assets() {
    let c = config();
    for (outcome, growth) in [(Outcome::Extraction, 127), (Outcome::Death, 52)] {
        let mut w = world(&c);
        kill_all(&mut w, &c);
        w.desired_outcome = Some(outcome);
        let Business::CommitSettlement {
            scoring: Some(s),
            cultivation,
            carried,
            looted,
            kept,
            dropped,
            ..
        } = w.settlement(&c)
        else {
            panic!()
        };
        assert_eq!(cultivation, growth);
        assert_eq!(s.kill_score, 140);
        assert_eq!(carried + looted, kept + dropped);
        assert!(c.valid_scoring(&s, 3000));
    }
}
#[test]
fn old_mechanism_keeps_original_views_events_body_and_cultivation() {
    let mut c = config();
    c.mechanism_version = 6;
    let mut w = world(&c);
    let mut events = vec![];
    for i in 0..3 {
        events.extend(hit(&mut w, &c, i, 10000));
    }
    assert!(!events.iter().any(|e| matches!(e, Event::KillScored { .. })));
    let p = w.snapshot(&c, 0).pve.unwrap();
    assert!(p.scoring.is_none() && p.monsters.iter().all(|m| m.kill_score.is_none()));
    w.desired_outcome = Some(Outcome::Extraction);
    assert!(matches!(
        w.settlement(&c),
        Business::CommitSettlement {
            scoring: None,
            cultivation: 45,
            ..
        }
    ));
}
#[test]
fn zero_coefficient_preserves_actual_tally_and_item_bounds() {
    let mut c = config();
    c.cultivation_coefficient = 0.0;
    let mut w = world(&c);
    kill_all(&mut w, &c);
    let s = score(&w, &c);
    assert_eq!(s.kill_score, 140);
    assert!(c.valid_scoring(&s, 0));
    assert_eq!(c.cultivation_from_score(140, Outcome::Extraction), 0);
}
#[test]
fn real_two_wave_completion_adds_each_weight_and_does_not_open_boxes() {
    let c = crate::config::default_catalog()
        .active("b3-8")
        .unwrap()
        .as_ref()
        .clone();
    let mut w = world(&c);
    w.position = c.pve.as_ref().unwrap().encounters[0].trigger.center;
    w.position[0] -= 80.0;
    w.tick(&c, 30);
    for i in 0..2 {
        hit(&mut w, &c, i, 10000);
    }
    w.tick(&c, 60);
    w.tick(&c, 1590);
    hit(&mut w, &c, 2, 10000);
    w.tick(&c, 1620);
    assert_eq!(score(&w, &c).kill_score, 140);
    let p = w.snapshot(&c, 1620).pve.unwrap();
    assert_eq!(p.encounters[0].phase, EncounterPhase::Completed);
    assert!(p.chests.iter().all(|c| !c.opened));
}
#[test]
fn guardian_new_generation_can_score_but_old_identity_cannot_repeat() {
    let mut c = config();
    let g = crate::config::default_catalog()
        .active("b3-4")
        .unwrap()
        .pve
        .as_ref()
        .unwrap()
        .guardian()
        .unwrap()
        .clone();
    let p = c.pve_mut();
    p.extra_spawns.clear();
    p.chest_spawns.truncate(1);
    let r = p.treasure_rules.as_mut().unwrap();
    r.template.respawn_delay_ms = 1000;
    let mut g = g;
    g.template_id = "verification-phase-boss".into();
    g.chance = 1.0;
    g.spawn_offsets = vec![[64.0, 0.0], [-64.0, 0.0]];
    r.template.guardian = Some(g);
    let mut w = world(&c);
    w.tick(&c, 30);
    let old = w.pve.as_ref().unwrap().monsters[0].id.clone();
    hit(&mut w, &c, 0, 10000);
    assert!(hit(&mut w, &c, 0, 10000).is_empty());
    w.position = [800.0, 400.0];
    let open = |g| Action::OpenChest {
        chest_id: "scoring-cache-1".into(),
        expected_generation: g,
    };
    w.action(&open(1), "unseal", &c, 30).unwrap();
    w.tick(&c, 2430);
    w.action(&open(1), "open", &c, 2430).unwrap();
    w.tick(&c, 3430);
    assert_ne!(w.pve.as_ref().unwrap().monsters[0].id, old);
    hit(&mut w, &c, 0, 10000);
    assert_eq!((score(&w, &c).kills, score(&w, &c).kill_score), (2, 200));
}
