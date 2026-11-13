use super::combat::DamageIntent;
use super::periodic_tests::bind;
use super::*;
use crate::config::AttackRule;
use serde_json::{Value, json};

const CARRIER: &str = "effect_xianwang_seal_stage_5";
const REPLY: &str = "effect_xianwang_retaliation_silence";
const END: &str = "effect_xianwang_retaliation_slow";

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(false, false);
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/source-xianwang-jiufeng.json"
    ))
    .unwrap();
    let p = c.pve_mut();
    p.status_definitions.extend(
        serde_json::from_value::<Vec<crate::config::StatusDefinition>>(
            sample["status_definitions"].clone(),
        )
        .unwrap(),
    );
    for (id, attribute) in [("old-boon", "atk"), ("new-boon", "def")] {
        p.status_definitions.push(serde_json::from_value(json!({
            "id":id,"mechanism":"timed_attribute_modifier","attribute":attribute,"modifier_bps":100,
            "duration_ms":20000,"duplicate_policy":"refresh","source_scoped":true
        })).unwrap());
    }
    let r = c.combat_rules.as_mut().unwrap();
    r.status_policies =
        serde_json::from_value(sample["combat_rules"]["status_policies"].clone()).unwrap();
    for id in ["old-boon", "new-boon"] {
        r.status_policies.insert(
            id.into(),
            serde_json::from_value(json!({
                "disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]
            }))
            .unwrap(),
        );
    }
    r.status_retaliations =
        serde_json::from_value(sample["combat_rules"]["status_retaliations"].clone()).unwrap();
    r.status_retaliations.get_mut(CARRIER).unwrap().chance_bps = 10000;
    for p in r.profiles.values_mut() {
        p.tier = 0;
    }
    w.members[0].facts.hp_points = 2000;
    w.members[0].hp = 2000;
    bind_self(&mut w, &c, Owner::Member(0), CARRIER, 0);
    (c, w)
}

fn bind_self(w: &mut World, c: &Config, owner: Owner, id: &str, now: u64) {
    bind(w, c, owner, w.owner_entity(owner), id, now);
}

fn hit(w: &mut World, c: &Config, npc: usize, direct: bool, raw: u32, now: u64) -> Events {
    let mut events = vec![];
    w.resolve_damage_with_rule(
        DamageIntent {
            source: w.owner_entity(Owner::Npc(npc)),
            target: w.owner_entity(Owner::Member(0)),
            raw,
            direct,
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
    events
}

fn applied<'a>(events: &'a Events, id: &str) -> Vec<&'a StatusView> {
    events
        .iter()
        .filter_map(|(recipient, event)| match event {
            EventV4::Private {
                event: Event::StatusApplied { status } | Event::StatusRefreshed { status },
            } if status.definition_id == id => {
                assert_eq!(*recipient, Some(0));
                Some(status)
            }
            _ => None,
        })
        .collect()
}

fn expire(w: &mut World, c: &Config, now: u64) -> Events {
    let mut events = vec![];
    w.prepare_periodic(now, &mut events);
    w.finish_status_ends(c, now, &mut events);
    events
}

#[test]
fn retaliation_damage_reduction_dispel_and_source_cooldowns() {
    let (c, mut w) = setup();
    bind_self(&mut w, &c, Owner::Npc(0), "old-boon", 0);
    bind_self(&mut w, &c, Owner::Npc(0), "new-boon", 30);
    let first = hit(&mut w, &c, 0, true, 100, 60);
    assert_eq!(w.members[0].hp, 1925);
    let reply = applied(&first, REPLY)[0];
    assert_eq!(
        reply.source,
        status_entity(&w.owner_entity(Owner::Member(0)))
    );
    assert_eq!(reply.target, status_entity(&w.owner_entity(Owner::Npc(0))));
    assert_eq!(reply.expires_at_ms, 2060);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "old-boon")
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "new-boon")
    );
    assert_eq!(applied(&hit(&mut w, &c, 1, true, 100, 90), REPLY).len(), 1);
    assert!(applied(&hit(&mut w, &c, 0, true, 100, 4059), REPLY).is_empty());
    assert_eq!(
        applied(&hit(&mut w, &c, 0, true, 100, 4060), REPLY).len(),
        1
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "new-boon")
    );
    let ended = expire(&mut w, &c, 8000);
    let slow = applied(&ended, END);
    assert_eq!(slow.len(), 2);
    for (npc, status) in slow.into_iter().enumerate() {
        assert_eq!(status.target, status_entity(&w.npcs[npc].entity));
        assert_eq!((status.modifier_bps, status.expires_at_ms), (-3000, 10000));
    }
    assert!(applied(&expire(&mut w, &c, 8030), END).is_empty());
}

