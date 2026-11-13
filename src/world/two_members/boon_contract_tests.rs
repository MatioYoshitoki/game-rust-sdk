use super::*;

fn boon_input(v: &mut Value) {
    v["pve"]["combat"]["cast_chance"] = json!(0);
    v["pve"]["status_definitions"] = json!([
        {"id":"status-test","mechanism":"timed_attribute_modifier","attribute":"atk",
         "modifier_bps":1000,"duration_ms":300,"duplicate_policy":"refresh","source_scoped":true}
    ]);
    v["world_boons"] = json!({
        "capacity":3,"interaction_range":20,
        "definitions":{
            "edge":{"levels":[[
                {"attribute":"atk","modifier_bps":2000},{"attribute":"max_hp","modifier_bps":2000},
                {"attribute":"attack_speed","modifier_bps":2000}
            ],[
                {"attribute":"atk","modifier_bps":5000},{"attribute":"max_hp","modifier_bps":5000},
                {"attribute":"attack_speed","modifier_bps":5000}
            ]]},
            "stride":{"levels":[[{"attribute":"spd","modifier_bps":10000}]]},
            "ward":{"levels":[[{"attribute":"def","modifier_bps":5000}]]},
            "reserve":{"levels":[[{"attribute":"max_spirit","modifier_bps":5000}]]}
        },
        "opportunities":[
            {"id":"edge-source","position":[20,30],"scope":"per_member",
             "offer":{"kind":"grant","choices":["edge"]},"cost":{"item_id":"item","quantity":1}},
            {"id":"stride-source","position":[20,30],"scope":"per_member","offer":{"kind":"grant","choices":["stride"]}},
            {"id":"ward-source","position":[20,30],"scope":"shared","offer":{"kind":"grant","choices":["ward"]}},
            {"id":"upgrade-source","position":[20,30],"scope":"per_member","offer":{"kind":"upgrade"}},
            {"id":"reserve-source","position":[20,30],"scope":"per_member","offer":{"kind":"grant","choices":["reserve"]}}
        ]
    });
}

fn claim(
    w: &mut World,
    i: usize,
    (source, id, replace_slot): (&str, &str, Option<u8>),
    c: &Config,
    now: u64,
) -> Result<Events> {
    let expected_revision = w.boon_view(i, c).unwrap().revision;
    w.action(
        i,
        &Action::ClaimBoon {
            opportunity_id: source.into(),
            boon_id: id.into(),
            expected_revision,
            expected_inventory_revision: w.members[i].revision,
            replace_slot,
        },
        "claim",
        c,
        now,
    )
}

fn shared_checkpoint_input(v: &mut Value) {
    boon_input(v);
    v["pve"]["combat"]["aggro_range"] = json!(2);
    v["members"]["capacity"] = json!(3);
    v["members"]["spawns"] = json!([[20, 20], [20, 40], [40, 30]]);
    v["members"]["checkpoint_schedules"] = json!([{
        "id":"route","first_delay_ms":30,"interval_ms":180,"window_ms":150,
        "radius":6,"required":1,"centers":[[20,20]]
    }]);
    let source = &mut v["world_boons"]["opportunities"][0];
    source["scope"] = json!("shared");
    source["shared_uses"] = json!(2);
    source["requires_checkpoint"] = json!("route");
    source["interaction_ms"] = json!(60);
}

