use super::*;
fn config() -> Config {
    crate::config::default_catalog()
        .active("b3-2")
        .unwrap()
        .as_ref()
        .clone()
}
fn flat(mut c: Config) -> Config {
    c.spawn = [400.0, 400.0];
    c.extraction_point = c.spawn;
    let spawn = c.spawn;
    let p = c.pve_mut();
    p.width = 32;
    p.height = 32;
    p.cells_rle = vec![(0, 1024)];
    p.monster_positions = vec![[500.0, 400.0]];
    p.chest_spawns.truncate(1);
    p.chest_spawns[0].position = spawn;
    c
}
fn world(c: &Config) -> World {
    let f = &c.params.initial_player;
    let mut w = World::new(
        c,
        ParticipantFacts {
            principal: "principal-A".into(),
            revision: 1,
            hp_points: f.hp_points,
            spirit_points: f.spirit_points,
            atk_points: f.atk_points,
            def_points: f.def_points,
            spd_points: f.spd_points,
            warehouse_quantity: 100,
        },
        7,
    );
    w.phase = Phase::Active;
    w.ready = true;
    w.joined = true;
    w.protected = false;
    w.active_at = Some(0);
    w.next_regen = u64::MAX;
    w
}
#[test]
fn initial_player_and_snapshot_limits_follow_configuration() {
    let mut c = flat(config());
    c.params.initial_player.hp_points = 2345;
    c.params.initial_player.spirit_points = 0;
    let w = world(&c);
    let s = w.snapshot(&c, 0);
    assert_eq!(s.hp, 2345);
    assert_eq!(s.max_hp, 2345);
    assert_eq!(s.max_spirit, 0);
}
#[test]
fn configured_skill_identity_cost_windup_and_cooldown_are_consumed() {
    let mut c = flat(config());
    c.skill.id = "different-projectile".into();
    c.skill.cost = 13;
    c.skill.windup_ms = 90;
    c.skill.cooldown_ms = 777;
    let mut w = world(&c);
    let mp = w.spirit;
    w.action(
        &Action::Cast {
            skill_id: c.skill.id.clone(),
            direction: [1.0, 0.0],
        },
        "cast",
        &c,
        0,
    )
    .unwrap();
    assert!(
        !w.tick(&c, 60)
            .iter()
            .any(|e| matches!(e, Event::ActionCommitted { .. }))
    );
    assert!(w.tick(&c, 90).iter().any(|e| matches!(
        e,
        Event::ActionCommitted {
            spirit_cost: 13,
            ..
        }
    )));
    assert_eq!(w.spirit, mp - 13);
    assert_eq!(w.skill_ready_at, 867);
}
#[test]
fn ai_melee_interval_and_zero_cast_probability_are_parameters() {
    let mut c = flat(config());
    let p = c.pve_mut();
    p.monster_positions[0] = [440.0, 400.0];
    p.combat.cast_chance = 0.0;
    p.combat.attack_interval_ms = 1234;
    p.stats.insert("spd".into(), 0);
    let mut w = world(&c);
    w.tick(&c, 30);
    assert!(
        !w.tick(&c, 1263)
            .iter()
            .any(|e| matches!(e, Event::Damage { .. }))
    );
    assert!(
        w.tick(&c, 1264)
            .iter()
            .any(|e| matches!(e,Event::Damage{target,..}if target=="participant"))
    );
    assert!(w.pve.as_ref().unwrap().monsters[0].casting.is_none());
}
#[test]
fn lifesteal_uses_actual_damage_and_configured_ratio() {
    let mut c = flat(config());
    c.params.initial_player.def_points = 0;
    let p = c.pve_mut();
    p.stats.insert("hp".into(), 1000);
    p.stats.insert("crit".into(), 0);
    p.combat.lifesteal_ratio = 0.5;
    let mut w = world(&c);
    let mut a = w.pve.take().unwrap();
    a.monsters[0].hp = 500;
    w.hp = 40;
    a.hurt_player(&mut w, &c, 0, (100, false), 0, &mut vec![]);
    assert_eq!(w.hp, 0);
    assert_eq!(a.monsters[0].hp, 520);
}
#[test]
fn regeneration_interval_and_out_of_combat_fraction_are_parameters() {
    let mut c = flat(config());
    c.params.initial_player.hp_points = 2000;
    c.params.regen.interval_ms = 100;
    c.params.regen.far_ratio = 0.1;
    c.params.regen.near_radius = 20.0;
    let p = c.pve_mut();
    p.monster_positions.clear();
    let mut w = world(&c);
    w.hp = 100;
    w.spirit = 50;
    w.next_regen = 100;
    w.tick(&c, 99);
    assert_eq!(w.hp, 100);
    w.tick(&c, 100);
    assert_eq!(w.hp, 300);
    assert_eq!(w.spirit, 60);
}
#[test]
fn movement_curve_and_cell_scale_are_configured() {
    let mut c = flat(config());
    c.params.movement.base = 90.0;
    c.params.movement.bonus = 0.0;
    let p = c.pve_mut();
    p.cell_size = 64.0;
    p.monster_positions.clear();
    let mut w = world(&c);
    w.action(
        &Action::Move {
            direction: [1.0, 0.0],
        },
        "move",
        &c,
        0,
    )
    .unwrap();
    w.tick(&c, 30);
    assert!((w.position[0] - 402.7).abs() < 1e-8);
    assert_eq!(w.pve.as_ref().unwrap().grid.center(0), [32.0, 32.0]);
    // A point can fit the player while a differently sized monster crosses the map edge.
    c.pve_mut().monster_positions = vec![[32.0, 32.0]];
    c.pve_mut().combat.radius = 64.0;
    assert!(!validate_map(
        c.pve.as_ref().unwrap(),
        c.spawn,
        c.extraction_point
    ));
    c.pve_mut().combat.radius = 14.0;
    assert!(validate_map(
        c.pve.as_ref().unwrap(),
        c.spawn,
        c.extraction_point
    ));
}
#[test]
fn arbitrary_instance_counts_and_template_names_use_the_same_ai() {
    for count in [0, 1, 5] {
        let mut c = flat(config());
        let p = c.pve_mut();
        p.template_id = "configured-beast".into();
        p.monster_positions = (0..count)
            .map(|i| [500.0 + i as f64 * 40.0, 400.0])
            .collect();
        p.combat.projectile.id = "beast-bolt".into();
        p.combat.heal.id = "beast-mend".into();
        let w = world(&c);
        let s = w.snapshot(&c, 0).pve.unwrap();
        assert_eq!(s.monsters.len(), count);
        assert!(
            s.monsters
                .iter()
                .all(|m| m.id.starts_with("configured-beast-"))
        );
    }
}
#[test]
fn container_range_and_respawn_delay_are_loaded_parameters() {
    let mut c = flat(config());
    let p = c.pve_mut();
    let rules = p.treasure_rules.as_mut().unwrap();
    rules.interaction_range = 20.0;
    rules.template.respawn_delay_ms = 1500;
    rules.template.id = "configured-pouch".into();
    let mut w = world(&c);
    w.position[0] -= 21.0;
    let action = Action::OpenChest {
        chest_id: "forest-pouch-1".into(),
        expected_generation: 1,
    };
    assert_eq!(w.action(&action, "far", &c, 0), Err(Error::OutOfRange));
    w.position[0] += 1.0;
    w.action(&action, "open", &c, 0).unwrap();
    assert_eq!(
        w.snapshot(&c, 0).pve.unwrap().chests[0].respawn_at_ms,
        Some(1500)
    );
    w.tick(&c, 1499);
    assert_eq!(w.snapshot(&c, 1499).pve.unwrap().chests[0].generation, 1);
    w.tick(&c, 1500);
    assert_eq!(w.snapshot(&c, 1500).pve.unwrap().chests[0].generation, 2);
}
#[test]
fn new_item_reference_quantity_and_stack_limit_need_no_item_code() {
    let mut c = flat(config());
    let p = c.pve_mut();
    p.items.insert(
        "new-material".into(),
        crate::config::Item {
            name: "New material".into(),
            stack_limit: 2,
        },
    );
    let loot = &mut p.treasure_rules.as_mut().unwrap().loot;
    loot.random.clear();
    loot.guaranteed[0].item_id = "new-material".into();
    loot.guaranteed[0].quantity_min = 7;
    loot.guaranteed[0].quantity_max = 7;
    let mut w = world(&c);
    w.action(
        &Action::OpenChest {
            chest_id: "forest-pouch-1".into(),
            expected_generation: 1,
        },
        "open",
        &c,
        0,
    )
    .unwrap();
    let id = w.resources[0].view.id.clone();
    assert_eq!(w.resources[0].view.quantity, 7);
    w.action(
        &Action::Pickup {
            resource_id: id,
            expected_revision: 1,
        },
        "pick",
        &c,
        0,
    )
    .unwrap();
    assert_eq!(w.pve.as_ref().unwrap().items["new-material"], 7);
}
#[test]
fn resource_lifetime_and_sweep_release_all_lookup_state() {
    let mut c = flat(config());
    c.params.resource_ttl_ms = 100;
    c.params.runtime.resource_sweep_ms = 20;
    let p = c.pve_mut();
    p.monster_positions.clear();
    let mut w = world(&c);
    w.action(
        &Action::OpenChest {
            chest_id: "forest-pouch-1".into(),
            expected_generation: 1,
        },
        "open",
        &c,
        0,
    )
    .unwrap();
    w.tick(&c, 99);
    assert!(!w.resources.is_empty());
    w.tick(&c, 120);
    assert!(w.resources.is_empty());
    assert!(w.pve.as_ref().unwrap().resource_items.is_empty());
    assert!(w.pve.as_ref().unwrap().resource_expiry.is_empty());
}
#[test]
fn death_distribution_verifies_multiple_full_and_partial_stacks() {
    let mut c = flat(config());
    c.stack_limit = 3;
    let p = c.pve_mut();
    p.items.get_mut("huo-ling").unwrap().stack_limit = 2;
    let mut w = world(&c);
    w.inventory = 8;
    w.looted = 3;
    w.pve.as_mut().unwrap().items.insert("huo-ling".into(), 7);
    w.desired_outcome = Some(Outcome::Death);
    let Business::CommitSettlement { dropped, items, .. } = w.settlement(&c) else {
        panic!()
    };
    assert_eq!(dropped + items[0].dropped, 4);
    assert!(c.valid_death_loss(&[("xuan-tie", 8, dropped), ("huo-ling", 7, items[0].dropped)]));
    assert!(!c.valid_death_loss(&[
        ("xuan-tie", 8, dropped + 1),
        ("huo-ling", 7, items[0].dropped)
    ]));
}
#[test]
fn zero_and_full_death_loss_policies_have_exact_bounds() {
    let mut c = flat(config());
    c.stack_limit = 3;
    c.params.death.slot_ratio = 0.0;
    assert!(c.valid_death_loss(&[("xuan-tie", 8, 0)]));
    assert!(!c.valid_death_loss(&[("xuan-tie", 8, 1)]));
    c.params.death.slot_ratio = 1.0;
    c.params.death.quantity_ratio = 1.0;
    assert!(c.valid_death_loss(&[("xuan-tie", 8, 8)]));
    assert!(!c.valid_death_loss(&[("xuan-tie", 8, 7)]));
    c.params.death.quantity_ratio = 0.0;
    c.params.death.minimum_quantity = 0;
    assert!(c.valid_death_loss(&[("xuan-tie", 8, 0)]));
}
#[test]
fn zero_cultivation_and_larger_rewards_remain_bounded() {
    let mut c = flat(config());
    c.cultivation_coefficient = 0.0;
    assert_eq!(c.cultivation(5, Outcome::Extraction), 0);
    assert_eq!(c.cultivation(5, Outcome::Death), 0);
    assert!(c.pve_loot_bound("huo-ling", 5, 300000) >= 10);
}
#[test]
fn shipped_legacy_versions_keep_their_hash_and_original_parameters() {
    let catalog = crate::config::default_catalog();
    for (id, hash) in [
        (
            "b1-verification-v1",
            "cb67c8fab4467f82b20af9cb1b0a259c642a6754b5cef62b0369e542caccca21",
        ),
        (
            "b3-1-misty-wolves-v1",
            "98e13c03a1d134d8df5ed3d29b9f0ddb813d7134cea23c251c713f0720ca5937",
        ),
        (
            "b3-2-misty-pouches-v1",
            "b5ee821f65f9d58c36eba63617fe5a5c869b91a75f95b516370513164cedb159",
        ),
    ] {
        let c = catalog.find(id, hash).unwrap();
        assert_eq!(c.content_hash(), hash);
        assert_eq!(c.params.runtime.io_timeout_ms, 5000);
        assert_eq!(c.params.player_radius, 16.0);
    }
}

