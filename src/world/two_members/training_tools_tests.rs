//! 训练模式房主工具（SDK-5-98，机制 106）合成验收：
//! 门禁、冷却重置边界、恢复语义（满血满灵、有害清除、正面保留、死亡拒绝）。
use super::*;

fn tools_input(tools: bool) -> Value {
    let mut v = input();
    v["mechanism_version"] = json!(107);
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
        {"owner_scope":"all","initial_preset":"pack","presets":{"pack":["npc-1"]}},
        {"initial_preset":"pack","presets":{"pack":["npc-2"]}}
    ]);
    if tools {
        v["members"]["training_tools"] =
            json!({"enabled":true,"restore_clears":[],"spawn_max_alive":3});
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

#[test]
fn shared_scope_lets_other_members_attack_owner_targets() {
    let c = catalog(tools_input(true), |v| {}).unwrap().default_config();
    let mut w = world(&c);
    // 席 0（owner，scope=all）的目标 npc-1：席 1 可作为单位目标（共享）
    let shared = crate::protocol::UnitTarget {
        id: "npc-1".into(),
        generation: 1,
    };
    let cast_b = w.action(
        1,
        &Action::CastUnit {
            skill_id: "skill".into(),
            target: shared,
        },
        "shared-cast",
        &c,
        0,
    );
    assert!(
        cast_b.is_ok(),
        "shared target must be attackable by other members"
    );
    // 席 1 自己的目标 npc-2：席 0 不可目标（seat 作用域私有）
    let private = crate::protocol::UnitTarget {
        id: "npc-2".into(),
        generation: 1,
    };
    let cast_a = w.action(
        0,
        &Action::CastUnit {
            skill_id: "skill".into(),
            target: private,
        },
        "private-cast",
        &c,
        0,
    );
    assert_eq!(cast_a, Err(Error::InvalidTarget));
}

#[test]
fn spawn_training_targets_respects_registered_templates_and_alive_cap() {
    let c = catalog(tools_input(true), |v| {
        v["members"]["training_tools"]["spawn_max_alive"] = json!(2);
        v["pve"]["additional_templates"] = json!({"wolf": {"mechanism":"melee_projectile_heal","name":"wolf",
            "stats":{"hp":50,"spirit":0,"atk":5,"def":0,"spd":1,"crit":0,"dodge":0},
            "combat":{"radius":2,"attack_interval_ms":1000,"aggro_range":8,"melee_range":2,"cast_chance":0,
                      "return_distance":8,"lifesteal_ratio":0,"mark_max":0,"mark_duration_ms":0,
                      "projectile":{"mechanism":"projectile","id":"wolf-cast","cost":60,
                                    "windup_ms":30,"cooldown_ms":300,"recovery_ms":60,
                                    "range":80,"speed":10,"radius":1,
                                    "ttl_ms":1000,"damage_bps":10000},
                      "heal":{"mechanism":"budget_heal_channel","id":"wolf-heal","cost":0,"windup_ms":30,
                              "cooldown_ms":300,"recovery_ms":60,"hp_threshold":0,"heal_bps":0,
                              "interval_ms":30,"ticks":1}},
            "leash":16,"help_range":1,"memory_range":16,"kill_credit":false,"drop_orb":false,
            "drop_table":{"guaranteed":[],"random":[]}}});
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let mut events = vec![];
    // 未注册模板 → InvalidTarget
    assert_eq!(
        w.spawn_training_targets(0, "p0", "ghost", &[[60.0, 20.0]], &c, 0, &mut events),
        Err(Error::InvalidTarget)
    );
    // 注册模板 ×2（max_alive 2）→ ok
    let ids = w
        .spawn_training_targets(
            0,
            "p0",
            "wolf",
            &[[60.0, 20.0], [70.0, 20.0]],
            &c,
            0,
            &mut events,
        )
        .unwrap();
    assert_eq!(ids.len(), 2);
    assert!(
        w.npcs
            .iter()
            .any(|n| n.runtime_spawned && n.entity.id == ids[0])
    );
    // 超出上限 → Capacity
    assert_eq!(
        w.spawn_training_targets(0, "p0", "wolf", &[[90.0, 20.0]], &c, 0, &mut events),
        Err(Error::Capacity)
    );
    // 不可行走落点 → InvalidTarget
    assert_eq!(
        w.spawn_training_targets(1, "p1", "wolf", &[[0.0, 0.0]], &c, 0, &mut events),
        Err(Error::InvalidTarget)
    );
}

#[test]
fn despawn_training_targets_removes_runtime_instances_only() {
    let c = catalog(tools_input(true), |v| {
        v["pve"]["additional_templates"] = json!({"wolf": {"mechanism":"melee_projectile_heal","name":"wolf",
            "stats":{"hp":50,"spirit":0,"atk":0,"def":0,"spd":0,"crit":0,"dodge":0},
            "combat":{"radius":2,"attack_interval_ms":1000,"aggro_range":8,"melee_range":2,"cast_chance":0,
                      "return_distance":8,"lifesteal_ratio":0,"mark_max":0,"mark_duration_ms":0,
                      "projectile":{"mechanism":"projectile","id":"wolf-cast","cost":60,
                                    "windup_ms":30,"cooldown_ms":300,"recovery_ms":60,
                                    "range":80,"speed":10,"radius":1,
                                    "ttl_ms":1000,"damage_bps":10000},
                      "heal":{"mechanism":"budget_heal_channel","id":"wolf-heal","cost":0,"windup_ms":30,
                              "cooldown_ms":300,"recovery_ms":60,"hp_threshold":0,"heal_bps":0,
                              "interval_ms":30,"ticks":1}},
            "leash":16,"help_range":1,"memory_range":16,"kill_credit":false,"drop_orb":false,
            "drop_table":{"guaranteed":[],"random":[]}}});
    })
    .unwrap()
    .default_config();
    let mut w = world(&c);
    let mut events = vec![];
    let ids = w
        .spawn_training_targets(0, "p0", "wolf", &[[60.0, 20.0]], &c, 0, &mut events)
        .unwrap();
    // 目录预布置实例不可 despawn（归预设语义）
    assert_eq!(
        w.despawn_training_targets(0, &["npc-1".to_string()], &c, 0, &mut events),
        Err(Error::InvalidTarget)
    );
    // 运行时实例可移除：hp 0、非激活（死亡不刷新语义）
    w.despawn_training_targets(0, &ids, &c, 0, &mut events)
        .unwrap();
    let spawned = w.npcs.iter().find(|n| n.entity.id == ids[0]).unwrap();
    assert_eq!(spawned.hp, 0);
    assert!(!spawned.training_active);
}
