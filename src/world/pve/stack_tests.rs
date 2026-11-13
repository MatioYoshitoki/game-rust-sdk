use super::status_tests::{active_engine_with_config, advance, bind, cast, damage, world};
use super::*;

fn config() -> Config {
    let mut c = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/stack-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone();
    c.pve_mut();
    c
}
#[test]
fn stack_real_hits_use_previous_layers_then_cap_and_restore_after_expiry() {
    let c = config();
    let mut w = world(&c);
    let mut events = vec![];
    let mut now = 0;
    for at in [0, 900, 1800, 2700, 3600] {
        events.extend(advance(&mut w, &c, now, at));
        cast(&mut w, &c, at);
        events.extend(advance(&mut w, &c, at, at + 600));
        now = at + 600;
    }
    assert_eq!(damage(&events), vec![66, 72, 80, 88, 88]);
    let s = w.snapshot(&c, now);
    assert_eq!(s.statuses[0].stacks, Some(3));
    assert_eq!(s.statuses[0].max_stacks, Some(3));
    assert_eq!(s.effective_def, Some(175));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::StatusStacked {
            added_stacks: 0,
            ..
        }
    )));
    events = advance(&mut w, &c, now, 6600);
    assert!(w.snapshot(&c, 6600).statuses.is_empty());
    cast(&mut w, &c, 6600);
    events.extend(advance(&mut w, &c, 6600, 7200));
    assert_eq!(damage(&events), vec![66]);
    assert_eq!(w.snapshot(&c, 7200).statuses[0].stacks, Some(1));
}
#[test]
fn stack_keeps_identity_and_refreshes_shared_deadline_even_at_cap() {
    let c = config();
    let mut w = world(&c);
    let who = status::participant();
    let mut id = None;
    for (n, now) in [0, 30, 60, 90].into_iter().enumerate() {
        let events = bind(&mut w, "guard", who.clone(), who.clone(), now);
        let s = w.snapshot(&c, now).statuses.remove(0);
        if let Some(ref id) = id {
            assert_eq!(&s.instance_id, id);
        } else {
            id = Some(s.instance_id.clone());
        }
        assert_eq!(s.stacks, Some((n as u32 + 1).min(3)));
        assert_eq!(s.expires_at_ms, now + 2400);
        assert_eq!(s.applied_at_ms, now);
        if n == 3 {
            assert!(matches!(
                events[0],
                Event::StatusStacked {
                    added_stacks: 0,
                    ..
                }
            ));
        }
    }
}
#[test]
fn stack_at_deadline_replaces_whole_group_and_same_tick_hits_are_ordered() {
    let mut c = config();
    c.pve_mut().status_definitions[0].duration_ms = 101;
    let mut w = world(&c);
    let who = status::participant();
    bind(&mut w, "guard", who.clone(), who.clone(), 0);
    let first = w.snapshot(&c, 100).statuses[0].instance_id.clone();
    let e = bind(&mut w, "guard", who.clone(), who.clone(), 101);
    assert!(matches!(
        e[0],
        Event::StatusRemoved {
            reason: StatusRemovalReason::Expired,
            ..
        }
    ));
    let fresh = w.snapshot(&c, 101).statuses.remove(0);
    assert_ne!(fresh.instance_id, first);
    assert_eq!(fresh.stacks, Some(1));
    bind(&mut w, "guard", who.clone(), who.clone(), 101);
    bind(&mut w, "guard", who.clone(), who, 101);
    let s = w.snapshot(&c, 101);
    assert_eq!(s.statuses[0].stacks, Some(3));
    assert_eq!(s.statuses[0].expires_at_ms, 202);
    assert_eq!(w.snapshot(&c, 201).effective_def, Some(175));
    assert_eq!(w.snapshot(&c, 202).effective_def, Some(100));
}
#[test]
fn stack_source_scoping_and_latest_owner_are_consistent_for_all_layers() {
    for scoped in [false, true] {
        let mut c = config();
        c.pve_mut().status_definitions[0].source_scoped = scoped;
        let mut w = world(&c);
        let p = status::participant();
        let m = w.pve.as_ref().unwrap().status_entity(0);
        bind(&mut w, "guard", p.clone(), p.clone(), 0);
        bind(&mut w, "guard", p.clone(), p.clone(), 30);
        bind(&mut w, "guard", m.clone(), p.clone(), 60);
        let s = w.snapshot(&c, 60);
        assert_eq!(s.statuses.len(), if scoped { 2 } else { 1 });
        if !scoped {
            assert_eq!(s.statuses[0].stacks, Some(3));
            assert_eq!(s.statuses[0].source, m);
        }
        let mut a = w.pve.take().unwrap();
        a.clear_status_entity(&m, StatusRemovalReason::SourceInvalid, &mut vec![]);
        w.pve = Some(a);
        let s = w.snapshot(&c, 60);
        if scoped {
            assert_eq!(s.statuses[0].stacks, Some(2));
            assert_eq!(s.statuses[0].expires_at_ms, 2430);
        } else {
            assert!(s.statuses.is_empty());
        }
    }
}
#[test]
fn stack_full_slots_allow_existing_layers_but_mixed_group_rejection_is_atomic() {
    let mut c = config();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    let mut w = world(&c);
    let p = status::participant();
    for now in [0, 30] {
        bind(&mut w, "guard", p.clone(), p.clone(), now);
    }
    let before = w.snapshot(&c, 60).statuses;
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.hit_statuses(&w, &["guard".into(), "exposed".into()], &p, &p, 60, &mut e);
    w.pve = Some(a);
    assert_eq!(w.snapshot(&c, 60).statuses, before);
    assert!(matches!(
        e[0],
        Event::StatusRejected {
            reason: Error::Capacity,
            ..
        }
    ));
    bind(&mut w, "guard", p.clone(), p, 60);
    assert_eq!(w.snapshot(&c, 60).statuses[0].stacks, Some(3));
}
#[test]
fn stack_self_rejection_preserves_existing_layers_and_costs() {
    let mut c = config();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    c.skill.on_commit_self_statuses = vec!["guard".into(), "exposed".into()];
    let mut w = world(&c);
    let p = status::participant();
    bind(&mut w, "guard", p.clone(), p, 0);
    let before = w.snapshot(&c, 0).statuses;
    cast(&mut w, &c, 0);
    let e = advance(&mut w, &c, 0, 600);
    assert!(e.iter().any(|e| matches!(
        e,
        Event::ActionRejected {
            reason: Error::Capacity,
            ..
        }
    )));
    assert_eq!(w.spirit, 200);
    assert_eq!(w.skill_ready_at, 0);
    assert_eq!(w.snapshot(&c, 600).statuses, before);
}
#[test]
fn stack_modifier_multiplies_before_single_rounding_and_global_caps() {
    let mut c = config();
    let mut w = world(&c);
    w.facts.def_points = 7;
    let p = status::participant();
    for now in [0, 30, 60] {
        bind(&mut w, "guard", p.clone(), p.clone(), now);
    }
    assert_eq!(w.snapshot(&c, 60).effective_def, Some(12));
    for now in [90, 120, 150] {
        bind(&mut w, "exposed", p.clone(), p.clone(), now);
    }
    assert_eq!(w.snapshot(&c, 150).effective_def, Some(7));
    c.pve_mut().status_definitions[0].modifier_bps = 100000;
    c.pve_mut().status_definitions[0].max_stacks = Some(128);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .defense_bonus_cap_bps = 100000;
    let mut w = world(&c);
    w.facts.def_points = 1000000;
    for _ in 0..128 {
        bind(&mut w, "guard", p.clone(), p.clone(), 0);
    }
    assert_eq!(w.snapshot(&c, 0).effective_def, Some(11000000));
    let mut c = config();
    c.pve_mut().status_definitions[1].modifier_bps = -10000;
    let mut w = world(&c);
    for _ in 0..3 {
        bind(&mut w, "exposed", p.clone(), p.clone(), 0);
    }
    assert_eq!(w.snapshot(&c, 0).effective_def, Some(0));
    w.facts.def_points = 0;
    assert_eq!(w.snapshot(&c, 0).effective_def, Some(0));
}
#[test]
fn stack_many_layers_use_one_slot_and_one_close_removal() {
    let mut c = config();
    c.pve_mut().status_definitions[0].max_stacks = Some(128);
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 1;
    for _ in 0..100 {
        let mut w = world(&c);
        let p = status::participant();
        for _ in 0..130 {
            bind(&mut w, "guard", p.clone(), p.clone(), 0);
        }
        let s = w.snapshot(&c, 0);
        assert_eq!(s.statuses.len(), 1);
        assert_eq!(s.statuses[0].stacks, Some(128));
        let e = w.close_container_interaction(0);
        assert!(w.pve.as_ref().unwrap().statuses.is_empty());
        assert!(
            e.iter().any(
                |e| matches!(e,Event::StatusRemoved{instance_ids,..} if instance_ids.len()==1)
            )
        );
    }
}
#[test]
fn stack_background_time_and_reset_clear_whole_group_without_reusing_identity() {
    let c = config();
    let mut w = world(&c);
    let p = status::participant();
    bind(&mut w, "guard", p.clone(), p.clone(), 0);
    bind(&mut w, "guard", p.clone(), p.clone(), 30);
    w.action(&Action::Background {}, "bg", &c, 30).unwrap();
    advance(&mut w, &c, 30, 1200);
    assert_eq!(w.snapshot(&c, 1200).statuses[0].stacks, Some(2));
    advance(&mut w, &c, 1200, 2460);
    assert!(w.snapshot(&c, 2460).statuses.is_empty());
    let mut w = world(&c);
    let m = w.pve.as_ref().unwrap().status_entity(0);
    bind(&mut w, "guard", m.clone(), p.clone(), 0);
    bind(&mut w, "guard", m.clone(), p.clone(), 30);
    let mut a = w.pve.take().unwrap();
    let mut e = vec![];
    a.reset_boss(0, 60, &mut e);
    a.hit_statuses(&w, &["guard".into()], &m, &p, 60, &mut e);
    assert!(a.statuses.is_empty());
}
#[test]
fn stack_public_retries_and_transport_receipts_never_add_layers_or_extend_deadline() {
    use crate::asset_contract::tests as s;
    let mut e = active_engine_with_config(config());
    let c = s::create(&e.config);
    let make = |seq| Input {
        key: c.key.clone(),
        participant_id: c.participant_id.clone(),
        entity_generation: 1,
        connection: c.connection.clone(),
        request_id: format!("stack-{seq}"),
        sequence: seq,
        action: Action::Cast {
            skill_id: "duanjin-shou".into(),
            direction: [1., 0.],
        },
    };
    let r = make(3);
    e.input(r.clone()).unwrap();
    e.input(r).unwrap();
    for _ in 0..30 {
        e.tick();
        e.drain();
    }
    let r = make(4);
    e.input(r.clone()).unwrap();
    e.input(r).unwrap();
    let mut outputs = vec![];
    for _ in 0..20 {
        e.tick();
        outputs.extend(e.drain());
    }
    let before = e
        .world
        .as_ref()
        .unwrap()
        .snapshot(&e.config, e.status().logical_ms)
        .statuses;
    assert_eq!(before[0].stacks, Some(2));
    let output = outputs
        .iter()
        .rev()
        .find(|o| matches!(o.body, OutputBody::Publication { .. }))
        .unwrap();
    let receipt = TransportReceipt {
        key: c.key,
        recipient_id: c.participant_id,
        connection: c.connection,
        output_id: output.id.clone(),
        outcome: TransportOutcome::Sent,
    };
    e.transport(receipt.clone()).unwrap();
    e.transport(receipt).unwrap();
    assert_eq!(
        e.world
            .as_ref()
            .unwrap()
            .snapshot(&e.config, e.status().logical_ms)
            .statuses,
        before
    );
    assert_eq!(outputs.iter().filter(|o|matches!(&o.body,OutputBody::Event{event:Event::StatusStacked{status,..}} if status.definition_id=="guard")).count(),1);
}
#[test]
fn stack_does_not_change_legacy19_status_wire_shape() {
    let old = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/status-host/catalog.json"),
    )
    .unwrap()
    .default_config()
    .as_ref()
    .clone();
    let mut w = world(&old);
    cast(&mut w, &old, 0);
    advance(&mut w, &old, 0, 600);
    let v = serde_json::to_value(w.snapshot(&old, 600)).unwrap();
    let s = &v["statuses"][0];
    assert!(s.get("stacks").is_none() && s.get("max_stacks").is_none());
}

