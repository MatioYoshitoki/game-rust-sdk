use super::control_immunity_tests::{hit, own_events};
use super::periodic_tests::bind;
use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};

const ITEM: &str = "cleanse-tonic";

#[test]
fn member_item_cleanse_requires_explicit_unique_silence_definitions() {
    let doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-item-cleanse-host/arena-a.json"
    ))
    .unwrap();
    let package = Package::new();
    let load = |doc: &Value| {
        package.write(doc);
        package.load()
    };
    let c = load(&doc).unwrap().default_config();
    let item = &c.member_item_use.as_ref().unwrap()[ITEM];
    assert_eq!(
        serde_json::to_value(item).unwrap(),
        doc["member_item_use"][ITEM]
    );
    let mut old = doc.clone();
    old["mechanism_version"] = json!(86);
    assert!(load(&old).is_err());
    for ids in [
        json!(null),
        json!([]),
        json!(["missing"]),
        json!(["wound"]),
        json!(["npc-silence", "npc-silence"]),
        json!(["npc-silence", "wound", "ward"]),
    ] {
        let mut invalid = doc.clone();
        invalid["member_item_use"][ITEM]["cleanse_self_statuses"] = ids;
        assert!(load(&invalid).is_err());
    }
    for (field, value) in [
        ("heal_hp", json!(10)),
        ("restore_spirit", json!(5)),
        ("quality_multiplier_bps", json!(10000)),
        ("on_use_self_statuses", json!(["ward"])),
    ] {
        let mut invalid = doc.clone();
        invalid["member_item_use"][ITEM][field] = value;
        assert!(load(&invalid).is_err());
    }
}

fn setup() -> (Config, World) {
    let (mut c, mut w) = super::control_immunity_tests::setup();
    c.mechanism_version = 87;
    c.member_item_use = serde_json::from_value(json!({
        ITEM:{"cleanse_self_statuses":["npc-silence"],"cooldown_ms":300}
    }))
    .unwrap();
    let pve = c.pve_mut();
    pve.status_rules.as_mut().unwrap().max_active_per_entity = 8;
    let mut extra = pve
        .status_definitions
        .iter()
        .find(|d| d.id == "npc-silence")
        .unwrap()
        .clone();
    extra.id = "other-silence".into();
    pve.status_definitions.push(extra);
    w.members[0].inventory.insert(ITEM.into(), 2);
    w.members[0].looted.insert(ITEM.into(), 2);
    (c, w)
}

fn use_item(w: &mut World, c: &Config, revision: u32, now: u64) -> Result<Events> {
    w.action(
        0,
        &Action::UseItem {
            item_id: ITEM.into(),
            expected_revision: revision,
        },
        "cleanse",
        c,
        now,
    )
}

#[test]
fn member_item_cleanse_consumes_once_and_allows_a_new_cast_and_silence() {
    let (c, mut w) = setup();
    hit(&mut w, &c, &["npc-silence", "wound"], 30);
    let before = w.snapshot(0, &c, Phase::Active, 30).own;
    let control = before
        .statuses
        .iter()
        .find(|s| s.attribute == Some(StatusAttribute::Silence))
        .unwrap();
    let wound = before
        .statuses
        .iter()
        .find(|s| s.definition_id == "wound")
        .unwrap();
    let revision = before.inventory_revision;
    let cast = Action::Cast {
        skill_id: "cast".into(),
        direction: [1., 0.],
    };
    assert_eq!(
        w.action(0, &cast, "blocked", &c, 60),
        Err(Error::InvalidState)
    );
    let events = use_item(&mut w, &c, revision, 60).unwrap();
    assert!(matches!(own_events(&events).as_slice(), [
        Event::StatusRemoved {instance_ids, reason: StatusRemovalReason::Cleansed},
        Event::ItemUsed {item_id, hp_restored: 0, spirit_restored: None, ..}
    ] if item_id == ITEM && instance_ids == std::slice::from_ref(&control.instance_id)));
    let after = w.snapshot(0, &c, Phase::Active, 60).own;
    assert_eq!(after.statuses, std::slice::from_ref(wound));
    assert_eq!((after.hp, after.spirit), (before.hp, before.spirit));
    assert_eq!(after.inventory_revision, revision + 1);
    assert_eq!(
        (w.members[0].inventory[ITEM], w.members[0].looted[ITEM]),
        (1, 1)
    );
    assert_eq!(w.members[0].item_cooldowns[ITEM], 360);
    assert!(w.members[0].pending.is_none());
    w.action(0, &cast, "new-cast", &c, 60).unwrap();
    hit(&mut w, &c, &["npc-silence"], 90);
    assert!(w.skill_silenced(0, 90) && w.members[0].pending.is_none());
}

