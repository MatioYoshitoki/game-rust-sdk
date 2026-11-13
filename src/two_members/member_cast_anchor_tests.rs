use super::*;

const PLACE: &str = "anchor-place";
const BURST: &str = "anchor-burst";
const BOLT: &str = "anchor-bolt";
const POINT: [f64; 2] = [560., 400.];

fn configure(v: &mut Value) {
    *v = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/mastery-stages.json"
    ))
    .unwrap();
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/member-cast-anchor.json"
    ))
    .unwrap();
    v["additional_skills"]
        .as_array_mut()
        .unwrap()
        .extend(sample["skills"].as_array().unwrap().iter().cloned());
    v["combat_rules"]["member_cast_anchors"] = sample["member_cast_anchors"].clone();
    for id in [BURST, BOLT] {
        v["combat_rules"]["skills"][id] = sample["damage_rule"].clone();
    }
    v["pve"]["monster_positions"] = json!([[600, 400]]);
}

fn host(change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let mut h = DeltaHarness::new(2, |v| {
        configure(v);
        change(v);
    });
    h.active();
    h
}

fn go_to(h: &mut DeltaHarness, at: u64) {
    let steps = at
        .saturating_sub(h.engine.now)
        .div_ceil(h.engine.config.step_ms);
    h.ticks(steps as usize);
}

fn anchor(h: &DeltaHarness, recipient: usize, source: usize) -> Option<&AreaPlanViewV4> {
    let entity = &h.snapshot(source).own.participant.entity;
    h.snapshot(recipient)
        .areas
        .as_ref()?
        .iter()
        .find(|a| a.mechanism == AreaPlanKind::MemberCastAnchor && a.source == *entity)
}

fn place(h: &mut DeltaHarness, i: usize, point: [f64; 2]) -> AreaPlanViewV4 {
    h.action(
        i,
        Action::CastGround {
            skill_id: PLACE.into(),
            position: point,
        },
    )
    .unwrap();
    h.ticks(2);
    anchor(h, i, i).unwrap().clone()
}

fn use_anchor(id: &str, skill: &str, direction: Option<[f64; 2]>) -> Action {
    Action::CastAtAnchor {
        skill_id: skill.into(),
        anchor_id: id.into(),
        direction,
    }
}

fn ended(h: &DeltaHarness, id: &str, reason: AreaEndReason) -> bool {
    h.events.iter().any(
        |(_, e)| matches!(e, EventV4::AreaEnded {area_id,reason:r} if area_id==id && *r==reason),
    )
}

#[test]
fn member_cast_anchor_delivers_a_circle_and_projectile_from_the_same_fixed_point() {
    let mut h = host(|_| {});
    let initial = h.snapshot(0).own.spirit;
    let hp = h.snapshot(0).monsters[0].hp;
    let p = place(&mut h, 0, POINT);
    assert_eq!(
        (p.center, p.radius, p.created_at_ms, p.activates_at_ms),
        (POINT, 0., p.created_at_ms, p.created_at_ms)
    );
    assert_eq!(p.expires_at_ms - p.created_at_ms, 3000);
    assert_eq!(h.snapshot(0).own.spirit, initial - 2);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp);
    assert_ne!(h.snapshot(0).own.position, POINT);
    assert_eq!(
        h.action(
            0,
            Action::CastGround {
                skill_id: BURST.into(),
                position: POINT
            }
        ),
        Err(Error::OutOfRange)
    );
    h.action(0, use_anchor(&p.id, BURST, None)).unwrap();
    h.ticks(10);
    assert!(h.snapshot(0).areas.as_ref().unwrap().iter().any(|a| {
        a.mechanism == AreaPlanKind::Circle
            && a.skill_id == BURST
            && a.center == POINT
            && a.source == p.source
    }));
    h.ticks(4);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp - 100);
    h.action(0, use_anchor(&p.id, BOLT, Some([2., 0.])))
        .unwrap();
    h.ticks(3);
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::ProjectileStarted{projectile} if projectile.position==POINT && projectile.source==p.source)));
    h.ticks(4);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp - 200);
    assert_eq!(h.snapshot(0).own.spirit, initial - 8);
    assert_eq!(anchor(&h, 0, 0), Some(&p));
}