#[test]
fn shared_boon_reservations_commit_two_shares_with_private_checkpoint_progress() {
    let c = catalog(input(), shared_checkpoint_input)
        .unwrap()
        .default_config();
    let mut w = world_with_seats(&c, &["a", "b", "c"]);
    let take = |w: &mut World, i, now| claim(w, i, ("edge-source", "edge", None), &c, now);
    let source = |w: &World, i| {
        w.boon_view(i, &c)
            .unwrap()
            .opportunities
            .into_iter()
            .find(|source| source.id == "edge-source")
            .unwrap()
    };
    assert_eq!(take(&mut w, 0, 0).unwrap_err(), Error::InvalidState);
    w.tick(&c, 30);
    assert!(source(&w, 0).available);
    assert!(!source(&w, 1).available);
    assert_eq!(take(&mut w, 1, 30).unwrap_err(), Error::InvalidState);
    assert_eq!(take(&mut w, 0, 30).unwrap_err(), Error::NoResource);
    assert_eq!(source(&w, 0).shared_remaining, Some(2));
    w.members[1].position = [24., 20.];
    w.members[2].position = [20., 24.];
    w.tick(&c, 60);
    for i in 0..3 {
        w.members[i].credit_stack("item", 1, false, &c).unwrap();
    }

    take(&mut w, 1, 60).unwrap();
    assert_eq!(source(&w, 0).shared_remaining, Some(2));
    assert_eq!(
        w.boon_view(1, &c)
            .unwrap()
            .pending_claim
            .unwrap()
            .completes_at_ms,
        120
    );
    assert!(w.boon_view(2, &c).unwrap().pending_claim.is_none());
    assert_eq!(take(&mut w, 2, 60).unwrap_err(), Error::Conflict);
    let canceled = w
        .action(1, &Action::CancelAction {}, "cancel", &c, 60)
        .unwrap();
    assert!(canceled.iter().any(|(i, e)| *i == Some(1)
        && matches!(
            e,
            EventV4::Private {
                event: Event::BoonClaimInterrupted { .. }
            }
        )));
    take(&mut w, 2, 60).unwrap();
    let mut events = vec![];
    w.hurt_member_effect(2, w.npcs[0].entity.clone(), (5, true), &c, 90, &mut events);
    assert!(w.boon_view(2, &c).unwrap().pending_claim.is_none());
    assert_eq!(source(&w, 1).shared_remaining, Some(2));

    take(&mut w, 1, 90).unwrap();
    w.members[1].position = [90., 90.];
    w.tick(&c, 120);
    assert!(w.boon_view(1, &c).unwrap().pending_claim.is_none());
    w.members[1].position = [24., 20.];
    take(&mut w, 0, 120).unwrap();
    w.protect(0, 150, &mut vec![]);
    assert_eq!(source(&w, 1).shared_remaining, Some(2));
    assert!(source(&w, 1).available);
    w.action(0, &Action::Foreground {}, "fg", &c, 150).unwrap();
    w.action(
        0,
        &Action::CombatReady {
            publication_id: "full".into(),
        },
        "ready",
        &c,
        150,
    )
    .unwrap();
    take(&mut w, 0, 150).unwrap();
    w.tick(&c, 180);
    assert!(w.boon_view(0, &c).unwrap().slots.is_empty());
    assert_eq!(w.members[0].inventory["item"], 2);
    w.tick(&c, 210);
    assert_eq!(w.boon_level(0, "edge"), Some(1));
    assert_eq!(w.members[0].inventory["item"], 1);
    assert_eq!(source(&w, 1).shared_remaining, Some(1));
    assert_eq!(take(&mut w, 0, 210).unwrap_err(), Error::Conflict);

    take(&mut w, 2, 210).unwrap();
    w.members[2].credit_stack("item", 1, false, &c).unwrap();
    w.members[2].revision += 1;
    let invalid = w.tick(&c, 240);
    assert!(invalid.iter().any(|(i, e)| *i == Some(2)
        && matches!(
            e,
            EventV4::Private {
                event: Event::BoonClaimInterrupted {
                    reason: Error::RevisionConflict,
                    ..
                }
            }
        )));
    assert_eq!(source(&w, 1).shared_remaining, Some(1));
    take(&mut w, 2, 240).unwrap();
    w.tick(&c, 300);
    assert_eq!(w.boon_level(2, "edge"), Some(1));
    assert_eq!(w.members[2].inventory["item"], 2);
    assert_eq!(w.members[1].inventory["item"], 2);
    assert_eq!(source(&w, 1).shared_remaining, Some(0));
    assert_eq!(take(&mut w, 1, 300).unwrap_err(), Error::Conflict);
}

#[test]
fn shared_boon_rejects_invalid_quota_timer_and_checkpoint_bindings() {
    for (key, bad) in [
        ("shared_uses", json!(0)),
        ("shared_uses", json!(4)),
        ("scope", json!("per_member")),
        ("requires_checkpoint", json!("missing")),
        ("interaction_ms", json!(29)),
    ] {
        let config = catalog(input(), |v| {
            shared_checkpoint_input(v);
            v["world_boons"]["opportunities"][0][key] = bad;
        });
        assert!(
            config
                .err()
                .unwrap()
                .to_string()
                .contains("CONFIG_WORLD_BOONS_OPPORTUNITY")
        );
    }
}