#[test]
fn stack_rejected_contacts_do_not_change_existing_layers_or_deadlines() {
    for mode in ["cancel", "background", "dodge", "wall", "lethal", "zero"] {
        let mut c = config();
        if mode == "dodge" {
            c.pve_mut().stats.insert("dodge".into(), 100);
        }
        if mode == "lethal" {
            c.pve_mut().stats.insert("hp".into(), 1);
        }
        if mode == "zero" {
            c.params.minimum_damage = 0;
            c.skill.damage_bps = 0;
        }
        if mode == "wall" {
            let p = c.pve_mut();
            let mut t = p.palette[0].clone();
            t.blocks_projectiles = true;
            p.palette.push(t);
            p.cells_rle = vec![(0, 332), (1, 1), (0, 691)];
        }
        let mut w = world(&c);
        let m = w.pve.as_ref().unwrap().status_entity(0);
        bind(&mut w, "exposed", status::participant(), m, 0);
        let initial = w.snapshot(&c, 0).pve.unwrap().monsters[0].statuses[0].clone();
        cast(&mut w, &c, 0);
        if mode == "cancel" {
            w.action(&Action::CancelAction {}, "cancel", &c, 30)
                .unwrap();
        }
        if mode == "background" {
            w.action(&Action::Background {}, "background", &c, 30)
                .unwrap();
        }
        let e = advance(&mut w, &c, 0, 600);
        let views = w.snapshot(&c, 600).pve.unwrap().monsters[0]
            .statuses
            .clone();
        if mode == "lethal" {
            assert!(views.is_empty());
        } else if mode == "zero" {
            assert_eq!(views[0].stacks, Some(2));
            assert_eq!(damage(&e), vec![0]);
        } else {
            assert_eq!(views, vec![initial], "{mode}");
        }
    }
}

