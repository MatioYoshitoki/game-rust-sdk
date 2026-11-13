use super::*;
use serde_json::json;

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::hp_cost_tests::setup(2);
    c.mechanism_version = 58;
    c.member_item_use = serde_json::from_value(json!({
        "pill_healing":{"heal_hp":40},
        c.item_id.clone():{"heal_hp":40}
    }))
    .unwrap();
    w.members[0].inventory.insert("pill_healing".into(), 2);
    w.members[0].looted.insert("pill_healing".into(), 2);
    (c, w)
}

fn use_item(w: &mut World, c: &Config, item: &str, revision: u32) -> Result<Events> {
    w.action(
        0,
        &Action::UseItem {
            item_id: item.into(),
            expected_revision: revision,
        },
        "heal",
        c,
        0,
    )
}

#[test]
fn member_item_status_quality_refreshes_same_instance_in_both_directions() {
    for reverse in [false, true] {
        let (mut c, mut w) = setup();
        c.mechanism_version = 66;
        let low = "pill_healing";
        let high = c.item_id.clone();
        let effect = "quality-fury";
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({
            "id":effect,"mechanism":"timed_attribute_modifier","attribute":"atk",
            "modifier_bps":3000,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true,
        })).unwrap());
        let variants = [(low, 1500, 300), (high.as_str(), 3900, 900)];
        for (item, modifier, duration) in variants {
            c.member_item_use.as_mut().unwrap().insert(item.into(), serde_json::from_value(json!({
                "heal_hp":40,"cooldown_ms":600,"on_use_self_statuses":[effect],
                "on_use_self_status_overrides":{effect:{"modifier_bps":modifier,"duration_ms":duration}},
            })).unwrap());
            w.members[0].inventory.insert(item.into(), 2);
            w.members[0].looted.insert(item.into(), 2);
        }
        let [first, second] = if reverse {
            [variants[1], variants[0]]
        } else {
            variants
        };
        let initial = w.members[0].revision;
        w.members[0].hp -= 150;
        use_item(&mut w, &c, first.0, initial).unwrap();
        let before = w.snapshot(0, &c, Phase::Active, 0).own;
        let original = &before.statuses[0];
        assert_eq!(
            (original.modifier_bps, original.expires_at_ms),
            (first.1, first.2)
        );
        let action = |revision| Action::UseItem {
            item_id: second.0.into(),
            expected_revision: revision,
        };
        assert_eq!(
            w.action(0, &action(initial), "stale", &c, 30),
            Err(Error::RevisionConflict)
        );
        w.members[0].hp = w.members[0].facts.hp_points;
        let full = w.snapshot(0, &c, Phase::Active, 30).own;
        assert_eq!(
            w.action(0, &action(initial + 1), "full", &c, 30),
            Err(Error::InvalidState)
        );
        assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own, full);
        w.members[0].hp = before.hp;
        let events = w
            .action(0, &action(initial + 1), "refresh", &c, 30)
            .unwrap();
        let after = w.snapshot(0, &c, Phase::Active, 30).own;
        assert_eq!(after.statuses.len(), 1);
        let status = &after.statuses[0];
        assert_eq!(status.instance_id, original.instance_id);
        assert_eq!(
            (status.modifier_bps, status.expires_at_ms),
            (second.1, 30 + second.2)
        );
        assert_eq!(status.expires_at_ms < original.expires_at_ms, reverse);
        assert_eq!(
            after.effective_atk,
            w.members[0].facts.atk_points * (10000 + second.1 as u32) / 10000
        );
        assert_eq!(after.inventory_revision, initial + 2);
        assert_eq!(w.members[0].looted[first.0], 1);
        assert_eq!(w.members[0].looted[second.0], 1);
        assert!(events.iter().any(|(recipient, event)| *recipient == Some(0) && matches!(event,
            EventV4::Private {event: Event::StatusRefreshed {status: refreshed}} if refreshed == status)));
    }
}

