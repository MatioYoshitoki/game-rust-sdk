use super::*;
fn config() -> Config {
    let mut c = crate::config::default_catalog()
        .active("b3-4")
        .unwrap()
        .as_ref()
        .clone();
    c.spawn = [400.0, 400.0];
    c.extraction_point = [400.0, 400.0];
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.monster_positions = vec![[200.0, 200.0]];
    p.chest_spawns[0].position = [500.0, 400.0];
    p.chest_spawns[1].position = [800.0, 400.0];
    let g = p
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .guardian
        .as_mut()
        .unwrap();
    g.chance = 1.0;
    g.spawn_offsets = vec![[64.0, 0.0], [-64.0, 0.0], [0.0, 64.0], [0.0, -64.0]];
    let t = p
        .additional_templates
        .get_mut("verification-guardian")
        .unwrap();
    t.combat.cast_chance = 0.0;
    t.stats.insert("spd".into(), 0);
    t.stats.insert("dodge".into(), 0);
    t.stats.insert("atk".into(), 0);
    t.stats.insert("crit".into(), 0);
    c
}
fn world(c: &Config, seed: u32) -> World {
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
        seed,
    );
    w.phase = Phase::Active;
    w.ready = true;
    w.joined = true;
    w.protected = false;
    w.active_at = Some(0);
    w.position = c.spawn;
    w.pve.as_mut().unwrap().monsters[0].hp = 0;
    w
}
fn views(w: &World, c: &Config) -> Vec<ChestView> {
    w.snapshot(c, 0).pve.unwrap().chests
}
fn open(n: usize, g: u32) -> Action {
    Action::OpenChest {
        chest_id: format!("guarded-cache-{n}"),
        expected_generation: g,
    }
}
fn guard_indices(w: &World, box_id: &str) -> Vec<usize> {
    w.pve
        .as_ref()
        .unwrap()
        .monsters
        .iter()
        .enumerate()
        .filter(|(_, m)| m.enabled && m.binding.as_ref().is_some_and(|b| b.chest_id == box_id))
        .map(|(i, _)| i)
        .collect()
}
fn kill(w: &mut World, c: &Config, i: usize) -> Vec<Event> {
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hurt_monster(w, c, i, 10000, false, &mut e);
    w.pve = Some(a);
    e
}
fn clear(w: &mut World, c: &Config) {
    let indices: Vec<_> = w
        .pve
        .as_ref()
        .unwrap()
        .monsters
        .iter()
        .enumerate()
        .filter(|(_, m)| m.hp > 0)
        .map(|(i, _)| i)
        .collect();
    for i in indices {
        kill(w, c, i);
    }
}
fn unseal_and_open(w: &mut World, c: &Config, g: u32, now: u64) {
    w.position = [500.0, 400.0];
    w.action(&open(1, g), "begin", c, now).unwrap();
    w.tick(c, now + 2400);
    w.action(&open(1, g), "claim", c, now + 2400).unwrap();
}

