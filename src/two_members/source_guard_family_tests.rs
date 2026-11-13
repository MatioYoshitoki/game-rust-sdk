use super::*;
use crate::protocol::SkillLoadout;

const BASES: [&str; 2] = ["jinxia-buhuai", "hanshan-jin"];
const SHIELDS: [&str; 2] = ["effect_guard_value", "effect_hanshan_guard"];

fn catalog(change: impl FnOnce(&mut Value)) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    catalog_with(|v| {
        *v = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/mastery-stages.json"
        ))
        .unwrap();
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/source-guard-families.json"
        ))
        .unwrap();
        v["skill_library"] = sample["skill_library"].clone();
        v["pve"]["status_rules"]["shield_cap_bps"] = sample["shield_cap_bps"].clone();
        v["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .extend(
                sample["status_definitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
        for (key, entries) in sample["combat_rules"].as_object().unwrap() {
            let table = &mut v["combat_rules"][key];
            if table.is_null() {
                *table = json!({});
            }
            table
                .as_object_mut()
                .unwrap()
                .extend(entries.as_object().unwrap().clone());
        }
        v["combat_rules"]["build_rules"]["power_models"]["mastery-demo"]["rank_base"] =
            json!([100]);
        change(v);
    })
}

fn host(stages: [usize; 2], change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let cat = catalog(change).unwrap();
    let mut request = create(&cat.default_config());
    request.protocol_version = 6;
    for (i, seat) in request.participants.iter_mut().enumerate() {
        let base = BASES[i];
        let mastery = stages[i];
        let mut skills = vec![base.into()];
        if mastery >= 2 {
            skills.push(format!("{base}-advanced"));
        }
        seat.skills = Some(SkillLoadout {
            owned_skill_ids: skills.clone(),
            equipped_skill_ids: skills.clone(),
        });
        seat.combat_build = Some(CombatBuild {
            power_points: 0,
            skill_power_points: 0,
            element_resistance_points: 0,
            powers: skills
                .into_iter()
                .map(|id| {
                    (
                        id,
                        PowerGrowth {
                            model_id: "mastery-demo".into(),
                            rank: 0,
                            quality: 0,
                            level: 0,
                            mastery,
                        },
                    )
                })
                .collect(),
        });
    }
    let mut h = DeltaHarness::from_request(cat, request);
    for store in &mut h.stores {
        store.hp = 1000;
        store.spirit = 200;
    }
    h.active();
    h
}

fn cast(h: &mut DeltaHarness, i: usize, advanced: bool) -> Result<Status> {
    let skill_id = if advanced {
        format!("{}-advanced", BASES[i])
    } else {
        BASES[i].into()
    };
    let result = h.action(i, Action::CastSelf { skill_id });
    h.pump();
    result
}

fn shield(h: &DeltaHarness, i: usize) -> &StatusView {
    let own = &h.snapshot(i).own;
    assert_eq!(
        own.statuses.iter().filter(|s| s.shield.is_some()).count(),
        1
    );
    own.statuses.iter().find(|s| s.shield.is_some()).unwrap()
}

#[test]
fn source_guard_family_uses_original_stage_grants_and_source_power_caps() {
    let balances = [[180, 200, 280, 280, 280], [240, 270, 320, 320, 320]];
    let durations = [
        [5000, 6000, 6000, 6000, 6000],
        [6000, 7000, 7000, 7000, 7000],
    ];
    let reductions = [
        [2000, 2000, 2000, 3500, 3500],
        [2500, 2500, 2500, 4000, 4000],
    ];
    let caps = [[120, 120, 120, 200, 200], [120, 120, 120, 220, 220]];
    for stage in 0..5 {
        let mut h = host([stage; 2], |_| {});
        for i in 0..2 {
            let advanced = stage >= 2;
            if !advanced {
                assert_eq!(cast(&mut h, i, true), Err(Error::InvalidTarget));
            }
            cast(&mut h, i, advanced).unwrap();
            let active = shield(&h, i);
            let balance = active.shield.as_ref().unwrap();
            assert!(active.definition_id.starts_with(SHIELDS[i]));
            assert_eq!(active.source, active.target);
            assert_eq!(balance.remaining, balances[i][stage]);
            assert_eq!(active.expires_at_ms, durations[i][stage]);
            assert_eq!(active.modifiers[0].modifier_bps, reductions[i][stage]);
            assert_eq!(balance.record.as_ref().unwrap().cap, caps[i][stage]);
            assert_eq!(
                h.snapshot(i).own.spirit,
                200 - if advanced { [45, 67][i] } else { [30, 45][i] }
            );
            let skill = h
                .snapshot(i)
                .own
                .player_skills
                .iter()
                .find(|s| {
                    s.skill_id
                        == if advanced {
                            format!("{}-advanced", BASES[i])
                        } else {
                            BASES[i].into()
                        }
                })
                .unwrap();
            assert_eq!(skill.ready_at_ms, if advanced { 42000 } else { 60000 });
        }
    }
}

