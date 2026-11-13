use super::*;
use crate::protocol::SkillLoadout;

const GROVE: &str = "wanmu-guiyuan";
const ADVANCED: &str = "wanmu-guiyuan-advanced";
const MARK: &str = "effect_guiyuan_mark";
const SPIRIT: u32 = 8_000_000;

fn catalog(change: impl FnOnce(&mut Value)) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    super::source_ally_recovery::catalog(|v| {
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/source-ally-self-and-marks.json"
        ))
        .unwrap();
        v["skill_library"]
            .as_array_mut()
            .unwrap()
            .extend(sample["skill_library"].as_array().unwrap().iter().cloned());
        v["resource_limits"]
            .as_object_mut()
            .unwrap()
            .extend(sample["resource_limits"].as_object().unwrap().clone());
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
        for (key, table) in sample["combat_rules"].as_object().unwrap() {
            if let Some(entries) = table.as_object() {
                v["combat_rules"][key]
                    .as_object_mut()
                    .unwrap()
                    .extend(entries.clone());
            } else {
                v["combat_rules"][key] = table.clone();
            }
        }
        change(v);
    })
}

fn host(change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let cat = catalog(change).unwrap();
    let mut request = create(&cat.default_config());
    request.protocol_version = 6;
    for seat in &mut request.participants {
        let ids = [GROVE, ADVANCED, "huichun-jue", "huichun-jue-advanced"]
            .map(str::to_owned)
            .to_vec();
        seat.skills = Some(SkillLoadout {
            owned_skill_ids: ids.clone(),
            equipped_skill_ids: ids.clone(),
        });
        seat.combat_build = Some(CombatBuild {
            power_points: 0,
            skill_power_points: 0,
            element_resistance_points: 0,
            powers: ids
                .into_iter()
                .map(|id| {
                    (
                        id,
                        PowerGrowth {
                            model_id: "mastery-demo".into(),
                            rank: 0,
                            quality: 0,
                            level: 0,
                            mastery: 4,
                        },
                    )
                })
                .collect(),
        });
    }
    let mut h = DeltaHarness::from_request(cat, request);
    for store in &mut h.stores {
        store.hp = 1000;
        store.spirit = SPIRIT;
    }
    h.active();
    h
}

fn action(h: &DeltaHarness, target: usize, skill: &str) -> Action {
    let entity = &h.snapshot(target).own.participant.entity;
    Action::CastUnit {
        skill_id: skill.into(),
        target: UnitTarget {
            id: entity.id.clone(),
            generation: entity.generation,
        },
    }
}

fn cast(h: &mut DeltaHarness, source: usize, target: usize, skill: &str) {
    h.action(source, action(h, target, skill)).unwrap();
    h.pump();
    h.ticks(4);
}

fn mark<'a>(h: &'a DeltaHarness, target: usize, source: &str) -> Option<&'a StatusView> {
    h.snapshot(target)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == MARK && s.source.id == source)
}

#[test]
fn source_ally_self_explicit_self_target_reserves_cancels_and_commits_once() {
    let mut h = host(|v| v["combat_rules"]["ally_self_target_skills"] = json!([GROVE, ADVANCED]));
    assert_eq!(
        h.action(0, action(&h, 0, "huichun-jue")),
        Err(Error::InvalidTarget)
    );
    let Action::CastUnit {
        skill_id,
        mut target,
    } = action(&h, 0, ADVANCED)
    else {
        unreachable!()
    };
    target.generation += 1;
    assert_eq!(
        h.action(0, Action::CastUnit { skill_id, target }),
        Err(Error::InvalidTarget)
    );
    h.action(0, action(&h, 0, ADVANCED)).unwrap();
    h.pump();
    let pending = h.snapshot(0).own.pending_action.as_ref().unwrap();
    assert_eq!(pending.ally_target_is_self, Some(true));
    assert_eq!(pending.ally_target.as_ref().unwrap().id, "A");
    assert_eq!(
        pending.resource_reservation.as_ref().unwrap().spirit,
        2_500_000
    );
    assert!(h.snapshot(1).own.pending_action.is_none());
    h.action(0, Action::CancelAction {}).unwrap();
    h.pump();
    assert!(h.snapshot(0).own.pending_action.is_none());
    assert!(mark(&h, 0, "A").is_none());
    assert_eq!(h.snapshot(0).own.spirit, SPIRIT);
    let input = h.input(0, action(&h, 0, ADVANCED));
    let first = h.engine.input(input.clone()).unwrap();
    assert_eq!(h.engine.input(input).unwrap(), first);
    h.pump();
    h.ticks(4);
    assert_eq!(h.snapshot(0).own.spirit, SPIRIT - 2_500_000);
    assert_eq!(mark(&h, 0, "A").unwrap().stacks, Some(2));
    let own_heals=h.events.iter().filter(|(who,event)| {
        matches!(event,EventV4::Private {event:Event::HealResolved{source,target,heal_kind:HealKind::SelfSkill,raw_heal:200,..}}
            if source==target && who.as_deref()==Some("A"))
    }).count();
    assert_eq!(own_heals, 1);
}

