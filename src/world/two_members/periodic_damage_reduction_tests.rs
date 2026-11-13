use super::periodic_tests::{bind, pulse};
use super::*;
use serde_json::json;

fn setup(mechanism: u32, reduction: u32, shield: u32, duration: u64, raw: u32) -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(2);
    c.mechanism_version = mechanism;
    c.pve_mut().status_definitions.extend([
        serde_json::from_value(json!({"id":"test-dot","mechanism":"periodic_damage","damage_bps":10000,
            "duration_ms":1800,"tick_interval_ms":300,"duplicate_policy":"refresh","source_scoped":true})).unwrap(),
        serde_json::from_value(json!({"id":"test-guard","mechanism":"timed_attribute_modifier","attribute":"damage_reduction",
            "modifier_bps":reduction,"duration_ms":duration,"duplicate_policy":"refresh","source_scoped":true})).unwrap(),
        serde_json::from_value(json!({"id":"test-shield","mechanism":"absorption_shield","shield_bps":shield,
            "duration_ms":duration,"duplicate_policy":"refresh","source_scoped":true})).unwrap(),
    ]);
    let item = c.item_id.clone();
    c.member_item_use = serde_json::from_value(json!({item.clone(): {
        "heal_hp":200,"cooldown_ms":900,"on_use_self_statuses":["test-guard","test-shield"],
    }}))
    .unwrap();
    std::sync::Arc::make_mut(&mut w.npcs[0].profile)
        .stats
        .insert("atk".into(), raw);
    w.members[0].inventory.insert(item.clone(), 2);
    w.members[0].looted.insert(item, 2);
    let target = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Npc(0), target, "test-dot", 0);
    (c, w)
}

fn use_item(w: &mut World, c: &Config) {
    let revision = w.members[0].revision;
    w.action(
        0,
        &Action::UseItem {
            item_id: c.item_id.clone(),
            expected_revision: revision,
        },
        "dot-mitigation-dose",
        c,
        330,
    )
    .unwrap();
    assert_eq!(w.members[0].looted[&c.item_id], 1);
}

fn assert_tick(
    events: &Events,
    amounts: (u32, u32),
    times: (u64, u64),
    expected: PeriodicTickOutcome,
) {
    assert!(events.iter().any(|(recipient, event)| *recipient == Some(0)
        && matches!(event,
        EventV4::Private { event: Event::PeriodicTick { definition_id, absorbed, hp_damage,
            scheduled_at_ms, executed_at_ms, outcome, .. } }
        if definition_id == "test-dot" && (*absorbed, *hp_damage) == amounts &&
            (*scheduled_at_ms, *executed_at_ms) == times && *outcome == expected)));
}

#[test]
fn member_periodic_damage_reduction_applies_after_defense_before_shield_and_hp() {
    for (version, reduction, shield, absorbed, hp_damage) in [
        (67, 2500, 750, 75, 25),
        (68, 2500, 750, 75, 0),
        (68, 3200, 400, 40, 28),
    ] {
        let (c, mut w) = setup(version, reduction, shield, 570, 150);
        assert_tick(
            &pulse(&mut w, &c, 300),
            (0, 100),
            (300, 300),
            PeriodicTickOutcome::Applied,
        );
        use_item(&mut w, &c);
        let hp = w.members[0].hp;
        let events = pulse(&mut w, &c, 600);
        assert_tick(
            &events,
            (absorbed, hp_damage),
            (600, 600),
            PeriodicTickOutcome::Applied,
        );
        assert_eq!(w.members[0].hp, hp - hp_damage);
        assert_eq!(
            w.snapshot(0, &c, Phase::Active, 600).own.shield_total,
            Some(0)
        );
        let shield_at = events
            .iter()
            .position(|(recipient, event)| {
                *recipient == Some(0)
                    && matches!(event,
            EventV4::Private {event: Event::ShieldAbsorbed {amount, ..}} if *amount == absorbed)
            })
            .unwrap();
        let hp_at = events
            .iter()
            .position(|(_, event)| {
                matches!(event,
            EventV4::Damage {amount, ..} if *amount == hp_damage)
            })
            .unwrap();
        let tick_at = events
            .iter()
            .position(|(_, event)| {
                matches!(
                    event,
                    EventV4::Private {
                        event: Event::PeriodicTick { .. }
                    }
                )
            })
            .unwrap();
        assert!(shield_at < hp_at && hp_at < tick_at);
        assert_tick(
            &pulse(&mut w, &c, 900),
            (0, 100),
            (900, 900),
            PeriodicTickOutcome::Applied,
        );
    }
}

#[test]
fn member_periodic_damage_reduction_late_tick_uses_execution_time_at_expiry() {
    let (c, mut w) = setup(68, 2500, 750, 300, 150);
    pulse(&mut w, &c, 300);
    use_item(&mut w, &c);
    // The tick was due while the item effects were active, but executes on their deadline.
    assert_tick(
        &pulse(&mut w, &c, 630),
        (0, 100),
        (600, 630),
        PeriodicTickOutcome::Applied,
    );
}

#[test]
fn member_periodic_damage_reduction_protected_tick_preserves_shield_and_rng() {
    let (c, mut w) = setup(68, 2500, 400, 1800, 152);
    assert_tick(
        &pulse(&mut w, &c, 300),
        (0, 101),
        (300, 300),
        PeriodicTickOutcome::Applied,
    );
    use_item(&mut w, &c);
    let hp = w.members[0].hp;
    w.rng = 1;
    w.protect(0, 360, &mut vec![]);
    assert_tick(
        &pulse(&mut w, &c, 600),
        (0, 0),
        (600, 600),
        PeriodicTickOutcome::Protected,
    );
    assert_eq!(w.members[0].hp, hp);
    assert_eq!(w.rng, 1);
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 600).own.shield_total,
        Some(40)
    );
    w.members[0].foreground = true;
    w.members[0].ready = true;
    w.members[0].protected = false;
    // Armor leaves 101, 25% mitigation leaves 75.75, and seed 1 rounds up to 76.
    assert_tick(
        &pulse(&mut w, &c, 900),
        (40, 36),
        (900, 900),
        PeriodicTickOutcome::Applied,
    );
    assert_eq!(w.members[0].hp, hp - 36);
    assert_ne!(w.rng, 1);
}
