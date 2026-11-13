use super::*;
use crate::asset_contract::tests::Package;
use serde_json::{Value, json};
fn rules() -> Value {
    json!({"conversion":{"hp_bps":10000,"spirit_bps":10000,"atk_bps":15000,"def_bps":5000,"spd_bps":10000,"element_resistance_bps":5000},"maximum_input_points":10000,"power_models":{"growth":{"rank_base":[30,90],"quality_add_bps":[0,2000],"level_add_bps":[0,3000],"mastery_add_bps":[0,5000],"external_bps":12500,"maximum_power":1000000000}}})
}
fn build(skill: &str) -> CombatBuild {
    serde_json::from_value(json!({"power_points":20,"skill_power_points":10,"element_resistance_points":61,"powers":{skill:{"model_id":"growth","rank":0,"quality":1,"level":1,"mastery":1}}})).unwrap()
}
fn setup() -> (Config, World) {
    let (mut c, mut w) = super::unit_effect_tests::setup(true, false);
    let r = c.combat_rules.as_mut().unwrap();
    r.build_rules = Some(serde_json::from_value(rules()).unwrap());
    r.skills.get_mut(&c.skill.id).unwrap().base_power = None;
    w.members[0].combat_build = Some(build(&c.skill.id));
    w.members[0].facts.def_points = 0;
    (c, w)
}
fn bind(w: &mut World, c: &mut Config, d: Value) {
    let id = d["id"].as_str().unwrap().to_string();
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
    let e = w.members[0].reference.entity.clone();
    super::periodic_tests::bind(w, c, Owner::Member(0), e, &id, 0);
}
#[test]
fn character_build_growth_adds_quality_level_and_mastery_before_external_multiplier() {
    let (c, w) = setup();
    assert_eq!(
        w.selected_power_with(&w.statuses, Owner::Member(0), &c.skill.id, &c, 0),
        99
    );
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), 99);
}
#[test]
fn character_build_raw_points_conversion_is_independent_of_tier_and_temporary_modifiers() {
    let (mut c, mut w) = setup();
    assert_eq!(w.stats(0, &c, 0), (150, 1));
    for p in c.combat_rules.as_mut().unwrap().profiles.values_mut() {
        p.tier = 8;
    }
    assert_eq!(w.stats(0, &c, 0), (150, 1));
    bind(
        &mut w,
        &mut c,
        json!({"id":"atk-boost","mechanism":"timed_attribute_modifier","attribute":"atk","modifier_bps":5000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    assert_eq!(w.stats(0, &c, 60).0, 225);
    assert_eq!(w.stats(0, &c, 300).0, 150);
    assert_eq!(w.members[0].facts.atk_points, 100);
}
#[test]
fn character_build_basic_uses_attack_while_skill_uses_its_owned_growth() {
    let (mut c, mut w) = setup();
    w.members[0].facts.atk_points = 200;
    assert_eq!(w.configured_budget(0, None, &c, 0), 300);
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), 99);
    bind(
        &mut w,
        &mut c,
        json!({"id":"power-boost","mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    assert_eq!(w.configured_budget(0, None, &c, 60), 300);
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 60), 108);
}
#[test]
fn character_build_final_attribute_multipliers_floor_once() {
    let (mut c, mut w) = setup();
    let r = c
        .combat_rules
        .as_mut()
        .unwrap()
        .build_rules
        .as_mut()
        .unwrap();
    let m = r.power_models.get_mut("growth").unwrap();
    m.rank_base = vec![1];
    m.external_bps = 10000;
    m.quality_add_bps = vec![0, 0];
    m.level_add_bps = vec![0, 0];
    m.mastery_add_bps = vec![0, 0];
    let b = w.members[0].combat_build.as_mut().unwrap();
    b.power_points = 50;
    b.skill_power_points = 50;
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), 2);
}
#[test]
fn character_build_each_growth_phase_has_its_declared_floor() {
    let (mut c, w) = setup();
    let m = c
        .combat_rules
        .as_mut()
        .unwrap()
        .build_rules
        .as_mut()
        .unwrap()
        .power_models
        .get_mut("growth")
        .unwrap();
    m.rank_base = vec![31];
    m.quality_add_bps[1] = 1001;
    m.level_add_bps[1] = 1002;
    m.mastery_add_bps[1] = 1003;
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), 66);
}
#[test]
fn character_build_member_inputs_are_not_borrowed_from_peers() {
    let (c, mut w) = setup();
    let mut b = build(&c.skill.id);
    b.power_points = 0;
    b.skill_power_points = 0;
    w.members[1].combat_build = Some(b);
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), 99);
    assert_eq!(w.configured_budget(1, Some(&c.skill), &c, 0), 75);
}
#[test]
fn character_build_missing_skill_model_uses_live_attack_with_one_point_multiplier() {
    let (c, mut w) = setup();
    w.members[0].combat_build.as_mut().unwrap().powers.clear();
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), 198);
    w.members[0].facts.atk_points = 200;
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), 396);
}
#[test]
fn character_build_growth_model_is_not_modified_by_live_power_status() {
    let (mut c, mut w) = setup();
    let original = serde_json::to_value(&w.members[0].combat_build).unwrap();
    bind(
        &mut w,
        &mut c,
        json!({"id":"power-boost","mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 60), 108);
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 300), 99);
    assert_eq!(
        serde_json::to_value(&w.members[0].combat_build).unwrap(),
        original
    );
}
#[test]
fn character_build_permanent_and_temporary_skill_bonus_share_one_cap() {
    let (mut c, mut w) = setup();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .attribute_caps_bps
        .insert(StatusAttribute::SkillPowerBonus, 1500);
    bind(
        &mut w,
        &mut c,
        json!({"id":"power-boost","mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 60), 103);
}
#[test]
fn character_build_committed_channel_budget_does_not_resample_live_power() {
    let (mut c, mut w) = setup();
    super::unit_effect_tests::start(&c, &mut w, false);
    assert_eq!(w.members[0].channel.as_ref().unwrap().raw, 99);
    bind(
        &mut w,
        &mut c,
        json!({"id":"power-boost","mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","modifier_bps":10000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let mut e = vec![];
    for now in [120, 180, 240] {
        w.advance_channel(0, &c, now, &mut e);
    }
    let pulses = e
        .iter()
        .filter_map(|(_, e)| {
            if let EventV4::Private {
                event: Event::UnitChannelPulse { raw_damage, .. },
            } = e
            {
                Some(*raw_damage)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(pulses, vec![33, 33, 33]);
}
#[test]
fn character_build_npc_skill_and_basic_keep_distinct_bases() {
    let (mut c, w) = setup();
    let npc = w.npcs[0].template_id.clone();
    let profile = c.combat_rules.as_ref().unwrap().npc(&npc).clone();
    let mut b = build(&w.npcs[0].profile.combat.projectile.id);
    b.power_points = 0;
    b.skill_power_points = 0;
    let mut p = profile;
    p.build = Some(b);
    c.combat_rules
        .as_mut()
        .unwrap()
        .profiles
        .insert("built-npc".into(), p);
    c.combat_rules
        .as_mut()
        .unwrap()
        .npc_profiles
        .insert(npc, "built-npc".into());
    let mut r = c
        .combat_rules
        .as_ref()
        .unwrap()
        .npc_attacks
        .get(&w.npcs[0].template_id)
        .cloned()
        .unwrap_or_default();
    r.base_power = None;
    r.fixed = 0;
    c.combat_rules
        .as_mut()
        .unwrap()
        .npc_attacks
        .insert(w.npcs[0].template_id.clone(), r);
    assert_eq!(w.npc_attack_budget(0, 40, 10000, true, &c, 0), 75);
    assert_eq!(w.npc_attack_budget(0, 40, 10000, false, &c, 0), 40);
}
#[test]
fn character_build_zero_and_large_growth_are_bounded_without_wrap() {
    let (mut c, mut w) = setup();
    let m = c
        .combat_rules
        .as_mut()
        .unwrap()
        .build_rules
        .as_mut()
        .unwrap()
        .power_models
        .get_mut("growth")
        .unwrap();
    m.rank_base[0] = 1000000000;
    m.quality_add_bps[1] = 1000000;
    m.level_add_bps[1] = 1000000;
    m.mastery_add_bps[1] = 1000000;
    m.external_bps = 1000000;
    let b = w.members[0].combat_build.as_mut().unwrap();
    b.power_points = 10000;
    b.skill_power_points = 10000;
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), u32::MAX);
    let m = c
        .combat_rules
        .as_mut()
        .unwrap()
        .build_rules
        .as_mut()
        .unwrap()
        .power_models
        .get_mut("growth")
        .unwrap();
    m.quality_add_bps[1] = -10000;
    m.level_add_bps[1] = -10000;
    m.mastery_add_bps[1] = -10000;
    assert_eq!(w.configured_budget(0, Some(&c.skill), &c, 0), 0);
}
#[test]
fn character_build_selected_record_power_ignores_unrelated_attack_changes() {
    let (mut c, mut w) = setup();
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_power_sources
        .insert("guard".into(), c.skill.id.clone());
    assert_eq!(
        w.status_source_power(&w.statuses, Owner::Member(0), "guard", &c, 0),
        Some(99)
    );
    w.members[0].facts.atk_points = 1000;
    assert_eq!(
        w.status_source_power(&w.statuses, Owner::Member(0), "guard", &c, 0),
        Some(99)
    );
    assert_eq!(w.record_power(Owner::Member(0), &c, 0), 1980);
}
#[test]
fn character_build_selected_finisher_preflight_uses_same_current_power() {
    let (c, mut w) = setup();
    w.npcs[0].hp = 10;
    let rule=serde_json::from_value(json!({"source_skill_id":c.skill.id,"hp_threshold_bps":10000,"comparison":"at_most","default":{"mode":"power_damage","fixed":0,"power_bps":10000,"cap_power_bps":5000}})).unwrap();
    let plan = w
        .plan_finish_other(
            &w.statuses,
            (Owner::Member(0), Owner::Npc(0)),
            &rule,
            &c,
            0,
            None,
        )
        .unwrap();
    assert_eq!(plan.raw, 49);
}
#[test]
fn character_build_healing_uses_selected_skill_growth_and_caps_actual_return() {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().healing.insert(
        c.skill.id.clone(),
        serde_json::from_value(json!({"fixed":0,"source_attack_bps":10000,"target_max_hp_bps":0}))
            .unwrap(),
    );
    w.members[1].hp = 1;
    let heal = serde_json::from_value(json!({"amount":1})).unwrap();
    let mut e = vec![];
    w.resolve_skill_heal((0, 1), &heal, &c.skill.id, &c, 0, &mut e);
    assert_eq!(w.members[1].hp, 100);
    assert!(e.iter().any(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::HealResolved {
                raw_heal: 99,
                hp_restored: 99,
                ..
            }
        }
    )));
}
#[test]
fn character_build_projection_contains_only_effective_owned_slot_powers() {
    let (c, w) = setup();
    let own = w.snapshot(0, &c, Phase::Active, 0);
    assert_eq!(own.own.skill_powers.as_ref().unwrap()[&c.skill.id], 99);
    let serialized = serde_json::to_string(&own).unwrap();
    assert!(
        !serialized.contains("model_id")
            && !serialized.contains("rank_base")
            && !serialized.contains("power_points")
    );
    assert!(
        w.snapshot(1, &c, Phase::Active, 0)
            .own
            .skill_powers
            .is_none()
    );
}
#[test]
fn character_build_constant_conversion_floors_and_keeps_required_base_minimum() {
    let (mut c, w) = setup();
    let cv = &mut c
        .combat_rules
        .as_mut()
        .unwrap()
        .build_rules
        .as_mut()
        .unwrap()
        .conversion;
    cv.hp_bps = 5000;
    cv.spirit_bps = 5000;
    cv.spd_bps = 1250;
    assert_eq!(
        w.entity_max_hp(&w.members[0].reference.entity, &c, 0),
        Some(50)
    );
    assert_eq!(w.member_max_spirit(0, &c, 0), 50);
    assert_eq!(w.effective_spd(Owner::Member(0), &c, 0), 1);
    assert_eq!(w.source_element_resistance(Owner::Member(0), &c), Some(30));
    assert_eq!(w.members[0].base_points(&c, StatusAttribute::Def), 1);
}
#[test]
fn character_build_cold_models_reject_null_bounds_unknown_and_ambiguous_base() {
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/units-a.json"
    ))
    .unwrap();
    d["combat_rules"]["build_rules"] = rules();
    p.write(&d);
    assert!(p.load().is_ok());
    for n in 0..10 {
        let mut bad = d.clone();
        match n {
            0 => bad["combat_rules"]["build_rules"] = Value::Null,
            1 => bad["combat_rules"]["build_rules"]["power_models"] = json!({}),
            2 => bad["combat_rules"]["build_rules"]["conversion"]["atk_bps"] = json!(0),
            3 => {
                bad["combat_rules"]["build_rules"]["power_models"]["growth"]["rank_base"] =
                    json!([])
            }
            4 => {
                bad["combat_rules"]["build_rules"]["power_models"]["growth"]["external_bps"] =
                    json!(1000001)
            }
            5 => bad["combat_rules"]["build_rules"]["power_models"]["growth"]["typo"] = json!(1),
            6 => bad["mechanism_version"] = json!(104),
            7 => {
                bad["combat_rules"]["build_rules"]["power_models"]["growth"]["quality_add_bps"] =
                    json!([-10001])
            }
            8 => bad["combat_rules"]["build_rules"]["maximum_input_points"] = json!(1000001),
            _ => {
                bad["combat_rules"]["profiles"]["member"]["build"] =
                    serde_json::to_value(build("unit-strike")).unwrap()
            }
        };
        p.write(&bad);
        assert!(p.load().is_err(), "case {n}");
    }
}

fn public_build_fixture() -> (Package, Arc<crate::ConfigCatalog>, Config, CreateV4) {
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/units-a.json"
    ))
    .unwrap();
    d["combat_rules"]["build_rules"] = rules();
    d["combat_rules"]["build_rules"]["conversion"]["hp_bps"] = json!(20000);
    d["combat_rules"]["skills"]["unit-strike"]
        .as_object_mut()
        .unwrap()
        .remove("base_power");
    p.write(&d);
    let cat = Arc::new(p.load().unwrap());
    let c = cat.default_config().as_ref().clone();
    let mut seats:Vec<Value>=(0..4).map(|i|json!({"participant_id":format!("P{i:02}"),"authenticated_principal":format!("principal-{i}"),"connection":{"id":format!("connection-{i}"),"generation":1}})).collect();
    seats[0]["combat_build"] = serde_json::to_value(build("unit-strike")).unwrap();
    let request=serde_json::from_value(json!({"protocol_version":6,"key":{"id":"build-native","incarnation":1},"request_id":"create","config_id":c.id,"config_hash":c.hash(),"seed":7,"participants":seats})).unwrap();
    (p, cat, c, request)
}
#[test]
fn character_build_invalid_trusted_growth_is_rejected_before_any_load_or_admission() {
    let (_p, cat, c, request) = public_build_fixture();
    for n in 0..5 {
        let mut bad = request.clone();
        let b = bad.participants[0].combat_build.as_mut().unwrap();
        match n {
            0 => b.powers.get_mut("unit-strike").unwrap().rank = 2,
            1 => b.power_points = 10001,
            2 => b.powers.get_mut("unit-strike").unwrap().model_id = "missing".into(),
            3 => {
                let p = b.powers.remove("unit-strike").unwrap();
                b.powers.insert("unknown-skill".into(), p);
            }
            _ => b.powers.get_mut("unit-strike").unwrap().quality = usize::MAX,
        };
        assert!(matches!(
            crate::two_members::TwoMembers::create(c.clone(), cat.clone(), bad),
            Err(Error::InvalidMessage)
        ));
    }
    for change in [
        json!({"combat_build":null}),
        json!({"combat_build":{"final_power":99}}),
    ] {
        let mut seat = serde_json::to_value(&request.participants[0]).unwrap();
        seat.as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(serde_json::from_value::<ParticipantSeat>(seat).is_err());
    }
}
#[test]
fn character_build_recovery_identity_fences_growth_and_never_recreates_a_world() {
    let (_p, cat, c, request) = public_build_fixture();
    let mut runtime =
        crate::two_members::TwoMembers::create(c.clone(), cat.clone(), request.clone()).unwrap();
    let checkpoint = runtime
        .drain()
        .into_iter()
        .find_map(|o| {
            if let OutputBody::ResponsibilityV4 { checkpoint } = o.body {
                Some(*checkpoint)
            } else {
                None
            }
        })
        .unwrap();
    let mut resumed = request.clone();
    resumed.recovery = Some(RecoveryV4 {
        generation: 1,
        checkpoint: checkpoint.clone(),
    });
    let recovered =
        crate::two_members::TwoMembers::create(c.clone(), cat.clone(), resumed.clone()).unwrap();
    assert_eq!(recovered.status().phase, Phase::Closed);
    resumed.participants[0]
        .combat_build
        .as_mut()
        .unwrap()
        .power_points += 1;
    assert!(matches!(
        crate::two_members::TwoMembers::create(c, cat, resumed),
        Err(Error::InvalidMessage)
    ));
}
#[test]
fn character_build_converted_resource_overflow_rejects_external_load_before_debit() {
    let (_p, cat, c, request) = public_build_fixture();
    let mut runtime =
        crate::two_members::TwoMembers::create(c.clone(), cat, request.clone()).unwrap();
    let load = runtime
        .drain()
        .into_iter()
        .find_map(|o| {
            if let OutputBody::BusinessRequestV4 {
                participant_id,
                request,
                ..
            } = o.body
            {
                if participant_id == "P00" {
                    Some(request)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    let mut facts = AssetParticipantFacts {
        instance_count: None,
        warehouse_other_slots: None,
        location: None,
        principal: "principal-0".into(),
        revision: 0,
        hp_points: 600000,
        spirit_points: 200,
        atk_points: 100,
        def_points: 10,
        spd_points: 8,
        assets: vec![
            AssetAmount {
                asset_id: c.item_id.clone(),
                quantity: 100,
            },
            AssetAmount {
                asset_id: c
                    .asset_contract
                    .as_ref()
                    .unwrap()
                    .entry_cost_asset_id
                    .clone(),
                quantity: 100,
            },
        ],
    };
    facts.assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    let mut receipt = MemberBusinessReceipt {
        key: request.key,
        participant_id: "P00".into(),
        operation_id: load.operation_id,
        attempt_id: load.attempt_id,
        outcome: ReceiptOutcome::AssetsLoaded { facts },
    };
    assert_eq!(runtime.business(receipt.clone()), Err(Error::NoResource));
    assert!(!runtime.drain().iter().any(|o|matches!(&o.body,OutputBody::BusinessRequestV4{request,..}if matches!(request.body,Business::CommitAssets{..}))));
    if let ReceiptOutcome::AssetsLoaded { facts } = &mut receipt.outcome {
        facts.hp_points = 1000;
    }
    runtime.business(receipt).unwrap();
    assert!(runtime.drain().iter().any(|o|matches!(&o.body,OutputBody::BusinessRequestV4{request,..}if matches!(request.body,Business::CommitAssets{..}))));
}
#[test]
fn character_build_initial_resource_conversion_is_applied_once() {
    let (mut c, w) = setup();
    let cv = &mut c
        .combat_rules
        .as_mut()
        .unwrap()
        .build_rules
        .as_mut()
        .unwrap()
        .conversion;
    cv.hp_bps = 5000;
    cv.spirit_bps = 5000;
    let seats: Vec<_> = w
        .members
        .iter()
        .map(|m| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            combat_build: m.combat_build.clone(),
            skills: None,
            participant_id: m.reference.entity.id.clone(),
            authenticated_principal: m.facts.principal.clone(),
            connection: Connection {
                id: format!("c-{}", m.reference.entity.id),
                generation: 1,
            },
        })
        .collect();
    let facts = w.members.iter().map(|m| m.facts.clone()).collect();
    let created = World::new(&c, &seats, facts, 7);
    assert_eq!((created.members[0].hp, created.members[0].spirit), (50, 50));
    assert_eq!(
        created.entity_max_hp(&created.members[0].reference.entity, &c, 0),
        Some(50)
    );
}
#[test]
fn character_build_record_cap_snapshots_selected_skill_then_rebinds_live_power() {
    let (mut c, mut w) = setup();
    let rule=serde_json::from_value(json!({"basis":"incoming","cap_fixed":0,"cap_power_bps":10000,"damage_reduction_bps":0,"refresh":"reset","on_depleted":false,"on_expired":false,"on_consumed":false})).unwrap();
    c.combat_rules
        .as_mut()
        .unwrap()
        .shield_records
        .insert("built-record".into(), rule);
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_power_sources
        .insert("built-record".into(), c.skill.id.clone());
    bind(
        &mut w,
        &mut c,
        json!({"id":"built-record","mechanism":"absorption_shield","shield_bps":1000,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true}),
    );
    let cap = |w: &World| {
        w.statuses
            .entries()
            .iter()
            .find(|s| s.definition_id == "built-record")
            .unwrap()
            .shield
            .as_ref()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .cap
    };
    assert_eq!(cap(&w), 99);
    bind(
        &mut w,
        &mut c,
        json!({"id":"new-power","mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
    );
    assert_eq!(cap(&w), 99);
    let target = w.members[0].reference.entity.clone();
    w.apply_owned_statuses(
        Owner::Member(0),
        (target, 100),
        &["built-record".into()],
        &c,
        60,
        &mut vec![],
    );
    assert_eq!(cap(&w), 108);
}
#[test]
fn character_build_periodic_healing_preserves_explicit_source_snapshot_or_live_sampling() {
    for live in [false, true] {
        let (mut c, mut w) = setup();
        let id = "built-hot";
        c.combat_rules
            .as_mut()
            .unwrap()
            .status_power_sources
            .insert(id.into(), c.skill.id.clone());
        c.combat_rules.as_mut().unwrap().healing.insert(
            id.into(),
            serde_json::from_value(
                json!({"fixed":0,"source_attack_bps":10000,"target_max_hp_bps":0}),
            )
            .unwrap(),
        );
        c.combat_rules.as_mut().unwrap().periodic_sampling.insert(
            id.into(),
            if live {
                PeriodicSampling::SourceLive
            } else {
                PeriodicSampling::SourceSnapshot
            },
        );
        bind(
            &mut w,
            &mut c,
            json!({"id":id,"mechanism":"periodic_heal","heal_bps":0,"tick_interval_ms":60,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
        );
        bind(
            &mut w,
            &mut c,
            json!({"id":"new-power","mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}),
        );
        let mut events = vec![];
        w.prepare_periodic(60, &mut events);
        w.advance_periodic(&c, 60, &mut events);
        assert!(events.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::PeriodicHealTick{raw_heal,..}}if *raw_heal==if live{108}else{99})));
    }
}
#[test]
fn character_build_contact_reaction_reads_original_sources_selected_skill_at_execution() {
    let (mut c, mut w) = setup();
    let id = "built-mark";
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":-1000,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    let r = c.combat_rules.as_mut().unwrap();
    r.status_power_sources.insert(id.into(), c.skill.id.clone());
    r.status_policies.insert(
        id.into(),
        serde_json::from_value(
            json!({"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]}),
        )
        .unwrap(),
    );
    r.contact_reactions.insert(id.into(),serde_json::from_value(json!({"when":"direct_contact","attacker":"any_hostile","fixed":0,"source_power_bps":10000,"damage_kind":"physical","ignore_armor":true,"armor_penetration_cap_bps":10000})).unwrap());
    let target = w.npcs[0].entity.clone();
    w.apply_owned_statuses(
        Owner::Member(0),
        (target, 1000),
        &[id.into()],
        &c,
        0,
        &mut vec![],
    );
    w.members[0].facts.atk_points = 1000;
    let provoker = w.members[1].reference.entity.clone();
    let mut e = vec![];
    w.direct_contact(
        Owner::Member(1),
        Owner::Npc(0),
        combat::DirectContact {
            origin: None,
            action_id: Some("provoke"),
            rule: None,
            ignore_armor: true,
            unavoidable: true,
            dead_source: false,
            source: &provoker,
            raw: 1,
            metal: false,
            hit_statuses: &[],
        },
        &c,
        60,
        &mut e,
    );
    assert!(e.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::ContactReactionResolved{raw_damage:99,source,..}}if source.id==w.members[0].reference.entity.id)));
}
#[test]
fn character_build_private_dto_rejects_raw_build_and_null_power_tables() {
    let (c, w) = setup();
    let own = w.snapshot(0, &c, Phase::Active, 0).own;
    for bad in [json!(null), json!({"growth":{"model_id":"growth"}})] {
        let mut v = serde_json::to_value(&own).unwrap();
        v["skill_powers"] = bad;
        assert!(serde_json::from_value::<PrivateMemberView>(v).is_err());
    }
    let mut v = serde_json::to_value(&own).unwrap();
    v["combat_build"] = serde_json::to_value(build(&c.skill.id)).unwrap();
    assert!(serde_json::from_value::<PrivateMemberView>(v).is_err());
}

#[test]
fn character_build_elemental_damage_uses_converted_resistance_in_the_shared_pipeline() {
    let (mut c, mut w) = setup();
    for p in c.combat_rules.as_mut().unwrap().profiles.values_mut() {
        p.tier = 0;
        p.element = Some("water".into());
        p.defense_baseline_attack = 100;
    }
    let source = w.npcs[0].entity.clone();
    let target = w.members[0].reference.entity.clone();
    let hit = combat::DamageIntent {
        source,
        target,
        raw: 100,
        ignore_armor: false,
        direct: true,
    };
    let rule = crate::config::AttackRule {
        damage_kind: crate::config::DamageKind::Elemental,
        element: Some("water".into()),
        can_crit: false,
        unavoidable: true,
        ..Default::default()
    };
    assert_eq!(
        w.configured_incoming(
            &hit,
            &rule,
            (Owner::Npc(0), Owner::Member(0)),
            hit.direct,
            &c,
            0
        ),
        (86, false)
    );
}

#[test]
fn character_build_source_contract_vectors_execute_through_real_world_reads() {
    let f: Value = serde_json::from_str(crate::test_data::fixture!(
        "src/verification/source_character_power_vectors.json"
    ))
    .unwrap();
    let (mut c, mut w) = setup();
    for row in f["pointVectors"].as_array().unwrap() {
        let a = row["axis"].as_str().unwrap();
        let points = row["points"].as_u64().unwrap() as u32;
        let k = row["conversionBps"].as_u64().unwrap() as u32;
        let conversion = &mut c
            .combat_rules
            .as_mut()
            .unwrap()
            .build_rules
            .as_mut()
            .unwrap()
            .conversion;
        let actual = match a {
            "atk" => {
                conversion.atk_bps = k;
                w.members[0].facts.atk_points = points;
                w.stats(0, &c, 0).0
            }
            "def" => {
                conversion.def_bps = k;
                w.members[0].facts.def_points = points;
                w.stats(0, &c, 0).1
            }
            "spd" => {
                conversion.spd_bps = k;
                w.members[0].facts.spd_points = points;
                w.effective_spd(Owner::Member(0), &c, 0)
            }
            _ => unreachable!(),
        };
        assert_eq!(actual, row["expected"].as_u64().unwrap() as u32);
    }
    for row in f["powerVectors"].as_array().unwrap() {
        let m = c
            .combat_rules
            .as_mut()
            .unwrap()
            .build_rules
            .as_mut()
            .unwrap()
            .power_models
            .get_mut("growth")
            .unwrap();
        m.rank_base[0] = row["inputPower"].as_u64().unwrap() as u32;
        m.external_bps = 10000;
        m.quality_add_bps[1] = 0;
        m.level_add_bps[1] = 0;
        m.mastery_add_bps[1] = 0;
        let b = w.members[0].combat_build.as_mut().unwrap();
        b.power_points = row["powerPointsBps"].as_u64().unwrap() as u32 / 100;
        b.skill_power_points = (row["skillPowerBps"].as_u64().unwrap() as u32 - 10000) / 100;
        assert_eq!(
            w.configured_budget(0, Some(&c.skill), &c, 0),
            row["expected"].as_u64().unwrap() as u32
        );
    }
}

#[test]
fn character_build_zero_base_points_enter_with_one_derived_hp_and_spirit() {
    let (mut c, w) = setup();
    let seats: Vec<_> = w
        .members
        .iter()
        .map(|m| ParticipantSeat {
            weapon: None,
            basic_profile: None,
            combat_build: m.combat_build.clone(),
            skills: None,
            participant_id: m.reference.entity.id.clone(),
            authenticated_principal: m.facts.principal.clone(),
            connection: Connection {
                id: format!("c-{}", m.reference.entity.id),
                generation: 1,
            },
        })
        .collect();
    let mut facts: Vec<_> = w.members.iter().map(|m| m.facts.clone()).collect();
    facts[0].hp_points = 0;
    facts[0].spirit_points = 0;
    let created = World::new(&c, &seats, facts, 7);
    assert_eq!((created.members[0].hp, created.members[0].spirit), (1, 1));
    let p = Package::new();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/units-a.json"
    ))
    .unwrap();
    d["combat_rules"]["build_rules"] = rules();
    d["params"]["initial_player"]["hp_points"] = json!(0);
    p.write(&d);
    assert!(p.load().is_ok());
    c.combat_rules.as_mut().unwrap().build_rules = None;
    assert_eq!(c.derived_base(0, StatusAttribute::MaxHp), 0);
}
#[test]
fn character_build_load_bounds_apply_to_converted_resources_not_raw_point_count() {
    let (p, _cat, _c, request) = public_build_fixture();
    let mut d: Value = serde_json::from_str(crate::test_data::fixture!(
        "examples/combat-host/units-a.json"
    ))
    .unwrap();
    d["combat_rules"]["build_rules"] = rules();
    d["combat_rules"]["build_rules"]["conversion"]["hp_bps"] = json!(5000);
    d["combat_rules"]["skills"]["unit-strike"]
        .as_object_mut()
        .unwrap()
        .remove("base_power");
    p.write(&d);
    let cat = Arc::new(p.load().unwrap());
    let c = cat.default_config().as_ref().clone();
    let mut create = request;
    create.config_hash = c.hash();
    let mut runtime =
        crate::two_members::TwoMembers::create(c.clone(), cat, create.clone()).unwrap();
    let load = runtime
        .drain()
        .into_iter()
        .find_map(|o| {
            if let OutputBody::BusinessRequestV4 {
                participant_id,
                request,
                ..
            } = o.body
            {
                if participant_id == "P00" {
                    Some(request)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    for hp in [0, 1500000] {
        let mut fresh = crate::two_members::TwoMembers::create(
            c.clone(),
            Arc::new(p.load().unwrap()),
            create.clone(),
        )
        .unwrap();
        let mut assets = vec![
            AssetAmount {
                asset_id: c.item_id.clone(),
                quantity: 100,
            },
            AssetAmount {
                asset_id: c
                    .asset_contract
                    .as_ref()
                    .unwrap()
                    .entry_cost_asset_id
                    .clone(),
                quantity: 100,
            },
        ];
        assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        let outcome = ReceiptOutcome::AssetsLoaded {
            facts: AssetParticipantFacts {
                instance_count: None,
                warehouse_other_slots: None,
                location: None,
                principal: "principal-0".into(),
                revision: 0,
                hp_points: hp,
                spirit_points: 0,
                atk_points: 100,
                def_points: 0,
                spd_points: 8,
                assets,
            },
        };
        fresh
            .business(MemberBusinessReceipt {
                key: create.key.clone(),
                participant_id: "P00".into(),
                operation_id: load.operation_id.clone(),
                attempt_id: load.attempt_id,
                outcome,
            })
            .unwrap();
        assert!(fresh.drain().iter().any(|o|matches!(&o.body,OutputBody::BusinessRequestV4{request,..}if matches!(request.body,Business::CommitAssets{..}))));
    }
}
