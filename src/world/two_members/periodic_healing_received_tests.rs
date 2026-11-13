use super::periodic_tests::{bind, pulse, setup};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_periodic_healing_received_configuration_gates_bindings_and_strength() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-periodic-healing-received-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let care = doc["pve"]["status_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|row| row["id"] == "pill-care")
        .unwrap();
    let mut skill = doc.clone();
    skill["additional_skills"][0]["on_commit_self_statuses"] = json!(["pill-care", "pill-hot"]);
    for allowed in [&doc, &skill] {
        pkg.write(allowed);
        assert!(pkg.load().is_ok());
    }
    for (path, value) in [
        ("/mechanism_version".to_owned(), json!(68)),
        ("/schema_version".to_owned(), json!(6)),
        (
            format!("/pve/status_definitions/{care}/modifier_bps"),
            json!(-10001),
        ),
        (
            format!("/pve/status_definitions/{care}/modifier_bps"),
            json!(100001),
        ),
        (
            format!("/pve/status_definitions/{care}/duplicate_policy"),
            json!("stack"),
        ),
        (
            "/skill/on_hit_target_statuses".to_owned(),
            json!(["pill-care"]),
        ),
        (
            "/pve/combat/projectile/on_commit_self_statuses".to_owned(),
            json!(["pill-care"]),
        ),
    ] {
        let mut invalid = doc.clone();
        *invalid.pointer_mut(&path).unwrap() = value;
        pkg.write(&invalid);
        assert!(pkg.load().is_err(), "{path}");
    }
}

fn member_hot(w: &mut World, c: &Config, member: usize, id: &str) {
    bind(
        w,
        c,
        Owner::Member(member),
        w.members[member].reference.entity.clone(),
        id,
        0,
    );
}

fn config_with_healing_modifiers(bps: [i32; 2]) -> (Config, World) {
    let (mut c, w) = setup(2);
    c.mechanism_version = 69;
    let defs = &mut c.pve_mut().status_definitions;
    defs.iter_mut()
        .find(|d| d.id == "renewal")
        .unwrap()
        .heal_bps = Some(510);
    for (id, bps) in ["care-a", "care-b"].into_iter().zip(bps) {
        defs.push(serde_json::from_value(json!({
            "id": id, "mechanism": "timed_attribute_modifier", "attribute": "healing_received",
            "modifier_bps": bps, "duration_ms": 1200, "duplicate_policy": "refresh", "source_scoped": true,
        })).unwrap());
    }
    (c, w)
}

fn heals(events: &Events) -> Vec<(usize, u64, u64, u32, u32, PeriodicTickOutcome)> {
    events
        .iter()
        .filter_map(|(recipient, e)| match e {
            EventV4::Private {
                event:
                    Event::PeriodicHealTick {
                        source,
                        target,
                        raw_heal,
                        hp_restored,
                        scheduled_at_ms,
                        executed_at_ms,
                        outcome,
                        ..
                    },
            } => {
                assert_eq!(source, target);
                let recipient = recipient.unwrap();
                assert_eq!(target.id, format!("P{recipient:02}"));
                Some((
                    recipient,
                    *scheduled_at_ms,
                    *executed_at_ms,
                    *raw_heal,
                    *hp_restored,
                    *outcome,
                ))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn member_periodic_healing_received_rounds_stacks_and_limits_the_actual_restore() {
    use PeriodicTickOutcome::Applied;
    for (bps, mechanism, first) in [
        ([4000, 1000], 69, 76),
        ([-4000, -1000], 69, 25),
        ([-5000, -6000], 69, 0),
        ([4000, 1000], 68, 51),
    ] {
        let (mut c, mut w) = config_with_healing_modifiers(bps);
        c.mechanism_version = mechanism;
        w.members[0].hp = 800;
        w.members[1].hp = 900;
        for id in ["renewal", "care-a", "care-b"] {
            member_hot(&mut w, &c, 0, id);
        }
        member_hot(&mut w, &c, 1, "renewal");
        assert_eq!(
            heals(&pulse(&mut w, &c, 450)),
            vec![
                (0, 450, 450, 51, first, Applied),
                (1, 450, 450, 56, 56, Applied)
            ]
        );
        assert_eq!(w.members[0].hp, 800 + first);
        assert_eq!(w.members[1].hp, 956);
        w.members[0].hp = 990;
        assert_eq!(heals(&pulse(&mut w, &c, 900))[0].4, first.min(10));
        w.members[0].hp = 1000;
        assert_eq!(heals(&pulse(&mut w, &c, 1350))[0].4, 0);
    }

    let (mut c, mut w) = config_with_healing_modifiers([100000, 100000]);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "renewal")
        .unwrap()
        .heal_bps = Some(10000);
    w.members[0].facts.hp_points = u32::MAX;
    w.members[0].hp = 1;
    for id in ["renewal", "care-a", "care-b"] {
        member_hot(&mut w, &c, 0, id);
    }
    assert_eq!(
        heals(&pulse(&mut w, &c, 450)),
        vec![(0, 450, 450, u32::MAX, u32::MAX - 1, Applied)]
    );
    assert_eq!(w.members[0].hp, u32::MAX);
}

#[test]
fn member_periodic_healing_received_checks_expiry_at_execution_and_protection() {
    let (c, mut w) = config_with_healing_modifiers([5000, 0]);
    w.members[0].hp = 800;
    for id in ["renewal", "care-a"] {
        member_hot(&mut w, &c, 0, id);
    }
    w.protect(0, 0, &mut vec![]);
    assert_eq!(
        heals(&pulse(&mut w, &c, 450)),
        vec![(0, 450, 450, 51, 0, PeriodicTickOutcome::Protected)]
    );
    assert_eq!(w.members[0].hp, 800);
    w.members[0].foreground = true;
    w.members[0].ready = true;
    w.members[0].protected = false;
    assert_eq!(
        heals(&pulse(&mut w, &c, 1200)),
        vec![(0, 900, 1200, 51, 51, PeriodicTickOutcome::Applied)]
    );
    assert_eq!(w.members[0].hp, 851);
}

#[test]
fn member_periodic_healing_received_cannot_revive_after_same_time_dot() {
    let (mut c, mut w) = config_with_healing_modifiers([5000, 0]);
    c.pve_mut()
        .status_definitions
        .iter_mut()
        .find(|d| d.id == "wound")
        .unwrap()
        .tick_interval_ms = Some(450);
    w.members[0].hp = 1;
    member_hot(&mut w, &c, 0, "renewal");
    member_hot(&mut w, &c, 0, "care-a");
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target, "wound", 0);
    let events = pulse(&mut w, &c, 450);
    assert_eq!(w.members[0].hp, 0);
    assert!(heals(&events).is_empty());
    assert!(w.statuses.entries().is_empty());
}