#[test]
fn member_cast_anchor_rejects_other_owners_unbound_skills_and_wrong_aim() {
    let mut h = host(|_| {});
    let p = place(&mut h, 0, POINT);
    let before = h.snapshot(1).own.spirit;
    assert_eq!(
        h.action(1, use_anchor(&p.id, BURST, None)),
        Err(Error::InvalidTarget)
    );
    assert_eq!(
        h.action(0, use_anchor(&p.id, "seed-field", None)),
        Err(Error::InvalidTarget)
    );
    assert_eq!(
        h.action(0, use_anchor("missing", BURST, None)),
        Err(Error::InvalidTarget)
    );
    assert_eq!(
        h.action(0, use_anchor(&p.id, BURST, Some([1., 0.]))),
        Err(Error::InvalidMessage)
    );
    assert_eq!(
        h.action(0, use_anchor(&p.id, BOLT, None)),
        Err(Error::InvalidMessage)
    );
    assert_eq!(
        h.action(0, use_anchor(&p.id, BOLT, Some([0., 0.]))),
        Err(Error::InvalidMessage)
    );
    h.ticks(1);
    assert_eq!(h.snapshot(1).own.spirit, before);
    assert!(h.snapshot(0).own.pending_action.is_none());
}

#[test]
fn member_cast_anchor_rechecks_expiry_and_reach_before_charging() {
    for expires in [false, true] {
        let mut h = host(|v| {
            if expires {
                v["combat_rules"]["member_cast_anchors"][PLACE]["duration_ms"] = json!(150);
            }
        });
        let p = place(&mut h, 0, POINT);
        let before = h.snapshot(0).own.spirit;
        h.action(0, use_anchor(&p.id, BURST, None)).unwrap();
        if !expires {
            h.engine.world.as_mut().unwrap().members[0].position = [100., 400.];
        }
        h.ticks(10);
        assert_eq!(h.snapshot(0).own.spirit, before);
        assert!(
            h.snapshot(0)
                .own
                .player_skills
                .iter()
                .any(|s| s.skill_id == BURST && s.ready_at_ms == 0)
        );
        let expected = if expires {
            Error::InvalidTarget
        } else {
            Error::OutOfRange
        };
        assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::Private{event:Event::ActionRejected{reason,..}} if *reason==expected)));
        assert!(
            h.snapshot(0)
                .areas
                .as_ref()
                .unwrap()
                .iter()
                .all(|a| a.skill_id != BURST)
        );
    }
}

#[test]
fn member_cast_anchor_replaces_in_a_full_pool_without_reusing_identity() {
    let mut h = host(|v| v["members"]["area_capacity"] = json!(1));
    let initial = h.snapshot(0).own.spirit;
    let p = place(&mut h, 0, POINT);
    go_to(&mut h, p.created_at_ms + 450);
    let next = place(&mut h, 0, [590., 400.]);
    assert_ne!(p.id, next.id);
    assert_eq!(next.center, [590., 400.]);
    assert!(ended(&h, &p.id, AreaEndReason::Replaced));
    assert_eq!(h.snapshot(0).areas.as_ref().unwrap().len(), 1);
    assert_eq!(h.snapshot(0).own.spirit, initial - 4);
    assert_eq!(
        h.action(0, use_anchor(&p.id, BOLT, Some([1., 0.]))),
        Err(Error::InvalidTarget)
    );
    h.action(0, use_anchor(&next.id, BOLT, Some([1., 0.])))
        .unwrap();
    h.ticks(3);
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::ProjectileStarted{projectile} if projectile.position==next.center)));
}

#[test]
fn member_cast_anchor_checks_projectile_footprint_at_the_anchor() {
    let mut h = host(|v| {
        let mut wall = v["pve"]["palette"][0].clone();
        wall["blocksProjectiles"] = json!(true);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let first = 10 * 32 + 14;
        v["pve"]["cells_rle"] = json!([[0, first], [1, 1], [0, 1024 - first - 1]]);
    });
    let p = place(&mut h, 0, [570., 410.]);
    assert_eq!(
        h.action(0, use_anchor(&p.id, BOLT, Some([1., 0.]))),
        Err(Error::InvalidTarget)
    );
    h.action(0, use_anchor(&p.id, BURST, None)).unwrap();
}

#[test]
fn member_cast_anchor_expiry_leaves_committed_area_at_the_same_center() {
    let mut h =
        host(|v| v["combat_rules"]["member_cast_anchors"][PLACE]["duration_ms"] = json!(600));
    let hp = h.snapshot(0).monsters[0].hp;
    let p = place(&mut h, 0, POINT);
    go_to(&mut h, p.created_at_ms + 210);
    h.action(0, use_anchor(&p.id, BURST, None)).unwrap();
    go_to(&mut h, p.expires_at_ms);
    assert!(ended(&h, &p.id, AreaEndReason::Expired));
    assert!(anchor(&h, 0, 0).is_none());
    assert!(
        h.snapshot(0)
            .areas
            .as_ref()
            .unwrap()
            .iter()
            .any(|a| a.skill_id == BURST && a.center == POINT)
    );
    assert_eq!(h.snapshot(0).monsters[0].hp, hp);
    h.ticks(1);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp - 100);
}

