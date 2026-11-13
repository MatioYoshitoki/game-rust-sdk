use super::*;
use crate::config::LifeStealTiming;

fn action_end_setup() -> (Config, World) {
    let (mut c, mut w) = setup();
    let rules = c.combat_rules.as_mut().unwrap();
    rules.triggers.clear();
    for profile in rules.profiles.values_mut() {
        profile.power_points_bps = 0;
    }
    rules.basic_attack = AttackRule {
        damage_kind: crate::config::DamageKind::TrueDamage,
        life_steal_bps: 5000,
        life_steal_timing: LifeStealTiming::ActionEnd,
        unavoidable: true,
        can_crit: false,
        ..Default::default()
    };
    w.members[0].hp = 100;
    w.members[1].hp = 100;
    (c, w)
}

fn hit(
    w: &mut World,
    c: &Config,
    (member, npc, id): (usize, usize, Option<&str>),
    raw: u32,
    now: u64,
    events: &mut Events,
) {
    let source = w.members[member].reference.entity.clone();
    assert!(w.direct_contact(
        Owner::Member(member),
        Owner::Npc(npc),
        DirectContact {
            origin: id.map(|_| crate::origin::ActionOrigin::Basic),
            action_id: id,
            rule: Some(&c.combat_rules.as_ref().unwrap().basic_attack),
            ignore_armor: true,
            unavoidable: true,
            dead_source: false,
            source: &source,
            raw,
            metal: true,
            hit_statuses: &[],
        },
        c,
        now,
        events,
    ));
}

fn heals(events: &Events) -> Vec<(usize, u32, u32)> {
    events
        .iter()
        .filter_map(|(recipient, e)| match e {
            EventV4::Private {
                event:
                    Event::HealResolved {
                        heal_kind: HealKind::LifeSteal,
                        raw_heal,
                        hp_restored,
                        ..
                    },
            } => Some((recipient.unwrap(), *raw_heal, *hp_restored)),
            _ => None,
        })
        .collect()
}

#[test]
fn action_life_steal_cold_loads_only_supported_rules() {
    let package = Package::new();
    let mut v = document();
    v["combat_rules"]["basic_attack"]["life_steal_timing"] = json!("action_end");
    v["combat_rules"]["skills"]["arc-bolt"]["life_steal_timing"] = json!("action_end");
    package.write(&v);
    let normalized = serde_json::to_value(package.load().unwrap().default_config()).unwrap();
    assert_eq!(
        normalized["combat_rules"]["basic_attack"]["life_steal_timing"],
        "action_end"
    );
    assert_eq!(
        normalized["combat_rules"]["skills"]["arc-bolt"]["life_steal_timing"],
        "action_end"
    );
    for (pointer, value) in [
        ("/combat_rules/basic_attack", json!(null)),
        ("/combat_rules/basic_attack", json!("after_attack")),
        ("/combat_rules/basic_attack", json!(0)),
        (
            "/combat_rules/npc_attacks/power-target",
            json!("action_end"),
        ),
        ("/combat_rules/periodic_damage/wound", json!("action_end")),
    ] {
        let mut bad = v.clone();
        bad.pointer_mut(pointer).unwrap()["life_steal_timing"] = value;
        package.write(&bad);
        assert!(package.load().is_err(), "{pointer}");
    }
    v["combat_rules"]["basic_attack"]["life_steal_timing"] = json!("contact");
    package.write(&v);
    assert_eq!(
        package
            .load()
            .unwrap()
            .default_config()
            .combat_rules
            .as_ref()
            .unwrap()
            .basic_attack
            .life_steal_timing,
        LifeStealTiming::Contact
    );
}

#[test]
fn action_life_steal_counts_only_actual_hp_and_keeps_owners_separate() {
    let (c, mut w) = action_end_setup();
    add_total_npc(&mut w);
    let target = w.npcs[0].entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target, "ward", 0);
    let mut events = vec![];
    let hp = w.npcs[0].hp;
    hit(&mut w, &c, (0, 0, Some("same")), 1, 30, &mut events);
    assert_eq!(w.npcs[0].hp, hp);
    assert_eq!(w.triggers.totals.pending_len(), 0);
    w.statuses
        .remove(StatusRemovalReason::Cleansed, |_| true, &mut vec![]);
    w.npcs[0].hp = 3;
    hit(&mut w, &c, (0, 0, Some("same")), 30, 60, &mut events);
    hit(&mut w, &c, (0, 1, Some("same")), 5, 60, &mut events);
    hit(&mut w, &c, (1, 1, Some("same")), 6, 60, &mut events);
    assert_eq!(w.npcs[0].hp, 0);
    assert!(heals(&events).is_empty());
    w.finish_trigger_actions(&c, 60, &mut events);
    w.finish_trigger_actions(&c, 90, &mut events);
    assert_eq!(heals(&events), [(0, 4, 4), (1, 3, 3)]);
    assert_eq!((w.members[0].hp, w.members[1].hp), (104, 103));
}

#[test]
fn action_life_steal_uses_the_normal_basic_projectile_and_ignores_empty_attacks() {
    let (c, mut w) = action_end_setup();
    commit(&mut w, &c, "basic");
    let hp = w.npcs[0].hp;
    let mut events = vec![];
    w.advance_shots(&c, 60, 1., &mut events);
    let damage = hp - w.npcs[0].hp;
    assert!(damage > 0);
    assert!(heals(&events).is_empty());
    w.finish_trigger_actions(&c, 60, &mut events);
    assert_eq!(heals(&events), [(0, damage / 2, damage / 2)]);
}