#[test]
fn boon_healing_penalties_use_the_stronger_lane_and_keep_positive_recovery() {
    let c = catalog(input(), |v| {
        boon_input(v);
        v["world_boons"]["opportunities"][2]["scope"] = json!("per_member");
        let statuses = v["pve"]["status_definitions"].as_array_mut().unwrap();
        for (id,bps,duration) in [("weak",-3000,600),("strong",-6000,90),("ordinary",-4000,210),("bonus",1000,600)] {
            statuses.push(json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"healing_received",
                "modifier_bps":bps,"duration_ms":duration,"duplicate_policy":"refresh","source_scoped":true}));
        }
    }).unwrap().default_config();
    let mut w = world(&c);
    w.members[0].credit_stack("item", 1, false, &c).unwrap();
    claim(&mut w, 0, ("edge-source", "edge", None), &c, 0).unwrap();
    claim(&mut w, 1, ("ward-source", "ward", None), &c, 0).unwrap();
    let target = w.owner_entity(Owner::Member(0));
    for (owner, boon, id) in [
        (Owner::Member(0), "edge", "weak"),
        (Owner::Member(1), "ward", "strong"),
    ] {
        w.apply_boon_status(owner, target.clone(), (boon, id.into()), &c, 0, &mut vec![]);
    }
    for (owner, id) in [(Owner::Npc(0), "ordinary"), (Owner::Member(0), "bonus")] {
        w.apply_owned_statuses(
            owner,
            (target.clone(), 100),
            &[id.into()],
            &c,
            0,
            &mut vec![],
        );
    }
    let heal = |w: &mut World, now, bps, restored| {
        w.members[0].hp = 10;
        let mut events = vec![];
        w.resolve_member_heal((0, 0), (40, HealKind::SelfSkill), &c, now, &mut events);
        assert_eq!(w.members[0].hp, 10 + restored);
        assert!(events.iter().any(|(_, e)| matches!(e,EventV4::Private {
            event:Event::HealResolved { multiplier_bps,hp_restored,.. }
        } if *multiplier_bps == bps && *hp_restored == restored)));
    };
    heal(&mut w, 30, 5000, 20);
    heal(&mut w, 120, 7000, 28);
    heal(&mut w, 240, 8000, 32);
    claim(&mut w, 0, ("stride-source", "stride", None), &c, 240).unwrap();
    claim(&mut w, 0, ("reserve-source", "reserve", None), &c, 240).unwrap();
    claim(&mut w, 0, ("ward-source", "ward", Some(1)), &c, 240).unwrap();
    heal(&mut w, 240, 11000, 44);
}

#[test]
fn boon_cost_slots_upgrade_and_replacement_use_native_effective_stats() {
    let c = catalog(input(), boon_input).unwrap().default_config();
    let mut w = world(&c);
    let a = &mut w.members[0];
    a.hp = 80;
    a.spirit = 90;
    a.attack_clock = Some(attack_speed::AttackCooldown::new(0, 300));
    a.basic_ready = 300;
    assert_eq!(
        claim(&mut w, 0, ("edge-source", "edge", None), &c, 30).unwrap_err(),
        Error::NoResource
    );
    assert_eq!(w.boon_view(0, &c).unwrap().revision, 1);
    assert_eq!(w.members[0].inventory["item"], 1);
    w.members[0].credit_stack("item", 1, false, &c).unwrap();
    let events = claim(&mut w, 0, ("edge-source", "edge", None), &c, 30).unwrap();
    assert!(matches!(
        &events[..],
        [(
            Some(0),
            EventV4::Private {
                event: Event::BoonClaimed { revision: 2, .. }
            }
        )]
    ));
    let own = w.snapshot(0, &c, Phase::Active, 30).own;
    assert_eq!(
        (own.effective_atk, own.hp, own.max_hp, own.basic_ready_at_ms),
        (12, 80, 120, 255)
    );
    assert_eq!(w.members[0].inventory["item"], 1);
    assert!(w.members[0].looted.is_empty());
    claim(&mut w, 0, ("stride-source", "stride", None), &c, 30).unwrap();
    claim(&mut w, 0, ("ward-source", "ward", None), &c, 30).unwrap();
    assert_eq!(
        claim(&mut w, 1, ("ward-source", "ward", None), &c, 30).unwrap_err(),
        Error::Conflict
    );
    assert_eq!(w.effective_spd(Owner::Member(0), &c, 30), 2);
    assert_eq!(w.effective_spd(Owner::Member(1), &c, 30), 1);
    assert_eq!(
        claim(&mut w, 0, ("reserve-source", "reserve", None), &c, 30).unwrap_err(),
        Error::Capacity
    );
    assert_eq!(w.boon_view(0, &c).unwrap().revision, 4);

    claim(&mut w, 0, ("upgrade-source", "edge", None), &c, 60).unwrap();
    let own = w.snapshot(0, &c, Phase::Active, 60).own;
    assert_eq!(
        (own.effective_atk, own.hp, own.max_hp, own.basic_ready_at_ms),
        (15, 80, 150, 216)
    );
    let boons = own.boons.unwrap();
    assert_eq!(boons.slots[0].level, 2);
    w.protect(0, 60, &mut vec![]);
    assert_eq!(w.boon_view(0, &c).unwrap(), boons);
    w.action(0, &Action::Foreground {}, "fg", &c, 60).unwrap();
    w.action(
        0,
        &Action::CombatReady {
            publication_id: "full".into(),
        },
        "ready",
        &c,
        60,
    )
    .unwrap();
    assert_eq!(
        claim(&mut w, 0, ("upgrade-source", "edge", None), &c, 60).unwrap_err(),
        Error::Conflict
    );
    claim(&mut w, 0, ("reserve-source", "reserve", Some(1)), &c, 60).unwrap();
    let own = w.snapshot(0, &c, Phase::Active, 60).own;
    assert_eq!(
        (
            own.effective_atk,
            own.max_hp,
            own.spirit,
            own.max_spirit,
            own.basic_ready_at_ms
        ),
        (10, 100, 90, 150, 294)
    );
    assert_eq!(own.boons.unwrap().slots[0].boon_id, "reserve");
    w.terminal_intent(0, Outcome::Extraction, &mut vec![]);
    let terminal = w.snapshot(0, &c, Phase::Active, 60).own;
    assert!(terminal.boons.unwrap().slots.is_empty());
    assert_eq!(terminal.max_spirit, 100);
    assert!(matches!(
        w.settlement(0, &c),
        Business::CommitSettlement {
            looted: 0,
            kept: 1,
            ..
        }
    ));
}

