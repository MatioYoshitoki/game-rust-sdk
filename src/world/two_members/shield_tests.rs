use super::periodic_tests::{bind, pulse};
use super::*;
use crate::config::DuplicatePolicy;
fn setup(n: usize) -> (Config, World) {
    let (mut c, mut w) = super::periodic_tests::setup(n);
    let sample = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/multiplayer-shield-host/catalog.json"),
    )
    .unwrap();
    c.mechanism_version = 39;
    c.pve_mut().status_definitions = sample
        .default_config()
        .pve
        .as_ref()
        .unwrap()
        .status_definitions
        .clone();
    c.pve_mut().status_rules = sample
        .default_config()
        .pve
        .as_ref()
        .unwrap()
        .status_rules
        .clone();
    w.shields_enabled = true;
    (c, w)
}
fn grant(w: &mut World, c: &Config, owner: Owner, target: EntityRef, id: &str, now: u64) -> Events {
    bind(w, c, owner, target, id, now)
}
fn own(w: &mut World, c: &Config, i: usize) -> Events {
    let target = w.members[i].reference.entity.clone();
    grant(w, c, Owner::Member(i), target, "ward", 0)
}
fn npc(w: &mut World, c: &Config, owner: Owner, id: &str, now: u64) -> Events {
    let target = w.npcs[0].entity.clone();
    grant(w, c, owner, target, id, now)
}
fn shield_events(e: &Events) -> Vec<(usize, &Event)> {
    e.iter()
        .filter_map(|(i, e)| {
            if let (Some(i), EventV4::Private { event }) = (i, e) {
                matches!(
                    event,
                    Event::ShieldGranted { .. } | Event::ShieldAbsorbed { .. }
                )
                .then_some((*i, event))
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn mp_shield_one_two_four_eight_individual_base_and_total() {
    for n in [1, 2, 4, 8] {
        let (c, mut w) = setup(n);
        for i in 0..n {
            own(&mut w, &c, i);
            assert_eq!(
                w.snapshot(i, &c, Phase::Active, 0).own.shield_total,
                Some(100 + i as u32 * 10)
            );
        }
        assert_eq!(w.statuses.entries().len(), n);
    }
}
#[test]
fn mp_shield_npc_aggregate_contains_hidden_foreign_groups() {
    let (c, mut w) = setup(4);
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    npc(&mut w, &c, Owner::Member(1), "ward", 0);
    for i in 0..4 {
        let s = w.snapshot(i, &c, Phase::Active, 0);
        assert_eq!(s.monsters[0].shield_total, Some(360));
        assert_eq!(
            s.monsters[0].statuses.as_ref().unwrap().len(),
            usize::from(i < 2)
        );
        let v = serde_json::to_value(s).unwrap();
        assert!(
            v["members"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m.get("shield_total").is_none())
        );
    }
}
#[test]
fn mp_shield_shared_cap_not_per_source() {
    let (c, mut w) = setup(8);
    for i in 0..8 {
        npc(&mut w, &c, Owner::Member(i), "ward", 0);
    }
    assert_eq!(
        w.snapshot(7, &c, Phase::Active, 0).monsters[0].shield_total,
        Some(540)
    );
    assert_eq!(w.statuses.entries().len(), 3);
}
#[test]
fn mp_shield_refresh_refills_own_group_keeps_id_and_foreign_balance() {
    let (c, mut w) = setup(2);
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    npc(&mut w, &c, Owner::Member(1), "ward", 0);
    let id = w.statuses.entries()[0].instance_id.clone();
    w.hurt_npc(0, 0, 150, &c, 30, &mut vec![]);
    npc(&mut w, &c, Owner::Member(0), "ward", 60);
    assert_eq!(w.statuses.entries()[0].instance_id, id);
    assert_eq!(w.statuses.entries()[0].expires_at_ms, 6060);
    assert_eq!(
        w.statuses.entries()[1].shield.as_ref().unwrap().remaining,
        180
    );
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 60).monsters[0].shield_total,
        Some(360)
    );
}
#[test]
fn mp_shield_ignore_preserves_balance_source_and_deadline() {
    let (mut c, mut w) = setup(2);
    c.pve_mut().status_definitions[2].source_scoped = false;
    c.pve_mut().status_definitions[2].duplicate_policy = DuplicatePolicy::Ignore;
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    w.hurt_npc(0, 0, 100, &c, 30, &mut vec![]);
    let old = w.statuses.entries().to_vec();
    let e = npc(&mut w, &c, Owner::Member(1), "ward", 60);
    assert_eq!(w.statuses.entries(), old);
    assert!(e.iter().all(|(i, _)| *i == Some(0)));
}
#[test]
fn mp_shield_refresh_transfers_cleanup_to_actual_source() {
    let (mut c, mut w) = setup(2);
    c.pve_mut().status_definitions[2].source_scoped = false;
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    let id = w.statuses.entries()[0].instance_id.clone();
    npc(&mut w, &c, Owner::Member(1), "ward", 30);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert_eq!(w.statuses.entries()[0].instance_id, id);
    assert_eq!(w.statuses.entries()[0].source.id, "P01");
    w.terminal_intent(1, Outcome::Death, &mut vec![]);
    assert!(w.statuses.is_empty());
}
#[test]
fn mp_shield_consumption_slices_never_leak_foreign_ids() {
    let (c, mut w) = setup(3);
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    npc(&mut w, &c, Owner::Member(1), "ward", 0);
    let ids: Vec<_> = w
        .statuses
        .entries()
        .iter()
        .map(|s| s.instance_id.clone())
        .collect();
    let mut e = vec![];
    let r = w.hurt_npc_effect(0, 0, 400, &c, 30, &mut e);
    assert_eq!(r.absorbed, 266);
    let amounts: Vec<_> = shield_events(&e)
        .iter()
        .filter_map(|(i, e)| {
            if let Event::ShieldAbsorbed {
                amount,
                consumptions,
                ..
            } = e
            {
                assert!(consumptions.iter().all(|s| s.instance_id == ids[*i]));
                Some((*i, *amount))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(amounts, vec![(0, 180), (1, 86)]);
}
#[test]
fn mp_shield_npc_self_consumption_is_shared_but_not_multiplied_in_dot() {
    let (c, mut w) = setup(4);
    npc(&mut w, &c, Owner::Npc(0), "ward", 0);
    npc(&mut w, &c, Owner::Member(0), "wound", 0);
    let e = pulse(&mut w, &c, 600);
    assert_eq!(
        shield_events(&e)
            .iter()
            .filter(|(_, e)| matches!(e, Event::ShieldAbsorbed { amount: 40, .. }))
            .count(),
        4
    );
    let ticks: Vec<_> = e
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::Private {
                event:
                    Event::PeriodicTick {
                        absorbed,
                        hp_damage,
                        ..
                    },
            } = e
            {
                Some((*absorbed, *hp_damage))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ticks, vec![(40, 0)]);
}
#[test]
fn mp_shield_member_target_details_are_only_target_private() {
    let (c, mut w) = setup(4);
    own(&mut w, &c, 2);
    let mut e = vec![];
    w.hurt_member(2, w.npcs[0].entity.clone(), 100, &c, 30, &mut e);
    assert!(shield_events(&e).iter().all(|(i, _)| *i == 2));
}
#[test]
fn mp_shield_post_defense_before_shield_and_hp() {
    let (c, mut w) = setup(1);
    own(&mut w, &c, 0);
    let r = w.hurt_member_effect(
        0,
        w.npcs[0].entity.clone(),
        (300, true),
        &c,
        30,
        &mut vec![],
    );
    assert_eq!((r.absorbed, r.hp_damage), (100, 100));
    assert_eq!(w.members[0].hp, 900);
}
#[test]
fn mp_shield_overkill_consumes_full_shield_before_hp_clamp() {
    let (c, mut w) = setup(1);
    own(&mut w, &c, 0);
    w.members[0].hp = 1;
    let r = w.hurt_member_effect(
        0,
        w.npcs[0].entity.clone(),
        (1000, true),
        &c,
        30,
        &mut vec![],
    );
    assert_eq!((r.absorbed, r.hp_damage), (100, 1));
    assert!(w.statuses.is_empty());
}
#[test]
fn mp_shield_minimum_damage_consumes_one_then_depletes() {
    let (c, mut w) = setup(1);
    own(&mut w, &c, 0);
    let r = w.hurt_member_effect(0, w.npcs[0].entity.clone(), (0, true), &c, 30, &mut vec![]);
    assert_eq!((r.absorbed, r.hp_damage), (c.params.minimum_damage, 0));
}
#[test]
fn mp_shield_expiry_excludes_equal_deadline_before_damage() {
    let (c, mut w) = setup(1);
    own(&mut w, &c, 0);
    let r = w.hurt_member_effect(
        0,
        w.npcs[0].entity.clone(),
        (100, true),
        &c,
        6000,
        &mut vec![],
    );
    assert_eq!((r.absorbed, r.hp_damage), (0, 66));
    assert!(w.statuses.is_empty());
}
#[test]
fn mp_shield_order_earliest_expiry_then_application_then_id() {
    let (c, mut w) = setup(2);
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    npc(&mut w, &c, Owner::Member(1), "barrier", 30);
    let early = w.statuses.entries()[1].instance_id.clone();
    let mut e = vec![];
    w.hurt_npc(0, 0, 60, &c, 60, &mut e);
    let v = shield_events(&e);
    let entries = v
        .iter()
        .find_map(|(_, e)| {
            if let Event::ShieldAbsorbed { consumptions, .. } = e {
                Some(consumptions)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(entries[0].absorbed, 22);
    assert!(
        shield_events(&e)
            .iter()
            .any(|(i, e)| *i == 1 && matches!(e, Event::ShieldAbsorbed { amount: 18, .. }))
    );
    assert!(w.statuses.entries().iter().all(|s| s.instance_id != early));
}
#[test]
fn mp_shield_zero_grant_no_instance_or_expiration() {
    let (mut c, mut w) = setup(1);
    c.pve_mut().status_definitions[2].shield_bps = Some(0);
    let e = own(&mut w, &c, 0);
    assert!(w.statuses.is_empty());
    assert!(shield_events(&e).iter().any(|(_, e)| matches!(
        e,
        Event::ShieldGranted {
            instance_id: None,
            granted: 0,
            ..
        }
    )));
}
#[test]
fn mp_shield_group_capacity_failure_does_not_partially_refresh() {
    let (mut c, mut w) = setup(1);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    own(&mut w, &c, 0);
    let old = w.statuses.entries().to_vec();
    let t = w.members[0].reference.entity.clone();
    w.apply_owned_statuses(
        Owner::Member(0),
        (t, 1000),
        &["ward".into(), "renewal".into()],
        &c,
        30,
        &mut vec![],
    );
    assert_eq!(w.statuses.entries(), old);
}
#[test]
fn mp_shield_depleted_regrant_gets_new_id() {
    let (c, mut w) = setup(1);
    own(&mut w, &c, 0);
    let id = w.statuses.entries()[0].instance_id.clone();
    w.hurt_member(0, w.npcs[0].entity.clone(), 300, &c, 30, &mut vec![]);
    own(&mut w, &c, 0);
    assert_ne!(w.statuses.entries()[0].instance_id, id);
}
#[test]
fn mp_shield_full_direct_contact_interrupts_extraction() {
    let (c, mut w) = setup(1);
    own(&mut w, &c, 0);
    w.members[0].extract_end = Some(1000);
    let mut e = vec![];
    w.hurt_member(0, w.npcs[0].entity.clone(), 100, &c, 30, &mut e);
    assert_eq!(w.members[0].hp, 1000);
    assert_eq!(w.members[0].extract_end, None);
}
#[test]
fn mp_shield_full_dot_does_not_interrupt_but_hp_loss_does() {
    let (c, mut w) = setup(1);
    own(&mut w, &c, 0);
    let t = w.members[0].reference.entity.clone();
    grant(&mut w, &c, Owner::Npc(0), t, "wound", 0);
    w.members[0].extract_end = Some(5000);
    pulse(&mut w, &c, 600);
    assert_eq!(w.members[0].extract_end, Some(5000));
    w.statuses.remove(
        StatusRemovalReason::Depleted,
        |s| s.shield.is_some(),
        &mut vec![],
    );
    pulse(&mut w, &c, 1200);
    assert_eq!(w.members[0].extract_end, None);
}
#[test]
fn mp_shield_protection_and_returning_skip_without_consumption() {
    let (c, mut w) = setup(2);
    own(&mut w, &c, 0);
    npc(&mut w, &c, Owner::Member(1), "ward", 0);
    w.protect(0, 0, &mut vec![]);
    w.npcs[0].returning = true;
    let old = w.statuses.entries().to_vec();
    w.hurt_member(0, w.npcs[0].entity.clone(), 1000, &c, 30, &mut vec![]);
    assert_eq!(w.hurt_npc(0, 1, 1000, &c, 30, &mut vec![]), 0);
    assert_eq!(w.statuses.entries(), old);
}
#[test]
fn mp_shield_hot_preserves_balance_and_does_not_convert_overheal() {
    let (c, mut w) = setup(1);
    own(&mut w, &c, 0);
    let t = w.members[0].reference.entity.clone();
    grant(&mut w, &c, Owner::Member(0), t, "renewal", 0);
    pulse(&mut w, &c, 450);
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 450).own.shield_total,
        Some(100)
    );
}
#[test]
fn mp_shield_exit_cleans_target_and_outgoing_leaves_seven_peers() {
    let (c, mut w) = setup(8);
    for i in 0..8 {
        own(&mut w, &c, i);
    }
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 7);
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.source.id != "P00" && s.target.id != "P00")
    );
}
#[test]
fn mp_shield_npc_reset_cleans_both_relations_and_increments_generation() {
    let (c, mut w) = setup(2);
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    let t = w.members[0].reference.entity.clone();
    grant(&mut w, &c, Owner::Npc(0), t, "ward", 0);
    let old = w.npcs[0].entity.clone();
    w.npcs[0].returning = true;
    w.npcs[0].position = w.npcs[0].home;
    w.advance_npcs(&c, 30, 0.03, &mut vec![]);
    assert!(w.statuses.is_empty());
    assert_eq!(w.npcs[0].entity.generation, old.generation + 1);
}
#[test]
fn mp_shield_terminal_not_delayed_and_never_double_defeats() {
    let (c, mut w) = setup(2);
    npc(&mut w, &c, Owner::Member(0), "ward", 0);
    let mut e = vec![];
    w.hurt_npc(0, 1, u32::MAX, &c, 30, &mut e);
    let count = w.resources.len();
    w.hurt_npc(0, 1, u32::MAX, &c, 60, &mut e);
    assert_eq!(w.resources.len(), count);
    assert_eq!(
        e.iter()
            .filter(|(_, e)| matches!(e, EventV4::MonsterDefeated { .. }))
            .count(),
        1
    );
    own(&mut w, &c, 1);
    w.tick(&c, c.terminal_ms);
    assert!(w.statuses.is_empty());
}
#[test]
fn mp_shield_friendly_and_stale_generation_grants_rejected() {
    let (c, mut w) = setup(2);
    let t = w.members[1].reference.entity.clone();
    grant(&mut w, &c, Owner::Member(0), t, "ward", 0);
    let mut t = w.npcs[0].entity.clone();
    t.generation += 1;
    grant(&mut w, &c, Owner::Member(0), t, "ward", 0);
    assert!(w.statuses.is_empty());
}