#[test]
fn member_cast_anchor_recovers_on_reconnect_and_clears_only_the_leaving_owner() {
    let mut h = host(|_| {});
    let p = place(&mut h, 0, POINT);
    let peer = place(&mut h, 1, [560., 480.]);
    let previous = h.engine.seats[0].connection.clone();
    h.action(0, Action::ConnectionLost {}).unwrap();
    let mut reconnect = h.input(
        0,
        Action::Reconnect {
            previous_connection: previous.clone(),
            authenticated_principal: h.stores[0].principal.clone(),
        },
    );
    reconnect.connection = Connection {
        id: "anchor-reconnect".into(),
        generation: previous.generation + 1,
    };
    reconnect.sequence = 1;
    h.engine.input(reconnect).unwrap();
    h.pump();
    assert_eq!(anchor(&h, 0, 0), Some(&p));
    h.action(0, Action::Foreground {}).unwrap();
    h.pump();
    let publication_id = h.replicas[0].last_full_id().unwrap().to_string();
    h.action(0, Action::CombatReady { publication_id }).unwrap();
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(ended(&h, &p.id, AreaEndReason::SourceInvalid));
    assert!(anchor(&h, 1, 0).is_none());
    assert_eq!(anchor(&h, 1, 1), Some(&peer));
    assert!(anchor(&h, 0, 1).is_none());
    h.close();
    assert!(anchor(&h, 1, 1).is_none());
}

#[test]
fn member_cast_anchor_rejects_invalid_or_recursive_bindings() {
    for (key, value) in [
        ("duration_ms", json!(0)),
        ("max_use_distance", json!(10001)),
        ("consumer_skill_ids", json!([PLACE])),
        ("consumer_skill_ids", json!([BURST, BURST])),
        ("consumer_skill_ids", json!(["seed-strike"])),
    ] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                v["combat_rules"]["member_cast_anchors"][PLACE][key] = value;
            })
            .is_err()
        );
    }
}

fn enable_recast(v: &mut Value) {
    v["combat_rules"]["member_cast_anchors"][PLACE]["recast"] =
        json!({"window_ms":1800,"max_count":2,"spirit_cost":4});
}

fn recast(p: &AreaPlanViewV4, recast_index: u32, position: [f64; 2]) -> Action {
    Action::RecastAnchor {
        anchor_id: p.id.clone(),
        action_id: p.anchor_recast.as_ref().unwrap().action_id.clone(),
        recast_index,
        position,
    }
}

#[test]
fn member_cast_anchor_recast_moves_same_area_once_and_preserves_released_effects() {
    let mut h = host(enable_recast);
    let spirit = h.snapshot(0).own.spirit;
    let hp = h.snapshot(0).monsters[0].hp;
    let p = place(&mut h, 0, POINT);
    let cooldown = p.created_at_ms + 450;
    let moved = [580., 400.];
    let request = h.input(0, recast(&p, 1, moved));
    let accepted = h.engine.input(request.clone()).unwrap();
    assert_eq!(h.engine.input(request).unwrap(), accepted);
    assert_eq!(
        h.action(0, recast(&p, 1, [570., 400.])),
        Err(Error::InvalidTarget)
    );
    h.ticks(1);
    let mut expected = p.clone();
    expected.center = moved;
    expected.anchor_recast.as_mut().unwrap().used_count = 1;
    assert_eq!(anchor(&h, 1, 0), Some(&expected));
    assert_eq!(h.snapshot(0).own.spirit, spirit - 6);
    assert!(
        h.snapshot(0)
            .own
            .player_skills
            .iter()
            .any(|s| { s.skill_id == PLACE && s.ready_at_ms == cooldown })
    );
    h.action(0, use_anchor(&p.id, BURST, None)).unwrap();
    h.ticks(11);
    let moved_again = [560., 400.];
    h.action(0, recast(&p, 2, moved_again)).unwrap();
    h.ticks(1);
    assert!(
        h.snapshot(1)
            .areas
            .as_ref()
            .unwrap()
            .iter()
            .any(|a| { a.skill_id == BURST && a.center == moved })
    );
    assert_eq!(anchor(&h, 1, 0).unwrap().center, moved_again);
    assert_eq!(h.action(0, recast(&p, 3, moved)), Err(Error::InvalidTarget));
    h.action(0, use_anchor(&p.id, BOLT, Some([1., 0.])))
        .unwrap();
    h.ticks(7);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp - 200);
    assert_eq!(h.snapshot(0).own.spirit, spirit - 16);
    assert!(h.events.iter().any(|(_, e)| matches!(
        e, EventV4::ProjectileStarted { projectile }
        if projectile.position == moved_again && projectile.source == p.source
    )));
    let relocations: Vec<_> = h
        .events
        .iter()
        .filter_map(|(recipient, e)| match e {
            EventV4::AnchorRelocated {
                area_id,
                action_id,
                source,
                recast_index,
                center,
            } => {
                assert!(recipient.is_none());
                assert_eq!(area_id, &p.id);
                assert_eq!(action_id, &p.anchor_recast.as_ref().unwrap().action_id);
                assert_eq!(source, &p.source);
                Some((*recast_index, *center))
            }
            _ => None,
        })
        .collect();
    assert_eq!(relocations, [(1, moved), (2, moved_again)]);
    assert_eq!(
        h.events
            .iter()
            .filter(|(_, e)| matches!(
                e, EventV4::Private { event: Event::ActionCommitted { skill_id: Some(id), .. } }
                if id == PLACE
            ))
            .count(),
        1
    );
}

