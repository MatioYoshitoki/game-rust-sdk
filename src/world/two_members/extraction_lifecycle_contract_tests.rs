use super::*;

fn lifecycle_input(v: &mut Value) {
    v["extraction_point"] = json!([20, 30]);
    v["extraction_channel_ms"] = json!(60);
    v["terminal_ms"] = json!(420);
    v["params"]["entry_years"] = json!(0);
    v["carried_quantity"] = json!(0);
    v["pve"]["combat"]["aggro_range"] = json!(2);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["status_definitions"] = json!([
        {"id":"ward","mechanism":"absorption_shield","shield_bps":1000,
            "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true},
        {"id":"immunity","mechanism":"timed_control","attribute":"damage_immunity",
            "duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}
    ]);
    v["members"]["member_conflict"] = json!(true);
    v["extraction_lifecycle"] = json!({
        "broadcast_channels":true,
        "pressure":{
            "grace_ms":150,"damage_interval_ms":80,"damage_max_hp_bps":1500,
            "warnings":[{"id":"one","remaining_ms":330},{"id":"two","remaining_ms":240},{"id":"three","remaining_ms":180}]
        }
    });
}

fn channel(
    events: &Events,
    state: ExtractionChannelState,
    pid: &str,
) -> Option<ExtractionChannelView> {
    events.iter().find_map(|(recipient, event)| match event {
        EventV4::ExtractionChannelChanged(ch)
            if recipient.is_none() && ch.state == state && ch.entity.id == pid =>
        {
            Some(ch.channel.clone())
        }
        _ => None,
    })
}

fn start(w: &mut World, i: usize, c: &Config, now: u64) -> Events {
    w.action(i, &Action::ExtractStart {}, "extraction", c, now)
        .unwrap()
}

fn status(w: &mut World, i: usize, id: &str, c: &Config, now: u64) {
    let entity = w.members[i].reference.entity.clone();
    w.apply_owned_statuses(
        Owner::Member(i),
        (entity, 100),
        &[id.into()],
        c,
        now,
        &mut vec![],
    );
}

#[test]
fn deadline_damage_and_public_channels_follow_the_existing_extraction_path() {
    let c = catalog(input(), lifecycle_input).unwrap().default_config();
    let mut w = world(&c);
    assert_eq!(w.extraction_pressure_view(&c, 89).unwrap().warning, None);
    assert_eq!(
        w.extraction_pressure_view(&c, 90)
            .unwrap()
            .warning
            .unwrap()
            .index,
        1
    );
    assert_eq!(
        w.extraction_pressure_view(&c, 180)
            .unwrap()
            .warning
            .unwrap()
            .index,
        2
    );
    assert_eq!(
        w.extraction_pressure_view(&c, 240)
            .unwrap()
            .warning
            .unwrap()
            .index,
        3
    );
    let initial = start(&mut w, 0, &c, 30);
    let view = channel(&initial, ExtractionChannelState::Started, "a").unwrap();
    assert_eq!(
        view,
        ExtractionChannelView {
            position: [20., 20.],
            started_at_ms: 30,
            ends_at_ms: 90
        }
    );
    assert_eq!(
        w.snapshot(1, &c, Phase::Active, 30).members[0]
            .extraction_channel
            .as_ref(),
        Some(&view)
    );
    assert_eq!(
        w.spectator_snapshot(&c, Phase::Active, 30).unwrap().members[0]
            .extraction_channel
            .as_ref(),
        Some(&view)
    );
    let a = w.members[0].reference.entity.clone();
    w.action(
        1,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&a),
        },
        "hit",
        &c,
        30,
    )
    .unwrap();
    assert_eq!(
        channel(&w.tick(&c, 60), ExtractionChannelState::Interrupted, "a"),
        Some(view)
    );
    start(&mut w, 0, &c, 90);
    assert!(
        channel(
            &w.action(0, &Action::ExtractCancel {}, "cancel", &c, 90)
                .unwrap(),
            ExtractionChannelState::Interrupted,
            "a"
        )
        .is_some()
    );
    assert!(
        w.action(0, &Action::ExtractCancel {}, "repeat", &c, 90)
            .unwrap()
            .is_empty()
    );
    start(&mut w, 0, &c, 120);
    let completed = w.tick(&c, 180);
    assert!(channel(&completed, ExtractionChannelState::Completed, "a").is_some());
    assert!(!completed.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::ExtractionInterrupted
        }
    )));
    assert_eq!(w.members[0].desired, Some(Outcome::Extraction));
    assert_eq!(
        w.extraction_pressure_view(&c, 270).unwrap().phase,
        ExtractionPressurePhase::Grace
    );
    status(&mut w, 1, "ward", &c, 270);
    start(&mut w, 1, &c, 330);
    assert!(
        w.tick(&c, 330)
            .iter()
            .all(|(_, e)| !matches!(e, EventV4::DeadlineDamage { .. }))
    );
    let pulse = w.tick(&c, 360);
    assert!(pulse.iter().any(|(_,e)| matches!(e, EventV4::DeadlineDamage { target, amount:5, absorbed:10, hp:95 } if target.id=="b")));
    assert!(channel(&pulse, ExtractionChannelState::Interrupted, "b").is_some());
    assert_eq!(w.members[1].hp, 95);
    assert!(
        w.tick(&c, 360)
            .iter()
            .all(|(_, e)| !matches!(e, EventV4::DeadlineDamage { .. }))
    );
    let ending = start(&mut w, 1, &c, 360);
    assert_eq!(
        channel(&ending, ExtractionChannelState::Started, "b")
            .unwrap()
            .ends_at_ms,
        420
    );
    w.tick(&c, 390);
    let expired = w.tick(&c, 420);
    assert!(channel(&expired, ExtractionChannelState::Completed, "b").is_none());
    assert!(channel(&expired, ExtractionChannelState::Interrupted, "b").is_some());
    assert_eq!(w.members[1].desired, Some(Outcome::Death));
    assert_eq!(w.members[1].hp, 0);
    assert_eq!(w.members[0].desired, Some(Outcome::Extraction));
    assert!(w.npcs.iter().all(|n| n.hp == 0));
    assert!(w.resources.is_empty() && w.members.iter().all(|m| m.tally.is_empty()));
    assert_eq!(
        w.extraction_pressure_view(&c, 420).unwrap().phase,
        ExtractionPressurePhase::Expired
    );
}