#[test]
fn source_guard_family_stronger_replaces_at_capacity_and_weaker_keeps_original() {
    let mut h = host([4, 4], |v| {
        v["pve"]["status_rules"]["max_active_per_entity"] = json!(1)
    });
    cast(&mut h, 0, false).unwrap();
    let original = shield(&h, 0).clone();
    cast(&mut h, 0, true).unwrap();
    let upgraded = shield(&h, 0);
    assert_ne!(upgraded.instance_id, original.instance_id);
    assert_eq!(upgraded.shield.as_ref().unwrap().remaining, 280);
    assert!(upgraded.definition_id.contains("_advanced"));
    assert!(h.events.iter().any(|(who,e)| matches!(e,
        EventV4::Private { event: Event::ShieldRecordEnded { instance_id, reason: StatusRemovalReason::Reset, raw_counter: 0, targets_hit: 0, .. } }
        if who.as_deref()==Some("A") && instance_id==&original.instance_id
    )));

    cast(&mut h, 1, true).unwrap();
    let stronger = shield(&h, 1).clone();
    h.ticks(3);
    cast(&mut h, 1, false).unwrap();
    assert_eq!(shield(&h, 1), &stronger);
    assert_eq!(h.snapshot(1).own.shield_total, Some(320));
    assert_eq!(h.snapshot(1).own.spirit, 88);
}

#[test]
fn source_guard_family_rejects_invalid_grouping_and_counter_reasons() {
    catalog(|_| {}).unwrap();
    for value in [json!(""), json!("invalid name"), Value::Null] {
        assert!(
            catalog(|v| {
                let definition = v["pve"]["status_definitions"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|d| d["id"] == SHIELDS[0])
                    .unwrap();
                definition["shield_family"] = value;
            })
            .is_err()
        );
    }
    assert!(
        catalog(|v| v["combat_rules"]["status_policies"][SHIELDS[0]]["transferable"] = json!(true))
            .is_err()
    );
    let id = "effect_guard_value_advanced_stage_5";
    for reasons in [
        json!([]),
        json!(["cleansed"]),
        json!(["depleted", "depleted"]),
        json!(["consumed"]),
    ] {
        assert!(
            catalog(|v| v["combat_rules"]["shield_records"][id]["counter_on"] = reasons).is_err()
        );
    }
}