#[test]
fn member_cast_anchor_recast_rejects_invalid_intents_and_resource_without_mutation() {
    let mut h = host(|v| {
        enable_recast(v);
        let mut wall = v["pve"]["palette"][0].clone();
        wall["walkable"] = json!(false);
        v["pve"]["palette"].as_array_mut().unwrap().push(wall);
        let first = 11 * 32 + 15;
        v["pve"]["cells_rle"] = json!([[0, first], [1, 1], [0, 1024 - first - 1]]);
    });
    let p = place(&mut h, 0, POINT);
    let spirit = h.snapshot(0).own.spirit;
    let valid = [580., 400.];
    assert_eq!(h.action(1, recast(&p, 1, valid)), Err(Error::InvalidTarget));
    let mut forged = p.clone();
    forged.anchor_recast.as_mut().unwrap().action_id = "forged".into();
    assert_eq!(
        h.action(0, recast(&forged, 1, valid)),
        Err(Error::InvalidTarget)
    );
    for (index, position, err) in [
        (0, valid, Error::InvalidMessage),
        (2, valid, Error::InvalidTarget),
        (1, POINT, Error::InvalidTarget),
        (1, [f64::NAN, 400.], Error::InvalidMessage),
        (1, [-1., 400.], Error::InvalidTarget),
        (1, [610., 450.], Error::InvalidTarget),
        (1, [1260., 400.], Error::OutOfRange),
    ] {
        assert_eq!(h.action(0, recast(&p, index, position)), Err(err));
    }
    h.action(0, use_anchor(&p.id, BURST, None)).unwrap();
    assert_eq!(h.action(0, recast(&p, 1, valid)), Err(Error::InvalidState));
    h.ticks(1);
    assert_eq!(h.snapshot(0).own.spirit, spirit);
    h.action(0, Action::CancelAction {}).unwrap();
    h.engine.world.as_mut().unwrap().members[0].spirit = 3;
    assert_eq!(h.action(0, recast(&p, 1, valid)), Err(Error::NoResource));
    h.ticks(1);
    assert_eq!(h.snapshot(0).own.spirit, 3);
    assert_eq!(anchor(&h, 1, 0), Some(&p));
    assert!(
        !h.events
            .iter()
            .any(|(_, e)| matches!(e, EventV4::AnchorRelocated { .. }))
    );
}

#[test]
fn member_cast_anchor_recast_uses_exclusive_window_and_replacement_identity() {
    let mut h = host(|v| {
        enable_recast(v);
        v["combat_rules"]["member_cast_anchors"][PLACE]["recast"]["window_ms"] = json!(600);
        v["members"]["area_capacity"] = json!(1);
    });
    let p = place(&mut h, 0, POINT);
    let window = p.anchor_recast.as_ref().unwrap().expires_at_ms;
    go_to(&mut h, window);
    assert_eq!(
        h.action(0, recast(&p, 1, [580., 400.])),
        Err(Error::InvalidTarget)
    );
    h.action(0, use_anchor(&p.id, BOLT, Some([1., 0.])))
        .unwrap();
    h.ticks(5);
    let replacement = place(&mut h, 0, [570., 400.]);
    assert_ne!(replacement.id, p.id);
    assert!(ended(&h, &p.id, AreaEndReason::Replaced));
    assert_eq!(replacement.anchor_recast.as_ref().unwrap().used_count, 0);
    assert_eq!(
        h.action(0, recast(&p, 1, [580., 400.])),
        Err(Error::InvalidTarget)
    );
    h.action(0, recast(&replacement, 1, [580., 400.])).unwrap();
    h.ticks(1);
    assert_eq!(anchor(&h, 0, 0).unwrap().center, [580., 400.]);
}

