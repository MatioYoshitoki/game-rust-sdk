use super::*;
use crate::protocol::SkillLoadout;

const JADE: &str = "huichun-jue";
const ADVANCED: &str = "huichun-jue-advanced";
const GROVE: &str = "wanmu-guiyuan";
const HOT: &str = "effect_vitality";
const SPIRIT: u32 = 3_000_000;

pub(super) fn catalog(
    change: impl FnOnce(&mut Value),
) -> std::io::Result<Arc<crate::ConfigCatalog>> {
    catalog_with(|v| {
        *v = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/mastery-stages.json"
        ))
        .unwrap();
        let sample: Value = serde_json::from_str(crate::test_data::fixture!(
            "examples/combat-host/source-ally-recovery.json"
        ))
        .unwrap();
        v["skill_library"] = sample["skill_library"].clone();
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

fn host(mastery: usize) -> DeltaHarness {
    let cat = catalog(|_| {}).unwrap();
    let mut request = create(&cat.default_config());
    request.protocol_version = 6;
    for seat in &mut request.participants {
        let mut ids = vec![JADE.into(), GROVE.into()];
        if mastery >= 2 {
            ids.push(ADVANCED.into());
        }
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
        store.spirit = SPIRIT;
    }
    h.active();
    h
}

fn cast(h: &mut DeltaHarness, i: usize, skill: &str) {
    let entity = &h.snapshot(1 - i).own.participant.entity;
    let action = Action::CastUnit {
        skill_id: skill.into(),
        target: UnitTarget {
            id: entity.id.clone(),
            generation: entity.generation,
        },
    };
    h.action(i, action).unwrap();
    h.pump();
}

#[test]
fn source_ally_recovery_stage_formulas_and_costs() {
    for stage in 0..5 {
        let mut h = host(stage);
        for member in &mut h.engine.world.as_mut().unwrap().members {
            member.hp = 100;
        }
        let skill = if stage >= 2 { ADVANCED } else { JADE };
        cast(&mut h, 0, skill);
        cast(&mut h, 1, GROVE);
        h.ticks(4);
        let direct = |from: &str, to: &str| {
            h.events
                .iter()
                .find_map(|(who, e)| match e {
                    EventV4::Private {
                        event:
                            Event::HealResolved {
                                source,
                                target,
                                raw_heal,
                                ..
                            },
                    } if source.id == from && target.id == to && who.as_deref() == Some(to) => {
                        Some(*raw_heal)
                    }
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(direct("A", "B"), [500, 320, 700, 700, 700][stage]);
        assert_eq!(direct("B", "A"), [340, 380, 420, 460, 500][stage]);
        assert_eq!(
            h.snapshot(0).own.spirit,
            SPIRIT - if stage >= 2 { 37 } else { 25 }
        );
        assert_eq!(h.snapshot(1).own.spirit, SPIRIT - 2_000_000);
        let periodic = h
            .snapshot(1)
            .own
            .statuses
            .iter()
            .find(|s| s.definition_id == HOT)
            .unwrap()
            .periodic
            .as_ref()
            .unwrap();
        assert_eq!(periodic.raw_heal, Some([25, 21, 42, 42, 42][stage]));
        assert_eq!(periodic.total_ticks, if stage >= 2 { 5 } else { 3 });
        assert_eq!(periodic.tick_on_expiry, Some(true));
    }
}

#[test]
fn source_ally_recovery_splash_overflow_and_last_tick_at_deadline() {
    let mut h = host(4);
    let members = &mut h.engine.world.as_mut().unwrap().members;
    members[0].hp = 900;
    members[1].hp = 960;
    cast(&mut h, 0, ADVANCED);
    h.ticks(4);
    for i in 0..2 {
        let own = &h.snapshot(i).own;
        assert_eq!(own.hp, 1000);
        assert_eq!(own.shield_total, Some(100));
        let hot = own
            .statuses
            .iter()
            .find(|s| s.definition_id == HOT)
            .unwrap();
        let periodic = hot.periodic.as_ref().unwrap();
        assert_eq!(hot.expires_at_ms - hot.applied_at_ms, 5000);
        assert_eq!(periodic.raw_heal, Some([21, 42][i]));
        assert_eq!(hot.source.id, "A");
    }
    h.ticks(167);
    for i in 0..2 {
        let own = &h.snapshot(i).own;
        let id = &own.participant.entity.id;
        let ticks = h
            .events
            .iter()
            .filter_map(|(who, e)| match e {
                EventV4::Private {
                    event:
                        Event::PeriodicHealTick {
                            definition_id,
                            target,
                            tick_index,
                            scheduled_at_ms,
                            raw_heal,
                            ..
                        },
                } if definition_id == HOT && &target.id == id && who.as_deref() == Some(id) => {
                    Some((*tick_index, *scheduled_at_ms, *raw_heal))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(ticks.len(), 5);
        assert_eq!(ticks[4], (5, 5120, [21, 42][i]));
        assert!(!own.statuses.iter().any(|s| s.definition_id == HOT));
    }
}

#[test]
fn source_ally_recovery_refresh_is_source_scoped_and_exit_cleans_its_effects() {
    let mut h = host(4);
    cast(&mut h, 0, JADE);
    h.ticks(4);
    let original = h
        .snapshot(1)
        .own
        .statuses
        .iter()
        .find(|s| s.definition_id == HOT)
        .unwrap()
        .instance_id
        .clone();
    cast(&mut h, 0, ADVANCED);
    cast(&mut h, 1, ADVANCED);
    h.ticks(4);
    let statuses = h
        .snapshot(1)
        .own
        .statuses
        .iter()
        .filter(|s| s.definition_id == HOT)
        .collect::<Vec<_>>();
    assert_eq!(statuses.len(), 2);
    let refreshed = statuses.iter().find(|s| s.source.id == "A").unwrap();
    assert_eq!(refreshed.instance_id, original);
    assert_eq!(refreshed.periodic.as_ref().unwrap().total_ticks, 5);
    h.action(0, Action::Leave {}).unwrap();
    h.pump();
    assert!(
        h.snapshot(1)
            .own
            .statuses
            .iter()
            .all(|s| s.source.id != "A")
    );
    assert!(
        h.snapshot(1)
            .own
            .statuses
            .iter()
            .any(|s| s.definition_id == HOT && s.source.id == "B")
    );
}

#[test]
fn source_ally_recovery_target_loss_releases_reservation_and_config_is_strict() {
    let mut h = host(4);
    cast(&mut h, 0, GROVE);
    assert_eq!(
        h.snapshot(0)
            .own
            .pending_action
            .as_ref()
            .unwrap()
            .resource_reservation
            .as_ref()
            .unwrap()
            .spirit,
        2_000_000
    );
    h.action(1, Action::Leave {}).unwrap();
    h.pump();
    h.ticks(4);
    assert_eq!(h.snapshot(0).own.spirit, SPIRIT);
    assert!(h.snapshot(0).own.pending_action.is_none());
    assert!(
        h.snapshot(0)
            .own
            .player_skills
            .iter()
            .all(|s| s.ready_at_ms == 0)
    );
    for (key, value) in [
        ("minimum_mastery", json!(1)),
        (
            "hot",
            json!({"status_id":HOT,"duration_ms":3001,"tick_bps":500}),
        ),
        (
            "overheal_shield",
            json!({"status_id":HOT,"conversion_bps":10000}),
        ),
        (
            "splash",
            json!({"radius":513,"heal_bps":6000,"hot_bps":5000}),
        ),
    ] {
        assert!(
            catalog(|v| v["combat_rules"]["ally_recovery_by_mastery"][JADE][0][key] = value)
                .is_err(),
            "{key}"
        );
    }
}