#[test]
fn member_item_self_status_capacity_rejects_before_recovery_or_refresh() {
    let (mut c, mut w) = setup();
    c.mechanism_version = 65;
    let item = "pill_healing";
    let pve = c.pve_mut();
    pve.status_rules.as_mut().unwrap().max_active_per_entity = 1;
    pve.status_definitions.push(serde_json::from_value(json!({
        "id":"item-fury","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":3000,
        "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true,
    })).unwrap());
    c.member_item_use.as_mut().unwrap().insert(item.into(), serde_json::from_value(json!({
        "heal_hp":40,"restore_spirit":40,"cooldown_ms":300,"on_use_self_statuses":["ward","item-fury"],
    })).unwrap());
    w.apply_statuses(
        0,
        (
            w.members[0].reference.entity.clone(),
            w.members[0].facts.hp_points,
        ),
        &["ward".into()],
        &c,
        0,
        &mut vec![],
    );
    let revision = w.members[0].revision;
    let action = |revision| Action::UseItem {
        item_id: item.into(),
        expected_revision: revision,
    };
    let full = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        w.action(0, &action(revision), "full", &c, 30),
        Err(Error::InvalidState)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own, full);

    w.members[0].hp -= 50;
    w.members[0].spirit -= 10;
    let before = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        w.action(0, &action(revision + 1), "stale", &c, 30),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        w.action(0, &action(revision), "capacity", &c, 30),
        Err(Error::Capacity)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own, before);

    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 2;
    let events = w.action(0, &action(revision), "commit", &c, 30).unwrap();
    let after = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        (after.hp, after.spirit),
        (before.hp + 40, before.spirit + 10)
    );
    assert_eq!(after.inventory_revision, revision + 1);
    assert_eq!(w.members[0].looted[item], 1);
    assert_eq!(w.members[0].item_cooldowns[item], 330);
    assert_eq!(after.statuses.len(), 2);
    assert_eq!(
        after
            .statuses
            .iter()
            .find(|s| s.definition_id == "ward")
            .unwrap()
            .applied_at_ms,
        30
    );
    assert!(events.iter().any(|(recipient, event)| *recipient == Some(0) && matches!(event,
        EventV4::Private {event: Event::StatusRefreshed {status}} if status.definition_id == "ward")));
}

#[test]
fn member_item_quality_recovery_scales_before_rounding_and_clamps_safely() {
    for (maxima, deficits, definition, expected) in [
        (
            [1003, 203],
            [20, 20],
            json!({"heal_hp":13,"restore_spirit":13,"quality_multiplier_bps":7000}),
            [9, 9],
        ),
        (
            [3, 203],
            [2, 7],
            json!({"heal_max_hp_bps":2000,"restore_spirit":7,"quality_multiplier_bps":20000}),
            [1, 7],
        ),
        (
            [1003, 203],
            [300, 50],
            json!({"heal_max_hp_bps":2000,"restore_max_spirit_bps":2000}),
            [200, 40],
        ),
        (
            [9, 8],
            [2, 2],
            json!({"heal_hp":1,"restore_max_spirit_bps":1,"quality_multiplier_bps":1}),
            [0, 0],
        ),
        (
            [u32::MAX, u32::MAX],
            [u32::MAX - 1, u32::MAX],
            json!({"heal_max_hp_bps":10000,"restore_max_spirit_bps":10000,"quality_multiplier_bps":40000}),
            [u32::MAX - 1, u32::MAX],
        ),
    ] {
        let (mut c, mut w) = setup();
        c.mechanism_version = 64;
        let item = "pill_healing";
        let mut definition = definition;
        definition["cooldown_ms"] = json!(300);
        c.member_item_use
            .as_mut()
            .unwrap()
            .insert(item.into(), serde_json::from_value(definition).unwrap());
        let member = &mut w.members[0];
        member.facts.hp_points = maxima[0];
        member.facts.spirit_points = maxima[1];
        member.hp = maxima[0] - deficits[0];
        member.spirit = maxima[1] - deficits[1];
        let before = w.snapshot(0, &c, Phase::Active, 0).own;
        let result = use_item(&mut w, &c, item, before.inventory_revision);
        let after = w.snapshot(0, &c, Phase::Active, 0).own;
        if expected == [0, 0] {
            assert_eq!(result, Err(Error::InvalidState));
            assert_eq!(after, before);
            continue;
        }
        assert!(matches!(result.unwrap().as_slice(),
            [(Some(0), EventV4::Private {event: Event::ItemUsed {hp_restored, spirit_restored: Some(spirit), ..}})]
            if [*hp_restored, *spirit] == expected));
        assert_eq!(
            [after.hp - before.hp, after.spirit - before.spirit],
            expected
        );
        assert_eq!(after.inventory_revision, before.inventory_revision + 1);
        assert_eq!(w.members[0].looted[item], 1);
        assert_eq!(w.members[0].item_cooldowns[item], 300);
    }
}