#[test]
fn member_cast_anchor_recast_validates_opt_in_window_count_and_cost() {
    for bad in [
        Value::Null,
        json!({"window_ms":1799,"max_count":2,"spirit_cost":4}),
        json!({"window_ms":3030,"max_count":2,"spirit_cost":4}),
        json!({"window_ms":1800,"max_count":0,"spirit_cost":4}),
        json!({"window_ms":1800,"max_count":9,"spirit_cost":4}),
        json!({"window_ms":1800,"max_count":2,"spirit_cost":4294967295u32}),
        json!({"window_ms":1800,"max_count":2,"spirit_cost":4,"extra":1}),
    ] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                v["combat_rules"]["member_cast_anchors"][PLACE]["recast"] = bad;
            })
            .is_err()
        );
    }
    let mut h = host(|_| {});
    let mut p = place(&mut h, 0, POINT);
    p.anchor_recast = Some(AnchorRecastView {
        action_id: "missing".into(),
        expires_at_ms: p.expires_at_ms,
        max_count: 2,
        used_count: 0,
        behaviors: None,
    });
    assert_eq!(
        h.action(0, recast(&p, 1, [580., 400.])),
        Err(Error::InvalidTarget)
    );
}

#[test]
fn member_cast_anchor_recast_rejects_forged_replica_transitions_atomically() {
    let mut h = host(enable_recast);
    let p = place(&mut h, 0, POINT);
    let before = h.snapshot(1).clone();
    let mut initial = h.latest(1);
    initial.payload = PublicationPayloadV6::Full {
        snapshot: Box::new(before.clone()),
        reason: FullReason::Initial,
    };
    let mut replica = ReplicaV6::new(initial.key.clone(), initial.recipient_id.clone());
    replica.apply(&initial).unwrap();
    h.action(0, recast(&p, 1, [580., 400.])).unwrap();
    h.ticks(1);
    let moved = h.snapshot(1).clone();
    let delta = |next: &SnapshotV4| PublicationEnvelopeV6 {
        publication_id: "recast-update".into(),
        revision: initial.revision + 1,
        payload: PublicationPayloadV6::Delta {
            base_publication_id: initial.publication_id.clone(),
            base_revision: initial.revision,
            delta: Box::new(crate::replication::diff(&before, next)),
        },
        ..initial.clone()
    };
    for mode in 0..4 {
        let mut forged = moved.clone();
        let area = &mut forged.areas.as_mut().unwrap()[0];
        let r = area.anchor_recast.as_mut().unwrap();
        match mode {
            0 => r.used_count = 0,
            1 => r.expires_at_ms += 30,
            2 => area.expires_at_ms += 30,
            _ => area.center = p.center,
        }
        assert_eq!(
            replica.apply(&delta(&forged)),
            Err(DecodeError::InvalidFrame)
        );
        assert_eq!(replica.snapshot(), Some(&before));
    }
    assert_eq!(replica.apply(&delta(&moved)).unwrap(), &moved);
}

const FOLLOWING_FIELD: &str = "anchor-field";
const MOVED_POINT: [f64; 2] = [560., 600.];

fn enable_following_field(v: &mut Value) {
    enable_recast(v);
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/anchor-following-field.json"
    ))
    .unwrap();
    v["additional_skills"]
        .as_array_mut()
        .unwrap()
        .push(sample["skill"].clone());
    v["combat_rules"]["skills"][FOLLOWING_FIELD] = v["combat_rules"]["skills"][BURST].clone();
    let rule = &mut v["combat_rules"]["member_cast_anchors"][PLACE];
    assert_eq!(sample["anchor_skill_id"], PLACE);
    rule["following_field_skill_ids"] = sample["following_field_skill_ids"].clone();
    rule["consumer_skill_ids"]
        .as_array_mut()
        .unwrap()
        .push(json!(FOLLOWING_FIELD));
    rule["recast"]["max_count"] = json!(3);
    v["pve"]["monster_positions"] = json!([[600, 400], [600, 600]]);
}

fn field<'a>(
    h: &'a DeltaHarness,
    recipient: usize,
    p: &AreaPlanViewV4,
) -> Option<&'a AreaPlanViewV4> {
    h.snapshot(recipient)
        .areas
        .as_ref()?
        .iter()
        .find(|a| a.skill_id == FOLLOWING_FIELD && a.source == p.source)
}

fn place_field(h: &mut DeltaHarness, owner: usize, p: &AreaPlanViewV4) -> AreaPlanViewV4 {
    h.action(owner, use_anchor(&p.id, FOLLOWING_FIELD, None))
        .unwrap();
    h.ticks(2);
    field(h, owner, p).unwrap().clone()
}

