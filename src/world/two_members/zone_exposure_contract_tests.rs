use super::*;

fn setup(v: &mut Value) {
    v["pve"].as_object_mut().unwrap().remove("status_rules");
    v["pve"]["combat"]["aggro_range"] = json!(2);
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["monster_positions"] = json!([[100, 100]]);
    v["members"]["member_conflict"] = json!(true);
    v["members"]["spectators"] = json!({"capacity":1,"terminal_retention_ms":400});
    v["zone_exposure"] = json!({
        "axis":[0,1],"bands":[
            {"id":"fast","below":30,"interval_ms":60},
            {"id":"slow","interval_ms":120}
        ],
        "max_stacks":3,"reduction_per_stack_bps":1000,
        "interaction":{"range":12,"cooldown_ms":90,"remove_stacks":2,
            "boost_bps":2500,"boost_duration_ms":120,
            "nodes":[{"id":"node","position":[20,30]}]}
    });
}

fn view(w: &World, seat: usize, c: &Config, now: u64) -> ZoneExposureView {
    w.snapshot(seat, c, Phase::Active, now)
        .own
        .zone_exposure
        .unwrap()
}

fn use_node(
    w: &mut World,
    seat: usize,
    revision: u32,
    choice: ExposureNodeChoice,
    c: &Config,
    now: u64,
) -> Result<Events> {
    w.action(
        seat,
        &Action::UseExposureNode {
            node_id: "node".into(),
            expected_revision: revision,
            choice,
        },
        "node-action",
        c,
        now,
    )
}

#[test]
fn zone_exposure_preserves_fraction_on_crossing_and_stops_accumulating_at_cap() {
    let c = catalog(input(), setup).unwrap().default_config();
    let mut w = world(&c);
    assert_eq!(view(&w, 0, &c, 0).band_id, "fast");
    assert_eq!(view(&w, 1, &c, 0).band_id, "slow");
    w.tick(&c, 30);
    assert_eq!(view(&w, 0, &c, 30).stacks, 0);
    w.members[0].position[1] = 30.;
    w.tick(&c, 60);
    assert_eq!(view(&w, 0, &c, 60).band_id, "slow");
    assert_eq!(view(&w, 0, &c, 60).stacks, 0);
    w.tick(&c, 60);
    w.tick(&c, 90);
    assert_eq!(view(&w, 0, &c, 90).stacks, 1);
    assert_eq!(view(&w, 1, &c, 90).stacks, 0);
    w.tick(&c, 600);
    assert_eq!(view(&w, 0, &c, 600).stacks, 3);
    use_node(&mut w, 0, 1, ExposureNodeChoice::Reduce, &c, 600).unwrap();
    assert_eq!(view(&w, 0, &c, 600).stacks, 1);
    w.tick(&c, 630);
    assert_eq!(view(&w, 0, &c, 630).stacks, 1);
    assert_eq!(view(&w, 1, &c, 630).stacks, 3);
    w.tick(&c, 720);
    assert_eq!(view(&w, 0, &c, 720).stacks, 2);
    w.tick(&c, 3000);
    assert!(w.members.iter().all(|m| m.exposure.is_none()));
}