#[test]
fn source_guard_family_releases_only_configured_end_effects() {
    for allow_expiry_counter in [true, false] {
        let mut h = host([4, 4], |v| {
            v["pve"]["stats"]["atk"] = json!(500);
            v["pve"]["combat"]["aggro_range"] = json!(60);
            v["pve"]["combat"]["melee_range"] = json!(60);
            v["pve"]["combat"]["attack_interval_ms"] = json!(3900);
            v["pve"]["monster_positions"] = json!([[440, 400], [440, 480]]);
            let npc = v["pve"]["template_id"].as_str().unwrap().to_owned();
            let mut attack = v["combat_rules"]["basic_attack"].clone();
            attack.as_object_mut().unwrap().remove("life_steal_timing");
            v["combat_rules"]["npc_attacks"][npc] = attack;
            if !allow_expiry_counter {
                v["combat_rules"]["shield_records"]["effect_hanshan_guard_advanced_stage_5"]["counter_on"] =
                    json!(["depleted"]);
            }
        });
        cast(&mut h, 0, true).unwrap();
        cast(&mut h, 1, true).unwrap();
        h.ticks(130);
        assert_eq!(h.snapshot(0).own.hp, 955);
        assert_eq!(h.snapshot(1).own.hp, 1000);
        assert_eq!(shield(&h, 1).shield.as_ref().unwrap().remaining, 20);
        h.ticks(104);
        let ended = h
            .events
            .iter()
            .filter_map(|(who, e)| match e {
                EventV4::Private {
                    event:
                        Event::ShieldRecordEnded {
                            reason,
                            recorded,
                            raw_counter,
                            targets_hit,
                            ..
                        },
                } => Some((
                    who.as_deref(),
                    *reason,
                    *recorded,
                    *raw_counter,
                    *targets_hit,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ended,
            [
                (Some("A"), StatusRemovalReason::Depleted, 200, 120, 2),
                (
                    Some("B"),
                    StatusRemovalReason::Expired,
                    220,
                    if allow_expiry_counter { 132 } else { 0 },
                    if allow_expiry_counter { 2 } else { 0 }
                ),
            ]
        );
        assert_eq!(h.snapshot(1).own.shield_total, Some(0));
        let linger = h
            .snapshot(1)
            .own
            .statuses
            .iter()
            .find(|s| s.definition_id == "effect_hanshan_guard_linger")
            .unwrap();
        assert_eq!(
            (
                linger.modifier_bps,
                linger.expires_at_ms - linger.applied_at_ms
            ),
            (2000, 2500)
        );
        let slow = h.snapshot(1).monsters[1]
            .statuses
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|s| s.definition_id == "effect_hanshan_guard_counter_slow");
        assert_eq!(
            slow.map(|s| (s.modifier_bps, s.expires_at_ms - s.applied_at_ms)),
            allow_expiry_counter.then_some((-3500, 2500))
        );
    }
}

#[test]
fn source_guard_family_isolates_sources_under_one_target_cap() {
    let catalog =
        catalog(|v| v["pve"]["status_rules"]["max_active_per_entity"] = json!(2)).unwrap();
    let c = catalog.default_config();
    let pve = c.pve.as_ref().unwrap();
    let rule = pve.status_rules.as_ref().unwrap();
    let definition = |id| pve.status_definitions.iter().find(|d| d.id == id).unwrap();
    let entity = |id: &str| StatusEntity {
        kind: StatusEntityKind::Participant,
        id: id.into(),
        generation: 1,
        encounter_round: None,
    };
    let (a, b, target) = (entity("A"), entity("B"), entity("T"));
    let mut statuses = crate::status::Statuses::default();
    let mut events = Vec::new();
    for source in [&a, &b] {
        assert!(statuses.apply(
            std::slice::from_ref(definition(SHIELDS[0])),
            (source, 100),
            (&target, 1000),
            0,
            rule,
            &mut events
        ));
    }
    let b_original = statuses
        .entries()
        .iter()
        .find(|s| s.source == b)
        .unwrap()
        .clone();
    let stronger = std::slice::from_ref(definition("effect_guard_value_advanced"));
    assert!(statuses.apply(stronger, (&a, 100), (&target, 1000), 30, rule, &mut events));
    assert_eq!(statuses.shield_total(&target, 30), 400);
    let replaced = statuses
        .entries()
        .iter()
        .find(|s| s.source == a)
        .unwrap()
        .clone();
    assert_eq!(replaced.shield.as_ref().unwrap().remaining, 220);
    assert_eq!(
        statuses.entries().iter().find(|s| s.source == b),
        Some(&b_original)
    );
    assert!(statuses.apply(stronger, (&a, 100), (&target, 1000), 60, rule, &mut events));
    assert_eq!(
        statuses.entries().iter().find(|s| s.source == a),
        Some(&replaced)
    );
}