#[test]
fn member_cast_anchor_following_field_moves_warning_and_active_ticks_without_reset() {
    let mut h = host(enable_following_field);
    let hp = |h: &DeltaHarness| {
        let monsters = &h.snapshot(1).monsters;
        [monsters[0].hp, monsters[1].hp]
    };
    let initial_hp = hp(&h);
    let initial_spirit = h.snapshot(0).own.spirit;
    let p = place(&mut h, 0, POINT);
    let f = place_field(&mut h, 0, &p);
    assert_eq!(f.attached_anchor_id.as_deref(), Some(p.id.as_str()));
    assert_eq!(f.center, POINT);
    assert!(h.engine.now < f.activates_at_ms);
    h.action(0, recast(&p, 1, MOVED_POINT)).unwrap();
    h.ticks(1);
    let mut expected = f.clone();
    expected.center = MOVED_POINT;
    assert_eq!(field(&h, 1, &p), Some(&expected));
    assert_eq!(hp(&h), initial_hp);
    let first_tick = f.next_tick_at_ms.unwrap();
    go_to(&mut h, first_tick);
    assert_eq!(hp(&h), [initial_hp[0], initial_hp[1] - 100]);
    let first = field(&h, 1, &p).unwrap().clone();
    assert_eq!(first.contact_policy.as_ref().unwrap().used_hits, 1);
    h.action(0, recast(&p, 2, POINT)).unwrap();
    h.ticks(1);
    expected = first.clone();
    expected.center = POINT;
    assert_eq!(field(&h, 1, &p), Some(&expected));
    go_to(&mut h, first_tick + f.interval_ms);
    assert_eq!(hp(&h), [initial_hp[0] - 100, initial_hp[1] - 100]);
    h.action(0, recast(&p, 3, MOVED_POINT)).unwrap();
    go_to(&mut h, first_tick + 2 * f.interval_ms);
    let current = field(&h, 1, &p).unwrap();
    assert_eq!(current.completed_ticks, 3);
    assert_eq!(current.contact_policy.as_ref().unwrap().used_hits, 2);
    assert_eq!(hp(&h), [initial_hp[0] - 100, initial_hp[1] - 100]);
    assert_eq!(h.snapshot(0).own.spirit, initial_spirit - 17);
}

#[test]
fn member_cast_anchor_following_field_expires_before_a_same_step_pulse() {
    let mut h = host(|v| {
        enable_following_field(v);
        let rule = &mut v["combat_rules"]["member_cast_anchors"][PLACE];
        rule["duration_ms"] = json!(480);
        rule["recast"]["window_ms"] = json!(480);
    });
    let p = place(&mut h, 0, POINT);
    let f = place_field(&mut h, 0, &p);
    assert_eq!(f.next_tick_at_ms, Some(p.expires_at_ms));
    let hp = h.snapshot(1).monsters[0].hp;
    go_to(&mut h, p.expires_at_ms);
    assert_eq!(h.snapshot(1).monsters[0].hp, hp);
    assert!(field(&h, 1, &p).is_none());
    assert!(ended(&h, &f.id, AreaEndReason::AnchorInvalid));
    assert!(ended(&h, &p.id, AreaEndReason::Expired));
    assert!(!h.events.iter().any(|(_, e)| matches!(
        e, EventV4::AreaTick { area_id, .. } if area_id == &f.id
    )));
}

#[test]
fn member_cast_anchor_following_field_replacement_and_departure_isolate_peers() {
    let mut h = host(enable_following_field);
    let p = place(&mut h, 0, POINT);
    let f = place_field(&mut h, 0, &p);
    let peer = place(&mut h, 1, MOVED_POINT);
    let peer_field = place_field(&mut h, 1, &peer);
    go_to(&mut h, p.created_at_ms + 450);
    let next = place(&mut h, 0, [550., 400.]);
    assert_ne!(next.id, p.id);
    assert!(field(&h, 1, &p).is_none());
    assert!(ended(&h, &f.id, AreaEndReason::AnchorInvalid));
    assert_eq!(field(&h, 1, &peer).unwrap().id, peer_field.id);
    let own_field = place_field(&mut h, 0, &next);
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(ended(&h, &own_field.id, AreaEndReason::SourceInvalid));
    assert!(field(&h, 1, &next).is_none());
    assert_eq!(field(&h, 1, &peer).unwrap().id, peer_field.id);
    assert!(field(&h, 0, &peer).is_none());
    h.close();
    assert!(field(&h, 1, &peer).is_none());
}

