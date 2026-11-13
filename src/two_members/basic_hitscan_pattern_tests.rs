use super::*;

const REPEAT: &str = "mirror_triple_refraction";
const BURST: &str = "mirror_split_light";

fn at(h: &DeltaHarness, position: [f64; 2]) -> usize {
    h.snapshot(0)
        .monsters
        .iter()
        .position(|npc| npc.position == position)
        .unwrap()
}

fn trace<'a>(h: &'a DeltaHarness, source: &str) -> Vec<&'a BasicImpactSegment> {
    let id = h
        .events
        .iter()
        .find_map(|(_, event)| match event {
            EventV4::BasicImpactStarted { impact } if impact.source.id == source => {
                Some(&impact.id)
            }
            _ => None,
        })
        .unwrap();
    let contacts = h
        .events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::BasicImpactContact {
                impact_id,
                segment: Some(segment),
                ..
            } if impact_id == id => Some(segment),
            _ => None,
        })
        .collect::<Vec<_>>();
    let terminal = h
        .events
        .iter()
        .find_map(|(_, event)| match event {
            EventV4::BasicImpactEnded {
                impact_id,
                segments: Some(segments),
                ..
            } if impact_id == id => Some(segments),
            _ => None,
        })
        .unwrap();
    assert_eq!(contacts, terminal.iter().collect::<Vec<_>>());
    assert!(
        contacts
            .iter()
            .enumerate()
            .all(|(i, segment)| usize::from(segment.index) == i)
    );
    contacts
}

fn damage(h: &DeltaHarness, source: &str) -> Vec<u32> {
    h.events
        .iter()
        .filter_map(|(_, event)| match event {
            EventV4::Damage {
                source: actor,
                amount,
                ..
            } if actor.id == source => Some(*amount),
            _ => None,
        })
        .collect()
}

#[test]
fn basic_hitscan_patterns_share_one_world_and_settle_each_actions_actual_damage_once() {
    let mut h = profile_host([Some(REPEAT), Some(BURST)], |v| {
        v["pve"]["monster_positions"] = json!([
            [700, 400],
            [740, 400],
            [700, 440],
            [400, 600],
            [440, 600],
            [400, 650],
            [480, 600],
            [400, 700],
            [520, 600],
            [400, 740]
        ]);
        v["combat_rules"]["basic_attack"]["life_steal_bps"] = json!(1500);
    });
    for member in &mut h.engine.world.as_mut().unwrap().members {
        member.hp = 500;
    }
    let a_index = at(&h, [700., 400.]);
    let b_index = at(&h, [400., 600.]);
    h.action(0, attack(&h, 0, a_index)).unwrap();
    h.action(1, attack(&h, 1, b_index)).unwrap();
    h.ticks(4);

    let rows = &h.snapshot(0).monsters;
    let a = trace(&h, "A");
    let ids =
        [[700., 400.], [740., 400.], [700., 440.]].map(|pos| rows[at(&h, pos)].entity.id.as_str());
    assert_eq!(
        a.iter().map(|s| s.target.id.as_str()).collect::<Vec<_>>(),
        [
            ids[0], ids[1], ids[0], ids[2], ids[0], ids[1], ids[0], ids[2]
        ]
    );
    assert_eq!(
        a.iter().map(|s| s.role).collect::<Vec<_>>(),
        [
            BasicImpactSegmentRole::Primary,
            BasicImpactSegmentRole::Secondary,
            BasicImpactSegmentRole::Return,
            BasicImpactSegmentRole::Secondary,
            BasicImpactSegmentRole::Return,
            BasicImpactSegmentRole::Secondary,
            BasicImpactSegmentRole::Return,
            BasicImpactSegmentRole::Secondary,
        ]
    );
    assert_eq!(
        a.iter().map(|s| s.origin).collect::<Vec<_>>(),
        [
            [400., 400.],
            [700., 400.],
            [740., 400.],
            [700., 400.],
            [700., 440.],
            [700., 400.],
            [740., 400.],
            [700., 400.],
        ]
    );
    assert_eq!(damage(&h, "A"), [90, 22, 76, 22, 65, 18, 55, 18]);

    let b = trace(&h, "B");
    let expected = [
        [400., 600.],
        [440., 600.],
        [400., 650.],
        [480., 600.],
        [400., 700.],
        [520., 600.],
    ];
    assert_eq!(
        b.iter().map(|s| s.target.id.as_str()).collect::<Vec<_>>(),
        expected.map(|pos| rows[at(&h, pos)].entity.id.as_str())
    );
    assert!(
        b[1..]
            .iter()
            .all(|s| s.role == BasicImpactSegmentRole::Blast && s.origin == [400., 600.])
    );
    assert_eq!(damage(&h, "B"), [120, 35, 35, 35, 35, 35]);
    assert!(impact_contacts(&h).iter().all(|(_, _, t)| *t == 120));
    assert_eq!(h.snapshot(0).own.hp, 554);
    assert_eq!(h.snapshot(1).own.hp, 544);
    assert_eq!(
        h.events
            .iter()
            .filter(|(_, event)| matches!(
                event,
                EventV4::Private {
                    event: Event::HealResolved {
                        heal_kind: HealKind::LifeSteal,
                        ..
                    }
                }
            ))
            .count(),
        2
    );
    for i in 0..2 {
        assert_eq!(h.snapshot(i).own.spirit, 102);
        assert_eq!(h.snapshot(i).own.basic_ready_at_ms, 1020);
        assert!(h.snapshot(i).basic_impacts.as_ref().unwrap().is_empty());
        assert!(h.snapshot(i).projectiles.is_empty());
    }
    h.close();
    assert!(h.stores.iter().all(|s| s.settlements == 1));
}