#[test]
fn source_ally_self_mark_both_owners_refresh_and_exit_with_source_identity() {
    for owner in ["primary_target", "caster"] {
        let mut h = host(|v| {
            for id in [GROVE, ADVANCED] {
                for row in v["combat_rules"]["ally_recovery_by_mastery"][id]
                    .as_array_mut()
                    .unwrap()
                {
                    row["mark"]["owner"] = json!(owner);
                }
            }
        });
        let holder = usize::from(owner == "primary_target");
        cast(&mut h, 0, 1, ADVANCED);
        let original = mark(&h, holder, "A").unwrap().clone();
        assert_eq!(original.stacks, Some(2));
        assert_eq!(original.expires_at_ms - original.applied_at_ms, 24000);
        assert!(mark(&h, 1 - holder, "A").is_none());
        cast(&mut h, 1, 1, ADVANCED);
        cast(&mut h, 0, 1, GROVE);
        let refreshed = mark(&h, holder, "A").unwrap();
        assert_eq!(refreshed.instance_id, original.instance_id);
        assert_eq!(refreshed.stacks, Some(3));
        assert_eq!(refreshed.expires_at_ms, h.engine.now + 24000);
        assert_eq!(mark(&h, 1, "B").unwrap().stacks, Some(2));
        h.action(0, Action::Leave {}).unwrap();
        h.pump();
        assert!(mark(&h, 1, "A").is_none());
        assert_eq!(mark(&h, 1, "B").unwrap().stacks, Some(2));
    }
}

#[test]
fn source_ally_self_mark_cap_consumes_once_and_advanced_uses_configured_shield() {
    let mut h = host(|v| {
        for id in [GROVE, ADVANCED] {
            for row in v["combat_rules"]["ally_recovery_by_mastery"][id]
                .as_array_mut()
                .unwrap()
            {
                row["mark"]["owner"] = json!("primary_target");
                row["mark"]["at_cap_heal"] =
                    json!({"fixed":30,"source_attack_bps":0,"target_max_hp_bps":0});
            }
        }
        v["combat_rules"]["ally_recovery_by_mastery"][ADVANCED][0]["overheal_shield"] =
            json!({"status_id":"effect_vitality_overheal","conversion_bps":10000});
    });
    cast(&mut h, 0, 1, ADVANCED);
    assert_eq!(h.snapshot(1).own.shield_total, Some(100));
    cast(&mut h, 0, 1, GROVE);
    assert_eq!(mark(&h, 1, "A").unwrap().stacks, Some(3));
    h.ticks(396);
    cast(&mut h, 0, 1, ADVANCED);
    assert!(mark(&h, 1, "A").is_none());
    assert_eq!(h.snapshot(1).own.shield_total, Some(100));
    let heals = h
        .events
        .iter()
        .filter_map(|(who, event)| match event {
            EventV4::Private {
                event:
                    Event::HealResolved {
                        source,
                        target,
                        heal_kind: HealKind::Triggered,
                        raw_heal,
                        ..
                    },
            } if source.id == "A" && who.as_deref() == Some(&target.id) => {
                Some((target.id.as_str(), *raw_heal))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(heals, [("A", 30), ("B", 30)]);
    let consumes=h.events.iter().filter(|(who,event)| who.as_deref()==Some("B") && matches!(event,
        EventV4::Private{event:Event::StatusConsumed{definition_id,consumed_stacks:5,remaining_stacks:0,..}}
        if definition_id==MARK)).count();
    assert_eq!(consumes, 1);
}

#[test]
fn source_ally_self_and_mark_tables_validate_references_and_bounds() {
    catalog(|_| {}).unwrap();
    for list in [json!([GROVE, GROVE]), json!(["yishen-weizhong"])] {
        assert!(catalog(|v| v["combat_rules"]["ally_self_target_skills"] = list).is_err());
    }
    for (key, value) in [
        ("stacks", json!(6)),
        ("status_id", json!("missing")),
        ("owner", json!("enemy")),
        (
            "at_cap_heal",
            json!({"fixed":0,"source_attack_bps":0,"target_max_hp_bps":0}),
        ),
    ] {
        assert!(
            catalog(
                |v| v["combat_rules"]["ally_recovery_by_mastery"][ADVANCED][0]["mark"][key] = value
            )
            .is_err(),
            "{key}"
        );
    }
}
