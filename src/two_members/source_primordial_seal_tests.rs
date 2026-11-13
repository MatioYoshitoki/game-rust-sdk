use super::*;

const SKILL: &str = "wushi-shu";
const COST: u32 = 3_500_000;
const SPIRIT: u32 = 8_000_000;
const SEALS: [&str; 5] = [
    "effect_wushi_seal",
    "effect_wushi_seal_extended",
    "effect_wushi_seal_extended",
    "effect_wushi_seal_vulnerable",
    "effect_wushi_seal_final",
];

fn document(v: &mut Value) {
    *v = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/mastery-stages.json"
    ))
    .unwrap();
    let sample: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/source-wushi-shu.json"
    ))
    .unwrap();
    v["additional_skills"]
        .as_array_mut()
        .unwrap()
        .push(sample["skill"].clone());
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
    v["combat_rules"]["build_rules"]["power_models"]["mastery-demo"]["rank_base"] = json!([100]);
    v["resource_limits"]["max_spirit"] = json!(SPIRIT);
    v["resource_limits"]["max_spirit_cost"] = json!(COST);
    v["pve"]["monster_positions"] = json!([[440, 400], [440, 480]]);
}

fn boons(v: &mut Value) {
    for (index, id) in ["boon-early", "boon-late"].into_iter().enumerate() {
        v["pve"]["status_definitions"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id":id,"mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":500,
                "duration_ms":10000 + index * 1000,"duplicate_policy":"refresh","source_scoped":true
            }));
        v["combat_rules"]["status_policies"][id] = json!({
            "disposition":"beneficial","dispellable":true,"transferable":false,"blocks":[]
        });
        v["combat_rules"]["triggers"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id":id,"profiles":["npc"],"when":"damage_taken","include_periodic":false,
                "every":1,"cooldown_ms":0,"priority":index,
                "effect":{"kind":"apply_self_status","status_id":id}
            }));
    }
}

fn host(stages: [Option<usize>; 2], change: impl FnOnce(&mut Value)) -> DeltaHarness {
    let cat = catalog_with(|v| {
        document(v);
        change(v);
    })
    .unwrap();
    let mut request = create(&cat.default_config());
    request.protocol_version = 6;
    for (seat, stage) in request.participants.iter_mut().zip(stages) {
        seat.combat_build = stage.map(|mastery| CombatBuild {
            power_points: 0,
            skill_power_points: 0,
            element_resistance_points: 0,
            powers: BTreeMap::from([(
                SKILL.into(),
                PowerGrowth {
                    model_id: "mastery-demo".into(),
                    rank: 0,
                    quality: 0,
                    level: 0,
                    mastery,
                },
            )]),
        });
    }
    let mut h = DeltaHarness::from_request(cat, request);
    for s in &mut h.stores {
        s.spirit = SPIRIT;
    }
    h.active();
    h
}

fn cast(h: &DeltaHarness, target: usize, skill: &str) -> Action {
    let entity = &h.snapshot(0).monsters[target].entity;
    Action::CastUnit {
        skill_id: skill.into(),
        target: UnitTarget {
            id: entity.id.clone(),
            generation: entity.generation,
        },
    }
}

fn status<'a>(
    h: &'a DeltaHarness,
    recipient: usize,
    target: usize,
    id: &str,
) -> Option<&'a StatusView> {
    h.snapshot(recipient).monsters[target]
        .statuses
        .as_ref()?
        .iter()
        .find(|s| s.definition_id == id)
}

fn tick_to(h: &mut DeltaHarness, deadline: u64) {
    h.ticks(
        deadline
            .saturating_sub(h.engine.now)
            .div_ceil(h.engine.config.step_ms) as usize,
    );
}

#[test]
fn source_primordial_seal_five_trusted_stages_select_original_durations_and_cost() {
    for stages in [[Some(0), Some(1)], [Some(2), Some(3)], [Some(4), None]] {
        let mut h = host(stages, |_| {});
        let initial: Vec<_> = h.snapshot(0).monsters.iter().map(|n| n.hp).collect();
        for i in 0..2 {
            let input = h.input(i, cast(&h, i, SKILL));
            let accepted = h.engine.input(input.clone()).unwrap();
            assert_eq!(h.engine.input(input).unwrap(), accepted);
        }
        h.ticks(5);
        for (i, expected_hp) in initial.iter().copied().enumerate() {
            assert_eq!(h.snapshot(i).own.spirit, SPIRIT);
            assert_eq!(
                h.snapshot(i)
                    .own
                    .pending_action
                    .as_ref()
                    .unwrap()
                    .resource_reservation
                    .as_ref()
                    .unwrap()
                    .spirit,
                COST
            );
            assert_eq!(h.snapshot(i).monsters[i].hp, expected_hp);
        }
        h.ticks(1);
        for (i, stage) in stages.into_iter().enumerate() {
            let s = status(&h, i, i, SEALS[stage.unwrap_or(0)]).unwrap();
            let own = &h.snapshot(i).own;
            assert_eq!(s.source.id, own.participant.entity.id);
            assert_eq!(s.applied_at_ms, 180);
            assert_eq!(
                s.expires_at_ms - s.applied_at_ms,
                if stage.unwrap_or(0) == 0 { 5000 } else { 6000 }
            );
            assert_eq!(s.modifier_bps, -5000);
            assert_eq!(own.spirit, SPIRIT - COST);
            assert_eq!(own.recovery_until_ms, 320);
            assert!(
                own.player_skills
                    .iter()
                    .any(|v| v.skill_id == SKILL && v.ready_at_ms == 2180)
            );
            assert_eq!(h.snapshot(i).monsters[i].hp, initial[i] - 400);
        }
    }
}

