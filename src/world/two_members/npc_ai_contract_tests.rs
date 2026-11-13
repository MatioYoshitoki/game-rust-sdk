use super::*;

fn ai_input(v: &mut Value) {
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["pve"]["combat"]["aggro_range"] = json!(8);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["memory_range"] = json!(100);
    v["pve"]["help_range"] = json!(16);
    v["pve"]["npc_ai"] = json!({"switch_threat_bps":12000,"profiles":{"npc":{"slots":[]}}});
}

fn damage_npc(w: &mut World, c: &Config, (npc, member): (usize, usize), raw: u32, now: u64) -> u32 {
    w.resolve_damage(
        combat::DamageIntent {
            source: w.members[member].reference.entity.clone(),
            target: w.npcs[npc].entity.clone(),
            raw,
            direct: false,
            ignore_armor: true,
        },
        c,
        now,
        &mut vec![],
    )
    .hp_damage
}

#[test]
fn threat_switching_and_help_use_independent_records_and_groups() {
    let c = catalog(input(), |v| {
        ai_input(v);
        v["pve"]["monster_positions"] = json!([[52, 20], [52, 29], [52, 34]]);
        v["pve"]["npc_ai"]["groups"] = json!({"npc-1":"g1","npc-2":"g1","npc-3":"g2"});
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let a = w.members[0].reference.entity.clone();
    let b = w.members[1].reference.entity.clone();
    assert_eq!(damage_npc(&mut w, &c, (0, 0), 10, 0), 10);
    w.tick(&c, 30);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&a));
    assert!(w.npcs[1].target.is_none());
    w.tick(&c, 60);
    assert_eq!(w.npcs[1].target.as_ref(), Some(&a));
    assert!(w.npcs[2].target.is_none());
    assert_eq!(damage_npc(&mut w, &c, (0, 1), 11, 60), 11);
    w.tick(&c, 90);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&a));
    assert_eq!(damage_npc(&mut w, &c, (0, 1), 1, 90), 1);
    w.tick(&c, 120);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&b));
    assert_eq!(w.npcs[1].target.as_ref(), Some(&a));
    w.protect(1, 120, &mut vec![]);
    assert!(w.npcs[0].target.is_none());
    w.tick(&c, 150);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&a));
}

#[test]
fn forced_target_overrides_threat_and_expires_back_to_it() {
    let c = catalog(input(), |v| {
        let rules = v["pve"]["status_rules"].clone();
        ai_input(v);
        v["pve"]["status_rules"] = rules;
        v["pve"]["status_definitions"] = json!([
            {"id":"forced","mechanism":"timed_attribute_modifier","attribute":"atk",
             "modifier_bps":-100,"duration_ms":90,"duplicate_policy":"refresh","source_scoped":true}
        ]);
        v["pve"]["npc_ai"]["forced_target_statuses"] = json!(["forced"]);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let a = w.members[0].reference.entity.clone();
    let b = w.members[1].reference.entity.clone();
    damage_npc(&mut w, &c, (0, 0), 30, 0);
    w.tick(&c, 30);
    w.apply_owned_statuses(
        Owner::Member(1),
        (w.npcs[0].entity.clone(), 10),
        &["forced".into()],
        &c,
        30,
        &mut vec![],
    );
    w.tick(&c, 60);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&b));
    w.tick(&c, 90);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&b));
    w.tick(&c, 120);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&a));
}