#[test]
fn basic_hitscan_patterns_single_or_occluded_neighbor_and_repeat_floor() {
    for available in [false, true] {
        let mut h = profile_host([Some(REPEAT), None], |v| {
            v["pve"]["monster_positions"] = json!([
                [700, 400],
                [790, 400],
                [700, if available { 630 } else { 631 }]
            ]);
            projectile_wall(v, 19, 10);
        });
        h.action(0, Action::Attack {}).unwrap();
        h.ticks(4);
        let rows = trace(&h, "A");
        assert_eq!(rows.len(), if available { 8 } else { 1 });
        if available {
            let target = &h.snapshot(0).monsters[at(&h, [700., 630.])].entity.id;
            assert!(
                rows.iter()
                    .skip(1)
                    .step_by(2)
                    .all(|s| s.target.id == *target)
            );
            assert_eq!(damage(&h, "A"), [90, 22, 76, 18, 65, 15, 55, 14]);
        } else {
            assert_eq!(damage(&h, "A"), [90]);
        }
        assert_eq!(h.snapshot(0).monsters[at(&h, [790., 400.])].hp, 10000);
    }
}

#[test]
fn basic_hitscan_patterns_lethal_hub_allows_first_secondary_but_no_invalid_return() {
    let mut h = profile_host([Some(REPEAT), None], |v| {
        v["pve"]["monster_positions"] = json!([[600, 400], [640, 400], [680, 400]]);
        v["pve"]["stats"]["hp"] = json!(80);
    });
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(4);
    assert_eq!(
        trace(&h, "A").iter().map(|s| s.role).collect::<Vec<_>>(),
        [
            BasicImpactSegmentRole::Primary,
            BasicImpactSegmentRole::Secondary
        ]
    );
    assert_eq!(damage(&h, "A"), [80, 22]);
    assert_eq!(h.snapshot(0).own.spirit, 102);
}

