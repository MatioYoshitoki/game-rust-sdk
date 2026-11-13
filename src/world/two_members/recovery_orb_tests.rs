use super::*;
use crate::{asset_contract::tests::Package, config::RecoveryOrb};
use serde_json::{Value, json};

#[test]
fn member_recovery_orb_configuration_requires_enabled_drop_and_valid_rates() {
    let mut doc: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/member-recovery-orb-host/arena-b.json"
    ))
    .unwrap();
    let pkg = Package::new();
    let load = |doc: &Value| {
        pkg.write(doc);
        pkg.load()
    };
    assert!(load(&doc).is_ok());
    let template = "/pve/additional_templates/orb-target";
    for value in [
        json!(null),
        json!({"hp_bps": 1500}),
        json!({"hp_bps": 1.5, "spirit_bps": 6000}),
        json!({"hp_bps": -1, "spirit_bps": 6000}),
        json!({"hp_bps": 0, "spirit_bps": 0}),
        json!({"hp_bps": 10001, "spirit_bps": 6000}),
        json!({"hp_bps": 1500, "spirit_bps": 10001}),
    ] {
        doc.pointer_mut(template).unwrap()["recovery_orb"] = value;
        assert!(load(&doc).is_err());
    }
    doc.pointer_mut(template).unwrap()["recovery_orb"] = json!({"hp_bps": 0, "spirit_bps": 10000});
    assert!(load(&doc).is_ok());
    doc.pointer_mut(template).unwrap()["drop_orb"] = json!(false);
    assert!(load(&doc).is_err());
    doc.pointer_mut(template).unwrap()["drop_orb"] = json!(true);
    doc["mechanism_version"] = json!(70);
    assert!(load(&doc).is_err());
    doc.pointer_mut(template)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("recovery_orb");
    assert!(load(&doc).is_ok());
}

fn pickup(
    w: &mut World,
    c: &Config,
    i: usize,
    orb: &ResourceViewV4,
    revision: u32,
    now: u64,
) -> Result<Events> {
    w.action(
        i,
        &Action::Pickup {
            resource_id: orb.id.clone(),
            expected_revision: revision,
        },
        &format!("orb-{i}-{now}"),
        c,
        now,
    )
}

#[test]
fn member_recovery_orb_freezes_killer_rates_and_only_changes_recovery_and_revision() {
    for (mechanism, rates, hp, spirit) in [
        (70, None, 30, 30),
        (71, Some((300, 1500)), 30, 30),
        (71, Some((1500, 6000)), 150, 121),
    ] {
        let (mut c, mut w) = super::combat_tests::setup(2);
        c.mechanism_version = mechanism;
        let profile = Arc::make_mut(&mut w.npcs[0].profile);
        profile.drop_orb = true;
        profile.recovery_orb = rates.map(|(hp_bps, spirit_bps)| RecoveryOrb { hp_bps, spirit_bps });
        w.members[0].facts.hp_points = 1003;
        w.members[0].facts.spirit_points = 203;
        w.members[0].hp = 10;
        w.members[0].spirit = 5;
        w.hurt_npc(0, 0, u32::MAX, &c, 30, &mut vec![]);
        assert_eq!((w.members[0].hp, w.members[0].spirit), (10, 5));
        let orb = w
            .resources
            .iter()
            .find(|r| r.view.item_id.is_none())
            .unwrap();
        assert_eq!((orb.hp, orb.spirit), (hp, spirit));
        let orb = orb.view.clone();
        assert_eq!(orb.owner.as_ref(), Some(&w.members[0].reference.entity));
        for m in &mut w.members {
            m.position = orb.position;
        }
        w.members[0].facts.hp_points = 2000;
        w.members[0].facts.spirit_points = 500;
        w.members[0].hp = 2000 - hp + 1;
        w.members[0].spirit = 10;

        let before = w.snapshot(0, &c, Phase::Active, 60).own;
        let other = w.snapshot(1, &c, Phase::Active, 60).own;
        assert_eq!(
            pickup(&mut w, &c, 1, &orb, other.inventory_revision, 60),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            pickup(&mut w, &c, 0, &orb, before.inventory_revision + 1, 60),
            Err(Error::RevisionConflict)
        );
        let events = pickup(&mut w, &c, 0, &orb, before.inventory_revision, 60).unwrap();
        assert!(events.iter().any(|(recipient, event)| *recipient == Some(0)
            && matches!(event, EventV4::Private { event: Event::PickedUp { resource_id, quantity: 0 } }
                if resource_id == &orb.id)));
        let after = w.snapshot(0, &c, Phase::Active, 60).own;
        assert_eq!((after.hp, after.spirit), (2000, 10 + spirit));
        assert_eq!(after.inventory, before.inventory);
        assert_eq!(after.inventory_revision, before.inventory_revision + 1);
        assert!(w.members[0].looted.is_empty());
        assert_eq!(w.snapshot(1, &c, Phase::Active, 60).own, other);
        assert_eq!(
            pickup(&mut w, &c, 0, &orb, after.inventory_revision, 90),
            Err(Error::InvalidTarget)
        );
    }
}

#[test]
fn member_recovery_orb_full_pickup_consumes_and_expiry_boundary_rejects() {
    for expired in [false, true] {
        let (mut c, mut w) = super::combat_tests::setup(2);
        c.mechanism_version = 71;
        let profile = Arc::make_mut(&mut w.npcs[0].profile);
        profile.drop_orb = true;
        profile.recovery_orb = Some(RecoveryOrb {
            hp_bps: 10000,
            spirit_bps: 0,
        });
        w.hurt_npc(0, 0, u32::MAX, &c, 30, &mut vec![]);
        let orb = w
            .resources
            .iter()
            .find(|r| r.view.item_id.is_none())
            .unwrap()
            .view
            .clone();
        w.members[0].position = orb.position;
        let now = if expired {
            orb.expires_at_ms
        } else {
            orb.expires_at_ms - 1
        };
        let before = w.snapshot(0, &c, Phase::Active, now).own;
        let result = pickup(&mut w, &c, 0, &orb, before.inventory_revision, now);
        let after = w.snapshot(0, &c, Phase::Active, now).own;
        if expired {
            assert_eq!(result, Err(Error::InvalidTarget));
            assert_eq!(after, before);
        } else {
            assert!(result.is_ok());
            assert_eq!((after.hp, after.spirit), (before.hp, before.spirit));
            assert_eq!(after.inventory_revision, before.inventory_revision + 1);
            assert!(w.resources.iter().all(|r| r.view.id != orb.id));
        }
    }
}
