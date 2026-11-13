use super::combat::DamageIntent;
use super::periodic_tests::bind;
use super::*;
use crate::config::AttackRule;
use serde_json::{Value, json};

const BASIC: &str = "effect_lin_secret_stage_3";
const PERFECT: &str = "effect_lin_secret_stage_5";
const IMMUNITY: &str = "effect_lin_survival_window";

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(false, false);
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/source-lin-mi.json"
    ))
    .unwrap();
    let p = c.pve_mut();
    p.status_definitions.extend(
        serde_json::from_value::<Vec<crate::config::StatusDefinition>>(
            sample["status_definitions"].clone(),
        )
        .unwrap(),
    );
    p.status_rules
        .as_mut()
        .unwrap()
        .attribute_caps_bps
        .insert(StatusAttribute::DamageReduction, 7000);
    let r = c.combat_rules.as_mut().unwrap();
    r.status_death_guards =
        serde_json::from_value(sample["combat_rules"]["status_death_guards"].clone()).unwrap();
    r.status_policies =
        serde_json::from_value(sample["combat_rules"]["status_policies"].clone()).unwrap();
    r.status_ends = serde_json::from_value(sample["combat_rules"]["status_ends"].clone()).unwrap();
    for profile in r.profiles.values_mut() {
        profile.tier = 0;
    }
    for m in &mut w.members {
        m.facts.hp_points = 1000;
        m.hp = 100;
    }
    (c, w)
}

fn guard(w: &mut World, c: &Config, i: usize, id: &str, now: u64) {
    bind(
        w,
        c,
        Owner::Member(i),
        w.owner_entity(Owner::Member(i)),
        id,
        now,
    );
}

fn hit(
    w: &mut World,
    c: &Config,
    target: usize,
    raw: u32,
    now: u64,
) -> (periodic::DamageResult, Events) {
    let mut events = vec![];
    let result = w.resolve_damage_with_rule(
        DamageIntent {
            source: w.owner_entity(Owner::Npc(0)),
            target: w.owner_entity(Owner::Member(target)),
            raw,
            direct: true,
            ignore_armor: true,
        },
        Some(&AttackRule {
            can_crit: false,
            unavoidable: true,
            ignore_armor: true,
            ..Default::default()
        }),
        c,
        now,
        &mut events,
    );
    (result, events)
}

#[test]
fn stillness_guard_actual_damage_immunity_heal_and_shared_deadline() {
    let (c, mut w) = setup();
    guard(&mut w, &c, 0, PERFECT, 0);
    guard(&mut w, &c, 1, BASIC, 0);
    let (loss, events) = hit(&mut w, &c, 0, 1000, 30);
    assert_eq!(
        (loss.hp_damage, loss.lethal, w.members[0].hp),
        (99, false, 151)
    );
    assert!(events.iter().any(|(i, e)| *i == Some(0)
        && matches!(
            e,
            EventV4::Private {
                event: Event::DeathGuardActivated {
                    hp: 151,
                    prevented_hp_loss: 1,
                    ..
                }
            }
        )));
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == IMMUNITY && s.expires_at_ms == 2030)
    );
    assert_eq!(hit(&mut w, &c, 0, 1000, 2029).0.hp_damage, 0);
    assert_eq!(hit(&mut w, &c, 1, 1000, 2030).0.hp_damage, 99);
    assert_eq!(w.members[1].hp, 1);
    assert!(w.triggers.death_used.is_empty());
    assert_eq!(w.triggers.status_death_ready.len(), 2);

    guard(&mut w, &c, 0, BASIC, 118000);
    guard(&mut w, &c, 0, BASIC, 119000);
    assert!(
        w.plan_death_guard(Owner::Member(0), 1000, &c, 120029, &mut vec![])
            .is_none()
    );
    let (loss, _) = hit(&mut w, &c, 0, 1000, 120030);
    assert_eq!(
        (loss.hp_damage, loss.lethal, w.members[0].hp),
        (150, false, 1)
    );

    let member = w.owner_entity(Owner::Member(0));
    w.clear_owned_statuses(&member, StatusRemovalReason::SourceInvalid, &mut vec![]);
    assert!(
        !w.triggers
            .status_death_ready
            .contains_key(&triggers::entity_key(&member))
    );
    assert_eq!(w.triggers.status_death_ready.len(), 1);
}

#[test]
fn stillness_guard_capacity_absorption_and_carrier_expiry_do_not_start_deadline() {
    let (mut c, mut w) = setup();
    guard(&mut w, &c, 0, PERFECT, 0);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut events = vec![];
    assert!(
        w.plan_death_guard(Owner::Member(0), 1000, &c, 30, &mut events)
            .is_none()
    );
    assert!(events.iter().any(|(i, e)| *i == Some(0)
        && matches!(
            e,
            EventV4::Private {
                event: Event::CombatTriggerSuppressed {
                    reason: Error::Capacity,
                    ..
                }
            }
        )));
    assert!(w.triggers.status_death_ready.is_empty());
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 8;
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "id":"stillness-test-shield","mechanism":"absorption_shield","shield_bps":3000,
            "duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    guard(&mut w, &c, 0, "stillness-test-shield", 60);
    let (loss, _) = hit(&mut w, &c, 0, 600, 90);
    assert!(loss.absorbed >= 179 && loss.hp_damage == 0);
    assert!(w.triggers.status_death_ready.is_empty());
    assert!(
        w.plan_death_guard(Owner::Member(0), 1000, &c, 6000, &mut vec![])
            .is_none()
    );
    assert!(w.triggers.status_death_ready.is_empty());
}

#[test]
fn stillness_guard_requires_own_live_carrier_and_uses_legacy_when_group_busy() {
    let (mut c, mut w) = setup();
    let member = w.owner_entity(Owner::Member(0));
    bind(&mut w, &c, Owner::Member(1), member.clone(), BASIC, 0);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == BASIC)
    );
    assert!(
        w.plan_death_guard(Owner::Member(0), 1000, &c, 30, &mut vec![])
            .is_none()
    );
    guard(&mut w, &c, 0, BASIC, 60);
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "id":"stillness-test-silence","mechanism":"timed_control","attribute":"silence",
            "duration_ms":1000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    let control = bind(
        &mut w,
        &c,
        Owner::Npc(0),
        member,
        "stillness-test-silence",
        61,
    );
    assert!(control.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusRejected {
                reason: Error::Immune,
                ..
            }
        }
    )));
    assert!(
        w.plan_death_guard(Owner::Member(0), 1000, &c, 90, &mut vec![])
            .is_some()
    );
    let rules = c.combat_rules.as_mut().unwrap();
    rules.death_guards.insert(
        rules.default_member_profile.clone(),
        serde_json::from_value(json!({
            "retain_hp":5,"heal_max_hp_bps":0,"self_statuses":[]
        }))
        .unwrap(),
    );
    assert_eq!(
        w.plan_death_guard(Owner::Member(0), 1000, &c, 120, &mut vec![])
            .unwrap()
            .retain_hp,
        5
    );
    assert!(
        w.plan_death_guard(Owner::Member(0), 1000, &c, 150, &mut vec![])
            .is_none()
    );
}
