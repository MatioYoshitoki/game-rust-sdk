use super::combat::DirectContact;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_npc_silence_validates_control_policy_and_exclusive_binding() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-npc-silence-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    let def = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    let catalog = load(&doc).unwrap();
    let c = catalog.default_config();
    assert_eq!(
        serde_json::to_value(c.pve.as_ref().unwrap().status_definitions.last().unwrap()).unwrap(),
        doc["pve"]["status_definitions"][def]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(82);
    assert!(load(&old).is_err());
    for (key, value) in [
        ("modifier_bps", json!(0)),
        ("source_scoped", json!(false)),
        ("duplicate_policy", json!("ignore")),
        ("mechanism", json!("timed_attribute_modifier")),
        ("attribute", json!("atk")),
    ] {
        let mut invalid = doc.clone();
        invalid["pve"]["status_definitions"][def][key] = value;
        assert!(load(&invalid).is_err());
    }
    for binding in ["on_commit_self_statuses", "on_hit_target_statuses"] {
        let mut invalid = doc.clone();
        invalid["skill"][binding] = json!(["npc-silence"]);
        assert!(load(&invalid).is_err());
    }
    let mut item = doc;
    let id = item["item_id"].as_str().unwrap().to_owned();
    item["member_item_use"] = json!({id:{"heal_hp":10,"on_use_self_statuses":["npc-silence"]}});
    assert!(load(&item).is_err());
}

pub(super) fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(2);
    c.mechanism_version = 83;
    c.skill = serde_json::from_value(json!({
        "mechanism":"projectile","id":"cast","cost":5,"windup_ms":60,
        "recovery_ms":200,"cooldown_ms":300,"damage_bps":10000,"range":100,"speed":200,"radius":4
    }))
    .unwrap();
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "mechanism":"timed_control","attribute":"silence","id":"npc-silence",
            "duration_ms":600,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    w.members[0].position = [400., 400.];
    (c, w)
}
fn cast(w: &mut World, c: &Config, i: usize, skill: &str, now: u64) -> Result<Events> {
    w.action(
        i,
        &Action::Cast {
            skill_id: skill.into(),
            direction: [1., 0.],
        },
        "cast",
        c,
        now,
    )
}
fn commit(w: &mut World, c: &Config, now: u64) -> Events {
    let pending = w.members[0].pending.take().unwrap();
    let mut events = vec![];
    w.commit(0, pending, c, now, &mut events);
    events
}
fn hit(w: &mut World, c: &Config, now: u64) -> Events {
    let source = w.npcs[0].entity.clone();
    let mut events = vec![];
    w.direct_contact(
        Owner::Npc(0),
        Owner::Member(0),
        DirectContact {
            origin: None,
            action_id: None,
            rule: None,
            ignore_armor: false,
            unavoidable: false,
            dead_source: false,
            source: &source,
            raw: 1,
            metal: true,
            hit_statuses: &["npc-silence".into()],
        },
        c,
        now,
        &mut events,
    );
    events
}
fn private_events(events: &Events) -> Vec<(usize, &Event)> {
    events
        .iter()
        .filter_map(|(recipient, row)| match row {
            EventV4::Private { event } => Some((recipient.unwrap(), event)),
            _ => None,
        })
        .collect()
}

#[test]
fn member_npc_silence_interrupts_windup_and_blocks_only_skill_inputs_until_expiry() {
    let (c, mut w) = setup();
    let spirit = w.members[0].spirit;
    cast(&mut w, &c, 0, "cast", 0).unwrap();
    let events = hit(&mut w, &c, 30);
    assert!(matches!(private_events(&events).as_slice(),[
        (0,Event::StatusApplied {status}), (0,Event::ActionInterrupted),
    ] if status.source == status_entity(&w.npcs[0].entity) && status.attribute==Some(StatusAttribute::Silence)));
    assert!(w.members[0].pending.is_none() && w.members[0].cooldowns.is_empty());
    assert_eq!(w.members[0].spirit, spirit);
    assert_eq!(cast(&mut w, &c, 0, "cast", 31), Err(Error::InvalidState));
    assert_eq!(
        w.action(
            0,
            &Action::RecastProjectile {
                action_id: "recast".into(),
                recast_index: 1,
                direction: [1., 0.]
            },
            "recast",
            &c,
            31
        ),
        Err(Error::InvalidState)
    );
    assert!(
        w.action(
            0,
            &Action::Move {
                direction: [1., 0.]
            },
            "move",
            &c,
            31
        )
        .is_ok()
    );
    assert!(w.action(0, &Action::Attack {}, "basic", &c, 32).is_ok());
    let refresh = hit(&mut w, &c, 35);
    assert!(matches!(
        private_events(&refresh).as_slice(),
        [(0, Event::StatusRefreshed { .. })]
    ));
    assert!(w.members[0].pending.as_ref().unwrap().skill.is_none());
    assert!(cast(&mut w, &c, 1, "cast", 32).is_ok());
    w.members[0].pending = None;
    assert_eq!(cast(&mut w, &c, 0, "cast", 634), Err(Error::InvalidState));
    assert!(cast(&mut w, &c, 0, "cast", 635).is_ok());
}