#[test]
fn action_life_steal_samples_live_bonus_and_received_at_the_end() {
    let (mut c, mut w) = action_end_setup();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({
        "id":"late-bonus","mechanism":"timed_compound_modifier","duration_ms":300,
        "duplicate_policy":"refresh","source_scoped":true,
        "modifiers":[{"attribute":"life_steal","modifier_bps":2500},{"attribute":"healing_received","modifier_bps":5000}]
    })).unwrap());
    let mut events = vec![];
    hit(&mut w, &c, (0, 0, Some("late")), 10, 30, &mut events);
    let source = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), source, "late-bonus", 60);
    hit(&mut w, &c, (0, 0, Some("late")), 10, 60, &mut events);
    w.finish_trigger_actions(&c, 60, &mut events);
    assert_eq!(heals(&events), [(0, 15, 22)]);
    assert_eq!(w.members[0].hp, 122);
}

#[test]
fn action_life_steal_discards_a_budget_rejected_action_without_paying_partial_damage() {
    let (mut c, mut w) = action_end_setup();
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    c.combat_rules.as_mut().unwrap().max_pending_action_totals = 1;
    w.members[0].spirit = 100;
    let before = w.npcs[0].hp;
    let mut events = vec![];
    hit(&mut w, &c, (0, 0, Some("rejected")), 10, 30, &mut events);
    assert_eq!(w.triggers.totals.pending_len(), 0);
    // Make a slot available. The original action is still ineligible for any partial recovery.
    c.combat_rules.as_mut().unwrap().triggers.clear();
    hit(&mut w, &c, (0, 0, Some("rejected")), 10, 60, &mut events);
    w.finish_trigger_actions(&c, 60, &mut events);
    assert_eq!(w.npcs[0].hp, before - 20);
    assert_eq!(w.members[0].spirit, 100);
    assert!(total_rows(&events).is_empty());
    assert!(heals(&events).is_empty());
    hit(&mut w, &c, (0, 0, Some("new")), 10, 90, &mut events);
    w.finish_trigger_actions(&c, 90, &mut events);
    assert_eq!(heals(&events), [(0, 5, 5)]);
}

#[test]
fn action_life_steal_waits_for_channel_shots_after_interrupt() {
    let (mut c, mut w) = action_end_setup();
    let mut projectile = skill("projectile");
    projectile["channel"] = json!({"duration_ms":390,"interval_ms":120,"spirit_per_pulse":1});
    c.skill = serde_json::from_value(projectile).unwrap();
    let rules = c.combat_rules.as_mut().unwrap();
    rules.skills.insert(
        c.skill.id.clone(),
        AttackRule {
            base_power: Some(10),
            ..rules.basic_attack.clone()
        },
    );
    commit(&mut w, &c, "projectile");
    let mut events = vec![];
    w.advance_channel(0, &c, 150, &mut events);
    w.advance_shots(&c, 180, 1., &mut events);
    w.finish_trigger_actions(&c, 180, &mut events);
    assert_eq!(w.triggers.totals.pending_len(), 1);
    assert!(heals(&events).is_empty());
    w.advance_channel(0, &c, 270, &mut events);
    w.finish_channel(0, ChannelEndReason::Cancelled, 270, &mut events);
    w.finish_trigger_actions(&c, 270, &mut events);
    assert!(heals(&events).is_empty());
    w.advance_shots(&c, 300, 1., &mut events);
    w.finish_trigger_actions(&c, 300, &mut events);
    assert_eq!(heals(&events), [(0, 10, 10)]);
    assert_eq!(w.triggers.totals.pending_len(), 0);
}

#[test]
fn action_life_steal_drops_protected_and_exiting_sources_even_with_committed_damage() {
    for reason in ["protect", "leave", "death"] {
        let (mut c, mut w) = action_end_setup();
        c.skill = serde_json::from_value(skill("ground_field")).unwrap();
        let rules = c.combat_rules.as_mut().unwrap();
        rules.skills.insert(
            c.skill.id.clone(),
            AttackRule {
                base_power: Some(10),
                ..rules.basic_attack.clone()
            },
        );
        commit(&mut w, &c, "ground_field");
        let mut events = vec![];
        w.advance_areas(&c, 150, &mut events);
        assert_eq!(w.triggers.totals.pending_len(), 1);
        match reason {
            "protect" => {
                w.protect(0, 150, &mut events);
                w.members[0].protected = false;
                w.members[0].ready = true;
                w.members[0].foreground = true;
            }
            "leave" => {
                w.action(0, &Action::Leave {}, "leave", &c, 150).unwrap();
            }
            _ => {
                w.members[0].hp = 0;
                w.terminal_intent(0, Outcome::Death, &mut events);
            }
        }
        assert_eq!(w.triggers.totals.pending_len(), 0);
        let before = w.npcs[0].hp;
        w.advance_areas(&c, 270, &mut events);
        w.finish_trigger_actions(&c, 270, &mut events);
        assert_eq!(
            w.npcs[0].hp,
            before - if reason == "protect" { 10 } else { 0 }
        );
        assert!(heals(&events).is_empty());
        assert_eq!(w.triggers.totals.pending_len(), 0);
    }
}

#[test]
fn action_life_steal_ignores_actionless_and_periodic_damage_even_if_other_totals_run() {
    let (mut c, mut w) = action_end_setup();
    total_trigger(&mut c, "owner", json!({"kind":"restore_spirit","amount":7}));
    let mut events = vec![];
    hit(&mut w, &c, (0, 0, None), 10, 30, &mut events);
    w.resolve_damage(intent(&w, 10, false), &c, 30, &mut events);
    w.finish_trigger_actions(&c, 30, &mut events);
    assert_eq!(total_rows(&events), [(10, 1, true)]);
    assert!(heals(&events).is_empty());
    assert_eq!(w.members[0].hp, 100);
}