#[test]
fn retaliation_requires_successful_direct_contact_and_reply() {
    let (mut c, mut w) = setup();
    let r = c.combat_rules.as_mut().unwrap();
    r.status_retaliations.get_mut(CARRIER).unwrap().max_sources = 1;
    r.status_retaliations.get_mut(CARRIER).unwrap().chance_bps = 0;
    r.status_policies.get_mut("old-boon").unwrap().blocks =
        vec![crate::config::StatusDisposition::Harmful];
    assert!(applied(&hit(&mut w, &c, 0, true, 100, 30), REPLY).is_empty());
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_retaliations
        .get_mut(CARRIER)
        .unwrap()
        .chance_bps = 10000;
    bind_self(&mut w, &c, Owner::Npc(0), "old-boon", 60);
    assert!(applied(&hit(&mut w, &c, 0, true, 100, 90), REPLY).is_empty());
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "old-boon")
    );
    w.statuses.remove(
        StatusRemovalReason::Cleansed,
        |s| s.definition_id == "old-boon",
        &mut vec![],
    );
    assert!(applied(&hit(&mut w, &c, 0, false, 100, 120), REPLY).is_empty());
    assert_eq!(applied(&hit(&mut w, &c, 0, true, 100, 180), REPLY).len(), 1);
    assert!(applied(&hit(&mut w, &c, 1, true, 100, 210), REPLY).is_empty());
    assert_eq!(applied(&expire(&mut w, &c, 8000), END).len(), 1);
}

#[test]
fn retaliation_refresh_cleanup_and_generation_never_replay_old_history() {
    let (c, mut w) = setup();
    assert_eq!(applied(&hit(&mut w, &c, 0, true, 100, 30), REPLY).len(), 1);
    bind_self(&mut w, &c, Owner::Member(0), CARRIER, 60);
    assert_eq!(applied(&hit(&mut w, &c, 1, true, 100, 90), REPLY).len(), 1);
    w.npcs[1].entity.generation += 1;
    assert!(applied(&expire(&mut w, &c, 8060), END).is_empty());
    bind_self(&mut w, &c, Owner::Member(0), CARRIER, 8100);
    assert_eq!(
        applied(&hit(&mut w, &c, 0, true, 100, 8130), REPLY).len(),
        1
    );
    w.statuses.remove(
        StatusRemovalReason::Cleansed,
        |s| s.definition_id == CARRIER,
        &mut vec![],
    );
    assert!(applied(&expire(&mut w, &c, 16100), END).is_empty());
}

#[test]
fn retaliation_counts_full_shield_absorption_and_skips_lethal_damage() {
    let (mut c, mut w) = setup();
    c.pve_mut().status_definitions.push(
        serde_json::from_value(json!({
            "id":"retaliation-shield","mechanism":"absorption_shield","shield_bps":1000,
            "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true
        }))
        .unwrap(),
    );
    bind_self(&mut w, &c, Owner::Member(0), "retaliation-shield", 0);
    let absorbed = hit(&mut w, &c, 0, true, 100, 30);
    assert_eq!(w.members[0].hp, 2000);
    assert_eq!(applied(&absorbed, REPLY).len(), 1);
    expire(&mut w, &c, 300);
    let lethal = hit(&mut w, &c, 1, true, 4000, 330);
    assert_eq!(w.members[0].hp, 0);
    assert!(applied(&lethal, REPLY).is_empty());
    assert!(applied(&expire(&mut w, &c, 8000), END).is_empty());
}