#[test]
fn member_item_ratio_recovery_rounds_then_clamps_at_attribute_limits() {
    for (maxima, deficits, ratios, expected) in [
        ([1003, 203], [300, 50], [2000, 2000], [200, 40]),
        ([1003, 203], [7, 5], [2000, 2000], [7, 5]),
        ([1003, 3], [300, 2], [2000, 2000], [200, 0]),
        ([9, 8], [2, 2], [1, 1], [0, 0]),
        (
            [u32::MAX, u32::MAX],
            [u32::MAX - 1, u32::MAX],
            [10_000, 10_000],
            [u32::MAX - 1, u32::MAX],
        ),
    ] {
        let (mut c, mut w) = setup();
        c.mechanism_version = 63;
        let item = "pill_healing";
        c.member_item_use.as_mut().unwrap().insert(
            item.into(),
            serde_json::from_value(json!({
                "heal_max_hp_bps":ratios[0], "restore_max_spirit_bps":ratios[1], "cooldown_ms":300,
            }))
            .unwrap(),
        );
        let member = &mut w.members[0];
        member.facts.hp_points = maxima[0];
        member.facts.spirit_points = maxima[1];
        member.hp = maxima[0] - deficits[0];
        member.spirit = maxima[1] - deficits[1];
        let before = w.snapshot(0, &c, Phase::Active, 0).own;
        assert_eq!(
            use_item(&mut w, &c, item, before.inventory_revision + 1),
            Err(Error::RevisionConflict)
        );
        assert_eq!(w.snapshot(0, &c, Phase::Active, 0).own, before);
        let result = use_item(&mut w, &c, item, before.inventory_revision);
        let after = w.snapshot(0, &c, Phase::Active, 0).own;
        if expected == [0, 0] {
            assert_eq!(result, Err(Error::InvalidState));
            assert_eq!(after, before);
            continue;
        }
        assert!(matches!(result.unwrap().as_slice(),
            [(Some(0), EventV4::Private {event: Event::ItemUsed {hp_restored, spirit_restored: Some(spirit), ..}})]
            if [*hp_restored, *spirit] == expected));
        assert_eq!(
            [after.hp - before.hp, after.spirit - before.spirit],
            expected
        );
        assert_eq!(after.inventory_revision, before.inventory_revision + 1);
        assert_eq!(w.members[0].looted[item], 1);
        assert_eq!(w.members[0].item_cooldowns[item], 300);
    }
}

#[test]
fn member_item_dual_recovery_clamps_each_attribute_independently() {
    for (hp_deficit, spirit_deficit) in [(0, 0), (13, 0), (0, 7), (60, 7)] {
        let (mut c, mut w) = setup();
        c.mechanism_version = 62;
        let item = "pill_healing";
        let definitions = c.member_item_use.as_mut().unwrap();
        let definition = definitions.get_mut(item).unwrap();
        definition.restore_spirit = Some(40);
        definition.cooldown_ms = Some(300);
        definition.cooldown_group = Some("recovery".into());
        let sibling = definitions.get_mut(&c.item_id).unwrap();
        sibling.cooldown_ms = Some(300);
        sibling.cooldown_group = Some("recovery".into());

        w.members[0].hp -= hp_deficit;
        w.members[0].spirit -= spirit_deficit;
        let before = w.snapshot(0, &c, Phase::Active, 0).own;
        assert_eq!(
            use_item(&mut w, &c, item, before.inventory_revision + 1),
            Err(Error::RevisionConflict)
        );
        assert_eq!(w.snapshot(0, &c, Phase::Active, 0).own, before);
        let result = use_item(&mut w, &c, item, before.inventory_revision);
        let after = w.snapshot(0, &c, Phase::Active, 0).own;
        if hp_deficit == 0 && spirit_deficit == 0 {
            assert_eq!(result, Err(Error::InvalidState));
            assert_eq!(after, before);
            continue;
        }
        let restored_hp = hp_deficit.min(40);
        let restored_spirit = spirit_deficit.min(40);
        assert!(matches!(result.unwrap().as_slice(),
            [(Some(0), EventV4::Private {event: Event::ItemUsed {hp_restored, spirit_restored: Some(spirit), ..}})]
            if *hp_restored == restored_hp && *spirit == restored_spirit));
        assert_eq!(after.hp, before.hp + restored_hp);
        assert_eq!(after.spirit, before.spirit + restored_spirit);
        assert_eq!(after.inventory_revision, before.inventory_revision + 1);
        assert_eq!(w.members[0].looted[item], 1);
        assert_eq!(w.members[0].item_cooldowns[item], 300);
        assert_eq!(w.members[0].item_cooldowns[&c.item_id], 300);
        assert_eq!(
            use_item(&mut w, &c, item, after.inventory_revision),
            Err(Error::InvalidState)
        );
        assert_eq!(w.snapshot(0, &c, Phase::Active, 0).own, after);
    }
}