#[test]
fn member_cast_anchor_following_field_requires_both_opt_in_and_anchor_input() {
    for anchored in [false, true] {
        let mut h = host(|v| {
            enable_following_field(v);
            if anchored {
                v["combat_rules"]["member_cast_anchors"][PLACE]
                    .as_object_mut()
                    .unwrap()
                    .remove("following_field_skill_ids");
            }
        });
        let p = place(&mut h, 0, POINT);
        let f = if anchored {
            place_field(&mut h, 0, &p)
        } else {
            h.action(
                0,
                Action::CastGround {
                    skill_id: FOLLOWING_FIELD.into(),
                    position: POINT,
                },
            )
            .unwrap();
            h.ticks(2);
            field(&h, 0, &p).unwrap().clone()
        };
        assert!(f.attached_anchor_id.is_none());
        h.action(0, recast(&p, 1, MOVED_POINT)).unwrap();
        h.ticks(1);
        assert_eq!(field(&h, 1, &p), Some(&f));
    }
}

#[test]
fn member_cast_anchor_following_field_config_rejects_nonfield_unbound_and_duplicate_skills() {
    for invalid in [
        json!([BOLT]),
        json!([BURST]),
        json!(["seed-field"]),
        json!([FOLLOWING_FIELD, FOLLOWING_FIELD]),
        Value::Null,
    ] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                enable_following_field(v);
                v["combat_rules"]["member_cast_anchors"][PLACE]["following_field_skill_ids"] =
                    invalid;
            })
            .is_err()
        );
    }
}

fn enable_anchor_lifecycle(v: &mut Value) {
    enable_following_field(v);
    v["combat_rules"]["member_cast_anchors"][PLACE]["recast"]["behaviors"] =
        json!(["relocate", "return_to_source", "end"]);
}

fn return_anchor(p: &AreaPlanViewV4, recast_index: u32) -> Action {
    Action::ReturnAnchor {
        anchor_id: p.id.clone(),
        action_id: p.anchor_recast.as_ref().unwrap().action_id.clone(),
        recast_index,
    }
}

fn end_anchor(p: &AreaPlanViewV4, recast_index: u32) -> Action {
    Action::EndAnchor {
        anchor_id: p.id.clone(),
        action_id: p.anchor_recast.as_ref().unwrap().action_id.clone(),
        recast_index,
    }
}

#[test]
fn member_cast_anchor_return_and_end_share_budget_and_keep_original_identity() {
    let mut h = host(enable_anchor_lifecycle);
    let spirit = h.snapshot(0).own.spirit;
    let p = place(&mut h, 0, POINT);
    let f = place_field(&mut h, 0, &p);
    let peer = place(&mut h, 1, [560., 480.]);
    let peer_field = place_field(&mut h, 1, &peer);
    h.action(0, recast(&p, 1, MOVED_POINT)).unwrap();
    h.action(
        0,
        Action::Move {
            direction: [0., 1.],
        },
    )
    .unwrap();
    h.ticks(2);
    h.action(0, Action::Stop {}).unwrap();
    let center = h.snapshot(0).own.position;
    assert_ne!(center, [400., 400.]);
    h.action(0, return_anchor(&p, 2)).unwrap();
    assert_eq!(h.action(0, return_anchor(&p, 3)), Err(Error::InvalidTarget));
    h.ticks(1);
    let returned = anchor(&h, 1, 0).unwrap();
    assert_eq!(
        (&returned.id, returned.center, returned.expires_at_ms),
        (&p.id, center, p.expires_at_ms)
    );
    assert_eq!(returned.anchor_recast.as_ref().unwrap().used_count, 2);
    assert_eq!(
        returned.anchor_recast.as_ref().unwrap().behaviors,
        p.anchor_recast.as_ref().unwrap().behaviors
    );
    assert_eq!(
        field(&h, 1, &p),
        Some(&AreaPlanViewV4 {
            center,
            ..f.clone()
        })
    );
    assert_eq!(h.snapshot(0).own.spirit, spirit - 13);
    assert!(
        h.snapshot(0)
            .own
            .player_skills
            .iter()
            .any(|s| s.skill_id == PLACE && s.ready_at_ms == p.created_at_ms + 450)
    );

    let input = h.input(0, end_anchor(&p, 3));
    let status = h.engine.input(input.clone()).unwrap();
    assert_eq!(h.engine.input(input).unwrap(), status);
    assert_eq!(h.action(0, end_anchor(&p, 3)), Err(Error::InvalidTarget));
    h.ticks(1);
    assert_eq!(h.snapshot(0).own.spirit, spirit - 17);
    assert!(anchor(&h, 1, 0).is_none());
    assert!(field(&h, 1, &p).is_none());
    assert_eq!(anchor(&h, 1, 1), Some(&peer));
    assert_eq!(field(&h, 1, &peer), Some(&peer_field));
    assert!(ended(&h, &p.id, AreaEndReason::RecastConsumed));
    assert!(ended(&h, &f.id, AreaEndReason::AnchorInvalid));
    assert_eq!(
        h.events
            .iter()
            .filter(|(recipient, event)| {
                matches!(event, EventV4::AnchorRecastEnded {area_id,action_id,source,recast_index:3}
            if recipient.is_none() && area_id == &p.id && source == &p.source
                && action_id == &p.anchor_recast.as_ref().unwrap().action_id)
            })
            .count(),
        1
    );
}