#[test]
fn aggregate_death_validator_matches_exhaustive_slot_selection() {
    let mut c = flat(config());
    c.stack_limit = 3;
    c.pve_mut().items.get_mut("huo-ling").unwrap().stack_limit = 2;
    // Six physical stacks: iron 3+1, material 2+2+2+1. Select exactly three.
    let stacks = [(0, 3u32), (0, 1), (1, 2), (1, 2), (1, 2), (1, 1)];
    let mut feasible = std::collections::BTreeSet::new();
    for bits in 0u32..64 {
        if bits.count_ones() != 3 {
            continue;
        }
        let mut loss = [0u32; 2];
        for (i, (kind, q)) in stacks.iter().enumerate() {
            if bits & (1 << i) != 0 {
                loss[*kind] += (q / 2).max(1);
            }
        }
        feasible.insert((loss[0], loss[1]));
    }
    for iron in 0..=4 {
        for material in 0..=7 {
            assert_eq!(
                c.valid_death_loss(&[("xuan-tie", 4, iron), ("huo-ling", 7, material)]),
                feasible.contains(&(iron, material))
            );
        }
    }
}
#[test]
fn monster_loot_probability_zero_and_one_are_exact() {
    let mut c = flat(config());
    let p = c.pve_mut();
    p.drop_table.guaranteed.clear();
    p.drop_table.random.truncate(2);
    p.drop_table.random[0].chance = Some(0.0);
    p.drop_table.random[1].chance = Some(1.0);
    for seed in 1..=32 {
        let mut w = world(&c);
        w.rng = seed;
        let mut a = w.pve.take().unwrap();
        a.drop(&mut w, &c, 0);
        assert_eq!(w.resources.iter().filter(|r| !r.view.orb).count(), 1);
        assert_eq!(a.resource_items.values().next().unwrap(), "huo-ling");
    }
}