#[test]
fn member_item_use_clamps_to_deficit_and_reports_private_amount() {
    let (c, mut w) = setup();
    let peer_hp = w.members[1].hp;
    let member = &mut w.members[0];
    member.hp -= 13;
    let revision = member.revision;
    let events = use_item(&mut w, &c, "pill_healing", revision).unwrap();
    assert_eq!(w.members[0].hp, w.members[0].facts.hp_points);
    assert_eq!(w.members[0].inventory["pill_healing"], 1);
    assert_eq!(w.members[0].looted["pill_healing"], 1);
    assert_eq!(w.members[0].revision, revision + 1);
    assert_eq!(w.members[1].hp, peer_hp);
    assert!(matches!(events.as_slice(),
        [(Some(0), EventV4::Private {event: Event::ItemUsed {action_id,item_id,hp_restored:13,spirit_restored:None}})]
        if action_id == "heal" && item_id == "pill_healing"));
}

#[test]
fn member_item_use_rejects_ineligible_or_stale_without_mutation() {
    let (c, mut w) = setup();
    let rev = w.members[0].revision;
    let hp = w.members[0].hp;
    assert_eq!(
        use_item(&mut w, &c, "pill_healing", rev),
        Err(Error::InvalidState)
    );
    assert_eq!(w.members[0].hp, hp);
    w.members[0].hp -= 50;
    let inventory = w.members[0].inventory.clone();
    assert_eq!(
        use_item(&mut w, &c, "unknown", rev),
        Err(Error::InvalidTarget)
    );
    assert_eq!(
        use_item(&mut w, &c, "pill_healing", rev + 1),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        use_item(&mut w, &c, &c.item_id, rev),
        Err(Error::InvalidTarget)
    );
    w.protect(0, 0, &mut vec![]);
    assert_eq!(
        use_item(&mut w, &c, "pill_healing", rev),
        Err(Error::NotReady)
    );
    assert_eq!(w.members[0].hp, hp - 50);
    assert_eq!(w.members[0].inventory, inventory);
    assert_eq!(w.members[0].looted["pill_healing"], 2);
    assert_eq!(w.members[0].revision, rev);
}

#[test]
fn member_item_cooldown_starts_on_commit_and_has_an_exclusive_deadline() {
    let (mut c, mut w) = setup();
    c.mechanism_version = 59;
    let alternate = c.item_id.clone();
    for definition in c.member_item_use.as_mut().unwrap().values_mut() {
        definition.cooldown_ms = Some(300);
    }
    let input = |item_id: &str, expected_revision| Action::UseItem {
        item_id: item_id.into(),
        expected_revision,
    };
    let initial = w.members[0].revision;
    assert_eq!(
        w.action(0, &input("pill_healing", initial), "full", &c, 0),
        Err(Error::InvalidState)
    );
    assert!(w.members[0].item_cooldowns.is_empty());
    w.members[0].hp -= 200;
    *w.members[0].inventory.entry(alternate.clone()).or_default() += 1;
    w.members[0].looted.insert(alternate.clone(), 1);

    w.action(0, &input("pill_healing", initial), "first", &c, 30)
        .unwrap();
    assert_eq!(w.members[0].item_cooldowns["pill_healing"], 330);
    let before = w.snapshot(0, &c, Phase::Active, 329).own;
    assert_eq!(
        w.action(0, &input("pill_healing", initial + 1), "early", &c, 329),
        Err(Error::InvalidState)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 329).own, before);
    w.action(0, &input(&alternate, initial + 1), "alternate", &c, 329)
        .unwrap();
    assert_eq!(w.members[0].item_cooldowns[&alternate], 629);
    assert!(
        w.snapshot(0, &c, Phase::Active, 330)
            .own
            .item_cooldowns
            .iter()
            .all(|r| r.item_id != "pill_healing")
    );
    w.action(0, &input("pill_healing", initial + 2), "boundary", &c, 330)
        .unwrap();
    assert_eq!(w.members[0].item_cooldowns["pill_healing"], 630);
    assert!(!w.members[0].inventory.contains_key("pill_healing"));
    assert_eq!(w.members[0].hp, w.members[0].facts.hp_points - 80);
}

