use super::combat::DamageIntent;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_ally_shield_config_requires_source_scoped_refresh_absorption() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-ally-shield-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    let slot = doc["additional_skills"].as_array().unwrap().len() - 1;
    let def = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    let c = load(&doc).unwrap().default_config();
    assert_eq!(
        serde_json::to_value(c.player_skill("ally-guard").unwrap()).unwrap(),
        doc["additional_skills"][slot]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(79);
    assert!(load(&old).is_err());
    for (field, value) in [
        ("shield_status", json!("demo-guard")),
        ("shield_status", json!(null)),
        ("heal", json!({"amount":100})),
        ("range", json!(0)),
    ] {
        let mut invalid = doc.clone();
        invalid["additional_skills"][slot][field] = value;
        assert!(load(&invalid).is_err());
    }
    for (field, value) in [
        ("duplicate_policy", json!("ignore")),
        ("source_scoped", json!(false)),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][def][field] = value;
        assert!(load(&invalid).is_err());
    }
    let mut primary = doc;
    primary["skill"] = primary["additional_skills"]
        .as_array_mut()
        .unwrap()
        .pop()
        .unwrap();
    assert!(load(&primary).is_ok());
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(4);
    c.mechanism_version = 80;
    c.additional_skills.push(
        serde_json::from_value(json!({
            "mechanism":"ally_shield","id":"ally-guard","shield_status":"ally-barrier",
            "cost":2,"windup_ms":120,"recovery_ms":0,"cooldown_ms":100,"range":100,
            "hp_cost":{"mode":"fixed","value":10,"min_hp_after_cost":1}
        }))
        .unwrap(),
    );
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"absorption_shield","id":"ally-barrier","shield_bps":2000,
            "duration_ms":7000,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    c.pve_mut().status_rules.as_mut().unwrap().shield_cap_bps = Some(3000);
    w.members[0].position = [400., 400.];
    w.members[1].position = [400., 440.];
    w.members[2].position = [420., 440.];
    w.members[1].facts.hp_points = 1000;
    w.members[1].facts.def_points = 0;
    w.members[1].hp = 1000;
    (c, w)
}

