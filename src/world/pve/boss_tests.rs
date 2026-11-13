use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-7")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = c.spawn;
    c.params.minimum_damage = 0;
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.monster_positions.clear();
    p.encounters.clear();
    p.extra_spawns = vec![crate::config::MonsterSpawn {
        id: "boss-one".into(),
        template_id: "verification-phase-boss".into(),
        position: [500.0, 400.0],
    }];
    p.chest_spawns[0].position = [800.0, 400.0];
    p.chest_spawns[1].position = [800.0, 650.0];
    let t = p
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    t.stats.insert("hp".into(), 300);
    t.stats.insert("spirit".into(), 100);
    for k in ["atk", "def", "spd", "crit", "dodge"] {
        t.stats.insert(k.into(), 0);
    }
    t.leash = 200.0;
    for phase in &mut t.boss.as_mut().unwrap().phases {
        phase.cast_chance = 0.0;
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
    w.foreground = true;
    w.joined = true;
    w.protected = false;
    w.active_at = Some(0);
    w
}
fn hit(w: &mut World, c: &Config, index: usize, damage: u32) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hurt_monster(w, c, index, damage, false, &mut e);
    w.pve = Some(a);
    e
}
fn boss(w: &World) -> BossPhaseView {
    w.pve.as_ref().unwrap().monsters[0].boss_view().unwrap()
}
fn lower(w: &mut World, c: &Config) {
    hit(w, c, 0, 200);
    assert_eq!(boss(w).phase_index, 2);
}
#[test]
fn new_instance_uses_first_configured_phase_and_fixed_template_attributes() {
    let c = config();
    let w = world(&c);
    let b = boss(&w);
    assert_eq!((b.phase_index, b.phase_count, b.epoch), (1, 2, 1));
    assert_eq!(b.attack_interval_ms, 2000);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].next_attack, 2000);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 300);
}
#[test]
fn threshold_before_equal_and_after_use_integer_inclusive_boundary() {
    let c = config();
    let mut w = world(&c);
    hit(&mut w, &c, 0, 149);
    assert_eq!(boss(&w).phase_index, 1);
    let e = hit(&mut w, &c, 0, 1);
    assert_eq!(boss(&w).phase_index, 2);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::BossPhaseChanged { .. }))
            .count(),
        1
    );
    assert!(
        !hit(&mut w, &c, 0, 1)
            .iter()
            .any(|e| matches!(e, Event::BossPhaseChanged { .. }))
    );
}
#[test]
fn one_hit_crossing_multiple_thresholds_emits_only_deepest_transition() {
    let mut c = config();
    let b = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .boss
        .as_mut()
        .unwrap();
    let mut third = b.phases[1].clone();
    third.id = "third".into();
    third.hp_threshold_bps = 2500;
    b.phases[1].hp_threshold_bps = 7500;
    b.phases.push(third);
    let mut w = world(&c);
    let e = hit(&mut w, &c, 0, 240);
    assert_eq!(boss(&w).phase_index, 3);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(
                e,
                Event::BossPhaseChanged {
                    from_index: 1,
                    phase_index: 3,
                    ..
                }
            ))
            .count(),
        1
    );
}
#[test]
fn phase_change_does_not_consume_extra_rng_or_create_resources_or_score() {
    let c = config();
    let mut normal = c.clone();
    normal
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .boss = None;
    let mut a = world(&c);
    let mut b = world(&normal);
    hit(&mut a, &c, 0, 200);
    hit(&mut b, &normal, 0, 200);
    assert_eq!(a.rng, b.rng);
    assert_eq!(a.kills, 0);
    assert!(a.resources.is_empty());
    assert_eq!(a.inventory, c.carried_quantity);
    assert_eq!(a.pve.as_ref().unwrap().monsters[0].spirit, 100);
}
#[test]
fn natural_healing_above_threshold_does_not_reverse_phase() {
    let mut c = config();
    c.params.regen.near_ratio = 0.8;
    c.params.regen.far_ratio = 0.8;
    c.pve_mut();
    let mut w = world(&c);
    lower(&mut w, &c);
    w.tick(&c, 1000);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 300);
    assert_eq!(boss(&w).phase_index, 2);
}
#[test]
fn channeled_heal_keeps_phase_after_healing_past_threshold() {
    let c = config();
    let mut w = world(&c);
    lower(&mut w, &c);
    w.pve.as_mut().unwrap().monsters[0].heal = Some((100, 0, 240));
    w.tick(&c, 100);
    w.tick(&c, 700);
    w.tick(&c, 1300);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 300);
    assert_eq!(boss(&w).phase_index, 2);
}
#[test]
fn lifesteal_heals_without_resetting_or_reentering_phase() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .combat
        .lifesteal_ratio = 1.0;
    let mut w = world(&c);
    lower(&mut w, &c);
    let mut a = w.pve.take().unwrap();
    a.hurt_player(&mut w, &c, 0, (200, false), 100, &mut vec![]);
    w.pve = Some(a);
    assert!(w.pve.as_ref().unwrap().monsters[0].hp > 150);
    assert_eq!(boss(&w).phase_index, 2);
}
#[test]
fn current_melee_deadline_is_preserved_and_next_attack_uses_new_interval() {
    let c = config();
    let mut w = world(&c);
    w.position = [455.0, 400.0];
    {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.aggro = true;
        m.next_attack = 2000;
        m.next_decision = 100000;
    }
    lower(&mut w, &c);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].next_attack, 2000);
    w.tick(&c, 1999);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].next_attack, 2000);
    w.tick(&c, 2000);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].next_attack, 2900);
}
#[test]
fn configured_zero_and_full_cast_chance_apply_at_next_ai_decision() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .boss
        .as_mut()
        .unwrap()
        .phases[1]
        .cast_chance = 1.0;
    let mut w = world(&c);
    w.tick(&c, 100);
    assert!(w.pve.as_ref().unwrap().monsters[0].casting.is_none());
    lower(&mut w, &c);
    w.tick(&c, 500);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].casting.unwrap().0, 0);
}
#[test]
fn in_progress_cast_and_skill_cooldown_cost_survive_transition() {
    let c = config();
    let mut w = world(&c);
    {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.aggro = true;
        m.casting = Some((0, 1000));
        m.cooldown = [0, 7000];
        m.recovery = 800;
        m.next_decision = 6000;
        m.next_attack = 9000;
    }
    lower(&mut w, &c);
    {
        let m = &w.pve.as_ref().unwrap().monsters[0];
        assert_eq!(m.casting, Some((0, 1000)));
        assert_eq!(m.cooldown, [0, 7000]);
        assert_eq!(
            (m.recovery, m.next_decision, m.next_attack),
            (800, 6000, 9000)
        );
    }
    let e = w.tick(&c, 1000);
    assert!(e.iter().any(|e| matches!(e, Event::MonsterSkill { .. })));
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].spirit, 90);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].cooldown[0], 11000);
}
#[test]
fn scheduled_heal_budget_and_recovery_are_not_restarted_on_transition() {
    let c = config();
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[0].heal = Some((1000, 1, 90));
    lower(&mut w, &c);
    assert_eq!(
        w.pve.as_ref().unwrap().monsters[0].heal,
        Some((1000, 1, 90))
    );
    w.tick(&c, 999);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 100);
    w.tick(&c, 1000);
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 130);
}
#[test]
fn lethal_hit_wins_over_thresholds_and_kill_and_drop_happen_once() {
    let c = config();
    let mut w = world(&c);
    w.pve.as_mut().unwrap().monsters[0].casting = Some((0, 100));
    let e = hit(&mut w, &c, 0, 10000);
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::BossPhaseChanged { .. }))
    );
    assert_eq!(w.kills, 1);
    assert!(w.pve.as_ref().unwrap().monsters[0].casting.is_none());
    let rng = w.rng;
    assert!(hit(&mut w, &c, 0, 10000).is_empty());
    assert_eq!(w.rng, rng);
    assert_eq!(w.kills, 1);
    assert!(w.resources.is_empty());
}
#[test]
fn disabled_dead_miss_and_zero_damage_cannot_advance_stage() {
    let mut c = config();
    c.pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .stats
        .insert("dodge".into(), 100);
    let mut w = world(&c);
    assert!(hit(&mut w, &c, 0, 200).is_empty());
    assert_eq!(boss(&w).phase_index, 1);
    let c = config();
    let mut w = world(&c);
    let e = hit(&mut w, &c, 0, 0);
    assert!(
        !e.iter()
            .any(|e| matches!(e, Event::BossPhaseChanged { .. }))
    );
    w.pve.as_mut().unwrap().monsters[0].enabled = false;
    let rng = w.rng;
    assert!(hit(&mut w, &c, 0, 200).is_empty());
    assert_eq!(w.rng, rng);
}
#[test]
fn instances_share_immutable_template_but_keep_independent_phases() {
    let mut c = config();
    c.pve_mut().extra_spawns.push(crate::config::MonsterSpawn {
        id: "boss-two".into(),
        template_id: "verification-phase-boss".into(),
        position: [650.0, 400.0],
    });
    let mut w = world(&c);
    lower(&mut w, &c);
    let a = w.pve.as_ref().unwrap();
    assert!(Arc::ptr_eq(&a.monsters[0].profile, &a.monsters[1].profile));
    assert_eq!(a.monsters[1].boss_view().unwrap().phase_index, 1);
    assert_eq!(a.monsters[1].hp, 300);
    assert_eq!(
        a.monsters[0].profile.boss.as_ref().unwrap().phases[0].attack_interval_ms,
        2000
    );
}
#[test]
fn ordinary_template_keeps_legacy_interval_probability_and_view_shape() {
    let mut c = config();
    let t = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap();
    t.boss = None;
    t.combat.attack_interval_ms = 1234;
    t.combat.cast_chance = 0.25;
    let mut w = world(&c);
    hit(&mut w, &c, 0, 200);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert!(m.boss_view().is_none());
    assert_eq!(m.attack_interval(), 1234);
    assert_eq!(m.cast_chance(), 0.25);
    assert!(
        !serde_json::to_value(w.snapshot(&c, 100)).unwrap()["pve"]["monsters"][0]
            .as_object()
            .unwrap()
            .contains_key("boss")
    );
}
#[test]
fn starting_return_does_not_reset_stage_or_epoch() {
    let c = config();
    let mut w = world(&c);
    lower(&mut w, &c);
    {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.position = [800.0, 400.0];
        m.casting = Some((0, 100));
        m.aggro = true;
    }
    let e = w.tick(&c, 100);
    assert!(w.pve.as_ref().unwrap().monsters[0].returning);
    assert_eq!((boss(&w).phase_index, boss(&w).epoch), (2, 1));
    assert!(!e.iter().any(|e| matches!(e, Event::BossReset { .. })));
    assert!(w.pve.as_ref().unwrap().monsters[0].casting.is_none());
}
#[test]
fn actual_home_completion_resets_hp_phase_epoch_and_melee_without_mp_or_cd_refund() {
    let c = config();
    let mut w = world(&c);
    lower(&mut w, &c);
    {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.returning = true;
        m.aggro = false;
        m.spirit = 40;
        m.cooldown = [9000, 8000];
        m.recovery = 7000;
    }
    let e = w.tick(&c, 1000);
    assert_eq!((boss(&w).phase_index, boss(&w).epoch), (1, 2));
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_eq!((m.hp, m.spirit, m.next_attack), (300, 40, 3000));
    assert_eq!(m.cooldown, [9000, 8000]);
    assert_eq!(m.recovery, 7000);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::BossReset { epoch: 2, .. }))
            .count(),
        1
    );
    assert!(
        !w.tick(&c, 1030)
            .iter()
            .any(|e| matches!(e, Event::BossReset { .. }))
    );
}
#[test]
fn return_outside_leash_is_immune_and_keeps_stage() {
    let c = config();
    let mut w = world(&c);
    lower(&mut w, &c);
    {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.returning = true;
        m.position = [800.0, 400.0];
    }
    assert!(hit(&mut w, &c, 0, 50).is_empty());
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 100);
    assert_eq!(boss(&w).phase_index, 2);
}
#[test]
fn legal_reactivation_before_home_preserves_phase_and_epoch() {
    let c = config();
    let mut w = world(&c);
    lower(&mut w, &c);
    {
        let m = &mut w.pve.as_mut().unwrap().monsters[0];
        m.returning = true;
        m.position = [600.0, 400.0];
    }
    hit(&mut w, &c, 0, 10);
    assert!(!w.pve.as_ref().unwrap().monsters[0].returning);
    assert_eq!((boss(&w).phase_index, boss(&w).epoch), (2, 1));
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].hp, 90);
}
#[test]
fn background_keeps_phase_and_committed_cooldown_state() {
    let c = config();
    let mut w = world(&c);
    lower(&mut w, &c);
    w.pve.as_mut().unwrap().monsters[0].cooldown = [5000, 6000];
    w.action(&Action::Background {}, "background", &c, 100)
        .unwrap();
    w.tick(&c, 200);
    assert_eq!((boss(&w).phase_index, boss(&w).epoch), (2, 1));
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].cooldown, [5000, 6000]);
}
#[test]
fn phase_change_does_not_complete_wave_but_real_boss_death_does() {
    let mut c = crate::config::default_catalog()
        .active("b3-7")
        .unwrap()
        .as_ref()
        .clone();
    c.params.regen.near_ratio = 0.0;
    c.params.regen.far_ratio = 0.0;
    let spawn = c.spawn;
    let p = c.pve_mut();
    let e = &mut p.encounters[0];
    e.waves = e.waves[1..].to_vec();
    e.waves[0].delay_ms = 0;
    e.trigger.center = spawn;
    let mut w = world(&c);
    w.tick(&c, 30);
    let i = w
        .pve
        .as_ref()
        .unwrap()
        .monsters
        .iter()
        .position(|m| m.template_id == "verification-phase-boss")
        .unwrap();
    hit(&mut w, &c, i, 300);
    w.tick(&c, 60);
    assert_eq!(
        w.snapshot(&c, 60).pve.unwrap().encounters[0].phase,
        EncounterPhase::Active
    );
    hit(&mut w, &c, i, 10000);
    w.tick(&c, 90);
    assert_eq!(
        w.snapshot(&c, 90).pve.unwrap().encounters[0].phase,
        EncounterPhase::Completed
    );
}
#[test]
fn guardian_rebirth_creates_new_identity_with_first_phase() {
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
    let rules = p.treasure_rules.as_mut().unwrap();
    rules.template.respawn_delay_ms = 1000;
    let mut g = g;
    g.template_id = "verification-phase-boss".into();
    g.chance = 1.0;
    g.spawn_offsets = vec![[64.0, 0.0], [-64.0, 0.0]];
    rules.template.guardian = Some(g);
    let mut w = world(&c);
    w.tick(&c, 30);
    let old = w.pve.as_ref().unwrap().monsters[0].id.clone();
    hit(&mut w, &c, 0, 200);
    assert_eq!(boss(&w).phase_index, 2);
    hit(&mut w, &c, 0, 10000);
    w.position = [800.0, 400.0];
    let open = |g| Action::OpenChest {
        chest_id: "boss-cache-1".into(),
        expected_generation: g,
    };
    w.action(&open(1), "unseal", &c, 30).unwrap();
    w.tick(&c, 2430);
    w.action(&open(1), "claim", &c, 2430).unwrap();
    w.tick(&c, 3430);
    let m = &w.pve.as_ref().unwrap().monsters[0];
    assert_ne!(m.id, old);
    assert_eq!((m.boss_view().unwrap().phase_index, m.boss_epoch), (1, 1));
}
#[test]
fn death_and_close_stop_future_phase_actions() {
    let c = config();
    for phase in [Phase::Draining, Phase::Closed] {
        let mut w = world(&c);
        lower(&mut w, &c);
        w.phase = phase;
        let before = boss(&w);
        assert!(w.tick(&c, 1000).is_empty());
        assert_eq!(boss(&w), before);
    }
}
#[test]
fn phases_do_not_add_population_rewards_or_recovery_loot_bounds() {
    let mut c = config();
    let b = c
        .pve_mut()
        .additional_templates
        .get_mut("verification-phase-boss")
        .unwrap()
        .boss
        .as_mut()
        .unwrap();
    for i in 2..8 {
        let mut p = b.phases[1].clone();
        p.id = format!("phase-{i}");
        p.hp_threshold_bps = 5000 - i * 100;
        b.phases.push(p);
    }
    assert_eq!(c.max_kills(c.terminal_ms), 1);
    assert_eq!(
        c.pve
            .as_ref()
            .unwrap()
            .population_bound(c.terminal_ms)
            .iter()
            .map(|(_, n)| *n)
            .sum::<u32>(),
        1
    );
    let mut w = world(&c);
    hit(&mut w, &c, 0, 290);
    assert_eq!(w.kills, 0);
    assert!(w.resources.is_empty());
    hit(&mut w, &c, 0, 10000);
    assert_eq!(w.kills, 1);
    assert!(w.resources.is_empty());
}