#[test]
fn leash_rejects_outside_damage_and_reentry_keeps_real_hp_loss() {
    let c = catalog(input(), |v| {
        ai_input(v);
        v["pve"]["leash"] = json!(32);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let b = w.members[1].reference.entity.clone();
    w.npcs[0].returning = true;
    w.npcs[0].position = [85., 20.];
    assert_eq!(damage_npc(&mut w, &c, (0, 1), 10, 0), 0);
    assert_eq!(w.npcs[0].hp, 100);
    w.npcs[0].position = [83., 20.];
    assert_eq!(damage_npc(&mut w, &c, (0, 1), 10, 30), 10);
    assert!(!w.npcs[0].returning);
    w.tick(&c, 60);
    assert_eq!(w.npcs[0].hp, 90);
    assert_eq!(w.npcs[0].target.as_ref(), Some(&b));
    w.protect(0, 60, &mut vec![]);
    w.protect(1, 60, &mut vec![]);
    for step in 3..=36 {
        w.tick(&c, step * 30);
    }
    assert_eq!(w.npcs[0].position, w.npcs[0].home);
    assert_eq!(w.npcs[0].hp, 100);
    assert_eq!(w.npcs[0].entity.generation, 2);
    assert!(w.npcs[0].target.is_none());
}

fn support_input(v: &mut Value) {
    ai_input(v);
    v["pve"]["combat"]["aggro_range"] = json!(64);
    let combat = v["pve"]["combat"].clone();
    let p = &mut v["pve"];
    p["additional_templates"] = json!({"ally":{
        "name":"synthetic ally","mechanism":"melee_projectile_heal","stats":p["stats"],
        "combat":combat,"drop_table":p["drop_table"],"leash":p["leash"],
        "help_range":p["help_range"],"memory_range":p["memory_range"],"kill_credit":true,"drop_orb":true
    }});
    p["extra_spawns"] = json!([
        {"id":"ally-a","template_id":"ally","position":[53,30]},
        {"id":"ally-b","template_id":"ally","position":[54,36]},
        {"id":"cross-group","template_id":"ally","position":[56,40]}
    ]);
    p["combat"]["heal"] = json!({
        "mechanism":"budget_heal_channel","id":"npc-heal","cost":10,"windup_ms":30,
        "cooldown_ms":300,"recovery_ms":0,"hp_threshold":1,"heal_bps":2000,"interval_ms":30,"ticks":2
    });
    p["combat"]["projectile"]["cost"] = json!(200);
    let mut alternative = p["combat"]["projectile"].clone();
    alternative["id"] = json!("alternate");
    alternative["cost"] = json!(3);
    p["npc_ai"]["groups"] = json!({"npc-1":"g1","ally-a":"g1","ally-b":"g1","cross-group":"g2"});
    p["npc_ai"]["profiles"]["npc"]["slots"] = json!([
        {"kind":"heal","chance":1,"range":60,"include_self":false},
        {"kind":"projectile","chance":1},
        {"kind":"projectile","chance":1,"ability":alternative}
    ]);
}

#[test]
fn healing_selects_lowest_same_group_then_cooldown_and_cost_fall_through() {
    let c = catalog(input(), support_input).unwrap().default_config();
    let mut w = world(&c);
    let healer = w.npcs.iter().position(|n| n.entity.id == "npc-1").unwrap();
    w.npcs[healer].hp = 2;
    w.npcs[0].hp = 80;
    w.npcs[1].hp = 50;
    w.npcs[2].hp = 1;
    w.tick(&c, 30);
    let heal = w.tick(&c, 60);
    assert!(heal.iter().any(|(_, e)| matches!(e, EventV4::NpcSkillCommitted { skill_id, target, .. } if skill_id == "npc-heal" && target.id == "ally-b")));
    assert!(heal.iter().any(|(_, e)| matches!(
        e,
        EventV4::NpcHealTick {
            amount: 10,
            hp: 60,
            ..
        }
    )));
    w.tick(&c, 90);
    assert_eq!(
        (w.npcs[0].hp, w.npcs[1].hp, w.npcs[2].hp, w.npcs[healer].hp),
        (80, 70, 1, 2)
    );
    w.tick(&c, 120);
    let attack = w.tick(&c, 150);
    assert!(attack.iter().any(
        |(_, e)| matches!(e, EventV4::NpcSkillCommitted { skill_id, .. } if skill_id == "alternate")
    ));
    assert_eq!(w.npcs[healer].spirit, 87);

    let mut healthy = world(&c);
    healthy.tick(&c, 30);
    assert!(healthy.tick(&c, 60).iter().any(|(_, e)| matches!(e,
        EventV4::NpcSkillCommitted { skill_id, .. } if skill_id == "alternate"
    )));
}

#[test]
fn ranged_retreat_precedes_cast_and_stops_at_configured_distance() {
    let c = catalog(input(), |v| {
        ai_input(v);
        v["pve"]["combat"]["melee_range"] = json!(20);
        v["pve"]["combat"]["aggro_range"] = json!(64);
        v["pve"]["npc_ai"]["profiles"]["npc"] =
            json!({"retreat_below":12,"slots":[{"kind":"projectile","chance":1}]});
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    w.members[0].position = [46., 20.];
    w.tick(&c, 30);
    assert!(w.npcs[0].position[0] > 52.);
    assert!(w.npcs[0].casting.is_none());
    for step in 2..=8 {
        w.tick(&c, step * 30);
    }
    assert!((distance(w.npcs[0].position, w.members[0].position) - 20.).abs() < 1e-7);
    w.tick(&c, 270);
    assert!(w.npcs[0].casting.is_some());
    w.protect(0, 270, &mut vec![]);
    let cancelled = w.tick(&c, 300);
    assert!(
        !cancelled
            .iter()
            .any(|(_, e)| matches!(e, EventV4::NpcSkillCommitted { .. }))
    );
    assert_eq!(w.npcs[0].spirit, 100);
}

#[test]
fn invalid_group_slot_and_force_references_are_rejected() {
    for (field, value, expected) in [
        ("groups", json!({"unknown":"g"}), "CONFIG_NPC_AI_GROUP"),
        (
            "forced_target_statuses",
            json!(["missing"]),
            "CONFIG_NPC_AI_FORCED_TARGET",
        ),
        (
            "profiles",
            json!({"npc":{"slots":[{"kind":"projectile","chance":1},{"kind":"projectile","chance":1}]}}),
            "CONFIG_NPC_AI_SLOT",
        ),
    ] {
        let error = catalog(input(), |v| {
            ai_input(v);
            v["pve"]["npc_ai"][field] = value;
        })
        .err()
        .unwrap();
        assert_eq!(error.to_string(), expected);
    }
}
