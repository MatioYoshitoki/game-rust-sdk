//! 训练模式房主工具（SDK-5-98，机制 106）合成验收：
//! 门禁、冷却重置边界、恢复语义（满血满灵、有害清除、正面保留、死亡拒绝）。
use super::*;

fn tools_input(tools: bool) -> Value {
    let mut v = input();
    v["mechanism_version"] = json!(106);
    v["pve"]["monster_positions"] = json!([[52, 20], [80, 20]]);
    v["pve"]["status_definitions"] = json!([
        {"id":"hold","mechanism":"timed_control","attribute":"root","duration_ms":900,
         "duplicate_policy":"refresh","source_scoped":true,"harmful":true},
        {"id":"ward","mechanism":"absorption_shield","shield_bps":2000,"duration_ms":900,
         "duplicate_policy":"refresh","source_scoped":true}
    ]);
    v["params"]["entry_years"] = json!(0);
    v["carried_quantity"] = json!(0);
    v["members"]["training_targets"] = json!([
        {"initial_preset":"pack","presets":{"pack":["npc-1"]}},
        {"initial_preset":"pack","presets":{"pack":["npc-2"]}}
    ]);
    if tools {
        v["members"]["training_tools"] = json!({"enabled":true,"restore_clears":[]});
    }
    v
}

fn npc_wire(id: &str) -> EntityRef {
    EntityRef {
        kind: crate::protocol::StatusEntityKind::Monster,
        id: id.into(),
        generation: 1,
    }
}

fn seat_entity(i: usize) -> EntityRef {
    EntityRef {
        kind: crate::protocol::StatusEntityKind::Participant,
        id: format!("m{i}"),
        generation: 1,
    }
}

fn apply_hold(w: &mut World, c: &Config, i: usize, now: u64) {
    let defs: Vec<_> = c
        .pve
        .as_ref()
        .unwrap()
        .status_definitions
        .iter()
        .filter(|d| d.id == "hold")
        .cloned()
        .collect();
    let rules = c.pve.as_ref().unwrap().status_rules.as_ref().unwrap();
    let source = status_entity(&npc_wire("npc-1"));
    let member_entity = w.members[i].reference.entity.clone();
    let target = status_entity(&member_entity);
    let mut events = vec![];
    let applied = w.statuses.apply(
        &defs,
        (&source, 0u32),
        (&target, 0u32),
        now,
        rules,
        &mut events,
    );
    assert!(applied, "hold must be applicable");
    let _ = events;
}

#[test]
fn training_tools_gate_requires_training_config_and_mechanism_106() {
    // 非训练配置（无 training_targets）+ 机制 105：双重拒绝。
    let c = catalog(tools_input(false), |v| {
        v["mechanism_version"] = json!(105);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let mut events = vec![];
    assert_eq!(
        w.reset_training_cooldowns(&[0], &c, &mut events),
        Err(Error::Unsupported)
    );
}

#[test]
fn training_tools_gate_requires_enabled_and_mechanism_106() {
    // training_targets 存在但 tools 未启用 / 机制 105：拒绝；106 + 启用：通过。
    let c105 = catalog(tools_input(true), |v| {
        v["mechanism_version"] = json!(105);
    })
    .unwrap()
    .default_config();
    let mut w = world(&c105);
    let mut events = vec![];
    assert_eq!(
        w.reset_training_cooldowns(&[0], &c105, &mut events),
        Err(Error::Unsupported)
    );

    let c = catalog(tools_input(true), |v| {}).unwrap().default_config();
    let mut w = world(&c);
    let mut events = vec![];
    assert!(w.reset_training_cooldowns(&[0], &c, &mut events).is_ok());
}

#[test]
fn reset_training_cooldowns_clears_skill_cooldowns_only() {
    let c = catalog(tools_input(true), |v| {}).unwrap().default_config();
    let mut w = world(&c);
    for member in &mut w.members {
        member.foreground = true;
    }
    let npc = npc_wire("npc-1");
    // 施放主技能（cost 20，CD 300ms）→ cooldowns 出现 'skill'
    let action = Action::CastUnit {
        skill_id: "skill".into(),
        target: crate::protocol::UnitTarget {
            id: npc.id.clone(),
            generation: npc.generation,
        },
    };
    let cast = w.action(0, &action, "cast", &c, 0);
    assert!(cast.is_ok(), "cast must succeed: {:?}", cast.as_ref().err());
    // 固定步提交：pending 由显式 commit 驱动（world tick 不推进 pending）
    let pending = w.members[0].pending.take().expect("pending after prepare");
    let mut commit_events = vec![];
    w.commit(0, pending, &c, 30, &mut commit_events);
    assert!(
        w.members[0].cooldowns.contains_key("skill"),
        "commit must arm the cooldown"
    );
    let mut events = vec![];
    assert!(w.reset_training_cooldowns(&[0], &c, &mut events).is_ok());
    assert!(
        w.members[0].cooldowns.is_empty(),
        "cooldowns must be cleared"
    );
    assert!(
        events.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::TrainingCooldownsReset { .. }
            }
        )),
        "private reset event must fire"
    );
    // 席位隔离：只清目标席（席位 1 的 pending 同样显式提交）
    let npc2 = w.members[1]
        .ai
        .as_ref()
        .map(|_| ())
        .map(|_| ())
        .unwrap_or(());
    let _ = npc2;
    let npc2_entity = w
        .npcs
        .iter()
        .find(|n| n.training_owner == Some(1))
        .map(|n| n.entity.clone())
        .expect("seat 1 target");
    let action_b = Action::CastUnit {
        skill_id: "skill".into(),
        target: crate::protocol::UnitTarget {
            id: npc2_entity.id.clone(),
            generation: npc2_entity.generation,
        },
    };
    assert!(w.action(1, &action_b, "cast-b", &c, 0).is_ok());
    let pending_b = w.members[1].pending.take().expect("pending b");
    let mut commit_b = vec![];
    w.commit(1, pending_b, &c, 30, &mut commit_b);
    assert!(w.members[1].cooldowns.contains_key("skill"));
    let mut events = vec![];
    assert!(w.reset_training_cooldowns(&[0], &c, &mut events).is_ok());
    assert!(
        w.members[1].cooldowns.contains_key("skill"),
        "other seat untouched"
    );
}