#[test]
fn zone_exposure_shared_cooldown_private_boost_and_actual_combat() {
    let c = catalog(input(), setup).unwrap().default_config();
    let mut w = world(&c);
    for m in &mut w.members {
        m.facts.atk_points = 40;
        m.facts.def_points = 20;
    }
    w.tick(&c, 60);
    assert_eq!(w.stats(0, &c, 60), (36, 18));
    assert_eq!(w.stats(1, &c, 60), (40, 20));
    assert_eq!(
        use_node(&mut w, 1, 1, ExposureNodeChoice::Reduce, &c, 60),
        Err(Error::InvalidState)
    );
    assert_eq!(view(&w, 1, &c, 60).nodes[0].revision, 1);
    use_node(&mut w, 0, 1, ExposureNodeChoice::Reduce, &c, 60).unwrap();
    assert_eq!(view(&w, 0, &c, 60).stacks, 0);
    assert_eq!(w.stats(0, &c, 60), (40, 20));
    assert_eq!(
        use_node(&mut w, 1, 1, ExposureNodeChoice::Boost, &c, 60),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        use_node(&mut w, 1, 2, ExposureNodeChoice::Boost, &c, 60),
        Err(Error::InvalidState)
    );
    assert_eq!(view(&w, 1, &c, 60).nodes[0].ready_at_ms, 150);

    w.tick(&c, 150);
    use_node(&mut w, 1, 2, ExposureNodeChoice::Boost, &c, 150).unwrap();
    assert_eq!(view(&w, 1, &c, 150).boost_ends_at_ms, Some(270));
    assert_eq!(view(&w, 0, &c, 150).boost_ends_at_ms, None);
    assert_eq!(w.stats(1, &c, 150), (45, 22));
    let observer =
        serde_json::to_value(w.spectator_snapshot(&c, Phase::Active, 150).unwrap()).unwrap();
    assert!(observer.get("zone_exposure").is_none());
    assert!(
        observer["members"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m.get("zone_exposure").is_none())
    );

    let a = w.members[0].reference.entity.clone();
    let b = w.members[1].reference.entity.clone();
    let hit = combat::DamageIntent {
        source: a,
        target: b,
        raw: 36,
        direct: true,
        ignore_armor: false,
    };
    let rule = crate::config::AttackRule::default();
    let (with_boost, _) = w.configured_incoming(
        &hit,
        &rule,
        (Owner::Member(0), Owner::Member(1)),
        false,
        &c,
        150,
    );
    let (without_boost, _) = w.configured_incoming(
        &hit,
        &rule,
        (Owner::Member(0), Owner::Member(1)),
        false,
        &c,
        270,
    );
    assert!(with_boost < without_boost);
    w.action(
        1,
        &Action::CastUnit {
            skill_id: c.skill.id.clone(),
            target: World::unit_wire(&hit.source),
        },
        "attack",
        &c,
        150,
    )
    .unwrap();
    w.tick(&c, 180);
    assert_eq!(w.members[0].hp, 55);
    w.protect(1, 180, &mut vec![]);
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert!(w.zone_exposure_view(0, &c, 180).is_none());
    w.tick(&c, 240);
    assert_eq!(view(&w, 1, &c, 240).stacks, 2);
    assert_eq!(view(&w, 1, &c, 240).nodes[0].revision, 3);
    w.tick(&c, 270);
    assert_eq!(view(&w, 1, &c, 270).boost_ends_at_ms, None);
    assert_eq!(w.stats(1, &c, 270), (32, 16));
    assert!(
        matches!(w.settlement(0,&c), Business::CommitSettlement { items, instances, .. }
        if items.is_empty() && instances.is_empty())
    );
}

#[test]
fn zone_exposure_rejects_invalid_config_and_checks_authoritative_reach() {
    for change in [
        |v: &mut Value| v["zone_exposure"]["axis"] = json!([0, 0]),
        |v: &mut Value| v["zone_exposure"]["bands"][1]["below"] = json!(45),
        |v: &mut Value| v["zone_exposure"]["reduction_per_stack_bps"] = json!(5000),
        |v: &mut Value| v["zone_exposure"]["interaction"]["nodes"][0]["position"] = json!([-1, 30]),
    ] {
        let result = catalog(input(), |v| {
            setup(v);
            change(v);
        });
        let error = result.err().unwrap().to_string();
        assert!(error.contains("CONFIG_ZONE_EXPOSURE"), "{error}");
    }
    let c = catalog(input(), |v| {
        setup(v);
        v["pve"]["world_geometry"] = json!({
            "boundary":[[0,0],[128,0],[128,128],[0,128]],
            "blockers":[{
                "id":"block","vertices":[[10,24],[30,24],[30,26],[10,26]],
                "blocks_movement":true,"blocks_projectiles":true,"blocks_vision":true
            }]
        });
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    assert!(view(&w, 0, &c, 0).nodes.is_empty());
    assert_eq!(
        use_node(&mut w, 0, 1, ExposureNodeChoice::Boost, &c, 0),
        Err(Error::InvalidTarget)
    );
    w.members[1].position = [20., 60.];
    assert_eq!(
        use_node(&mut w, 1, 1, ExposureNodeChoice::Boost, &c, 0),
        Err(Error::InvalidTarget)
    );
    w.members[1].position = [20., 40.];
    assert_eq!(view(&w, 1, &c, 0).nodes[0].revision, 1);
    use_node(&mut w, 1, 1, ExposureNodeChoice::Boost, &c, 0).unwrap();
}