#[test]
fn stack_encounter_round_reset_clears_all_layers_and_old_source_cannot_reapply() {
    let sample = config();
    let mut c = crate::config::default_catalog()
        .active("b3-13")
        .unwrap()
        .as_ref()
        .clone();
    c.schema_version = 4;
    c.mechanism_version = 20;
    c.asset_contract = sample.asset_contract;
    c.result_contract = sample.result_contract;
    let p = c.pve_mut();
    p.status_definitions = sample.pve.as_ref().unwrap().status_definitions.clone();
    p.status_rules = sample.pve.as_ref().unwrap().status_rules.clone();
    for d in &mut p.status_definitions {
        d.duration_ms = 60000;
    }
    let center = p.encounters[0].trigger.center;
    let delay = p.encounters[0].reset.as_ref().unwrap().delay_ms;
    let mut w = world(&c);
    w.position = center;
    advance(&mut w, &c, 0, 30);
    let a = w.pve.as_ref().unwrap();
    let index = a
        .monsters
        .iter()
        .position(|m| m.enabled && m.encounter_index == Some(0))
        .unwrap();
    let old = a.status_entity(index);
    for t in [30, 60, 90] {
        bind(&mut w, "guard", old.clone(), status::participant(), t);
    }
    assert_eq!(w.snapshot(&c, 90).statuses[0].stacks, Some(3));
    let mut a = w.pve.take().unwrap();
    w.position = [0., 0.];
    let mut e = vec![];
    a.advance_encounter_resets(&w, 120, &mut e);
    a.advance_encounter_resets(&w, 120 + delay, &mut e);
    assert!(a.statuses.is_empty());
    assert_ne!(a.status_entity(index), old);
    a.hit_statuses(
        &w,
        &["guard".into()],
        &old,
        &status::participant(),
        120 + delay,
        &mut e,
    );
    assert!(a.statuses.is_empty());
}
