//! Observe completed native action journals and bounded owned stack leaves.
use super::*;
use serde_json::{Value, json};

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(true, false);
    w.statuses = Statuses::default();
    w.npcs.truncate(1);
    let n = &mut w.npcs[0];
    n.home = n.position;
    let profile = Arc::make_mut(&mut n.profile);
    profile.combat.aggro_range = 1.;
    profile.combat.melee_range = 1.;
    profile.combat.cast_chance = 0.;
    profile.combat.heal.hp_threshold = 0.;
    let skill = c.skill.id.clone();
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({
        "id":"end-seed","mechanism":"timed_compound_modifier","duration_ms":12000,
        "duplicate_policy":"refresh","source_scoped":true,"base_modifiers":[{"attribute":"def","modifier_bps":0}],
        "stack_counter":{"initial":2,"maximum":3},
    })).unwrap());
    let r = c.combat_rules.as_mut().unwrap();
    let attack = r.skills.get_mut(&skill).unwrap();
    attack.base_power = Some(90);
    attack.damage_kind = crate::config::DamageKind::TrueDamage;
    r.status_policies.insert(
        "end-seed".into(),
        serde_json::from_value(json!({
            "disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]
        }))
        .unwrap(),
    );
    let profile = r.default_member_profile.clone();
    r.triggers=serde_json::from_value(json!([
        {"id":"at-end","profiles":[profile],"when":"damage_dealt","include_periodic":false,"every":1,
         "cooldown_ms":0,"priority":1,"counting":"action_total",
         "action_filter":{"basic":true,"skills":{"mode":"except","ids":[]}},
         "effect":{"kind":"add_status_stacks","subject":"owner","status_id":"end-seed","amount":1}},
        {"id":"reached","profiles":[profile],"when":"status_stacks_reached","include_periodic":false,"every":1,
         "cooldown_ms":0,"priority":1,"action_filter":{"basic":false,"skills":{"mode":"only","ids":[skill]}},
         "stack_change":{"definition_id":"end-seed","at_least":3},"effect":{"kind":"restore_spirit","amount":7}},
    ])).unwrap();
    (c, w)
}
fn status(w: &World) -> Option<&StatusView> {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "end-seed")
}
fn bind(w: &mut World, c: &Config) {
    let own = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(w, c, Owner::Member(0), own, "end-seed", 0);
}
fn start(w: &mut World, c: &Config) {
    let target = World::unit_wire(&w.npcs[0].entity);
    w.action(
        0,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target,
        },
        "one-three-contact-action",
        c,
        0,
    )
    .unwrap();
}
fn totals(e: &Events) -> Vec<(String, u64, u32, bool)> {
    e.iter()
        .filter_map(|(i, e)| match e {
            EventV4::Private {
                event:
                    Event::CombatActionTotal {
                        action_id,
                        total_amount,
                        contacts,
                        fired,
                        ..
                    },
            } if *i == Some(0) => Some((action_id.clone(), *total_amount, *contacts, *fired)),
            _ => None,
        })
        .collect()
}
#[test]
fn terminal_counter_adds_one_layer_only_after_the_third_native_pulse_and_crosses_once() {
    let (c, mut w) = setup();
    bind(&mut w, &c);
    start(&mut w, &c);
    let original = status(&w).unwrap().clone();
    let mut events = vec![];
    for at in (30..=210).step_by(30) {
        events.extend(w.tick(&c, at));
        assert_eq!(status(&w), Some(&original));
    }
    assert!(totals(&events).is_empty());
    assert!(w.npcs[0].hp < 1000);
    assert_eq!(w.triggers.totals.pending_len(), 1);
    let action_id = events
        .iter()
        .find_map(|(_, e)| match e {
            EventV4::Private {
                event: Event::ActionCommitted { action_id, .. },
            } => Some(action_id.clone()),
            _ => None,
        })
        .unwrap();
    let end = w.tick(&c, 240);
    let pulses = events
        .iter()
        .chain(&end)
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event:
                    Event::UnitChannelPulse {
                        pulse, raw_damage, ..
                    },
            } => Some((*pulse, *raw_damage)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(pulses, [(1, 30), (2, 30), (3, 30)]);
    let after = status(&w).unwrap();
    assert_eq!(after.stacks, Some(3));
    assert_eq!(after.instance_id, original.instance_id);
    assert_eq!(after.expires_at_ms, original.expires_at_ms);
    assert_eq!(
        totals(&end),
        vec![(action_id, u64::from(1000 - w.npcs[0].hp), 3, true)]
    );
    assert_eq!(end.iter().filter(|(_,e)|matches!(e,EventV4::Private{event:Event::CombatTriggerFired{trigger_id,..}}if trigger_id=="reached")).count(),1);
    assert_eq!(w.members[0].spirit, 52); // 50 − initial 2 − 3 pulses + 7
    assert!(totals(&w.tick(&c, 270)).is_empty());
    assert_eq!(w.members[0].spirit, 52);
}
#[test]
fn terminal_counter_excluded_absent_zero_and_exit_do_not_fabricate_a_layer() {
    for probe in 0..4 {
        let (mut c, mut w) = setup();
        if probe != 1 {
            bind(&mut w, &c);
        }
        if probe == 0 {
            c.combat_rules.as_mut().unwrap().triggers[0].action_filter = Some(
                serde_json::from_value(
                    json!({"basic":true,"skills":{"mode":"except","ids":[c.skill.id]}}),
                )
                .unwrap(),
            );
        }
        if probe == 2 {
            c.pve_mut().status_definitions.push(
                serde_json::from_value(json!({
                    "id":"immunity","mechanism":"timed_control","attribute":"damage_immunity",
                    "duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true,
                }))
                .unwrap(),
            );
            let target = w.npcs[0].entity.clone();
            super::periodic_tests::bind(&mut w, &c, Owner::Npc(0), target, "immunity", 0);
        }
        start(&mut w, &c);
        let mut events = vec![];
        for at in (30..=270).step_by(30) {
            events.extend(w.tick(&c, at));
            if probe == 3 && at == 120 {
                events.extend(
                    w.action(0, &Action::Leave {}, "real-source-exit", &c, at)
                        .unwrap(),
                );
            }
        }
        assert_eq!(
            status(&w).and_then(|s| s.stacks),
            if probe == 0 || probe == 2 {
                Some(2)
            } else {
                None
            },
            "probe {probe}"
        );
        assert!(totals(&events).iter().all(|(_, _, _, fired)| !*fired));
        assert!(!events.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::CombatTriggerFired{trigger_id,..}}if trigger_id=="reached")));
    }
}
#[test]
fn terminal_counter_cold_validation_keeps_one_positive_owned_direct_form() {
    use crate::asset_contract::tests::Package;
    let p = Package::new();
    let mut config: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/counter-a.json"
    ))
    .unwrap();
    config["combat_rules"]["triggers"][0]["counting"] = json!("action_total");
    config["combat_rules"]["triggers"][0]["counter_scope"] = json!("owner");
    p.write(&config);
    p.load().unwrap();
    for (pointer, value) in [
        ("/when", json!("damage_taken")),
        ("/when", json!("shield_absorbed")),
        ("/include_periodic", json!(true)),
        ("/counter_scope", json!("target")),
        ("/effect/subject", json!("other")),
    ] {
        let mut invalid = config.clone();
        *invalid
            .pointer_mut(&format!("/combat_rules/triggers/0{pointer}"))
            .unwrap_or_else(|| panic!("missing {pointer}")) = value;
        p.write(&invalid);
        assert!(p.load().is_err(), "{pointer}");
    }
}