#[test]
fn source_primordial_seal_amp_is_strongest_refreshes_and_final_expiry_dispels_one() {
    let mut h = host([Some(3), Some(4)], boons);
    let hp = h.snapshot(0).monsters[0].hp;
    for i in 0..2 {
        h.action(i, cast(&h, 0, SKILL)).unwrap();
    }
    h.ticks(6);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp - 400 - 460);
    let early = status(&h, 1, 0, "boon-early").unwrap().clone();
    let late = status(&h, 1, 0, "boon-late").unwrap().clone();
    let first = status(&h, 1, 0, SEALS[4]).unwrap().clone();
    tick_to(&mut h, first.applied_at_ms + 2000);
    h.action(1, cast(&h, 0, SKILL)).unwrap();
    h.ticks(6);
    let refreshed = status(&h, 1, 0, SEALS[4]).unwrap();
    assert_eq!(refreshed.instance_id, first.instance_id);
    assert!(refreshed.expires_at_ms > first.expires_at_ms);
    assert_eq!(h.snapshot(0).monsters[0].hp, hp - 400 - 460 - 460);
    assert_eq!(status(&h, 1, 0, "boon-early"), Some(&early));
    assert_eq!(status(&h, 1, 0, "boon-late"), Some(&late));
    let expiry = refreshed.expires_at_ms;
    tick_to(&mut h, expiry);
    assert!(status(&h, 1, 0, "boon-early").is_none());
    assert_eq!(status(&h, 1, 0, "boon-late"), Some(&late));
    assert!(h.events.iter().any(|(_,e)| matches!(e,EventV4::Private{
        event:Event::StatusCleanseResolved {definition_id,phase:StatusCleansePhase::Expired,removed_count:1,..}
    } if definition_id==SEALS[4])));
}

#[test]
fn source_primordial_seal_target_recheck_releases_reservation_without_cooldown() {
    let mut h = host([Some(4), Some(0)], |_| {});
    h.action(0, cast(&h, 0, SKILL)).unwrap();
    h.engine.world.as_mut().unwrap().members[0].position = [100., 100.];
    h.ticks(6);
    assert_eq!(h.snapshot(0).own.spirit, SPIRIT);
    assert!(
        h.snapshot(0)
            .own
            .player_skills
            .iter()
            .any(|v| v.skill_id == SKILL && v.ready_at_ms == 0)
    );
    assert!(status(&h, 1, 0, SEALS[4]).is_none());
}

#[test]
fn source_primordial_seal_rejects_invalid_configuration_rows_and_vulnerability_bindings() {
    for case in 0..8 {
        assert!(
            catalog_with(|v| {
                document(v);
                let r = &mut v["combat_rules"];
                match case {
                    0 => r["unit_hit_statuses_by_mastery"][SKILL] = json!([]),
                    1 => r["unit_hit_statuses_by_mastery"][SKILL][0]["minimum_mastery"] = json!(5),
                    2 => {
                        r["unit_hit_statuses_by_mastery"][SKILL][0]["statuses"] = json!(["missing"])
                    }
                    3 => {
                        r["unit_hit_statuses_by_mastery"][SKILL][0]["statuses"] =
                            json!([SEALS[0], SEALS[0]])
                    }
                    4 => {
                        r["unit_hit_statuses_by_mastery"]["yishen-weizhong"] =
                            r["unit_hit_statuses_by_mastery"][SKILL].clone()
                    }
                    5 => r["status_damage_taken_bps"][SEALS[3]] = json!(10001),
                    6 => r["status_damage_taken_bps"]["missing"] = json!(1500),
                    _ => r["status_policies"][SEALS[3]]["disposition"] = json!("beneficial"),
                }
            })
            .is_err(),
            "{case}"
        );
    }
}
