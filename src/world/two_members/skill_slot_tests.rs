use super::*;
fn setup(n: usize) -> (Config, World) {
    let (mut c, w) = super::combat_tests::setup(n);
    let cat = crate::ConfigCatalog::load(
        std::path::Path::new(env!("MATCH_SDK_TEST_DATA_DIR"))
            .join("examples/member-skills-host/catalog.json"),
    )
    .unwrap();
    let sample = cat.active("eight").unwrap();
    c.mechanism_version = 44;
    c.skill = sample.skill.clone();
    c.additional_skills = sample.additional_skills.clone();
    c.pve_mut().status_definitions.push(
        sample
            .pve
            .as_ref()
            .unwrap()
            .status_definitions
            .last()
            .unwrap()
            .clone(),
    );
    c.members.as_mut().unwrap().area_capacity = Some(8);
    c.members.as_mut().unwrap().projectile_capacity = 16;
    (c, w)
}
fn skill(c: &Config, slot: usize) -> SkillConfig {
    c.player_skills().nth(slot).unwrap().clone()
}
fn action(w: &World, c: &Config, _i: usize, slot: usize) -> Action {
    let s = skill(c, slot);
    if s.is_self() {
        Action::CastSelf { skill_id: s.id }
    } else if s.is_unit() {
        Action::CastUnit {
            skill_id: s.id,
            target: World::unit_wire(&w.npcs[0].entity),
        }
    } else if s.is_ground() {
        Action::CastGround {
            skill_id: s.id,
            position: w.npcs[0].position,
        }
    } else {
        Action::Cast {
            skill_id: s.id,
            direction: [1., 0.],
        }
    }
}
fn prepare(w: &mut World, c: &Config, i: usize, slot: usize, now: u64) -> Result<Events> {
    w.action(i, &action(w, c, i, slot), "slot-private", c, now)
}
fn commit(w: &mut World, c: &Config, i: usize, now: u64) -> Events {
    let p = w.members[i].pending.take().unwrap();
    let mut e = vec![];
    w.commit(i, p, c, now, &mut e);
    e
}
fn cast(w: &mut World, c: &Config, i: usize, slot: usize, now: u64) -> Events {
    prepare(w, c, i, slot, now).unwrap();
    commit(w, c, i, now + skill(c, slot).windup_ms)
}
#[test]
fn slots_one_two_four_eight_skills_and_members_publish_configuration_order() {
    for n in [1, 2, 4, 8] {
        for count in [1, 2, 4, 8] {
            let (mut c, w) = setup(n);
            c.additional_skills.truncate(count - 1);
            let expected = c.player_skills().map(|s| s.id.clone()).collect::<Vec<_>>();
            for i in 0..n {
                let s = w.snapshot(i, &c, Phase::Active, 0);
                assert_eq!(
                    s.own
                        .player_skills
                        .iter()
                        .map(|s| s.skill_id.clone())
                        .collect::<Vec<_>>(),
                    expected
                );
                assert!(s.own.player_skills.iter().all(|s| s.ready_at_ms == 0));
            }
        }
    }
}
#[test]
fn slots_every_registered_mechanism_resolves_by_id_and_commits() {
    let (c, mut w) = setup(1);
    for slot in 0..8 {
        let e = cast(&mut w, &c, 0, slot, slot as u64 * 1000);
        let id = skill(&c, slot).id;
        assert!(e.iter().any(|(_,e)|matches!(e,EventV4::Private{event:Event::ActionCommitted{skill_id:Some(s),..}}if *s==id)));
    }
    assert_eq!(w.members[0].cooldowns.len(), 8);
}
#[test]
fn slots_each_cost_and_cooldown_is_independent() {
    let (c, mut w) = setup(1);
    let initial = w.members[0].spirit;
    let mut total = 0;
    for slot in 0..8 {
        let s = skill(&c, slot);
        let now = slot as u64 * 1000;
        cast(&mut w, &c, 0, slot, now);
        total += s.cost;
        assert_eq!(w.members[0].spirit, initial - total);
        assert_eq!(
            w.members[0].cooldowns[&s.id],
            now + s.windup_ms + s.cooldown_ms
        );
        for later in slot + 1..8 {
            assert!(!w.members[0].cooldowns.contains_key(&skill(&c, later).id));
        }
    }
}
#[test]
fn slots_same_ids_have_independent_member_facts_and_status_sources() {
    let (c, mut w) = setup(8);
    for i in 0..8 {
        let before = w.members[i].spirit;
        cast(&mut w, &c, i, 7, 0);
        assert_eq!(w.members[i].spirit, before - skill(&c, 7).cost);
        assert_eq!(w.members[i].cooldowns.len(), 1);
    }
    assert_eq!(w.statuses.entries().len(), 8);
    for i in 0..8 {
        assert!(w.stats(i, &c, 120).0 > w.members[i].facts.atk_points);
    }
}
#[test]
fn slots_preparing_cannot_be_replaced_by_another_slot() {
    let (c, mut w) = setup(1);
    prepare(&mut w, &c, 0, 7, 0).unwrap();
    let before = w.members[0]
        .pending
        .as_ref()
        .unwrap()
        .skill
        .as_ref()
        .unwrap()
        .id
        .clone();
    assert_eq!(prepare(&mut w, &c, 0, 0, 0), Err(Error::InvalidState));
    assert_eq!(
        w.members[0]
            .pending
            .as_ref()
            .unwrap()
            .skill
            .as_ref()
            .unwrap()
            .id,
        before
    );
    assert!(w.members[0].cooldowns.is_empty() && w.shots.is_empty());
}
#[test]
fn slots_recovery_is_shared_and_cannot_be_shortened_by_switching() {
    let (c, mut w) = setup(1);
    cast(&mut w, &c, 0, 0, 0);
    let until = w.members[0].recovery_until;
    for slot in 1..8 {
        assert_eq!(
            prepare(&mut w, &c, 0, slot, until - 1),
            Err(Error::InvalidState)
        );
    }
    prepare(&mut w, &c, 0, 7, until).unwrap();
    assert_eq!(w.members[0].recovery_until, until);
}
#[test]
fn slots_cooldown_deadline_inclusive_and_other_slots_remain_ready() {
    let (c, mut w) = setup(1);
    cast(&mut w, &c, 0, 0, 0);
    let until = w.members[0].cooldowns[&c.skill.id];
    assert_eq!(
        prepare(&mut w, &c, 0, 0, until - 1),
        Err(Error::InvalidState)
    );
    prepare(&mut w, &c, 0, 0, until).unwrap();
    assert!(
        w.snapshot(0, &c, Phase::Active, until).own.player_skills[1..]
            .iter()
            .all(|s| s.ready_at_ms == 0)
    );
}
#[test]
fn slots_cancel_and_protection_interrupt_while_move_preserves_unpaid_preparation() {
    for mode in 0..3 {
        let (c, mut w) = setup(1);
        let before = w.members[0].spirit;
        prepare(&mut w, &c, 0, 7, 0).unwrap();
        match mode {
            0 => {
                w.action(0, &Action::CancelAction {}, "cancel", &c, 0)
                    .unwrap();
            }
            1 => {
                w.action(
                    0,
                    &Action::Move {
                        direction: [1., 0.],
                    },
                    "move",
                    &c,
                    0,
                )
                .unwrap();
                w.tick(&c, 30);
                assert!(w.members[0].pending.is_some());
                w.action(0, &Action::CancelAction {}, "cancel-after-move", &c, 30)
                    .unwrap();
            }
            _ => w.protect(0, 0, &mut vec![]),
        }
        assert!(
            w.members[0].pending.is_none()
                && w.members[0].cooldowns.is_empty()
                && w.statuses.entries().iter().all(|s| s.source.id != "P00")
        );
        assert_eq!(w.members[0].spirit, before);
    }
}
#[test]
fn slots_switching_after_cancel_never_resets_previous_cooldown() {
    let (c, mut w) = setup(1);
    cast(&mut w, &c, 0, 0, 0);
    let ready = w.members[0].cooldowns[&c.skill.id];
    prepare(&mut w, &c, 0, 7, 500).unwrap();
    w.action(0, &Action::CancelAction {}, "cancel", &c, 500)
        .unwrap();
    prepare(&mut w, &c, 0, 2, 500).unwrap();
    assert_eq!(w.members[0].cooldowns[&c.skill.id], ready);
    assert_eq!(w.members[0].cooldowns.len(), 1);
}
#[test]
fn slots_unknown_id_and_wrong_action_kind_never_fall_back_to_primary() {
    let (c, mut w) = setup(1);
    for a in [
        Action::Cast {
            skill_id: "missing-slot".into(),
            direction: [1., 0.],
        },
        Action::Cast {
            skill_id: skill(&c, 7).id,
            direction: [1., 0.],
        },
        Action::CastSelf {
            skill_id: skill(&c, 3).id,
        },
        Action::CastGround {
            skill_id: skill(&c, 6).id,
            position: [460., 400.],
        },
    ] {
        assert_eq!(w.action(0, &a, "bad", &c, 0), Err(Error::InvalidTarget));
    }
    assert!(w.members[0].pending.is_none() && w.members[0].cooldowns.is_empty());
}
#[test]
fn slots_prepare_resource_failure_is_atomic_for_each_slot() {
    for slot in 0..8 {
        let (c, mut w) = setup(1);
        w.members[0].spirit = 0;
        assert_eq!(prepare(&mut w, &c, 0, slot, 0), Err(Error::NoResource));
        assert!(w.members[0].pending.is_none() && w.members[0].cooldowns.is_empty());
    }
}
#[test]
fn slots_commit_resource_decline_leaves_all_other_slots_and_effects_unchanged() {
    for slot in 0..8 {
        let (c, mut w) = setup(1);
        prepare(&mut w, &c, 0, slot, 0).unwrap();
        w.members[0].spirit = 0;
        let e = commit(&mut w, &c, 0, 120);
        assert!(
            w.members[0].cooldowns.is_empty()
                && w.areas.is_empty()
                && w.shots.is_empty()
                && w.statuses.is_empty()
        );
        assert!(e.iter().any(|(_, e)| matches!(
            e,
            EventV4::Private {
                event: Event::ActionRejected {
                    reason: Error::NoResource,
                    ..
                }
            }
        )));
    }
}
#[test]
fn slots_last_self_status_group_capacity_failure_is_atomic() {
    let (mut c, mut w) = setup(1);
    prepare(&mut w, &c, 0, 7, 0).unwrap();
    c.pve_mut()
        .status_rules
        .as_mut()
        .unwrap()
        .max_active_per_entity = 0;
    let before = w.members[0].spirit;
    commit(&mut w, &c, 0, 120);
    assert_eq!(w.members[0].spirit, before);
    assert!(w.statuses.is_empty() && w.members[0].cooldowns.is_empty());
}
#[test]
fn slots_plan_capacity_competition_does_not_create_extra_capacity() {
    for slot in [0, 2, 3, 4, 5, 6] {
        let (mut c, mut w) = setup(1);
        prepare(&mut w, &c, 0, slot, 0).unwrap();
        if skill(&c, slot).is_ground() || skill(&c, slot).is_line() {
            c.members.as_mut().unwrap().area_capacity = Some(0);
        } else {
            c.members.as_mut().unwrap().projectile_capacity = 0;
        }
        let before = w.members[0].spirit;
        commit(&mut w, &c, 0, 120);
        assert_eq!(w.members[0].spirit, before);
        assert!(w.shots.is_empty() && w.areas.is_empty() && w.members[0].cooldowns.is_empty());
    }
}
#[test]
fn slots_late_unit_target_generation_is_revalidated_at_commit() {
    let (mut c, mut w) = setup(1);
    c.additional_skills[6] = skill(&c, 3);
    c.additional_skills[6].id = "last-unit".into();
    prepare(&mut w, &c, 0, 7, 0).unwrap();
    w.npcs[0].entity.generation += 1;
    let before = w.members[0].spirit;
    commit(&mut w, &c, 0, 120);
    assert_eq!(w.members[0].spirit, before);
    assert!(w.shots.is_empty() && w.members[0].cooldowns.is_empty());
}
#[test]
fn slots_ground_and_line_revalidation_before_commit_has_no_partial_cost() {
    for slot in [4, 6] {
        let (c, mut w) = setup(1);
        prepare(&mut w, &c, 0, slot, 0).unwrap();
        let p = Arc::make_mut(&mut w.grid.config);
        if slot == 4 {
            p.palette[0].walkable = false;
        } else {
            p.palette[0].blocks_projectiles = true;
        }
        let before = w.members[0].spirit;
        commit(&mut w, &c, 0, 120);
        assert_eq!(w.members[0].spirit, before);
        assert!(w.areas.is_empty() && w.members[0].cooldowns.is_empty());
    }
}
#[test]
fn slots_distinct_plans_and_self_effects_coexist_on_shared_world() {
    let (c, mut w) = setup(1);
    for (slot, now) in [(0, 0), (1, 300), (5, 700), (6, 1000), (7, 1270)] {
        cast(&mut w, &c, 0, slot, now);
    }
    w.advance_areas(&c, 1390, &mut vec![]);
    let s = w.snapshot(0, &c, Phase::Active, 1390);
    assert_eq!(s.projectiles.len(), 1);
    assert_eq!(s.areas.unwrap().len(), 2);
    assert!(s.own.statuses.iter().any(|s| s.definition_id == "focus"));
    assert_eq!(w.members[0].cooldowns.len(), 5);
}
#[test]
fn slots_last_field_uses_real_contact_budget_and_damage_pipeline() {
    let (mut c, mut w) = setup(1);
    c.additional_skills[6] = skill(&c, 5);
    c.additional_skills[6].id = "last-field".into();
    cast(&mut w, &c, 0, 7, 0);
    let hp = w.npcs[0].hp;
    let mut e = vec![];
    w.advance_areas(&c, 400, &mut e);
    assert!(w.npcs[0].hp < hp);
    assert!(e.iter().any(|(_,e)|matches!(e,EventV4::AreaTick{targets,..}if targets.iter().any(|t|t==&w.npcs[0].entity))));
    assert!(w.areas[0].view().contact_policy.unwrap().used_hits > 0);
}
#[test]
fn slots_background_preserves_committed_cooldowns_and_plan_deadlines() {
    let (c, mut w) = setup(1);
    cast(&mut w, &c, 0, 0, 0);
    cast(&mut w, &c, 0, 5, 1000);
    let cd = w.members[0].cooldowns.clone();
    let shots = w.shots[0].view();
    let areas = w.areas[0].view();
    w.protect(0, 0, &mut vec![]);
    assert_eq!(w.members[0].cooldowns, cd);
    assert_eq!(w.shots[0].view(), shots);
    assert_eq!(w.areas[0].view(), areas);
}
#[test]
fn slots_exit_cleans_owner_only_while_peer_state_and_cooldowns_remain() {
    let (c, mut w) = setup(2);
    for i in 0..2 {
        cast(&mut w, &c, i, 0, 0);
        cast(&mut w, &c, i, 5, 1000);
        cast(&mut w, &c, i, 7, 2000);
    }
    let cd = w.members[1].cooldowns.clone();
    w.terminal_intent(0, Outcome::Death, &mut vec![]);
    assert_eq!(w.members[1].cooldowns, cd);
    assert!(w.shots.iter().all(|s| s.view.source.id == "P01"));
    assert!(w.areas.iter().all(|a| a.view().source.id == "P01"));
    assert!(w.statuses.entries().iter().all(|s| s.source.id == "P01"));
}
#[test]
fn slots_private_ordered_cooldowns_do_not_leak_to_public_members() {
    let (c, mut w) = setup(2);
    cast(&mut w, &c, 0, 7, 0);
    let mine = w.snapshot(0, &c, Phase::Active, 120);
    let peer = w.snapshot(1, &c, Phase::Active, 120);
    assert!(mine.own.player_skills[7].ready_at_ms > 0);
    assert!(peer.own.player_skills.iter().all(|s| s.ready_at_ms == 0));
    let v = serde_json::to_value(mine).unwrap();
    for m in v["members"].as_array().unwrap() {
        for key in [
            "player_skills",
            "pending_action",
            "cooldowns",
            "spirit",
            "statuses",
        ] {
            assert!(m.get(key).is_none());
        }
    }
}
#[test]
fn slots_basic_attack_has_independent_cooldown_and_shared_preparation() {
    let (c, mut w) = setup(1);
    cast(&mut w, &c, 0, 0, 0);
    let ready = w.members[0].cooldowns[&c.skill.id];
    w.action(0, &Action::Attack {}, "basic", &c, 500).unwrap();
    assert_eq!(prepare(&mut w, &c, 0, 7, 500), Err(Error::InvalidState));
    commit(&mut w, &c, 0, 700);
    assert!(w.members[0].basic_ready > 700);
    assert_eq!(w.members[0].cooldowns[&c.skill.id], ready);
    prepare(&mut w, &c, 0, 7, 700).unwrap();
}
