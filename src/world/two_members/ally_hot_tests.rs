use super::periodic_tests::{bind, pulse};
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_ally_hot_configuration_requires_one_source_scoped_refresh_hot() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-ally-hot-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let slot = doc["additional_skills"].as_array().unwrap().len() - 1;
    let definition = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    let config = load(&doc).unwrap().default_config();
    assert_eq!(
        serde_json::to_value(config.player_skill("ally-renew").unwrap()).unwrap(),
        doc["additional_skills"][slot]
    );

    let mut old = doc.clone();
    old["mechanism_version"] = json!(78);
    assert!(load(&old).is_err());
    old["additional_skills"][slot]
        .as_object_mut()
        .unwrap()
        .remove("hot_status");
    old["additional_skills"][slot]["heal"] = json!({"amount":100});
    assert!(load(&old).is_ok());

    for id in [json!(null), json!("missing"), json!("demo-guard")] {
        let mut invalid = doc.clone();
        invalid["additional_skills"][slot]["hot_status"] = id;
        assert!(load(&invalid).is_err());
    }
    let mut both = doc.clone();
    both["additional_skills"][slot]["heal"] = json!({"amount":100});
    assert!(load(&both).is_err());
    for (field, value) in [
        ("source_scoped", json!(false)),
        ("duplicate_policy", json!("ignore")),
        (
            "overheal_shield",
            json!({"status_id":"overflow-shield","conversion_bps":5000}),
        ),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][definition][field] = value;
        assert!(load(&invalid).is_err());
    }
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(4);
    c.mechanism_version = 79;
    c.additional_skills.push(
        serde_json::from_value(json!({
            "mechanism":"ally_heal", "id":"ally-renew", "cost":2, "windup_ms":120,
            "recovery_ms":0, "cooldown_ms":0, "range":100,
            "hp_cost":{"mode":"fixed","value":10,"min_hp_after_cost":1}, "hot_status":"ally-renewal"
        }))
        .unwrap(),
    );
    c.pve_mut().status_definitions.extend([
        serde_json::from_value(json!({
            "mechanism":"periodic_heal","id":"ally-renewal","heal_bps":510,"tick_interval_ms":600,
            "duration_ms":2500,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
        serde_json::from_value(json!({
            "mechanism":"timed_attribute_modifier","id":"care","attribute":"healing_received",
            "modifier_bps":5000,"duration_ms":5000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    ]);
    w.members[0].position = [400., 400.];
    w.members[1].position = [400., 440.];
    w.members[2].position = [420., 440.];
    w.members[1].facts.hp_points = 1000;
    w.members[1].hp = 800;
    (c, w)
}

fn prepare(w: &mut World, c: &Config, i: usize, now: u64) -> Result<Events> {
    let target = World::unit_wire(&w.members[1].reference.entity);
    w.action(
        i,
        &Action::CastUnit {
            skill_id: "ally-renew".into(),
            target,
        },
        "hot",
        c,
        now,
    )
}

fn commit(w: &mut World, c: &Config, i: usize, now: u64) -> Events {
    let pending = w.members[i].pending.take().unwrap();
    let mut events = vec![];
    w.commit(i, pending, c, now, &mut events);
    events
}

pub(super) fn cast(w: &mut World, c: &Config, i: usize, now: u64) -> Events {
    prepare(w, c, i, now).unwrap();
    commit(w, c, i, now + 120)
}

fn ticks(
    events: &Events,
) -> Vec<(
    usize,
    &StatusEntity,
    &StatusEntity,
    u32,
    PeriodicTickOutcome,
)> {
    events
        .iter()
        .filter_map(|(recipient, row)| match row {
            EventV4::Private {
                event:
                    Event::PeriodicHealTick {
                        definition_id,
                        source,
                        target,
                        hp_restored,
                        outcome,
                        ..
                    },
            } if definition_id == "ally-renewal" => {
                Some((recipient.unwrap(), source, target, *hp_restored, *outcome))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn member_ally_hot_commit_rechecks_identity_sight_range_and_target_capacity() {
    for case in 0..4 {
        let (mut c, mut w) = setup();
        prepare(&mut w, &c, 0, 0).unwrap();
        let spirit = w.members[0].spirit;
        let hp = w.members[0].hp;
        let reason = match case {
            0 => {
                w.members[1].reference.entity.generation += 1;
                Error::InvalidTarget
            }
            1 => {
                w.members[1].position = [400., 540.];
                Error::OutOfRange
            }
            2 => {
                Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
                Error::OutOfRange
            }
            _ => {
                c.pve_mut()
                    .status_rules
                    .as_mut()
                    .unwrap()
                    .max_active_per_entity = 1;
                let target = w.members[1].reference.entity.clone();
                bind(&mut w, &c, Owner::Member(1), target, "care", 0);
                Error::Capacity
            }
        };
        assert_eq!(
            commit(&mut w, &c, 0, 120),
            vec![(
                Some(0),
                EventV4::Private {
                    event: Event::ActionRejected {
                        action_id: "hot".into(),
                        reason
                    },
                }
            )]
        );
        assert_eq!((w.members[0].hp, w.members[0].spirit), (hp, spirit));
        assert!(w.members[0].cooldowns.is_empty());
        assert!(
            w.statuses
                .entries()
                .iter()
                .all(|s| s.definition_id != "ally-renewal")
        );
    }
}

#[test]
fn member_ally_hot_refreshes_by_source_and_uses_target_multiplier_per_tick() {
    let (mut c, mut w) = setup();
    let source0 = status_entity(&w.members[0].reference.entity);
    let source2 = status_entity(&w.members[2].reference.entity);
    let target = status_entity(&w.members[1].reference.entity);
    let initial = cast(&mut w, &c, 0, 0);
    assert!(matches!(initial.as_slice(), [
        (Some(0), EventV4::Private {event: Event::ActionCommitted { .. }}),
        (Some(0), EventV4::Private {event: Event::StatusApplied { status:a }}),
        (Some(1), EventV4::Private {event: Event::StatusApplied { status:b }}),
    ] if a == b && a.source == source0 && a.target == target));
    let instance = w.statuses.entries()[0].instance_id.clone();
    cast(&mut w, &c, 2, 30);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 2;
    let refreshed = cast(&mut w, &c, 0, 300);
    assert!(matches!(refreshed.as_slice(), [
        (Some(0), EventV4::Private {event: Event::ActionCommitted { .. }}),
        (Some(0), EventV4::Private {event: Event::StatusRefreshed { status:a }}),
        (Some(1), EventV4::Private {event: Event::StatusRefreshed { status:b }}),
    ] if a == b && a.instance_id == instance && a.applied_at_ms == 420));
    let own = |w: &World, i| w.snapshot(i, &c, Phase::Active, 420).own.statuses;
    assert_eq!(own(&w, 1).len(), 2);
    assert!(own(&w, 0).is_empty() && own(&w, 2).is_empty() && own(&w, 3).is_empty());
    assert!(ticks(&pulse(&mut w, &c, 749)).is_empty());

    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 3;
    bind(
        &mut w,
        &c,
        Owner::Member(1),
        World::status_reference(&target),
        "care",
        749,
    );
    w.members[2].position = [400., 600.];
    assert_eq!(
        ticks(&pulse(&mut w, &c, 750)),
        vec![
            (2, &source2, &target, 76, PeriodicTickOutcome::Applied),
            (1, &source2, &target, 76, PeriodicTickOutcome::Applied),
        ]
    );
    w.protect(0, 1019, &mut vec![]);
    assert_eq!(
        ticks(&pulse(&mut w, &c, 1020)),
        vec![
            (0, &source0, &target, 0, PeriodicTickOutcome::Protected),
            (1, &source0, &target, 0, PeriodicTickOutcome::Protected),
        ]
    );
    w.members[0].ready = true;
    w.members[0].protected = false;
    pulse(&mut w, &c, 1350);
    assert_eq!(
        ticks(&pulse(&mut w, &c, 1620)),
        vec![
            (0, &source0, &target, 48, PeriodicTickOutcome::Applied),
            (1, &source0, &target, 48, PeriodicTickOutcome::Applied),
        ]
    );
    assert_eq!(w.members[1].hp, 1000);
    assert!(w.shots.is_empty());
}

#[test]
fn member_ally_hot_leave_cleans_only_the_exiting_source_then_cleans_the_target() {
    let (c, mut w) = setup();
    cast(&mut w, &c, 0, 0);
    cast(&mut w, &c, 2, 0);
    let entries = w.statuses.entries().to_vec();
    let left = w
        .action(0, &Action::Leave {}, "leave-source", &c, 200)
        .unwrap();
    assert!(left.iter().any(|(recipient, row)| *recipient == Some(1) && matches!(row,
        EventV4::Private {event: Event::StatusRemoved {instance_ids, reason:StatusRemovalReason::SourceInvalid}}
        if instance_ids == &[entries[0].instance_id.clone()])));
    assert_eq!(w.statuses.entries(), &entries[1..]);
    assert_eq!(
        ticks(&pulse(&mut w, &c, 720))
            .iter()
            .map(|row| row.3)
            .collect::<Vec<_>>(),
        [51, 51]
    );
    let left = w
        .action(1, &Action::Leave {}, "leave-target", &c, 800)
        .unwrap();
    assert!(left.iter().any(|(recipient, row)| *recipient == Some(2) && matches!(row,
        EventV4::Private {event: Event::StatusRemoved {instance_ids, reason:StatusRemovalReason::TargetInvalid}}
        if instance_ids == &[entries[1].instance_id.clone()])));
    assert!(w.statuses.is_empty());
    assert!(ticks(&pulse(&mut w, &c, 1800)).is_empty());
}
