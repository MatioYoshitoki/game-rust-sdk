use super::*;
use crate::protocol::{WindowBasis, WindowMaximumSampling};

fn setup_lifecycle(hot: bool, basis: &str, sampling: &str, scope: &str) -> (Config, World) {
    let (mut c, mut w) = setup();
    c.combat_rules.as_mut().unwrap().triggers.clear();
    let mut d = json!({"id":"cargo","mechanism":"timed_attribute_modifier","attribute":"def","modifier_bps":1,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true});
    if hot {
        d = json!({"id":"cargo","mechanism":"periodic_heal","heal_bps":1000,"tick_interval_ms":300,"duration_ms":1200,"duplicate_policy":"refresh","source_scoped":true});
    }
    c.pve_mut()
        .status_definitions
        .push(serde_json::from_value(d).unwrap());
    operation_policy(&mut c, "cargo", "beneficial", true);
    let r = &mut c.combat_rules.as_mut().unwrap().status_windows;
    r.insert("cargo".into(),serde_json::from_value(json!({"basis":basis,"include_periodic":false,"record_cap":if basis=="none"{0}else{10000},"refresh":"preserve","maximum_sampling":sampling,"on_expired":true,"on_consumed":true,"hp":{"fixed":0,"recorded_bps":if basis=="none"{0}else{10000},"max_resource_bps":if basis=="none"{1000}else{0},"cap_max_resource_bps":10000},"spirit":{"fixed":0,"recorded_bps":0,"max_resource_bps":1000,"cap_max_resource_bps":10000}})).unwrap());
    let mut cleanse = json!({"on_expired":{"disposition":"harmful","max_count":16,"scope":scope}});
    if hot {
        cleanse["on_tick"] = json!({"disposition":"harmful","max_count":1,"scope":scope});
    }
    c.combat_rules
        .as_mut()
        .unwrap()
        .status_cleanses
        .insert("cargo".into(), serde_json::from_value(cleanse).unwrap());
    for id in ["earlier", "between", "later"] {
        c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":id,"mechanism":"timed_attribute_modifier","attribute":"healing_received","modifier_bps":-1000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
        operation_policy(&mut c, id, "harmful", false);
    }
    w.members[0].facts.hp_points = 1000;
    w.members[0].hp = 1000;
    w.members[0].facts.spirit_points = 300;
    w.members[0].spirit = 300;
    w.members[1].facts.hp_points = 1800;
    w.members[1].hp = 1800;
    w.members[1].facts.spirit_points = 500;
    w.members[1].spirit = 500;
    w.npcs[0].hp = 10000;
    (c, w)
}
fn apply(w: &mut World, c: &Config, source: Owner, target: Owner, id: &str, now: u64) {
    let target = w.owner_entity(target);
    bind(w, c, source, target, id, now);
}
fn cargo(w: &World) -> StatusView {
    w.statuses
        .entries()
        .iter()
        .find(|s| s.definition_id == "cargo")
        .unwrap()
        .clone()
}
fn transfer(w: &mut World, c: &Config, from: Owner, to: Owner, now: u64) -> Events {
    let effect = serde_json::from_value(
        json!({"kind":"transfer_statuses","from":"other","disposition":"beneficial","max_count":2}),
    )
    .unwrap();
    let mut e = vec![];
    w.execute_status_operation((to, from), &effect, c, now, &mut e);
    e
}
fn expire(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.prepare_periodic(now, &mut e);
    w.finish_status_cleanses(c, now, &mut e);
    w.finish_window_records(c, now, &mut e);
    e
}
fn tick(w: &mut World, c: &Config, now: u64) -> Events {
    let mut e = vec![];
    w.advance_periodic(c, now, &mut e);
    e
}
fn hit(w: &mut World, c: &Config, source: Owner, target: Owner, amount: u32, now: u64) {
    w.resolve_damage_with_rule(
        DamageIntent {
            source: w.owner_entity(source),
            target: w.owner_entity(target),
            raw: amount,
            direct: true,
            ignore_armor: true,
        },
        Some(&AttackRule {
            damage_kind: crate::config::DamageKind::TrueDamage,
            can_crit: false,
            ..Default::default()
        }),
        c,
        now,
        &mut vec![],
    );
}
fn clean_rows(e: &Events) -> Vec<(StatusCleansePhase, u32)> {
    e.iter()
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event:
                    Event::StatusCleanseResolved {
                        phase,
                        removed_count,
                        ..
                    },
            } => Some((*phase, *removed_count)),
            _ => None,
        })
        .collect()
}
fn ended(e: &Events) -> Vec<(u32, u32, u64)> {
    e.iter()
        .filter_map(|(_, e)| match e {
            EventV4::Private {
                event:
                    Event::WindowRecordEnded {
                        raw_hp,
                        hp_restored,
                        recorded,
                        ..
                    },
            } => Some((*raw_hp, *hp_restored, *recorded)),
            _ => None,
        })
        .collect()
}
#[test]
fn lifecycle_transfer_loader_roundtrips_plain_hot_combined_and_cleanse_only() {
    let p = Package::new();
    for hot in [false, true] {
        for include_window in [false, true] {
            let mut v = document();
            let id = if hot { "ally-renewal" } else { "demo-guard" };
            v["combat_rules"]["status_policies"] = json!({id:{"disposition":"beneficial","dispellable":true,"transferable":true,"blocks":[]}});
            v["combat_rules"]["status_cleanses"] = json!({id:{"on_expired":{"disposition":"harmful","max_count":1,"scope":"since_application"}}});
            if hot {
                v["combat_rules"]["status_cleanses"][id]["on_tick"] =
                    json!({"disposition":"harmful","max_count":1,"scope":"all"});
            }
            if include_window {
                v["combat_rules"]["status_windows"] = json!({id:{"basis":"none","include_periodic":false,"record_cap":0,"refresh":"preserve","maximum_sampling":"snapshot","on_expired":true,"on_consumed":true,"hp":{"fixed":1,"recorded_bps":0,"max_resource_bps":0,"cap_max_resource_bps":10000}}});
            }
            p.write(&v);
            let c = p.load().unwrap();
            assert!(
                c.default_config()
                    .combat_rules
                    .as_ref()
                    .unwrap()
                    .status_policies[id]
                    .transferable
            );
            v["combat_rules"]["status_policies"][id]["dispellable"] = json!(false);
            p.write(&v);
            assert!(p.load().is_err());
        }
    }
}
#[test]
fn lifecycle_transfer_keeps_complete_history_and_does_not_queue_or_execute_end_hooks() {
    let (c, mut w) = setup_lifecycle(true, "damage_taken", "snapshot", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    hit(&mut w, &c, Owner::Npc(0), Owner::Member(1), 50, 60);
    tick(&mut w, &c, 300);
    let before = cargo(&w);
    let hp = [w.members[0].hp, w.members[1].hp];
    let e = transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 450);
    let mut expected = before;
    expected.target = status_entity(&w.members[0].reference.entity);
    assert_eq!(cargo(&w), expected);
    assert_eq!([w.members[0].hp, w.members[1].hp], hp);
    assert!(e.iter().all(|(_, e)| matches!(
        e,
        EventV4::Private {
            event: Event::StatusTransferred { .. }
        }
    )));
    assert!(w.statuses.take_window_ends().is_empty());
    assert!(w.statuses.take_cleanse_ends().is_empty());
}
#[test]
fn lifecycle_transfer_three_window_bases_follow_only_current_holder_and_preserve_prior_amounts() {
    for basis in ["damage_dealt", "damage_taken", "shield_absorbed"] {
        let (c, mut w) = setup_lifecycle(false, basis, "snapshot", "all");
        apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
        if basis == "shield_absorbed" {
            apply(&mut w, &c, Owner::Member(1), Owner::Member(1), "ward", 0);
            apply(&mut w, &c, Owner::Member(0), Owner::Member(0), "ward", 0);
        }
        let contact = |w: &mut World, holder: Owner, amount, now| {
            if basis == "damage_dealt" {
                hit(w, &c, holder, Owner::Npc(0), amount, now)
            } else {
                hit(w, &c, Owner::Npc(0), holder, amount, now)
            }
        };
        contact(&mut w, Owner::Member(1), 30, 30);
        assert_eq!(cargo(&w).window.unwrap().recorded, 30);
        transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
        contact(&mut w, Owner::Member(1), 10, 90);
        contact(&mut w, Owner::Member(0), 40, 120);
        let record = cargo(&w).window.unwrap();
        assert_eq!((record.recorded, record.contacts), (70, 2));
    }
}
#[test]
fn lifecycle_transfer_snapshot_and_live_end_maxima_use_correct_holder_without_resampling() {
    for live in [false, true] {
        let (c, mut w) =
            setup_lifecycle(false, "none", if live { "live" } else { "snapshot" }, "all");
        apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
        let original = cargo(&w).window.unwrap();
        assert_eq!(original.max_hp_at_apply, 1800);
        transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
        w.members[0].hp = 500;
        w.members[0].spirit = 200;
        let old_hp = w.members[1].hp;
        let e = expire(&mut w, &c, 1200);
        assert_eq!(
            ended(&e),
            vec![(
                if live { 100 } else { 180 },
                if live { 100 } else { 180 },
                0
            )]
        );
        assert_eq!(w.members[0].spirit, if live { 230 } else { 250 });
        assert_eq!(w.members[1].hp, old_hp);
        assert!(expire(&mut w, &c, 1230).is_empty());
    }
}
#[test]
fn lifecycle_transfer_new_holder_uses_live_healing_gap_and_spirit_lock_at_end() {
    let (mut c, mut w) = setup_lifecycle(false, "none", "snapshot", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
    w.members[0].hp = 990;
    w.members[0].spirit = 290;
    c.pve_mut().status_definitions.push(serde_json::from_value(json!({"id":"lock","mechanism":"timed_attribute_modifier","attribute":"spirit_recovery_lock","modifier_bps":-10000,"duration_ms":3000,"duplicate_policy":"refresh","source_scoped":true})).unwrap());
    apply(&mut w, &c, Owner::Npc(0), Owner::Member(0), "lock", 90);
    let e = expire(&mut w, &c, 1200);
    assert_eq!(ended(&e), vec![(180, 10, 0)]);
    assert_eq!(w.members[0].spirit, 290);
}
#[test]
fn lifecycle_transfer_since_application_uses_original_start_on_new_holder_and_ignores_old_holder() {
    let (c, mut w) = setup_lifecycle(true, "none", "live", "since_application");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(0), "earlier", 0);
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 100);
    for target in [Owner::Member(0), Owner::Member(1)] {
        apply(&mut w, &c, Owner::Member(0), target, "between", 150);
    }
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 200);
    let e = tick(&mut w, &c, 400);
    assert_eq!(clean_rows(&e), vec![(StatusCleansePhase::Tick, 1)]);
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "earlier")
    );
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "between" && s.target.id == "P01")
    );
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "between" && s.target.id == "P00")
    );
}
#[test]
fn lifecycle_transfer_all_scope_can_cleanse_a_status_older_than_original_carrier() {
    let (c, mut w) = setup_lifecycle(true, "none", "live", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(0), "earlier", 0);
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 100);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 200);
    tick(&mut w, &c, 400);
    assert!(
        !w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "earlier")
    );
}
#[test]
fn lifecycle_transfer_hot_history_new_target_and_cleanse_keep_original_schedule() {
    let (c, mut w) = setup_lifecycle(true, "none", "snapshot", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    tick(&mut w, &c, 300);
    let first = cargo(&w);
    apply(&mut w, &c, Owner::Member(0), Owner::Member(0), "later", 330);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 450);
    w.members[0].hp = 500;
    let old = w.members[1].hp;
    let e = tick(&mut w, &c, 600);
    assert_eq!(cargo(&w).periodic.unwrap().ticks_done, 2);
    assert_eq!(first.periodic.unwrap().ticks_done, 1);
    assert_eq!(w.members[1].hp, old);
    assert!(w.members[0].hp > 500);
    assert_eq!(clean_rows(&e), vec![(StatusCleansePhase::Tick, 1)]);
}
#[test]
fn lifecycle_transfer_expired_cleanse_runs_before_recovery_on_current_holder_only() {
    let (c, mut w) = setup_lifecycle(false, "none", "snapshot", "since_application");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
    apply(&mut w, &c, Owner::Member(0), Owner::Member(0), "later", 90);
    w.members[0].hp = 500;
    let e = expire(&mut w, &c, 1200);
    assert_eq!(ended(&e), vec![(180, 180, 0)]);
    let cleanse = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::StatusCleanseResolved { .. }
                }
            )
        })
        .unwrap();
    let recovery = e
        .iter()
        .position(|(_, e)| {
            matches!(
                e,
                EventV4::Private {
                    event: Event::WindowRecordEnded { .. }
                }
            )
        })
        .unwrap();
    assert!(cleanse < recovery);
}
#[test]
fn lifecycle_transfer_consumed_releases_window_once_without_expired_cleanse() {
    let (c, mut w) = setup_lifecycle(false, "none", "snapshot", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
    w.members[0].hp = 500;
    let before = w.statuses.entries().to_vec();
    let mut raw = vec![];
    assert!(w.statuses.consume(&cargo(&w).instance_id, 1, &mut raw));
    let mut e = vec![];
    w.route_status_events(raw, &before, &mut e);
    w.finish_status_cleanses(&c, 90, &mut e);
    w.finish_window_records(&c, 90, &mut e);
    assert_eq!(ended(&e), vec![(180, 180, 0)]);
    assert!(clean_rows(&e).is_empty());
    assert!(expire(&mut w, &c, 1200).is_empty());
}
#[test]
fn lifecycle_transfer_dispelling_carrier_never_cleanses_or_recovers() {
    let (c, mut w) = setup_lifecycle(false, "none", "live", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
    w.members[0].hp = 500;
    let mut e = vec![];
    w.execute_status_operation((Owner::Member(0),Owner::Member(1)),&serde_json::from_value(json!({"kind":"dispel_statuses","subject":"owner","disposition":"beneficial","max_count":1})).unwrap(),&c,90,&mut e);
    w.finish_status_cleanses(&c, 90, &mut e);
    w.finish_window_records(&c, 90, &mut e);
    assert_eq!(w.members[0].hp, 500);
    assert!(clean_rows(&e).is_empty());
    assert!(ended(&e).iter().all(|(_, actual, _)| *actual == 0));
}
#[test]
fn lifecycle_transfer_capacity_failure_is_atomic_and_keeps_record_for_retry() {
    let (mut c, mut w) = setup_lifecycle(false, "damage_taken", "live", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    hit(&mut w, &c, Owner::Npc(0), Owner::Member(1), 50, 30);
    apply(
        &mut w,
        &c,
        Owner::Member(0),
        Owner::Member(0),
        "earlier",
        30,
    );
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let old = cargo(&w);
    assert!(transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60).is_empty());
    assert_eq!(cargo(&w), old);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 4;
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 90);
    assert_eq!(cargo(&w).window.unwrap().recorded, 50);
}
#[test]
fn lifecycle_transfer_conflict_does_not_merge_or_refresh_independent_histories() {
    let (c, mut w) = setup_lifecycle(false, "damage_taken", "live", "all");
    for target in [Owner::Member(0), Owner::Member(1)] {
        apply(&mut w, &c, Owner::Member(0), target, "cargo", 0);
    }
    hit(&mut w, &c, Owner::Npc(0), Owner::Member(1), 50, 30);
    let before = w.statuses.entries().to_vec();
    assert!(transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60).is_empty());
    assert_eq!(w.statuses.entries(), before);
}
#[test]
fn lifecycle_transfer_roundtrip_does_not_duplicate_end_queues_or_reset_records() {
    let (c, mut w) = setup_lifecycle(false, "damage_taken", "snapshot", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    hit(&mut w, &c, Owner::Npc(0), Owner::Member(1), 50, 30);
    let before = cargo(&w);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
    transfer(&mut w, &c, Owner::Member(0), Owner::Member(1), 90);
    assert_eq!(cargo(&w), before);
    assert!(w.statuses.take_window_ends().is_empty());
    assert!(w.statuses.take_cleanse_ends().is_empty());
}
#[test]
fn lifecycle_transfer_protection_skips_native_tick_without_delaying_original_schedule() {
    let (c, mut w) = setup_lifecycle(true, "none", "live", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 150);
    w.members[0].ready = false;
    tick(&mut w, &c, 300);
    assert_eq!(cargo(&w).periodic.unwrap().ticks_done, 1);
    w.members[0].ready = true;
    let e = tick(&mut w, &c, 600);
    assert_eq!(cargo(&w).periodic.unwrap().ticks_done, 2);
    assert_eq!(clean_rows(&e), vec![(StatusCleansePhase::Tick, 0)]);
}
#[test]
fn lifecycle_transfer_protected_expiry_drops_end_rewards_permanently() {
    let (c, mut w) = setup_lifecycle(false, "none", "live", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
    w.members[0].ready = false;
    w.members[0].hp = 500;
    let e = expire(&mut w, &c, 1200);
    assert_eq!(w.members[0].hp, 500);
    assert!(ended(&e).iter().all(|(_, actual, _)| *actual == 0));
    w.members[0].ready = true;
    assert!(expire(&mut w, &c, 1230).is_empty());
}
#[test]
fn lifecycle_transfer_old_non_source_holder_exit_preserves_and_new_holder_exit_cleans() {
    let (c, mut w) = setup_lifecycle(false, "none", "snapshot", "all");
    apply(&mut w, &c, Owner::Member(0), Owner::Member(1), "cargo", 0);
    transfer(&mut w, &c, Owner::Member(1), Owner::Member(0), 60);
    w.action(1, &Action::Leave {}, "old-left", &c, 90).unwrap();
    assert!(
        w.statuses
            .entries()
            .iter()
            .any(|s| s.definition_id == "cargo")
    );
    w.action(0, &Action::Leave {}, "new-left", &c, 120).unwrap();
    assert!(
        w.statuses
            .entries()
            .iter()
            .all(|s| s.definition_id != "cargo")
    );
}
#[test]
fn lifecycle_transfer_explicit_cross_faction_hot_keeps_npc_source_and_new_holder_hooks() {
    let (c, mut w) = setup_lifecycle(true, "none", "snapshot", "all");
    apply(&mut w, &c, Owner::Npc(0), Owner::Npc(0), "cargo", 0);
    let original = cargo(&w);
    transfer(&mut w, &c, Owner::Npc(0), Owner::Member(0), 150);
    assert_eq!(cargo(&w).source, original.source);
    w.members[0].hp = 500;
    let e = tick(&mut w, &c, 300);
    assert!(w.members[0].hp > 500);
    assert_eq!(clean_rows(&e), vec![(StatusCleansePhase::Tick, 0)]);
    assert_eq!(
        cargo(&w).window.unwrap().maximum_sampling,
        WindowMaximumSampling::Snapshot
    );
    assert_eq!(cargo(&w).window.unwrap().basis, WindowBasis::None);
}