#[test]
fn member_item_cleanse_limits_selection_to_current_target_and_definition() {
    let (c, mut w) = setup();
    hit(&mut w, &c, &["npc-silence", "other-silence"], 30);
    let own = w.members[0].reference.entity.clone();
    let peer = w.members[1].reference.entity.clone();
    assert!(!bind(&mut w, &c, Owner::Npc(0), peer, "npc-silence", 30).is_empty());
    assert!(!bind(&mut w, &c, Owner::Npc(1), own, "npc-silence", 30).is_empty());
    let before = w.statuses.entries().to_vec();
    let expected: Vec<_> = before
        .iter()
        .filter(|s| {
            s.target.id == w.members[0].reference.entity.id && s.definition_id == "npc-silence"
        })
        .map(|s| s.instance_id.clone())
        .collect();
    assert_eq!(expected.len(), 2);
    let revision = w.members[0].revision;
    let events = use_item(&mut w, &c, revision, 60).unwrap();
    assert!(matches!(own_events(&events).as_slice(), [
        Event::StatusRemoved {instance_ids, reason: StatusRemovalReason::Cleansed}, Event::ItemUsed {..}
    ] if instance_ids == &expected));
    assert!(w.skill_silenced(0, 60) && w.skill_silenced(1, 60));
    assert_eq!(w.statuses.entries().len(), 2);
    assert!(w.statuses.entries().iter().all(|s| before.contains(s)));
}

#[test]
fn member_item_cleanse_rejects_failures_without_inventory_or_status_writes() {
    let (c, mut w) = setup();
    let revision = w.members[0].revision;
    let before = w.snapshot(0, &c, Phase::Active, 0).own;
    assert_eq!(use_item(&mut w, &c, revision, 0), Err(Error::InvalidState));
    assert_eq!(w.snapshot(0, &c, Phase::Active, 0).own, before);
    hit(&mut w, &c, &["npc-silence"], 30);
    let before = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        use_item(&mut w, &c, revision + 1, 30),
        Err(Error::RevisionConflict)
    );
    w.members[0].looted.remove(ITEM);
    assert_eq!(
        use_item(&mut w, &c, revision, 30),
        Err(Error::InvalidTarget)
    );
    w.members[0].looted.insert(ITEM.into(), 2);
    assert_eq!(w.snapshot(0, &c, Phase::Active, 30).own, before);
    let expired = w.snapshot(0, &c, Phase::Active, 630).own;
    assert_eq!(
        use_item(&mut w, &c, revision, 630),
        Err(Error::InvalidState)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 630).own, expired);
    hit(&mut w, &c, &["npc-silence"], 660);
    use_item(&mut w, &c, revision, 690).unwrap();
    hit(&mut w, &c, &["npc-silence"], 720);
    let before = w.snapshot(0, &c, Phase::Active, 720).own;
    assert_eq!(
        use_item(&mut w, &c, revision + 1, 720),
        Err(Error::InvalidState)
    );
    assert_eq!(w.snapshot(0, &c, Phase::Active, 720).own, before);
    assert_eq!(w.members[0].looted[ITEM], 1);
}