#[test]
fn member_item_spirit_clamps_and_preserves_state_on_rejection() {
    let (mut c, mut w) = setup();
    c.mechanism_version = 60;
    c.member_item_use.as_mut().unwrap().insert(
        "pill_healing".into(),
        serde_json::from_value(json!({"restore_spirit":40,"cooldown_ms":300})).unwrap(),
    );
    let revision = w.members[0].revision;
    let original = w.snapshot(0, &c, Phase::Active, 0).own;
    assert_eq!(
        use_item(&mut w, &c, "pill_healing", revision),
        Err(Error::InvalidState)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 0).own, original);
    w.members[0].spirit -= 9;
    w.members[0].hp -= 13;
    assert_eq!(
        use_item(&mut w, &c, "pill_healing", revision + 1),
        Err(Error::RevisionConflict)
    );
    let events = use_item(&mut w, &c, "pill_healing", revision).unwrap();
    assert!(matches!(events.as_slice(),
        [(Some(0), EventV4::Private {event: Event::ItemUsed {item_id,hp_restored:0,spirit_restored:Some(9),..}})]
        if item_id == "pill_healing"));
    assert_eq!(w.members[0].spirit, w.members[0].facts.spirit_points);
    assert_eq!(w.members[0].hp, original.hp - 13);
    assert_eq!(w.members[0].inventory["pill_healing"], 1);
    w.members[0].spirit -= 12;
    let cooling = w.snapshot(0, &c, Phase::Active, 0).own;
    assert_eq!(
        use_item(&mut w, &c, "pill_healing", revision + 1),
        Err(Error::InvalidState)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 0).own, cooling);
    assert_eq!(cooling.item_cooldowns[0].ready_at_ms, 300);
}

#[test]
fn member_item_cooldown_group_blocks_siblings_until_deadline() {
    let (mut c, mut w) = setup();
    c.mechanism_version = 61;
    let definitions = c.member_item_use.as_mut().unwrap();
    let health = definitions.get_mut("pill_healing").unwrap();
    health.cooldown_ms = Some(300);
    health.cooldown_group = Some("recovery".into());
    definitions.insert(
        "pill_ningqi".into(),
        serde_json::from_value(
            json!({"restore_spirit":40,"cooldown_ms":300,"cooldown_group":"recovery"}),
        )
        .unwrap(),
    );
    let outside = c.item_id.clone();
    let member = &mut w.members[0];
    let initial_revision = member.revision;
    for item in ["pill_ningqi", outside.as_str()] {
        *member.inventory.entry(item.into()).or_default() += 1;
        member.looted.insert(item.into(), 1);
    }
    let input = |item_id: &str, expected_revision| Action::UseItem {
        item_id: item_id.into(),
        expected_revision,
    };
    assert_eq!(
        w.action(0, &input("pill_healing", initial_revision), "full", &c, 0),
        Err(Error::InvalidState)
    );
    assert!(w.members[0].item_cooldowns.is_empty());
    w.members[0].hp -= 100;
    w.members[0].spirit -= 15;
    w.action(
        0,
        &input("pill_healing", initial_revision),
        "health",
        &c,
        30,
    )
    .unwrap();
    let before = w.snapshot(0, &c, Phase::Active, 329).own;
    assert_eq!(before.item_cooldowns.len(), 2);
    assert!(
        before
            .item_cooldowns
            .iter()
            .all(|entry| entry.ready_at_ms == 330)
    );
    assert_eq!(
        w.action(
            0,
            &input("pill_ningqi", initial_revision + 1),
            "early",
            &c,
            329
        ),
        Err(Error::InvalidState)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 329).own, before);
    w.action(
        0,
        &input(&outside, initial_revision + 1),
        "outside",
        &c,
        329,
    )
    .unwrap();
    let used = w
        .action(
            0,
            &input("pill_ningqi", initial_revision + 2),
            "boundary",
            &c,
            330,
        )
        .unwrap();
    assert!(matches!(
        used.as_slice(),
        [(
            Some(0),
            EventV4::Private {
                event: Event::ItemUsed {
                    spirit_restored: Some(15),
                    ..
                }
            }
        )]
    ));
    let after = w.snapshot(0, &c, Phase::Active, 330).own;
    assert_eq!(after.item_cooldowns.len(), 2);
    assert!(
        after
            .item_cooldowns
            .iter()
            .all(|entry| entry.ready_at_ms == 630)
    );
}