#[test]
fn boon_access_respects_visibility_guardians_and_automation() {
    let c = catalog(input(), |v| {
        boon_input(v);
        v["world_boons"]["opportunities"] = json!([
            {"id":"private","position":[20,30],"scope":"shared","human_only":true,"guardians":["npc-1"],
             "offer":{"kind":"grant","choices":["edge"]}},
            {"id":"public","position":[20,40],"scope":"per_member","offer":{"kind":"grant","choices":["stride"]}},
            {"id":"screened","position":[32,20],"scope":"shared","offer":{"kind":"grant","choices":["ward"]}},
            {"id":"far","position":[90,90],"scope":"shared","offer":{"kind":"grant","choices":["ward"]}}
        ]);
        v["pve"]["world_geometry"] = json!({
            "boundary":[[0,0],[128,0],[128,128],[0,128]],
            "blockers":[{"id":"screen","vertices":[[24,16],[26,16],[26,24],[24,24]],
                "blocks_movement":false,"blocks_projectiles":false,"blocks_vision":true}]
        });
        v["participant_ai"] = json!({"b":{
            "decision_interval_ms":30,"loot_radius":25,"threat_radius":0,"withdraw_hp_bps":0,
            "extract_after_looted":0,"extract_before_timeout_ms":0
        }});
    }).unwrap().default_config();
    let mut w = world(&c);
    let a = w.boon_view(0, &c).unwrap();
    assert!(
        a.opportunities
            .iter()
            .any(|row| row.id == "private" && !row.available)
    );
    assert!(a.opportunities.iter().all(|row| row.id != "far"));
    assert!(a.opportunities.iter().all(|row| row.id != "screened"));
    assert_eq!(
        claim(&mut w, 0, ("screened", "ward", None), &c, 0).unwrap_err(),
        Error::InvalidTarget
    );
    assert!(
        w.boon_view(1, &c)
            .unwrap()
            .opportunities
            .iter()
            .all(|row| row.id != "private")
    );
    assert_eq!(
        claim(&mut w, 0, ("private", "edge", None), &c, 0).unwrap_err(),
        Error::GuardianAlive
    );
    assert_eq!(
        claim(&mut w, 1, ("private", "edge", None), &c, 0).unwrap_err(),
        Error::Unauthorized
    );
    assert_eq!(
        claim(&mut w, 0, ("far", "ward", None), &c, 0).unwrap_err(),
        Error::OutOfRange
    );
    let events = w.tick(&c, 30);
    assert!(events.iter().any(|(i, event)| *i == Some(1)
        && matches!(event,EventV4::Private {
        event:Event::BoonClaimed {opportunity_id,..}
    } if opportunity_id=="public")));
    w.npcs[0].hp = 0;
    claim(&mut w, 0, ("private", "edge", None), &c, 30).unwrap();
}

#[test]
fn boon_catalog_rejects_downgrades_invalid_choices_and_out_of_bounds_slots() {
    for (case, error) in [
        (0, "CONFIG_WORLD_BOONS_DEFINITION"),
        (1, "CONFIG_WORLD_BOONS_OPPORTUNITY"),
        (2, "CONFIG_WORLD_BOONS_BOUNDS"),
        (3, "CONFIG_WORLD_BOONS_DEFINITION"),
        (4, "CONFIG_WORLD_BOONS_DEFINITION"),
        (5, "CONFIG_WORLD_BOONS_DEFINITION"),
        (6, "CONFIG_BOON_SHIELD_OUTPUT_BUDGET"),
    ] {
        let result = catalog(input(), |v| {
            boon_input(v);
            match case {
                0 => {
                    v["world_boons"]["definitions"]["edge"]["levels"][1][0]["modifier_bps"] =
                        json!(1000)
                }
                1 => v["world_boons"]["opportunities"][0]["offer"]["choices"] = json!(["unknown"]),
                2 => v["world_boons"]["capacity"] = json!(4),
                3 => v["world_boons"]["definitions"]["edge"]["spirit_cost_bps"] = json!([7000]),
                4 => {
                    v["world_boons"]["definitions"]["edge"]["spirit_cost_bps"] = json!([5000, 7000])
                }
                5 => {
                    v["world_boons"]["definitions"]["edge"]["shield"] =
                        json!({"capacity_bps":[4000,6000],"rebuild_ms":29})
                }
                _ => {
                    v["output_capacity"] = json!(128);
                    v["world_boons"]["definitions"]["edge"]["shield"] =
                        json!({"capacity_bps":[4000,6000],"rebuild_ms":30});
                }
            }
        });
        assert!(result.err().unwrap().to_string().contains(error));
    }
}