#[test]
fn basic_hitscan_patterns_dead_secondary_does_not_fabricate_extra_returns() {
    let mut h = profile_host([Some(REPEAT), None], |v| {
        v["pve"]["monster_positions"] = json!([[600, 400]]);
        let pve = &v["pve"];
        let mut stats = pve["stats"].clone();
        stats["hp"] = json!(20);
        v["pve"]["additional_templates"]["fragile"] = json!({
            "name":"fragile","mechanism":"melee_projectile_heal","stats":stats,
            "combat":pve["combat"],"drop_table":pve["drop_table"],"leash":pve["leash"],
            "help_range":pve["help_range"],"memory_range":pve["memory_range"],"kill_credit":false,"drop_orb":false
        });
        v["pve"]["extra_spawns"] =
            json!([{"id":"fragile-1","template_id":"fragile","position":[640,400]}]);
    });
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(4);
    assert_eq!(
        trace(&h, "A").iter().map(|s| s.role).collect::<Vec<_>>(),
        [
            BasicImpactSegmentRole::Primary,
            BasicImpactSegmentRole::Secondary,
            BasicImpactSegmentRole::Return
        ]
    );
    assert_eq!(damage(&h, "A"), [90, 20, 76]);
}

#[test]
fn basic_hitscan_patterns_burst_reaches_exact_edge_but_not_walls_or_secondary_centers() {
    let mut h = profile_host([Some(BURST), None], |v| {
        v["pve"]["monster_positions"] =
            json!([[700, 400], [700, 540], [700, 541], [790, 400], [700, 600]]);
        projectile_wall(v, 19, 10);
    });
    h.action(0, Action::Attack {}).unwrap();
    h.ticks(4);
    assert_eq!(damage(&h, "A"), [120, 35]);
    let rows = trace(&h, "A");
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[1].target.id,
        h.snapshot(0).monsters[at(&h, [700., 540.])].entity.id
    );
    assert_eq!(rows[1].origin, [700., 400.]);
}

#[test]
fn basic_hitscan_patterns_primary_dodge_stops_tails_and_legacy_events_omit_segments() {
    for profile in [REPEAT, BURST, "mirror"] {
        let mut h = profile_host([Some(profile), None], |v| {
            v["pve"]["monster_positions"] = json!([[490, 400], [520, 400]]);
            v["combat_rules"]["basic_attack"]["unavoidable"] = json!(false);
            v["combat_rules"]["dodge_cap_bps"] = json!(10000);
            v["combat_rules"]["profiles"]["member"]["hit_bps"] = json!(0);
            v["combat_rules"]["profiles"]["npc"]["dodge_bps"] = json!(10000);
        });
        h.action(0, Action::Attack {}).unwrap();
        h.ticks(4);
        assert_eq!(impact_contacts(&h).len(), 1);
        assert!(damage(&h, "A").is_empty());
        assert_eq!(h.snapshot(0).own.spirit, 100);
        let events = h
            .events
            .iter()
            .map(|(_, event)| serde_json::to_value(event).unwrap())
            .collect::<Vec<_>>();
        let contact = events
            .iter()
            .find(|e| e["kind"] == "basic_impact_contact")
            .unwrap();
        let end = events
            .iter()
            .find(|e| e["kind"] == "basic_impact_ended")
            .unwrap();
        if profile == "mirror" {
            assert!(contact.get("segment").is_none() && end.get("segments").is_none());
        } else {
            assert_eq!(trace(&h, "A").len(), 1);
            assert_eq!(contact["segment"]["index"], 0);
            assert_eq!(end["segments"].as_array().unwrap().len(), 1);
        }
    }
}

#[test]
fn basic_hitscan_patterns_config_rejects_ambiguous_or_unbounded_shapes() {
    for change in [
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][REPEAT]["impact"]["timing"]["refraction"] =
                json!({"radius":220,"damage_bps":6500})
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][REPEAT]["impact"]["timing"]["followup"]["max_contacts"] =
                json!(9)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][REPEAT]["impact"]["timing"]["followup"]["repeat_decay_bps"] =
                json!(10001)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][REPEAT]["impact"]["timing"]["followup"]["minimum_damage_bps"] =
                json!(2201)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][BURST]["impact"]["timing"]["followup"]["max_targets"] =
                json!(1)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][BURST]["impact"]["timing"]["followup"]["radius"] =
                json!(0)
        },
        |v: &mut Value| {
            v["combat_rules"]["basic_profiles"][BURST]["impact"]["timing"]["followup"] = Value::Null
        },
    ] {
        assert!(profile_catalog(change).is_err());
    }
}