#[test]
fn pending_gate_blocks_before_generation_plan_is_resolved() {
    let c = config();
    let mut w = world(&c, 7);
    w.position = [500.0, 400.0];
    assert_eq!(
        w.action(&open(1, 1), "pending", &c, 0),
        Err(Error::GuardianPending)
    );
    assert!(views(&w, &c)[0].guardians.as_ref().unwrap().pending);
}
#[test]
fn certain_generation_is_once_and_exactly_bound_to_container_and_epoch() {
    let c = config();
    let mut w = world(&c, 7);
    let e = w.tick(&c, 30);
    assert_eq!(
        e.iter()
            .filter(|e| matches!(e, Event::GuardiansReady { .. }))
            .count(),
        2
    );
    let before = w.snapshot(&c, 30).pve.unwrap().monsters;
    let rng = w.rng;
    for t in [30, 60, 90] {
        assert!(
            !w.tick(&c, t)
                .iter()
                .any(|e| matches!(e, Event::GuardiansReady { .. }))
        );
    }
    assert_eq!(w.rng, rng);
    for v in before.iter().filter(|m| m.guardian.is_some()) {
        assert_eq!(v.guardian.as_ref().unwrap().generation, 1);
        assert!(v.id.contains("-g1-"));
        assert_eq!(v.max_hp, 225);
    }
}
#[test]
fn zero_probability_has_no_guardians_or_interaction_gate() {
    let mut c = config();
    c.pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .guardian
        .as_mut()
        .unwrap()
        .chance = 0.0;
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    assert_eq!(w.snapshot(&c, 30).pve.unwrap().monsters.len(), 1);
    assert_eq!(
        views(&w, &c)[0].guardians.as_ref().unwrap().planned_count,
        Some(0)
    );
    w.position = [500.0, 400.0];
    w.action(&open(1, 1), "start", &c, 30).unwrap();
}
#[test]
fn probability_and_quantity_bounds_replay_with_both_endpoints() {
    let mut c = config();
    let g = c
        .pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .guardian
        .as_mut()
        .unwrap();
    g.chance = 0.25;
    g.max_count = 2;
    let mut counts = std::collections::BTreeSet::new();
    for seed in 1..=128 {
        let mut a = world(&c, seed * 1009);
        let mut b = world(&c, seed * 1009);
        a.tick(&c, 30);
        b.tick(&c, 30);
        assert_eq!(views(&a, &c), views(&b, &c));
        counts.insert(
            views(&a, &c)[0]
                .guardians
                .as_ref()
                .unwrap()
                .planned_count
                .unwrap(),
        );
    }
    assert_eq!(counts, std::collections::BTreeSet::from([0, 1, 2]));
}
#[test]
fn blocked_plan_has_no_partial_entities_or_reroll_then_recovers() {
    let mut c = config();
    let g = c
        .pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .guardian
        .as_mut()
        .unwrap();
    g.max_count = 2;
    g.min_count = 2;
    g.spawn_offsets = vec![[64.0, 0.0], [-64.0, 0.0]];
    let mut w = world(&c, 7);
    w.position = [564.0, 400.0];
    let e = w.tick(&c, 30);
    assert!(
        e.iter()
            .any(|e| matches!(e, Event::GuardianSpawnBlocked { .. }))
    );
    assert!(guard_indices(&w, "guarded-cache-1").is_empty());
    let rng = w.rng;
    assert_eq!(
        views(&w, &c)[0].guardians.as_ref().unwrap().planned_count,
        Some(2)
    );
    // Other live guardians may draw combat RNG; isolate the generation retry itself.
    let mut arena = w.pve.take().unwrap();
    arena.prepare_guardians(&mut w, 230, &mut vec![]);
    w.pve = Some(arena);
    assert_eq!(w.rng, rng);
    w.position = [400.0, 700.0];
    w.tick(&c, 430);
    assert_eq!(guard_indices(&w, "guarded-cache-1").len(), 2);
    assert!(!views(&w, &c)[0].guardians.as_ref().unwrap().pending);
}
#[test]
fn living_guardians_gate_both_unlock_and_open_even_when_returning() {
    let c = config();
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    let i = guard_indices(&w, "guarded-cache-1")[0];
    w.pve.as_mut().unwrap().monsters[i].returning = true;
    w.position = [500.0, 400.0];
    assert_eq!(
        w.action(&open(1, 1), "blocked", &c, 30),
        Err(Error::GuardianAlive)
    );
    assert!(w.resources.is_empty());
    assert_eq!(views(&w, &c)[0].seal.as_ref().unwrap().completed_layers, 0);
}
#[test]
fn other_container_death_does_not_release_this_container() {
    let c = config();
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    for i in guard_indices(&w, "guarded-cache-2") {
        kill(&mut w, &c, i);
    }
    w.position = [500.0, 400.0];
    assert_eq!(
        w.action(&open(1, 1), "blocked", &c, 30),
        Err(Error::GuardianAlive)
    );
    assert_eq!(views(&w, &c)[1].guardians.as_ref().unwrap().alive, 0);
}
#[test]
fn all_bound_guardians_must_die_and_repeated_death_cannot_pay_twice() {
    let mut c = config();
    let g = c
        .pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .guardian
        .as_mut()
        .unwrap();
    g.min_count = 2;
    g.max_count = 2;
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    let ids = guard_indices(&w, "guarded-cache-1");
    kill(&mut w, &c, ids[0]);
    w.position = [500.0, 400.0];
    assert_eq!(
        w.action(&open(1, 1), "one-alive", &c, 30),
        Err(Error::GuardianAlive)
    );
    let e = kill(&mut w, &c, ids[0]);
    assert!(e.is_empty());
    assert_eq!(w.kills, 1);
    kill(&mut w, &c, ids[1]);
    w.action(&open(1, 1), "cleared", &c, 30).unwrap();
    assert_eq!(w.kills, 2);
    assert!(w.resources.is_empty());
}
#[test]
fn mixed_profiles_keep_stats_skills_and_dimensions_independent() {
    let mut c = config();
    let p = c.pve_mut();
    let mut t = p.additional_templates["verification-guardian"].clone();
    t.stats.insert("hp".into(), 777);
    t.stats.insert("spirit".into(), 9);
    t.combat.radius = 22.0;
    t.combat.projectile.id = "extra-bolt".into();
    p.additional_templates.insert("extra".into(), t);
    p.extra_spawns.push(crate::config::MonsterSpawn {
        id: "extra-one".into(),
        template_id: "extra".into(),
        position: [800.0, 700.0],
    });
    let w = world(&c, 7);
    let s = w.snapshot(&c, 0).pve.unwrap();
    let extra = s.monsters.iter().find(|m| m.id == "extra-one").unwrap();
    assert_eq!(
        (extra.max_hp, extra.spirit, extra.radius),
        (777, 9, Some(22.0))
    );
    assert_eq!(w.pve.as_ref().unwrap().monsters[0].profile.stats["hp"], 82);
    assert_eq!(
        w.pve.as_ref().unwrap().monsters[1]
            .profile
            .combat
            .projectile
            .id,
        "extra-bolt"
    );
}
#[test]
fn guard_reward_policy_can_disable_credit_and_enable_item_drop_without_orb() {
    let mut c = config();
    let p = c.pve_mut();
    let t = p
        .additional_templates
        .get_mut("verification-guardian")
        .unwrap();
    t.kill_credit = false;
    t.drop_table.guaranteed.push(crate::config::LootRule {
        item_id: "han-tan".into(),
        chance: None,
        quantity_min: 2,
        quantity_max: 2,
    });
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    let i = guard_indices(&w, "guarded-cache-1")[0];
    kill(&mut w, &c, i);
    assert_eq!(w.kills, 0);
    assert_eq!(w.resources.len(), 1);
    assert!(!w.resources[0].view.orb);
    assert_eq!(w.resources[0].view.quantity, 2);
    assert!(c.pve_loot_bound("han-tan", 0, 30) >= 2);
}
#[test]
fn default_guard_kill_has_credit_but_no_item_or_orb_or_container_reward() {
    let c = config();
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    let i = guard_indices(&w, "guarded-cache-1")[0];
    kill(&mut w, &c, i);
    assert_eq!(w.kills, 1);
    assert!(w.resources.is_empty());
    assert!(!views(&w, &c)[0].opened);
    assert_eq!(w.inventory, 5);
}
#[test]
fn clearing_guards_then_unlocking_still_requires_explicit_loot_claim() {
    let c = config();
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    clear(&mut w, &c);
    assert_eq!(w.kills, 2);
    w.position = [500.0, 400.0];
    w.action(&open(1, 1), "start", &c, 30).unwrap();
    w.tick(&c, 2430);
    assert!(w.resources.is_empty());
    assert_eq!(views(&w, &c)[0].seal.as_ref().unwrap().completed_layers, 2);
    w.action(&open(1, 1), "claim", &c, 2430).unwrap();
    assert!(!w.resources.is_empty());
    assert_eq!(w.inventory, 5);
}
#[test]
fn respawn_reuses_bounded_slots_with_new_ids_and_live_gate() {
    let mut c = config();
    c.pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .respawn_delay_ms = 1000;
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    let old = guard_indices(&w, "guarded-cache-1")[0];
    let old_id = w.pve.as_ref().unwrap().id(old);
    clear(&mut w, &c);
    unseal_and_open(&mut w, &c, 1, 30);
    let slots = w.pve.as_ref().unwrap().monsters.len();
    w.tick(&c, 3430);
    let new = guard_indices(&w, "guarded-cache-1")[0];
    assert_eq!(old, new);
    assert_ne!(old_id, w.pve.as_ref().unwrap().id(new));
    assert_eq!(w.pve.as_ref().unwrap().monsters.len(), slots);
    assert_eq!(
        w.action(&open(1, 1), "old", &c, 3430),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        w.action(&open(1, 2), "new", &c, 3430),
        Err(Error::GuardianAlive)
    );
}
#[test]
fn old_guardian_projectiles_and_pending_target_lock_are_retired_at_respawn() {
    let mut c = config();
    c.pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .respawn_delay_ms = 1000;
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    let i = guard_indices(&w, "guarded-cache-1")[0];
    clear(&mut w, &c);
    unseal_and_open(&mut w, &c, 1, 30);
    let a = w.pve.as_mut().unwrap();
    a.locked = Some(i);
    a.shots.push(Shot {
        strict_target: None,
        status_source: None,
        hit_statuses: vec![],
        rounds: vec![],
        manual_started: None,
        position: [600.0, 400.0],
        direction: [-1.0, 0.0],
        remaining: 1000.0,
        speed: 1.0,
        radius: 0.0,
        raw: 1000,
        owner: Some(i),
        metal: true,
        mark: true,
        expires: 100000,
        created: 2430,
    });
    w.tick(&c, 3430);
    assert!(w.pve.as_ref().unwrap().locked.is_none());
    assert!(
        w.pve
            .as_ref()
            .unwrap()
            .shots
            .iter()
            .all(|s| s.owner != Some(i))
    );
}
#[test]
fn repeated_generations_have_unique_ground_ids_and_bounded_monster_storage() {
    let mut c = config();
    c.pve_mut()
        .treasure_rules
        .as_mut()
        .unwrap()
        .template
        .respawn_delay_ms = 1000;
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    let capacity = w.pve.as_ref().unwrap().monsters.len();
    let mut ids = std::collections::BTreeSet::new();
    for generation in 1..=4 {
        clear(&mut w, &c);
        let now = 30 + u64::from(generation - 1) * 3400;
        unseal_and_open(&mut w, &c, generation, now);
        for r in &w.resources {
            ids.insert(r.view.id.clone());
        }
        assert_eq!(ids.len(), w.resources.len());
        w.tick(&c, now + 3400);
        assert_eq!(w.pve.as_ref().unwrap().monsters.len(), capacity);
    }
}
#[test]
fn closed_world_never_replans_or_generates_new_guards() {
    let c = config();
    let mut w = world(&c, 7);
    w.action(&Action::Leave {}, "leave", &c, 0).unwrap();
    let rng = w.rng;
    let before = views(&w, &c);
    assert!(w.tick(&c, 600000).is_empty());
    assert_eq!(w.rng, rng);
    assert_eq!(views(&w, &c), before);
}
#[test]
fn recovery_bounds_cover_guardian_cycles_and_mixed_credited_loot() {
    let mut c = config();
    let p = c.pve_mut();
    p.additional_templates
        .get_mut("verification-guardian")
        .unwrap()
        .drop_table
        .guaranteed
        .push(crate::config::LootRule {
            item_id: "han-tan".into(),
            chance: None,
            quantity_min: 3,
            quantity_max: 3,
        });
    assert_eq!(c.max_kills(0), 3);
    assert_eq!(c.max_kills(602399), 3);
    assert_eq!(c.max_kills(602400), 5);
    assert_eq!(c.pve_loot_bound("han-tan", 0, 0), 0);
    assert_eq!(c.pve_loot_bound("han-tan", 1, 0), 3);
}
#[test]
fn zero_chance_excludes_guardians_from_credit_and_yield_budgets() {
    let mut c = config();
    let p = c.pve_mut();
    p.treasure_rules
        .as_mut()
        .unwrap()
        .template
        .guardian
        .as_mut()
        .unwrap()
        .chance = 0.0;
    assert_eq!(c.max_kills(c.terminal_ms), 1);
    assert_eq!(c.pve_loot_bound("han-tan", 1, 0), 0);
}
#[test]
fn seeded_generation_plans_do_not_change_after_background_and_resume() {
    let c = config();
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    let ids: Vec<_> = w
        .snapshot(&c, 30)
        .pve
        .unwrap()
        .monsters
        .iter()
        .map(|m| m.id.clone())
        .collect();
    w.action(&Action::Background {}, "bg", &c, 30).unwrap();
    w.tick(&c, 500);
    let after: Vec<_> = w
        .snapshot(&c, 500)
        .pve
        .unwrap()
        .monsters
        .iter()
        .map(|m| m.id.clone())
        .collect();
    assert_eq!(after, ids);
    assert!(
        !w.tick(&c, 600)
            .iter()
            .any(|e| matches!(e, Event::GuardiansReady { .. }))
    );
}
#[test]
fn own_guard_drop_identity_contains_generation_and_does_not_alias_other_boxes() {
    let mut c = config();
    let p = c.pve_mut();
    p.additional_templates
        .get_mut("verification-guardian")
        .unwrap()
        .drop_orb = true;
    let mut w = world(&c, 7);
    w.tick(&c, 30);
    clear(&mut w, &c);
    let ids: Vec<_> = w.resources.iter().map(|r| r.view.id.clone()).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert!(
        ids.iter()
            .all(|id| id.contains("-g1-") && id.ends_with("-orb"))
    );
}
