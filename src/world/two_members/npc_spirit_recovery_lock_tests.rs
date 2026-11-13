use super::combat::DirectContact;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_npc_spirit_recovery_lock_configuration_only_opens_npc_projectile_hits() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-npc-spirit-recovery-lock-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    assert!(load(&doc).is_ok());
    let mut old = doc.clone();
    old["mechanism_version"] = json!(73);
    assert!(load(&old).is_err());
    old["pve"]["additional_templates"]["seal-caster"]["combat"]["projectile"]["on_hit_target_statuses"] =
        json!([]);
    old["additional_skills"][0]["on_commit_self_statuses"] = json!(["spirit-seal"]);
    assert!(load(&old).is_ok());

    for path in [
        "/skill/on_hit_target_statuses",
        "/pve/additional_templates/seal-caster/combat/projectile/on_commit_self_statuses",
    ] {
        let mut invalid = doc.clone();
        *invalid.pointer_mut(path).unwrap() = json!(["spirit-seal"]);
        assert!(load(&invalid).is_err(), "{path}");
    }
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(2);
    c.mechanism_version = 74;
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({
        "id":"npc-seal","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock",
        "modifier_bps":-10000,"duration_ms":2000,"duplicate_policy":"refresh","source_scoped":true
    })).unwrap());
    for member in &mut w.members {
        member.spirit = 20;
        member.hp = 500;
    }
    (c, w)
}

fn contact(
    w: &mut World,
    c: &Config,
    owners: (usize, usize),
    source: &EntityRef,
    now: u64,
    events: &mut Events,
) -> bool {
    w.direct_contact(
        Owner::Npc(owners.0),
        Owner::Member(owners.1),
        DirectContact {
            origin: None,
            action_id: None,
            rule: None,
            ignore_armor: false,
            unavoidable: false,
            dead_source: false,
            source,
            raw: 20,
            metal: true,
            hit_statuses: &["npc-seal".into()],
        },
        c,
        now,
        events,
    )
}

#[test]
fn member_npc_spirit_recovery_lock_shield_contact_applies_only_to_surviving_eligible_target() {
    let (c, mut w) = setup();
    let source = w.npcs[0].entity.clone();
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), target.clone(), "ward", 0);
    let mut events = vec![];
    assert!(contact(&mut w, &c, (0, 0), &source, 30, &mut events));
    assert!(
        events
            .iter()
            .any(|(_, event)| matches!(event, EventV4::Damage { amount: 0, .. }))
    );
    assert_eq!(w.members[0].hp, 500);
    let applied = events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event: Event::StatusApplied { status },
            } if status.definition_id == "npc-seal" => Some((
                *recipient,
                status.source.clone(),
                status.target.clone(),
                status.expires_at_ms,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        applied,
        vec![(
            Some(0),
            status_entity(&source),
            status_entity(&target),
            2030
        )]
    );
    assert_eq!(
        w.members[0].spirit_recovery_gap(&w.statuses, 2029, w.members[0].facts.spirit_points),
        0
    );
    assert_eq!(
        w.members[0].spirit_recovery_gap(&w.statuses, 2030, w.members[0].facts.spirit_points),
        w.members[0].facts.spirit_points - 20
    );
    assert!(
        w.members[1].spirit_recovery_gap(&w.statuses, 30, w.members[1].facts.spirit_points) > 0
    );

    w.protect(1, 60, &mut vec![]);
    events.clear();
    assert!(!contact(&mut w, &c, (0, 1), &source, 60, &mut events));
    assert!(events.is_empty());
    assert_eq!(w.members[1].hp, 500);
    w.members[1].ready = true;
    w.members[1].protected = false;
    w.members[1].hp = 1;
    assert!(contact(&mut w, &c, (0, 1), &source, 90, &mut events));
    assert_eq!(w.members[1].hp, 0);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|status| status.target.id == w.members[1].reference.entity.id)
    );
}

#[test]
fn member_npc_spirit_recovery_lock_cleans_only_the_dead_or_reset_source_generation() {
    let (c, mut w) = setup();
    let first = w.npcs[0].entity.clone();
    let second = w.npcs[1].entity.clone();
    let mut events = vec![];
    assert!(contact(&mut w, &c, (0, 0), &first, 30, &mut events));
    assert!(contact(&mut w, &c, (1, 0), &second, 60, &mut events));
    let lock_sources = |w: &World| {
        w.statuses
            .entries()
            .iter()
            .filter(|status| status.definition_id == "npc-seal")
            .map(|status| status.source.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        lock_sources(&w),
        vec![status_entity(&first), status_entity(&second)]
    );

    events.clear();
    assert!(w.hurt_npc(0, 0, u32::MAX, &c, 90, &mut events) > 0);
    assert_eq!(lock_sources(&w), vec![status_entity(&second)]);
    assert_eq!(
        w.members[0].spirit_recovery_gap(&w.statuses, 90, w.members[0].facts.spirit_points),
        0
    );
    assert!(events.iter().any(|(recipient, event)| *recipient == Some(0)
        && matches!(
            event,
            EventV4::Private {
                event: Event::StatusRemoved {
                    reason: StatusRemovalReason::SourceInvalid,
                    ..
                }
            }
        )));

    w.npcs[1].position = w.npcs[1].home;
    w.npcs[1].returning = true;
    events.clear();
    w.advance_npcs(&c, 120, 0.03, &mut events);
    assert!(lock_sources(&w).is_empty());
    assert_eq!(w.npcs[1].entity.generation, second.generation + 1);
    assert!(
        w.members[0].spirit_recovery_gap(&w.statuses, 120, w.members[0].facts.spirit_points) > 0
    );
    assert!(events.iter().any(|(recipient, event)| *recipient == Some(0)
        && matches!(
            event,
            EventV4::Private {
                event: Event::StatusRemoved {
                    reason: StatusRemovalReason::Reset,
                    ..
                }
            }
        )));
    assert!(!contact(&mut w, &c, (1, 0), &second, 150, &mut vec![]));
}