fn conditional_boon_input(v: &mut Value) {
    boon_input(v);
    v["world_boons"]["definitions"]["edge"]["levels"] = json!([[], []]);
    for (id, bps) in [("mark-low", -3000), ("mark-high", -6000)] {
        v["pve"]["status_definitions"].as_array_mut().unwrap().push(
            json!({"id":id,"mechanism":"timed_attribute_modifier",
            "attribute":"healing_received","modifier_bps":bps,"duration_ms":600,
            "duplicate_policy":"refresh","source_scoped":true}),
        );
        v["combat_rules"]["status_policies"][id] =
            json!({"disposition":"harmful","dispellable":true,"transferable":false,"blocks":[]});
    }
    v["pve"]["monster_positions"] = json!([[52, 20], [52, 40]]);
    v["combat_rules"]["triggers"] = json!([{
        "id":"mark-contact","profiles":["test"],"when":"direct_hit","include_periodic":false,
        "counting":"action","every":1,"cooldown_ms":120,"priority":0,
        "effect":{"kind":"apply_other_status","status_id":"mark-low"},
        "requires_boon":{"id":"edge","npc_only":true,
            "higher_level_effects":[{"kind":"apply_other_status","status_id":"mark-high"}]}
    }]);
}

#[test]
fn boon_trigger_uses_current_level_without_refreshing_action_budget_or_cooldown() {
    let c = catalog(input(), conditional_boon_input)
        .unwrap()
        .default_config();
    let mut w = world(&c);
    let contact = |w: &mut World, (owner, target), id, at| {
        let source = w.owner_entity(owner);
        let rule = crate::config::AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            ..Default::default()
        };
        let mut events = vec![];
        assert_eq!(
            w.direct_contact_damage(
                owner,
                target,
                combat::DirectContact {
                    origin: Some(crate::origin::ActionOrigin::Basic),
                    action_id: Some(id),
                    rule: Some(&rule),
                    ignore_armor: true,
                    unavoidable: true,
                    dead_source: false,
                    source: &source,
                    raw: 0,
                    metal: false,
                    hit_statuses: &[],
                },
                &c,
                at,
                &mut events,
            ),
            Some(0)
        );
        events
            .iter()
            .filter_map(|(i, e)| match e {
                EventV4::Private {
                    event: Event::CombatTriggerFired { trigger_id, .. },
                } if trigger_id == "mark-contact" => *i,
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let a = Owner::Member(0);
    let b = Owner::Member(1);
    let n = Owner::Npc(0);
    let other = Owner::Npc(1);
    assert!(contact(&mut w, (a, n), "before", 0).is_empty());
    assert!(contact(&mut w, (n, a), "npc", 0).is_empty());
    w.members[0].credit_stack("item", 1, false, &c).unwrap();
    claim(&mut w, 0, ("edge-source", "edge", None), &c, 30).unwrap();
    assert_eq!(contact(&mut w, (a, n), "multi", 30), [0]);
    assert!(contact(&mut w, (a, other), "multi", 30).is_empty());
    assert!(contact(&mut w, (b, n), "seat-b", 30).is_empty());
    assert_eq!(w.statuses.entries()[0].definition_id, "mark-low");
    assert_eq!(
        w.statuses.entries()[0].source,
        status_entity(&w.owner_entity(a))
    );

    claim(&mut w, 0, ("upgrade-source", "edge", None), &c, 60).unwrap();
    w.protect(0, 60, &mut vec![]);
    w.action(0, &Action::Foreground {}, "fg", &c, 60).unwrap();
    w.action(
        0,
        &Action::CombatReady {
            publication_id: "full".into(),
        },
        "ready",
        &c,
        60,
    )
    .unwrap();
    assert!(contact(&mut w, (a, other), "cooldown", 90).is_empty());
    assert!(contact(&mut w, (a, other), "multi", 150).is_empty());
    w.triggers.executing = true;
    assert!(contact(&mut w, (a, other), "derived", 150).is_empty());
    w.triggers.executing = false;
    assert_eq!(contact(&mut w, (a, other), "ready-again", 150), [0]);
    assert_eq!(w.statuses.entries()[1].definition_id, "mark-high");
    assert_eq!(
        w.statuses.healing_received_bps(
            &status_entity(&w.owner_entity(other)),
            150,
            c.pve.as_ref().unwrap().status_rules.as_ref()
        ),
        4000
    );

    let target = w.owner_entity(n);
    let low = w.statuses.entries()[0].instance_id.clone();
    for source in [a, b] {
        w.apply_owned_statuses(
            source,
            (target.clone(), 100),
            &["mark-low".into()],
            &c,
            180,
            &mut vec![],
        );
    }
    let ordinary: Vec<_> = w.statuses.entries()[2..]
        .iter()
        .map(|s| s.instance_id.clone())
        .collect();
    assert_eq!(ordinary.len(), 2);
    claim(&mut w, 0, ("stride-source", "stride", None), &c, 180).unwrap();
    claim(&mut w, 0, ("ward-source", "ward", None), &c, 180).unwrap();
    claim(&mut w, 0, ("reserve-source", "reserve", Some(1)), &c, 180).unwrap();
    assert!(!w.statuses.entries().iter().any(|s| s.instance_id == low));
    assert_eq!(
        w.statuses
            .entries()
            .iter()
            .map(|s| &s.instance_id)
            .collect::<Vec<_>>(),
        ordinary.iter().collect::<Vec<_>>()
    );
    assert!(contact(&mut w, (a, other), "after-replace", 300).is_empty());
    w.terminal_intent(0, Outcome::Extraction, &mut vec![]);
    assert_eq!(w.statuses.entries().len(), 1);
    assert_eq!(w.statuses.entries()[0].instance_id, ordinary[1]);
}

#[test]
fn boon_spirit_price_controls_admission_reservation_commit_and_ai() {
    let c = catalog(input(), |v| {
        boon_input(v);
        v["world_boons"]["definitions"]["edge"]["spirit_cost_bps"] = json!([6750, 5000]);
        v["skill"]["cost"] = json!(19);
        v["skill"]["resource_commit"] = json!({"reserve_at":"accept","consume_at":"commit",
            "cooldown_starts_at":"commit","refund_before_commit":{"kind":"full"}});
        v["participant_ai"]["b"] = json!({"decision_interval_ms":30,"loot_radius":0,"threat_radius":60,
            "combat_skill_id":"skill","withdraw_hp_bps":0,"extract_after_looted":0,"extract_before_timeout_ms":0});
    }).unwrap().default_config();
    let mut w = world(&c);
    let enemy = w.npcs[0].entity.clone();
    let cast = Action::CastUnit {
        skill_id: "skill".into(),
        target: UnitTarget {
            id: enemy.id,
            generation: enemy.generation,
        },
    };
    w.members[0].spirit = 12;
    w.members[1].spirit = 12;
    assert_eq!(
        w.action(0, &cast, "before", &c, 0).unwrap_err(),
        Error::NoResource
    );
    for i in [0, 1] {
        w.members[i].credit_stack("item", 1, false, &c).unwrap();
        claim(&mut w, i, ("edge-source", "edge", None), &c, 0).unwrap();
    }
    assert_eq!(
        (
            w.members[0].effective_spirit_cost(0),
            w.members[0].effective_spirit_cost(1)
        ),
        (0, 1)
    );
    w.action(0, &cast, "cancel", &c, 0).unwrap();
    assert_eq!(
        w.members[0]
            .pending
            .as_ref()
            .unwrap()
            .reservation
            .unwrap()
            .spirit,
        12
    );
    w.action(0, &Action::CancelAction {}, "cancel", &c, 0)
        .unwrap();
    assert_eq!(w.members[0].spirit, 12);
    claim(&mut w, 0, ("upgrade-source", "edge", None), &c, 0).unwrap();
    w.action(0, &cast, "commit", &c, 0).unwrap();
    let snapshot = w.snapshot(0, &c, Phase::Active, 0);
    assert_eq!(
        snapshot
            .own
            .pending_action
            .unwrap()
            .resource_reservation
            .unwrap()
            .spirit,
        9
    );
    let mut events = w.tick(&c, 30);
    events.extend(w.tick(&c, 60));
    assert_eq!((w.members[0].spirit, w.members[1].spirit), (3, 0));
    for (i, cost) in [(0, 9), (1, 12)] {
        assert!(events.iter().any(|(recipient, e)| *recipient == Some(i) && matches!(e,
            EventV4::Private { event: Event::ActionCommitted { spirit_cost, .. } } if *spirit_cost == cost)));
    }
}

#[test]
fn boon_shield_filters_sources_and_rebuilds_once_with_the_current_level() {
    let c = catalog(input(), |v| {
        boon_input(v);
        v["pve"]["combat"]["aggro_range"] = json!(2);
        v["pve"]["status_rules"]["shield_cap_bps"] = json!(3000);
        v["pve"]["status_definitions"].as_array_mut().unwrap().push(json!({"id":"ordinary-shield",
            "mechanism":"absorption_shield","shield_bps":4000,"duration_ms":600,"duplicate_policy":"refresh","source_scoped":true}));
        v["world_boons"]["definitions"]["edge"] = json!({"levels":[[],[]],"shield":{"capacity_bps":[4000,6000],"rebuild_ms":90}});
        v["world_boons"]["definitions"]["ward"] = json!({"levels":[[]],"shield":{"capacity_bps":[5000],"rebuild_ms":90}});
    }).unwrap().default_config();
    let mut w = world(&c);
    let player = w.members[0].reference.entity.clone();
    let npc = w.npcs[0].entity.clone();
    let shield_total = |w: &World, now| {
        w.snapshot(0, &c, Phase::Active, now)
            .own
            .shield_total
            .unwrap()
    };
    let standard = |w: &mut World, now| {
        w.apply_owned_statuses(
            Owner::Member(0),
            (player.clone(), 100),
            &["ordinary-shield".into()],
            &c,
            now,
            &mut vec![],
        )
    };
    let hit = |w: &mut World, amount, now| {
        w.resolve_damage(
            combat::DamageIntent {
                source: npc.clone(),
                target: player.clone(),
                raw: amount,
                direct: true,
                ignore_armor: true,
            },
            &c,
            now,
            &mut vec![],
        )
    };
    standard(&mut w, 0);
    assert_eq!(shield_total(&w, 0), 30);
    w.members[0].credit_stack("item", 1, false, &c).unwrap();
    claim(&mut w, 0, ("edge-source", "edge", None), &c, 0).unwrap();
    assert_eq!(shield_total(&w, 0), 40);
    assert_eq!(
        claim(&mut w, 0, ("ward-source", "ward", None), &c, 0).unwrap_err(),
        Error::Capacity
    );
    assert_eq!(
        w.absorb_damage(&player, ("other-source", false), 35, 0, &mut vec![]),
        (5, 30)
    );
    assert_eq!(shield_total(&w, 0), 10);
    standard(&mut w, 0);
    assert_eq!(shield_total(&w, 0), 40);
    assert_eq!(hit(&mut w, 35, 30).absorbed, 35);
    claim(&mut w, 0, ("upgrade-source", "edge", None), &c, 30).unwrap();
    assert_eq!(shield_total(&w, 30), 5);
    let result = hit(&mut w, 7, 60);
    assert_eq!((result.absorbed, result.hp_damage), (5, 2));
    assert_eq!(shield_total(&w, 60), 0);
    w.protect(0, 60, &mut vec![]);
    w.tick(&c, 150);
    assert_eq!(shield_total(&w, 150), 0);
    w.action(0, &Action::Foreground {}, "fg", &c, 180).unwrap();
    w.action(
        0,
        &Action::CombatReady {
            publication_id: "full".into(),
        },
        "ready",
        &c,
        180,
    )
    .unwrap();
    w.tick(&c, 180);
    assert_eq!(shield_total(&w, 180), 60);
    assert_eq!(hit(&mut w, 20, 210).absorbed, 20);
    standard(&mut w, 210);
    assert_eq!(shield_total(&w, 210), 60);
    claim(&mut w, 0, ("stride-source", "stride", None), &c, 210).unwrap();
    claim(&mut w, 0, ("reserve-source", "reserve", None), &c, 210).unwrap();
    claim(&mut w, 0, ("ward-source", "ward", Some(1)), &c, 210).unwrap();
    assert_eq!(shield_total(&w, 210), 50);
    w.terminal_intent(0, Outcome::Extraction, &mut vec![]);
    assert_eq!(shield_total(&w, 210), 0);
}

#[test]
fn boon_trigger_validation_checks_higher_levels_and_scope() {
    for (field, value, error) in [
        (
            "/requires_boon/id",
            json!("missing"),
            "CONFIG_BOON_TRIGGER_BINDING",
        ),
        (
            "/counting",
            json!("action_total"),
            "CONFIG_BOON_TRIGGER_BINDING",
        ),
        (
            "/requires_boon/higher_level_effects/0/status_id",
            json!("missing"),
            "CONFIG_TRIGGER_TARGET_STATUS",
        ),
        (
            "/requires_boon/higher_level_effects",
            json!([{"kind":"restore_spirit","amount":1}]),
            "CONFIG_BOON_TRIGGER_BINDING",
        ),
    ] {
        let result = catalog(input(), |v| {
            conditional_boon_input(v);
            *v["combat_rules"]["triggers"][0].pointer_mut(field).unwrap() = value;
        });
        assert!(result.err().unwrap().to_string().contains(error));
    }
}

fn summon_boon_input(v: &mut Value) {
    summon_input(v, 180);
    v["pve"]["status_rules"] = input()["pve"]["status_rules"].clone();
    boon_input(v);
    v["pve"]["stats"]["hp"] = json!(1000);
    v["pve"]["combat"]["aggro_range"] = json!(2);
    let summons = &mut v["combat_rules"]["member_summons"];
    summons["max_active_per_member"] = json!(2);
    let binding = &mut summons["skills"]["summon-skill"];
    binding["count"] = json!(2);
    binding["owner_while_alive"] = json!({"damage_bonus_bps":5000,"damage_reduction_bps":2500});
}

#[test]
fn summon_lifetime_bonus_composes_with_boons_and_ends_with_last_owned_summon() {
    let c = catalog(input(), summon_boon_input)
        .unwrap()
        .default_config();
    let mut w = world(&c);
    let a = Owner::Member(0);
    let b = Owner::Member(1);
    let n = Owner::Npc(0);
    let hit = |w: &mut World, (source, target), raw, at| {
        w.resolve_damage(
            combat::DamageIntent {
                source: w.owner_entity(source),
                target: w.owner_entity(target),
                raw,
                direct: true,
                ignore_armor: true,
            },
            &c,
            at,
            &mut vec![],
        )
        .hp_damage
    };
    w.members[0].credit_stack("item", 1, false, &c).unwrap();
    claim(&mut w, 0, ("edge-source", "edge", None), &c, 0).unwrap();
    let budget = w.configured_budget(0, None, &c, 0);
    assert_eq!(budget, 12);
    assert_eq!(hit(&mut w, (a, n), budget, 0), 12);
    cast_summon(&mut w, &c, 0, 0, "cast-a");
    w.tick(&c, 30);
    assert_eq!(w.summons.len(), 2);
    assert_eq!(hit(&mut w, (a, n), budget, 30), 18);
    assert_eq!(hit(&mut w, (b, n), 10, 30), 10);
    assert_eq!(hit(&mut w, (n, a), 20, 30), 15);
    assert_eq!(hit(&mut w, (n, b), 20, 30), 20);
    assert_eq!(hit(&mut w, (Owner::Summon(0), n), 7, 30), 7);
    assert_eq!(hit(&mut w, (n, Owner::Summon(0)), 100, 60), 30);
    assert_eq!(hit(&mut w, (a, n), budget, 60), 18);
    w.protect(0, 60, &mut vec![]);
    w.action(0, &Action::Foreground {}, "fg", &c, 60).unwrap();
    w.action(
        0,
        &Action::CombatReady {
            publication_id: "full".into(),
        },
        "ready",
        &c,
        60,
    )
    .unwrap();
    assert_eq!(hit(&mut w, (a, n), budget, 60), 18);

    cast_summon(&mut w, &c, 1, 90, "cast-b");
    w.tick(&c, 120);
    assert_eq!(hit(&mut w, (b, n), 10, 120), 15);
    assert_eq!(hit(&mut w, (a, n), budget, 209), 18);
    assert_eq!(hit(&mut w, (a, n), budget, 210), 12);
    assert_eq!(hit(&mut w, (n, a), 20, 210), 20);
    assert_eq!(hit(&mut w, (b, n), 10, 210), 15);
    w.terminal_intent(1, Outcome::Extraction, &mut vec![]);
    assert_eq!(w.summon_owner_bonus(b, 210).damage_bonus_bps, 0);
    assert_eq!(w.boon_level(0, "edge"), Some(1));
}

#[test]
fn summon_owner_bonus_rejects_zero_or_out_of_range_values() {
    for (damage, reduction) in [(0, 0), (10001, 0), (0, 10001)] {
        let result = catalog(input(), |v| {
            summon_boon_input(v);
            v["combat_rules"]["member_summons"]["skills"]["summon-skill"]["owner_while_alive"] =
                json!({"damage_bonus_bps":damage,"damage_reduction_bps":reduction});
        });
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("CONFIG_MEMBER_SUMMON_EFFECT")
        );
    }
    let result = catalog(input(), |v| {
        summon_input(v, 180);
        v["combat_rules"]["member_summons"]["skills"]["summon-skill"]["owner_while_alive"] =
            json!({"damage_bonus_bps":0,"damage_reduction_bps":2500});
    });
    assert!(
        result
            .err()
            .unwrap()
            .to_string()
            .contains("CONFIG_MEMBER_SUMMON_EFFECT")
    );
}