#[test]
fn restore_training_member_heals_cleanses_harmful_and_keeps_beneficial() {
    let c = catalog(tools_input(true), |v| {}).unwrap().default_config();
    let mut w = world(&c);
    // 残血 + 空灵；挂有害 root 与有益 ward
    w.members[0].hp = 10;
    w.members[0].spirit = 5;
    apply_hold(&mut w, &c, 0, 0);
    let ward_defs: Vec<_> = c
        .pve
        .as_ref()
        .unwrap()
        .status_definitions
        .iter()
        .filter(|d| d.id == "ward")
        .cloned()
        .collect();
    let rules = c.pve.as_ref().unwrap().status_rules.as_ref().unwrap();
    let source = status_entity(&npc_wire("npc-1"));
    let member_entity = w.members[0].reference.entity.clone();
    let target = status_entity(&member_entity);
    let max_hp = match w.entity_max_hp(&member_entity, &c, 0) {
        Some(n) => n,
        None => panic!("member max hp unavailable"),
    };
    let mut apply_events = vec![];
    let ward_applied = w.statuses.apply(
        &ward_defs,
        (&source, 0u32),
        (&target, max_hp),
        0,
        rules,
        &mut apply_events,
    );
    assert!(ward_applied, "ward must be applicable");
    let mut events = vec![];
    assert!(
        w.restore_training_member(&[0], &[], &c, 30, &mut events)
            .is_ok()
    );
    assert_eq!(w.members[0].hp, 100, "hp restored to max");
    assert_eq!(w.members[0].spirit, 100, "spirit restored to max");
    let member_entity = w.members[0].reference.entity.clone();
    let status_entity = status_entity(&member_entity);
    let hold_gone = !w
        .statuses
        .entries()
        .iter()
        .any(|s| s.target == status_entity && s.definition_id == "hold");
    let ward_kept = w
        .statuses
        .entries()
        .iter()
        .any(|s| s.target == status_entity && s.definition_id == "ward");
    assert!(hold_gone, "harmful hold must be cleansed");
    assert!(ward_kept, "beneficial ward must be kept");
    let restored = events.iter().any(|(_, e)| {
        matches!(
            e,
            EventV4::Private {
                event: Event::TrainingMemberRestored { cleansed, .. }
            } if cleansed.as_slice() == ["hold"]
        )
    });
    assert!(restored, "restore event must carry cleansed ids");
}

#[test]
fn restore_training_member_rejects_dead_member() {
    let c = catalog(tools_input(true), |v| {}).unwrap().default_config();
    let mut w = world(&c);
    w.members[0].hp = 0;
    let mut events = vec![];
    assert_eq!(
        w.restore_training_member(&[0], &[], &c, 30, &mut events),
        Err(Error::InvalidState)
    );
}