#[test]
fn pressure_applies_to_disconnected_living_members_and_force_deadline_ignores_immunity() {
    let c = catalog(input(), lifecycle_input).unwrap().default_config();
    let mut w = world(&c);
    status(&mut w, 1, "immunity", &c, 330);
    w.protect(0, 330, &mut vec![]);
    w.protect(1, 330, &mut vec![]);
    w.tick(&c, 360);
    assert_eq!(w.members[0].hp, 85);
    assert_eq!(w.members[1].hp, 100);
    w.tick(&c, 420);
    assert!(
        w.members
            .iter()
            .all(|m| m.hp == 0 && m.desired == Some(Outcome::Death))
    );
}

#[test]
fn full_shield_keeps_extraction_and_legacy_room_keeps_the_channel_private() {
    let c = catalog(input(), |v| {
        lifecycle_input(v);
        v["pve"]["status_definitions"][0]["shield_bps"] = json!(2000);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    status(&mut w, 1, "ward", &c, 270);
    start(&mut w, 1, &c, 300);
    let complete = w.tick(&c, 360);
    assert!(complete.iter().any(|(_,e)| matches!(e, EventV4::DeadlineDamage { target, amount:0, absorbed:15, hp:100 } if target.id=="b")));
    assert!(channel(&complete, ExtractionChannelState::Completed, "b").is_some());
    assert_eq!(w.members[1].desired, Some(Outcome::Extraction));
    let c = catalog(input(), |v| {
        lifecycle_input(v);
        v.as_object_mut().unwrap().remove("extraction_lifecycle");
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let private_events = start(&mut w, 0, &c, 30);
    assert!(channel(&private_events, ExtractionChannelState::Started, "a").is_none());
    assert_eq!(
        w.snapshot(0, &c, Phase::Active, 30)
            .own
            .extraction_finishes_ms,
        Some(90)
    );
    assert!(
        w.snapshot(1, &c, Phase::Active, 30).members[0]
            .extraction_channel
            .is_none()
    );
    assert!(
        w.spectator_snapshot(&c, Phase::Active, 30).unwrap().members[0]
            .extraction_channel
            .is_none()
    );
}