fn prepare(w: &mut World, c: &Config, i: usize, target: UnitTarget, now: u64) -> Result<Events> {
    w.action(
        i,
        &Action::CastUnit {
            skill_id: "ally-guard".into(),
            target,
        },
        "guard",
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
fn cast(w: &mut World, c: &Config, i: usize, now: u64) -> Events {
    let target = World::unit_wire(&w.members[1].reference.entity);
    prepare(w, c, i, target, now).unwrap();
    commit(w, c, i, now + 120)
}

#[test]
fn member_ally_shield_revalidates_target_and_capacity_before_charging() {
    for case in 0..3 {
        let (mut c, mut w) = setup();
        let own = World::unit_wire(&w.members[0].reference.entity);
        assert_eq!(prepare(&mut w, &c, 0, own, 0), Err(Error::InvalidTarget));
        let target = w.members[1].reference.entity.clone();
        let (hp, spirit) = (w.members[0].hp, w.members[0].spirit);
        prepare(&mut w, &c, 0, World::unit_wire(&target), 0).unwrap();
        let reason = match case {
            0 => {
                w.members[1].reference.entity.generation += 1;
                Error::InvalidTarget
            }
            1 => {
                Arc::make_mut(&mut w.grid.config).palette[0].blocks_vision = true;
                Error::OutOfRange
            }
            _ => {
                c.pve_mut()
                    .status_rules
                    .as_mut()
                    .unwrap()
                    .max_active_per_entity = 1;
                bind(&mut w, &c, Owner::Member(1), target, "ward", 0);
                Error::Capacity
            }
        };
        assert_eq!(
            commit(&mut w, &c, 0, 120),
            vec![(
                Some(0),
                EventV4::Private {
                    event: Event::ActionRejected {
                        action_id: "guard".into(),
                        reason
                    }
                }
            )]
        );
        assert_eq!((w.members[0].hp, w.members[0].spirit), (hp, spirit));
        assert!(w.members[0].cooldowns.is_empty());
        assert!(
            w.statuses
                .entries()
                .iter()
                .all(|s| s.definition_id != "ally-barrier")
        );
    }
}

fn hit(w: &mut World, c: &Config, raw: u32, now: u64) -> Events {
    let hit = DamageIntent {
        ignore_armor: false,
        source: w.npcs[0].entity.clone(),
        target: w.members[1].reference.entity.clone(),
        raw,
        direct: true,
    };
    let mut events = vec![];
    w.resolve_damage(hit, c, now, &mut events);
    events
}

#[test]
fn member_ally_shield_caps_each_source_and_absorbs_before_health() {
    let (c, mut w) = setup();
    let first = cast(&mut w, &c, 0, 0);
    let second = cast(&mut w, &c, 2, 30);
    for (rows, caster, granted) in [(&first, 0, 200), (&second, 2, 100)] {
        let grant: Vec<_> = rows
            .iter()
            .filter_map(|(recipient, row)| match row {
                EventV4::Private {
                    event:
                        Event::ShieldGranted {
                            source,
                            target,
                            requested,
                            granted,
                            remaining,
                            ..
                        },
                } => Some((*recipient, source, target, *requested, *granted, *remaining)),
                _ => None,
            })
            .collect();
        let source = status_entity(&w.members[caster].reference.entity);
        let target = status_entity(&w.members[1].reference.entity);
        assert_eq!(
            grant,
            [
                (Some(caster), &source, &target, 200, granted, granted),
                (Some(1), &source, &target, 200, granted, granted)
            ]
        );
    }
    let own = |w: &World, i, now| w.snapshot(i, &c, Phase::Active, now).own;
    assert_eq!(own(&w, 1, 150).shield_total, Some(300));
    assert_eq!(own(&w, 0, 150).shield_total, Some(0));
    assert_eq!(own(&w, 3, 150).shield_total, Some(0));
    let ids = w
        .statuses
        .entries()
        .iter()
        .map(|s| s.instance_id.clone())
        .collect::<Vec<_>>();
    let damage = hit(&mut w, &c, 260, 200);
    let absorbed = damage
        .iter()
        .filter_map(|(recipient, row)| match row {
            EventV4::Private {
                event:
                    Event::ShieldAbsorbed {
                        amount,
                        consumptions,
                        ..
                    },
            } => Some((*recipient, *amount, consumptions)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(absorbed.len(), 1);
    let (recipient, amount, slices) = absorbed[0];
    assert_eq!((recipient, amount), (Some(1), 260));
    assert_eq!(
        slices
            .iter()
            .map(|slice| (&slice.instance_id, slice.absorbed))
            .collect::<Vec<_>>(),
        [(&ids[0], 200), (&ids[1], 60)]
    );
    assert_eq!(w.members[1].hp, 1000);
    assert_eq!(own(&w, 1, 200).shield_total, Some(40));
    cast(&mut w, &c, 2, 300);
    assert_eq!(w.statuses.entries()[0].instance_id, ids[1]);
    assert_eq!(own(&w, 1, 420).shield_total, Some(200));
    hit(&mut w, &c, 250, 450);
    assert_eq!(w.members[1].hp, 950);
    assert_eq!(own(&w, 1, 450).shield_total, Some(0));
}

#[test]
fn member_ally_shield_removes_only_the_affected_source_or_target_on_exit() {
    let (c, mut w) = setup();
    cast(&mut w, &c, 0, 0);
    cast(&mut w, &c, 2, 30);
    let ids = w
        .statuses
        .entries()
        .iter()
        .map(|s| s.instance_id.clone())
        .collect::<Vec<_>>();
    let removed = |rows: &Events, observer, id: &str, expected| {
        rows.iter().any(|(recipient, row)| {
            *recipient == Some(observer)
                && matches!(row, EventV4::Private {
            event:Event::StatusRemoved {instance_ids,reason}
        } if *reason==expected && instance_ids.len()==1 && instance_ids[0]==id)
        })
    };
    let left = w.action(0, &Action::Leave {}, "leave", &c, 200).unwrap();
    assert!(removed(
        &left,
        1,
        &ids[0],
        StatusRemovalReason::SourceInvalid
    ));
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(w.statuses.entries()[0].instance_id, ids[1]);
    let left = w.action(1, &Action::Leave {}, "leave", &c, 250).unwrap();
    assert!(removed(
        &left,
        2,
        &ids[1],
        StatusRemovalReason::TargetInvalid
    ));
    assert!(w.statuses.is_empty());
}
