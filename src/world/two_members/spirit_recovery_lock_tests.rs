use super::combat::DirectContact;
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

#[test]
fn member_spirit_recovery_lock_configuration_accepts_only_full_member_self_locks() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-spirit-recovery-lock-host/arena-a.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    assert!(load(&doc).is_ok());
    let mut item = doc.clone();
    item["member_item_use"] = json!({
        "xue-zhi": { "restore_spirit": 30, "on_use_self_statuses": ["spirit-seal"] }
    });
    assert!(load(&item).is_ok());

    let def = doc["pve"]["status_definitions"].as_array().unwrap().len() - 1;
    for (path, value) in [
        ("/mechanism_version".to_owned(), json!(72)),
        (
            "/skill/on_hit_target_statuses".to_owned(),
            json!(["spirit-seal"]),
        ),
        (
            "/pve/combat/projectile/on_commit_self_statuses".to_owned(),
            json!(["spirit-seal"]),
        ),
        (
            format!("/pve/status_definitions/{def}/modifier_bps"),
            json!(-9999),
        ),
        (
            format!("/pve/status_definitions/{def}/duplicate_policy"),
            json!("stack"),
        ),
    ] {
        let mut invalid = doc.clone();
        *invalid.pointer_mut(&path).unwrap() = value;
        assert!(load(&invalid).is_err(), "{path}");
    }
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::combat_tests::setup(2);
    c.mechanism_version = 73;
    c.params.regen.interval_ms = 1000;
    c.params.regen.near_ratio = 0.01;
    c.params.regen.far_ratio = 0.01;
    w.next_regen = 1000;
    for m in &mut w.members {
        m.facts.hp_points = 1000;
        m.facts.spirit_points = 200;
        m.hp = 400;
        m.spirit = 20;
    }
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({
        "id":"seal","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock",
        "modifier_bps":-10000,"duration_ms":2000,"duplicate_policy":"refresh","source_scoped":true
    })).unwrap());
    (c, w)
}

#[test]
fn member_spirit_recovery_lock_gates_regen_basic_hits_and_owned_orbs_until_exact_expiry() {
    let (c, mut w) = setup();
    let owner = w.members[0].reference.entity.clone();
    bind(&mut w, &c, Owner::Member(0), owner.clone(), "seal", 0);
    w.npcs.truncate(1);
    Arc::make_mut(&mut w.npcs[0].profile).combat.aggro_range = 0.1;
    for i in 0..2 {
        let source = w.members[i].reference.entity.clone();
        assert!(w.direct_contact(
            Owner::Member(i),
            Owner::Npc(0),
            DirectContact {
                origin: None,
                action_id: None,
                rule: None,
                ignore_armor: false,
                unavoidable: false,
                dead_source: false,
                source: &source,
                raw: 1,
                metal: false,
                hit_statuses: &[]
            },
            &c,
            30,
            &mut vec![]
        ));
    }
    assert_eq!((w.members[0].spirit, w.members[1].spirit), (20, 22));
    w.tick(&c, 1000);
    assert_eq!((w.members[0].hp, w.members[0].spirit), (410, 20));
    assert_eq!((w.members[1].hp, w.members[1].spirit), (410, 24));

    for (now, hp, spirit) in [(1100, 440, 20), (2000, 470, 50)] {
        let resource_id = format!("orb-{now}");
        w.resources.push(Resource {
            view: ResourceViewV4 {
                id: resource_id.clone(),
                position: w.members[0].position,
                quantity: 0,
                item_id: None,
                owner: Some(owner.clone()),
                expires_at_ms: now + 300,
                instance: None,
            },
            hp: 30,
            spirit: 30,
            player_drop: false,
            origin: Default::default(),
        });
        let before = w.snapshot(0, &c, Phase::Active, now).own;
        w.action(
            0,
            &Action::Pickup {
                resource_id: resource_id.clone(),
                expected_revision: before.inventory_revision,
            },
            &format!("pickup-{now}"),
            &c,
            now,
        )
        .unwrap();
        let after = w.snapshot(0, &c, Phase::Active, now).own;
        assert_eq!((after.hp, after.spirit), (hp, spirit));
        assert_eq!(after.inventory, before.inventory);
        assert_eq!(after.inventory_revision, before.inventory_revision + 1);
        assert!(w.resources.iter().all(|r| r.view.id != resource_id));
    }
}

#[test]
fn member_spirit_recovery_lock_preserves_pure_items_and_resolves_hp_on_dual_items() {
    let (mut c, mut w) = setup();
    c.member_item_use = serde_json::from_value(json!({
        "seal-tonic":{"restore_spirit":40,"on_use_self_statuses":["seal"]},
        "spirit-tonic":{"restore_spirit":30,"cooldown_ms":900},
        "dual-tonic":{"restore_spirit":30,"heal_hp":30,"cooldown_ms":900},
    }))
    .unwrap();
    for id in ["seal-tonic", "spirit-tonic", "dual-tonic"] {
        w.members[0].inventory.insert(id.into(), 2);
        w.members[0].looted.insert(id.into(), 2);
    }
    let dose = |w: &mut World, item: &str, now| {
        let revision = w.members[0].revision;
        w.use_item(0, item, revision, &c, now, &mut vec![])
    };
    assert_eq!(dose(&mut w, "seal-tonic", 0), Ok((0, Some(40))));
    assert_eq!(w.members[0].spirit, 60);
    let before = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(dose(&mut w, "spirit-tonic", 30), Err(Error::InvalidState));
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own, before);
    assert_eq!(w.members[0].looted["spirit-tonic"], 2);
    assert_eq!(dose(&mut w, "dual-tonic", 30), Ok((30, Some(0))));
    assert_eq!((w.members[0].hp, w.members[0].spirit), (430, 60));
    assert_eq!(w.members[0].looted["dual-tonic"], 1);
    assert_eq!(w.members[0].item_cooldowns["dual-tonic"], 930);
    assert_eq!(dose(&mut w, "spirit-tonic", 2000), Ok((0, Some(30))));
    assert_eq!(w.members[0].spirit, 90);
}