#[test]
fn member_cast_anchor_end_preserves_released_fixed_area_and_failed_attempts_are_atomic() {
    let mut h = host(enable_anchor_lifecycle);
    let p = place(&mut h, 0, POINT);
    let f = place_field(&mut h, 0, &p);
    let hp = h.snapshot(1).monsters[0].hp;
    h.action(0, use_anchor(&p.id, BURST, None)).unwrap();
    assert_eq!(h.action(0, end_anchor(&p, 1)), Err(Error::InvalidState));
    h.ticks(11);
    assert_eq!(h.action(1, end_anchor(&p, 1)), Err(Error::InvalidTarget));
    let spirit = h.snapshot(0).own.spirit;
    h.engine.world.as_mut().unwrap().members[0].spirit = 3;
    assert_eq!(h.action(0, end_anchor(&p, 1)), Err(Error::NoResource));
    h.engine.world.as_mut().unwrap().members[0].spirit = spirit;
    h.action(0, end_anchor(&p, 1)).unwrap();
    h.ticks(1);
    assert!(ended(&h, &f.id, AreaEndReason::AnchorInvalid));
    assert!(
        h.snapshot(1)
            .areas
            .as_ref()
            .unwrap()
            .iter()
            .any(|a| a.skill_id == BURST)
    );
    h.ticks(2);
    assert_eq!(h.snapshot(1).monsters[0].hp, hp - 100);
    assert_eq!(h.snapshot(0).own.spirit, spirit - 4);
}

#[test]
fn member_cast_anchor_return_and_end_reject_expired_and_out_of_reach() {
    let mut h = host(|v| {
        enable_anchor_lifecycle(v);
        v["combat_rules"]["member_cast_anchors"][PLACE]["recast"]["window_ms"] = json!(180);
    });
    let p = place(&mut h, 0, POINT);
    let origin = h.snapshot(0).own.position;
    let spirit = h.snapshot(0).own.spirit;
    h.engine.world.as_mut().unwrap().members[0].position = [100., 400.];
    assert_eq!(h.action(0, return_anchor(&p, 1)), Err(Error::OutOfRange));
    assert_eq!(h.action(0, end_anchor(&p, 1)), Err(Error::OutOfRange));
    h.engine.world.as_mut().unwrap().members[0].position = origin;
    go_to(&mut h, p.anchor_recast.as_ref().unwrap().expires_at_ms);
    assert_eq!(h.action(0, return_anchor(&p, 1)), Err(Error::InvalidTarget));
    assert_eq!(h.action(0, end_anchor(&p, 1)), Err(Error::InvalidTarget));
    assert_eq!(anchor(&h, 1, 0), Some(&p));
    assert_eq!(h.snapshot(0).own.spirit, spirit);
}

#[test]
fn member_cast_anchor_recast_behavior_config_is_explicit_and_strict() {
    for invalid in [
        Value::Null,
        json!([]),
        json!(["end", "end"]),
        json!(["redirect"]),
    ] {
        assert!(
            delta_catalog(2, |v| {
                configure(v);
                enable_recast(v);
                v["combat_rules"]["member_cast_anchors"][PLACE]["recast"]["behaviors"] = invalid;
            })
            .is_err()
        );
    }
    let mut legacy = host(enable_recast);
    let p = place(&mut legacy, 0, POINT);
    assert_eq!(
        legacy.action(0, return_anchor(&p, 1)),
        Err(Error::Unsupported)
    );
    assert_eq!(legacy.action(0, end_anchor(&p, 1)), Err(Error::Unsupported));
    legacy.action(0, recast(&p, 1, MOVED_POINT)).unwrap();
    let mut end_only = host(|v| {
        enable_recast(v);
        v["combat_rules"]["member_cast_anchors"][PLACE]["recast"]["behaviors"] = json!(["end"]);
    });
    let p = place(&mut end_only, 0, POINT);
    assert_eq!(
        end_only.action(0, recast(&p, 1, MOVED_POINT)),
        Err(Error::Unsupported)
    );
    assert_eq!(
        end_only.action(0, return_anchor(&p, 1)),
        Err(Error::Unsupported)
    );
    end_only.action(0, end_anchor(&p, 1)).unwrap();
    for kind in ["return_anchor", "end_anchor"] {
        assert!(
            serde_json::from_value::<Action>(json!({
                "kind":kind,"anchor_id":"a","action_id":"r","recast_index":1,"position":[10,20]
            }))
            .is_err()
        );
    }
}