#[test]
fn member_npc_silence_stops_queued_and_channel_work_without_undoing_paid_effects() {
    let (mut c, mut w) = setup();
    c.skill_followup = serde_json::from_value(json!({"window_ms":150})).unwrap();
    let mut next = c.skill.clone();
    next.id = "next".into();
    c.additional_skills.push(next);
    cast(&mut w, &c, 0, "cast", 0).unwrap();
    commit(&mut w, &c, 60);
    cast(&mut w, &c, 0, "next", 120).unwrap();
    assert!(w.members[0].queued_cast.is_some());
    let spirit = w.members[0].spirit;
    let shots = w.shots.len();
    assert!(matches!(
        private_events(&hit(&mut w, &c, 150)).as_slice(),
        [
            (0, Event::StatusApplied { .. }),
            (0, Event::ActionInterrupted),
        ]
    ));
    assert!(w.members[0].queued_cast.is_none());
    assert_eq!((w.members[0].spirit, w.shots.len()), (spirit, shots));
    assert_eq!(w.members[0].cooldowns["cast"], 360);
    assert!(!w.members[0].cooldowns.contains_key("next"));

    let (mut c, mut w) = setup();
    c.skill.channel =
        serde_json::from_value(json!({"duration_ms":360,"interval_ms":90,"spirit_per_pulse":3}))
            .unwrap();
    cast(&mut w, &c, 0, "cast", 0).unwrap();
    commit(&mut w, &c, 60);
    w.advance_channel(0, &c, 150, &mut vec![]);
    let spirit = w.members[0].spirit;
    let shots = w.shots.len();
    assert!(matches!(
        private_events(&hit(&mut w, &c, 180)).as_slice(),
        [
            (0, Event::StatusApplied { .. }),
            (
                0,
                Event::ChannelEnded {
                    reason: ChannelEndReason::Interrupted,
                    ..
                }
            ),
        ]
    ));
    w.advance_channel(0, &c, 240, &mut vec![]);
    assert_eq!((w.members[0].spirit, w.shots.len()), (spirit, shots));
    assert!(w.members[0].channel.is_none());
}

#[test]
fn member_npc_silence_checks_capacity_protection_and_source_cleanup() {
    let (mut c, mut w) = setup();
    let target = w.members[0].reference.entity.clone();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    bind(&mut w, &c, Owner::Member(0), target, "ward", 0);
    cast(&mut w, &c, 0, "cast", 0).unwrap();
    let events = hit(&mut w, &c, 30);
    assert!(private_events(&events).iter().any(|row| matches!(
        row,
        (
            0,
            Event::StatusRejected {
                reason: Error::Capacity,
                ..
            }
        )
    )));
    assert!(w.members[0].pending.is_some());
    assert!(!w.skill_silenced(0, 30));
    let (c, mut w) = setup();
    w.protect(0, 0, &mut vec![]);
    assert!(hit(&mut w, &c, 30).is_empty());
    w.members[0].ready = true;
    w.members[0].protected = false;
    hit(&mut w, &c, 60);
    let source = w.npcs[0].entity.clone();
    let mut events = vec![];
    w.clear_owned_statuses(&source, StatusRemovalReason::TargetInvalid, &mut events);
    assert!(matches!(
        private_events(&events).as_slice(),
        [(
            0,
            Event::StatusRemoved {
                reason: StatusRemovalReason::SourceInvalid,
                ..
            }
        ),]
    ));
    assert!(cast(&mut w, &c, 0, "cast", 61).is_ok());
}
