//! Data-only support information; querying this never opens a catalog or starts a worker.
pub(crate) const MAX_MECHANISM_VERSION: u32 = 106;
pub const JSONL_LINE_LIMIT: usize = 262144;

pub fn sdk_info() -> serde_json::Value {
    let mut info = serde_json::json!({
        "package": env!("CARGO_PKG_NAME"), "crateVersion": env!("CARGO_PKG_VERSION"),
        "clientMethods": ["create", "submit_input", "submit_business_receipt",
            "submit_transport_receipt", "drain_outputs", "close"],
        "protocolVersions": [1, 2, 3, 4, 5, 6],
        "configSchemaProtocols": [
            {"schemaVersion":2,"protocolVersion":1},
            {"schemaVersion":3,"protocolVersion":2}, {"schemaVersion":4,"protocolVersion":3}, {"schemaVersion":5,"protocolVersion":4}, {"schemaVersion":6,"protocolVersion":5}, {"schemaVersion":7,"protocolVersion":6}],
        "mechanismVersions": {"min":1,"max":MAX_MECHANISM_VERSION},
        "manifestSchemaVersion":1, "recoverySchemaVersion":1,
        "settlementPolicies":["preserve","attrition"],
        "timedStatus":{"minimumMechanismVersion":19,"stackMinimumMechanismVersion":20,
            "duplicatePolicies":["refresh","ignore","stack"],"attributeMinimumMechanismVersion":21,"attributes":["atk","def"],
            "attackCapture":"per_plan_boundary",
            "healingReceived":{"minimumMechanismVersion":25,"attribute":"healing_received","duplicatePolicies":["refresh","ignore"],"aggregation":"sum_bps_floor_zero","positiveGameplayCap":null,"capture":"per_heal","paths":["hot","self_channel","support_channel"]},
            "stackTiming":"refresh_all","maxStacks":128},
        "absorptionShield":{"minimumMechanismVersion":22,"grant":"target_max_hp_bps",
            "duplicatePolicies":["refresh","ignore"],"consumptionOrder":["expires_at_ms","applied_at_ms","instance_id"],
            "maxCapBps":10000,"damageAmount":"actual_hp_loss"},
        "periodicEffects":{"minimumMechanismVersion":23,"kinds":["physical_damage","max_hp_heal"],"healMinimumMechanismVersion":24,"healBasis":"base_max_hp","healBindings":["commit_self"],"sameDueOrder":["physical_damage","max_hp_heal","instance_id"],"duplicatePolicies":["refresh","ignore"],
            "bindings":["hit_target"],"sourceAttackCapture":"application_group","firstTick":"after_interval","endExclusive":true,"maxTicks":128},
        "playerSkills":{"minimumMechanismVersion":26,"maxSkills":2,"kind":"projectile","primary":"skill","additional":"additional_skills","cooldowns":"per_skill","actionConstraint":"shared_preparation_and_recovery","lineField":{"minimumMechanismVersion":34,"mechanism":"line_field","input":"cast","geometry":"fixed_capsule","lengthBounds":[0.1,10000.0],"widthBounds":[0.2,256.0],"maxGroundPlans":2,"sharesCapacityWith":["ground_circle","ground_field"],"wallPolicy":"clip_swept_disk_at_commit","targetOrder":["path_progress","path_distance_squared","id","generation"],"contactClock":"scheduled_at_ms","sourceInvalidPolicy":"end","costOwner":"commit_once"},"projectileRepeat":{"minimumMechanismVersion":33,"mechanisms":["piercing_projectile"],"field":"repeat_interval_ms","maxIntervalMs":60000,"clock":"logical_fixed_step","eligibility":"last_success_plus_interval_inclusive","maxAttemptsPerIdentityPerStep":1,"catchUp":false,"requiresSweptContact":true,"budget":"shared_total_successes","dodgeConsumes":false},"piercingProjectile":{"minimumMechanismVersion":32,"mechanism":"piercing_projectile","input":"cast","homing":false,"maxInFlight":2,"maxHits":128,"repeat":"once_per_identity","collision":"continuous_sweep","wallTie":"wall_first","sourceInvalidPolicy":"end"},"contactPolicy":{"minimumMechanismVersion":31,"mechanisms":["ground_field"],"maxTargetsPerTick":32,"maxTotalHits":128,"historyMax":128,"repeatZero":"once_per_identity","repeatClock":"scheduled_at_ms","consumption":"successful_direct_contact_including_zero_hp_loss","dodgeConsumes":false,"targetOrder":["center_distance_squared","id","generation"]},"groundField":{"minimumMechanismVersion":30,"mechanism":"ground_field","input":"cast_ground","geometry":"fixed_circle","maxGroundPlans":2,"maxTicks":128,"firstTick":"after_interval","endExclusive":true,"costOwner":"commit_once","sourceInvalidPolicy":"end","attackCapture":"commit","targets":"current_positions"},"groundCircle":{"minimumMechanismVersion":29,"mechanism":"ground_circle","input":"cast_ground","validation":["accept","commit"],"pointPolicy":"strict_walkable","maxPendingAreas":2,"pulses":1,"attackCapture":"commit","targets":"current_positions"},"unitProjectile":{"minimumMechanismVersion":28,"mechanism":"unit_projectile","input":"cast_unit","identity":["id","generation"],"relation":"hostile","validation":["accept","commit"],"retarget":false,"homing":false,"interveningUnit":"block_without_effect"},"selfStatus":{"minimumMechanismVersion":27,"mechanism":"self_status","input":"cast_self","binding":"on_commit_self_statuses","commit":"fixed_step","createsProjectile":false}},
        "clocks":{"ordinary":true,"replay":cfg!(feature="replay")},
        "twoMembers":{"protocolVersion":4,"schemaVersion":5,"mechanismVersion":35,"participants":2,"worlds":1,"publication":"private_full","recovery":"financial_obligations_only","playerSkills":["projectile","self_status_atk_def"],"unsupported":["pvp","delta","spectator","containers","encounters","fields","piercing","unit_skills","ground_skills","shield","dot","hot"]},
        "members":{"protocolVersion":5,"schemaVersion":6,"mechanismVersion":36,"minimum":1,"maximum":8,"roster":"fixed_required","outputs":"shared_v4_dtos","maxAssetFactRows":32,"botSeats":false,"publication":"private_full"},
        "replication":{"protocolVersion":6,"schemaVersion":7,"mechanismVersion":37,"kinds":["full","delta"],"base":"sent_per_recipient","maxInFlightPerRecipient":1,"clientResync":"request_full","recovery":"financial_only","spectator":false},
        "multiplayerPeriodic":{"minimumMechanismVersion":38,"schemaVersion":7,"protocolVersion":6,"dotRelations":["member_to_npc","npc_to_member"],"hotTargets":"self","kinds":["physical_damage","max_hp_heal"],"sourceIdentity":["kind","id","generation"],"npcReset":"increment_generation_and_clear","sourceExit":"clear_at_settling","reconnect":"preserve_schedule","recovery":"financial_only","maxTotalStatusInstances":128,"npcStatusView":"own_sources_and_npc_self"},
        "multiplayerShields":{"minimumMechanismVersion":39,"schemaVersion":7,"protocolVersion":6,"bindings":["commit_self","hostile_hit_target"],"damagePaths":["direct","physical_dot"],"totals":"own_private_npc_public","npcConsumption":"recipient_visible_instances","sourceExit":"clear_at_settling","reconnect":"preserve_balance_and_deadline","recovery":"financial_only"},
        "multiplayerCombat":{"minimumMechanismVersion":40,"schemaVersion":7,"protocolVersion":6,"relations":["self","friendly","hostile"],"policy":"pve_kind_groups","unitTargets":"published_hostile_npc","validation":["accept","commit","in_flight_identity"],"unitHoming":false,"interveningUnit":"block_without_effect","damagePipeline":"shared_direct_and_dot","healPipeline":"shared_entity_base_max_hp","recovery":"financial_only","pvp":false},
        "memberAreas":{"minimumMechanismVersion":41,"schemaVersion":7,"protocolVersion":6,"mechanisms":["ground_circle","ground_field"],"input":"cast_ground","geometry":"fixed_circle","validation":["accept","commit"],"firstFieldTick":"after_interval","endExclusive":true,"maxTicks":128,"maxPlans":8,"maxTotalContactHistory":128,"capacityField":"members.area_capacity","damage":"shared_direct_contact","sourceExit":"clear_at_settling","reconnect":"preserve_committed_plans","recovery":"financial_only"},
        "memberLineAreas":{"minimumMechanismVersion":42,"schemaVersion":7,"protocolVersion":6,"mechanism":"line_field","input":"cast","geometry":"fixed_capsule","wallPolicy":"clip_swept_disk_at_commit","targetOrder":["path_progress","path_distance_squared","id","generation"],"sharedPlans":["ground_circle","ground_field","line_field"],"capacityField":"members.area_capacity","damage":"shared_direct_contact","recovery":"financial_only"},
        "trainingTools":{"minimumMechanismVersion":106,"schemaVersion":7,"protocolVersion":6,"gate":"members.training_tools.enabled","commands":["reset_training_cooldowns","restore_training_member"],"idempotency":"request_id","targets":["all","participant"],"membership":"trusted_host_rpc_not_member_action"},"memberPiercing":{"minimumMechanismVersion":43,"schemaVersion":7,"protocolVersion":6,"mechanism":"piercing_projectile","input":"cast","collision":"continuous_swept_disk","wallTie":"wall_first","maxTotalContactHistory":128,"capacityField":"members.projectile_capacity","repeatClock":"logical_fixed_step","maxAttemptsPerIdentityPerStep":1,"catchUp":false,"damage":"shared_direct_contact","sourceExit":"clear_at_settling","reconnect":"preserve_flights_and_history","recovery":"financial_only"},
        "memberSkills":{"minimumMechanismVersion":44,"schemaVersion":7,"protocolVersion":6,"minimum":1,"maximum":crate::config::MAX_MEMBER_SKILLS,"source":"skill_and_additional_skills","order":"configuration","cooldowns":"per_skill_per_member","actionConstraint":"shared_preparation_and_recovery","hotSwap":false,"reconnect":"preserve_committed_cooldowns_and_plans","recovery":"financial_only"},
        "skillHpCost":{"minimumMechanismVersion":45,"schemaVersion":7,"protocolVersion":6,"modes":["fixed","current_hp_ratio"],"evaluation":"commit_current_hp_floor_then_clamp","minimumField":"min_hp_after_cost","costOwner":"atomic_skill_commit","damage":false,"privateFact":"action_committed.hp_cost","recovery":"financial_only"},
        "castMovement":{"minimumMechanismVersion":46,"schemaVersion":7,"protocolVersion":6,"phases":["preparation","recovery"],"policies":["free","slow","locked"],"slowSpeedBps":[1,9999],"lockedIntent":"resume_latest","commitStep":"preparation_movement_before_commit"},
        "directionCharge":{"minimumMechanismVersion":47,"schemaVersion":7,"protocolVersion":6,"mechanisms":["projectile","piercing_projectile"],"maxMs":crate::config::MAX_CHARGE_MS,"aimLock":"press","powerCurve":"linear","earlyRelease":"cancel","automaticAtMax":true,"costOwner":"atomic_skill_commit"},
        "chargeReaim":{"minimumMechanismVersion":48,"schemaVersion":7,"protocolVersion":6,"minimumIntervalMs":crate::config::MIN_AIM_INTERVAL_MS,"maxTurnDegPerSec":crate::config::MAX_AIM_TURN_DEG_PER_SEC,"overBudget":"reject","manualFinalDirection":true,"automaticDirection":"last_accepted"},
        "fixedChannel":{"minimumMechanismVersion":49,"schemaVersion":7,"protocolVersion":6,"mechanism":"projectile","maxMs":crate::config::MAX_CHANNEL_MS,"maxPulses":crate::config::MAX_CHANNEL_PULSES,"activeMovement":"locked","cooldownStart":"commit","pulseCostOwner":"channel"},
        "projectileRecast":{"minimumMechanismVersion":50,"schemaVersion":7,"protocolVersion":6,"mode":"redirect","maxPerAction":1,"maxOpenPerMember":1,"maxWindowMs":crate::config::MAX_RECAST_WINDOW_MS,"reconnect":"preserve_live_opportunity"},
        "skillFollowup":{"minimumMechanismVersion":51,"schemaVersion":7,"protocolVersion":6,"phase":"recovery","mechanism":"projectile","maxWindowMs":crate::config::MAX_FOLLOWUP_WINDOW_MS,"maxQueuedPerMember":1,"replacement":"latest_valid","costOwner":"commit"},
        "recoveryCancel":{"minimumMechanismVersion":52,"schemaVersion":7,"protocolVersion":6,"mechanism":"projectile","inputs":["cancel_action","cast"],"clock":"after_commit","replacement":"atomic_admission"},
        "basicFollowup":{"minimumMechanismVersion":53,"schemaVersion":7,"protocolVersion":6,"maxGraceMs":crate::config::MAX_FOLLOWUP_WINDOW_MS,"input":"attack","target":"frozen_identity","maxQueuedPerMember":1},
        "limits":{"participantsPerRuntime":8,"protocol4ParticipantsPerRuntime":2,"legacyParticipantsPerRuntime":1,"matchesPerRuntime":1,"jsonlLineBytes":JSONL_LINE_LIMIT}
    });
    info["lockedBasicTarget"] = serde_json::json!({"minimumMechanismVersion":54,"schemaVersion":7,"protocolVersion":6,"input":"attack_unit","target":"hostile_npc_generation","validation":["accept","promote","commit"],"flight":"ordinary_first_contact"});
    info["basicPursuit"] = serde_json::json!({"minimumMechanismVersion":55,"schemaVersion":7,"protocolVersion":6,"input":"attack_unit","maxDurationMs":crate::config::MAX_BASIC_PURSUIT_MS,"motion":"straight_swept_player","deadline":"exclusive","windupMovement":"locked"});
    info["autoBasicPursuit"] = serde_json::json!({"minimumMechanismVersion":56,"schemaVersion":7,"protocolVersion":6,"input":"attack","rangeField":"basic_pursuit.auto_max_range","targetOrder":["distance","id"]});
    info["memberLootDrop"] = serde_json::json!({"minimumMechanismVersion":57,"schemaVersion":7,"protocolVersion":6,"input":"drop_item","maxActive":crate::config::MAX_MEMBER_LOOT_DROPS,"position":"member_authoritative_position","scope":"net_acquired_loot","settlement":"net_loot"});
    info["memberItemUse"] = serde_json::json!({"minimumMechanismVersion":58,"schemaVersion":7,"protocolVersion":6,"input":"use_item","maxDefinitions":crate::config::MAX_MEMBER_ITEM_USES,"effect":"fixed_self_heal","quantity":1,"privateFact":"item_used","settlement":"net_loot"});
    info["memberItemCooldown"] = serde_json::json!({"minimumMechanismVersion":59,"schemaVersion":7,"protocolVersion":6,"input":"use_item","maxCooldownMs":crate::config::MAX_MEMBER_ITEM_COOLDOWN_MS,"owner":"member_and_item","privateView":"item_cooldowns"});
    info["memberItemSpirit"] = serde_json::json!({"minimumMechanismVersion":60,"schemaVersion":7,"protocolVersion":6,"input":"use_item","configField":"restore_spirit","privateFact":"spirit_restored","effect":"fixed_self_spirit"});
    info["memberItemCooldownGroup"] = serde_json::json!({"minimumMechanismVersion":61,"schemaVersion":7,"protocolVersion":6,"configField":"cooldown_group","owner":"member_and_group","privateView":"item_cooldowns"});
    info["memberItemDualRecovery"] = serde_json::json!({"minimumMechanismVersion":62,"schemaVersion":7,"protocolVersion":6,"input":"use_item","configFields":["heal_hp","restore_spirit"],"privateFacts":["hp_restored","spirit_restored"]});
    info["memberItemRatioRecovery"] = serde_json::json!({"minimumMechanismVersion":63,"schemaVersion":7,"protocolVersion":6,"input":"use_item","configFields":["heal_max_hp_bps","restore_max_spirit_bps"],"maxBps":crate::config::MAX_MEMBER_ITEM_RECOVERY_BPS,"rounding":"floor_then_clamp_to_deficit"});
    info["memberItemQualityRecovery"] = serde_json::json!({"minimumMechanismVersion":64,"schemaVersion":7,"protocolVersion":6,"configField":"quality_multiplier_bps","defaultBps":10_000,"maxBps":crate::config::MAX_MEMBER_ITEM_QUALITY_BPS,"identity":"item_id","rounding":"multiply_then_floor_then_clamp_to_deficit"});
    info["memberItemSelfStatus"] = serde_json::json!({"minimumMechanismVersion":65,"schemaVersion":7,"protocolVersion":6,"configField":"on_use_self_statuses","maxStatuses":2,"target":"self","commit":"positive_recovery_and_status_capacity"});
    info["memberItemStatusQuality"] = serde_json::json!({"minimumMechanismVersion":66,"schemaVersion":7,"protocolVersion":6,"configField":"on_use_self_status_overrides","fields":["modifier_bps","duration_ms"],"attributes":["atk","def"],"refresh":"same_instance_latest_successful_use"});
    info["memberItemDamageReduction"] = serde_json::json!({"minimumMechanismVersion":67,"schemaVersion":7,"protocolVersion":6,"attribute":"damage_reduction","maxBps":crate::config::MAX_DAMAGE_REDUCTION_BPS,"scope":"npc_direct_hit_on_member","order":["defense","damage_reduction","shield","hp"]});
    info["memberPeriodicOverhealShield"] = serde_json::json!({"minimumMechanismVersion":76,"schemaVersion":7,"protocolVersion":6,"configField":"overheal_shield","scope":"member_self_hot","maxConversionBps":10000});
    info["memberInstantSelfHeal"] = serde_json::json!({"minimumMechanismVersion":77,"schemaVersion":7,"protocolVersion":6,"configField":"self_heal","scope":"member_self_skill","optionalOverhealShield":true});
    info["memberAllyHeal"] = serde_json::json!({"minimumMechanismVersion":78,"schemaVersion":7,"protocolVersion":6,"mechanism":"ally_heal","input":"cast_unit","scope":"same_world_other_member","healKind":"ally_skill"});
    info["memberAllyHot"] = serde_json::json!({"minimumMechanismVersion":79,"schemaVersion":7,"protocolVersion":6,"mechanism":"ally_heal","input":"cast_unit","configField":"hot_status","sourceScoped":true});
    info["memberAllyShield"] = serde_json::json!({"minimumMechanismVersion":80,"schemaVersion":7,"protocolVersion":6,"mechanism":"ally_shield","input":"cast_unit","configField":"shield_status","sourceScoped":true});
    info["memberAllyOverhealShield"] = serde_json::json!({"minimumMechanismVersion":81,"schemaVersion":7,"protocolVersion":6,"mechanism":"ally_heal","configField":"heal.overheal_shield","sourceScoped":true,"maxConversionBps":10000});
    info["memberAllyHotOverhealShield"] = serde_json::json!({"minimumMechanismVersion":82,"schemaVersion":7,"protocolVersion":6,"mechanism":"ally_heal","configField":"hot_status","statusConfigField":"overheal_shield","sourceScoped":true});
    info["memberNpcSilence"] = serde_json::json!({"minimumMechanismVersion":83,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_control","attribute":"silence","binding":"npc_projectile_on_hit_target_statuses"});
    info["memberControlImmunity"] = serde_json::json!({"minimumMechanismVersion":84,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_control","attribute":"control_immunity","binding":"self_status_on_commit","rejects":"silence","reason":"IMMUNE"});
    info["memberControlDurationReduction"] = serde_json::json!({"minimumMechanismVersion":85,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"control_duration_reduction","binding":"self_status_on_commit","reduces":"silence","maximumReductionBps":crate::config::MAX_CONTROL_DURATION_REDUCTION_BPS});
    info["memberFirstControlImmunity"] = serde_json::json!({"minimumMechanismVersion":86,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_control","attribute":"first_control_immunity","binding":"self_status_on_commit","rejects":"silence","reason":"IMMUNE","removalReason":"depleted"});
    info["memberItemCleanse"] = serde_json::json!({"minimumMechanismVersion":87,"schemaVersion":7,"protocolVersion":6,"input":"use_item","configuration":"cleanse_self_statuses","target":"self","cleanses":"silence","removalReason":"cleansed"});
    info["memberNpcRoot"] = serde_json::json!({"minimumMechanismVersion":88,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_control","attribute":"root","binding":"npc_projectile_on_hit","blocks":["move","navigate","basic_pursuit"],"cleanse":"cleanse_self_statuses"});
    info["memberNpcStun"] = serde_json::json!({"minimumMechanismVersion":89,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_control","attribute":"stun","binding":"npc_projectile_on_hit","blocks":["movement","active_skill","basic_attack","extract_start"],"cleanse":"cleanse_self_statuses"});
    info["memberNpcMovementSlow"] = serde_json::json!({"minimumMechanismVersion":90,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"movement_slow","binding":"npc_projectile_on_hit","aggregation":"strongest","affects":["move","navigate","basic_pursuit"],"cleanse":"cleanse_self_statuses"});
    info["memberMovementHaste"] = serde_json::json!({"minimumMechanismVersion":91,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"movement_haste","binding":"self_status_on_commit","aggregation":"strongest","combinesWith":"movement_slow","affects":["move","navigate","basic_pursuit"]});
    info["memberCooldownReduction"] = serde_json::json!({"minimumMechanismVersion":92,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"cooldown_reduction","binding":"self_status_on_commit","maximumReductionBps":crate::config::MAX_COOLDOWN_REDUCTION_BPS,"deadline":"player_skills.ready_at_ms"});
    info["memberArmorPenetration"] = serde_json::json!({"minimumMechanismVersion":93,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"armor_penetration","binding":"self_status_on_commit","appliesTo":"member_direct_damage_against_npc","samplesAt":"impact"});
    info["memberCriticalChance"] = serde_json::json!({"minimumMechanismVersion":94,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"critical_chance","binding":"self_status_on_commit","appliesTo":"member_direct_damage_against_npc","multiplier":"params.critical_multiplier"});
    info["memberCriticalDamageBonus"] = serde_json::json!({"minimumMechanismVersion":95,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"critical_damage_bonus","binding":"self_status_on_commit","appliesTo":"member_critical_direct_damage_against_npc","maximumBonusBps":crate::config::MAX_CRITICAL_DAMAGE_BONUS_BPS});
    info["memberDodgePenalty"] = serde_json::json!({"minimumMechanismVersion":96,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"dodge_penalty","binding":"self_status_on_commit","appliesTo":"member_direct_contact_against_npc","maximumNpcDodgeBps":crate::config::MAX_DODGE_BPS});
    info["memberDodgeChance"] = serde_json::json!({"minimumMechanismVersion":97,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"dodge_chance","binding":"self_status_on_commit","appliesTo":["npc_projectile","npc_melee"],"maximumDodgeBps":crate::config::MAX_DODGE_BPS});
    info["memberAccuracy"] = serde_json::json!({"minimumMechanismVersion":98,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"accuracy","binding":"self_status_on_commit","appliesTo":"member_direct_contact_against_npc","maximumAccuracyBps":crate::config::MAX_ACCURACY_BPS});
    info["memberNpcHit"] = serde_json::json!({"minimumMechanismVersion":99,"schemaVersion":7,"protocolVersion":6,"field":"stats.hit","unit":"points","defaultHit":0,"maximumHit":1000000,"appliesTo":["npc_projectile","npc_melee"]});
    info["memberDamageImmunity"] = serde_json::json!({"minimumMechanismVersion":100,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_control","attribute":"damage_immunity","binding":"self_status_on_commit","appliesTo":["npc_projectile","npc_melee","npc_periodic_damage"]});
    info["memberNpcUnavoidable"] = serde_json::json!({"minimumMechanismVersion":101,"schemaVersion":7,"protocolVersion":6,"field":"combat.projectile.unavoidable","default":false,"sampledAt":"projectile_release","appliesTo":"npc_projectile","respectsDamageImmunity":true});
    info["memberUnavoidable"] = serde_json::json!({"minimumMechanismVersion":102,"schemaVersion":7,"protocolVersion":6,"field":"unavoidable","default":false,"sampledAt":"plan_materialization","appliesTo":["basic","projectile","unit_projectile","piercing_projectile","ground_circle","ground_field","line_field"]});
    info["directIgnoreArmor"] = serde_json::json!({"minimumMechanismVersion":103,"schemaVersion":7,"protocolVersion":6,"field":"ignore_armor","default":false,"sampledAt":"plan_materialization","appliesTo":["basic","projectile","unit_projectile","piercing_projectile","ground_circle","ground_field","line_field","npc_projectile"]});
    info["combatRules"] = serde_json::json!({"minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,"configuration":"combat_rules","damageKinds":["physical","elemental","true_damage"],"tables":["profiles","tier_multipliers","element_multipliers_bps","weapon_skill_multipliers_bps"],"compoundStatus":"timed_compound_modifier","maximumModifiersPerStatus":8,"maximumTriggerDefinitions":32,"maximumTriggersPerStep":128,"triggerRecursion":false,"sourceGrowthDerivation":"R5.1","temporaryAttributes":["max_hp","max_spirit","spd","element_resistance"],"periodicSampling":["source_snapshot","target_snapshot","source_live","target_live"],"triggerCounting":["contact","action","action_total"],"attributeCapsConfiguration":"pve.status_rules.attribute_caps_bps","statusOnlyItems":true,"shieldRecords":{"configuration":"combat_rules.shield_records","maximumDefinitions":16,"basis":["incoming","own_absorbed"],"endReasons":["depleted","expired","consumed"],"sharedPool":true,"recursiveRelease":false},"triggerConditions":["minimum_amount","owner_hp_at_most_bps","other_hp_at_most_bps","status"],"triggerStatusSource":"trigger_owner_only","ordinaryAttackSpeed":{"attribute":"attack_speed","sampledAt":"live_cooldown_progress","zeroRate":"pause_until_expiry","affects":["member_basic","npc_melee"],"affectsSkillCooldown":false}});
    info["combatRules"]["shieldRecords"]["grantSources"] = serde_json::json!([
        "status_binding",
        "instant_overheal",
        "hot_overheal",
        "item_overheal"
    ]);
    info["combatRules"]["shieldRecords"]["family"] = serde_json::json!({"configuration":"pve.status_definitions[].shield_family","scope":"same_source_and_target","replacement":"strictly_higher_remaining","transferable":false,"overheal":false});
    info["combatRules"]["shieldRecords"]["counterReasonsConfiguration"] =
        serde_json::json!("counter_on");
    info["combatRules"]["actionTerminalCounters"] = serde_json::json!({
        "counting":"action_total","when":"damage_dealt","effect":"add_status_stacks",
        "subject":"owner","counterScope":"owner","requiresExistingCounter":true
    });
    info["combatRules"]["castInterceptions"] = serde_json::json!({
        "configuration":"combat_rules.cast_interceptions",
        "maximumMarks":16,
        "maximumProfilesPerMark":16,
        "point":"validated_initial_commit_before_payment",
        "appliesTo":["member_skill","npc_projectile"],
        "onImmunity":"consume_mark_allow_without_followup",
        "outcomes":["interrupted","immune_allowed","allowed_with_followup"],
        "event":"cast_interception_resolved",
        "private":true,
        "newPlayerInput":false
    });
    info["combatRules"]["conditionalActivation"] = serde_json::json!({"fact":"unit_skill_committed","ownerStage":"trusted_closed_interval","bundleEffect":"apply_status_bundle","maximumStatusesPerEnd":2,"preflight":"both_ends_after_condition_consumption","failure":"no_grant_no_consumption_no_icd","resourceBudget":"before_condition_consumption","durationRefresh":"refresh_status_duration","refreshRequiresExistingOwnedNumericStatus":true,"refreshPreserves":["identity","layers","applied_time","source","window_record"],"newPlayerInput":false});
    info["combatRules"]["stackWindows"] = serde_json::json!({"configuration":["StatusDefinition.base_modifiers","StatusDefinition.stack_counter","CombatTrigger.stack_change"],"effect":"add_status_stacks","fact":"status_stacks_reached","maximumCounter":128,"maximumMovementCounter":32,"initialZero":true,"consumeToZeroKeepsCarrier":true,"adjustmentRequiresExistingOwnedSource":true,"defaultAdjustmentRefresh":false,"fixedAndLayeredShareIdentityAndWindow":true,"thresholdLeaves":["dispel_statuses","restore_spirit","apply_self_status"],"thresholdRecursion":false});
    info["combatRules"]["sourceGates"] = serde_json::json!({"configuration":["CombatTrigger.action_filter","CombatTrigger.owner_skill","ContactReactionRule.source_requirement","CombatTrigger.maximum_per_action"],"classification":["native_basic","native_skill","unknown_fail_closed"],"selection":["any","only","except"],"qualification":"live_equipped_alternatives_and_trusted_canonical_mastery_row","maximumAttemptsPerAction":8,"defaultAttemptsPerAction":1,"preservedBy":["projectile","area","channel","periodic_status","transfer","action_total"],"publicSourceOverride":false});
    info["combatRules"]["statusEnds"] = serde_json::json!({"configuration":"combat_rules.status_ends","maximumDefinitions":16,"leavesPerEnd":2,"maximumPendingEnds":1024,"defaultReleasesPerStep":32,"maximumReleasesPerStep":128,"derivedLineage":"private_preserved_through_ticks_and_transfer","recursive":false,"delayedConfiguration":"combat_rules.delayed_damage","delayedClock":"single_native_periodic_deadline","missingHp":"live_target","sourceSampling":["source_snapshot","source_live"],"events":["status_end_resolved","status_ends_suppressed"]});
    info["combatRules"]["statusRetaliations"] = serde_json::json!({"configuration":"combat_rules.status_retaliations","carrier":"member_self_status","maximumDefinitions":16,"maximumSourcesPerInstance":32,"damage":"ordinary_direct_hp_or_shield","cooldown":"per_source_generation_on_success","naturalExpiry":"only_successful_live_hostile_sources","history":"private_world_state","newPlayerInput":false});
    info["combatRules"]["controlPolicies"] = serde_json::json!({"configuration":"combat_rules.control_policies","target":"profile","maximumProfiles":64,"maximumMappings":128,"durationCap":"before_actual_kind_scaling","replacement":"whole_numeric_instance","replacementDuration":"original_request","transfer":"reject_if_replacement_or_remaining_over_cap","basicIntervalDelay":{"attribute":"basic_interval_delay","formula":"base_interval*(10000+delay)/attack_speed_factor","scope":["member_basic","npc_melee"],"rounding":"native_work_with_fraction_credit","sample":"live"},"compoundMovementSlow":true});
    info["combatRules"]["statusOperations"] = serde_json::json!({"configuration":"combat_rules.status_policies","effects":["dispel_statuses","transfer_statuses"],"maximumPolicies":128,"maximumSelected":16,"transfer":"preserve_instance_source_stacks_expiry","transferKinds":["timed_attribute_modifier","timed_compound_modifier","absorption_shield","periodic_damage","periodic_heal","timed_control"],"failure":"atomic","blocks":"dispellable_disposition"});
    info["combatRules"]["statusOperations"]["shieldTransfer"] = serde_json::json!({"balance":"preserve","record":"preserve","destinationBudget":"final_live_shared_pool","partialTransfer":false,"regrant":false});
    info["combatRules"]["statusOperations"]["periodicTransfer"] = serde_json::json!({"schedule":"preserve","source":"preserve","snapshot":"preserve","liveTarget":"new_holder","dotRelation":"hostile_to_original_source","hotPermission":"explicit_transfer_policy"});
    info["combatRules"]["chainDamage"] = serde_json::json!({"effect":"chain_damage","maximumHops":8,"selection":"nearest_unvisited_hostile_with_line_of_sight","budget":"combat_rules.max_chain_hits_per_step","defaultBudget":32,"maximumBudget":128,"sample":"post_consumption_once","periodicSourceFacts":true,"derivedTriggers":false,"derivedRecording":false,"delay":false,"event":"chain_damage_resolved"});
    info["combatRules"]["postConsumption"] = serde_json::json!({"fact":"contact_status_consumed","predicate":"contact_consumption","amount":"enhanced_pre_defense_raw","derived":false,"forwardChain":"positive_dot_product_per_hop","cooldownEffect":"reduce_skill_cooldowns","maximumSkills":8,"basis":"current_remaining","event":"skill_cooldowns_reduced","private":"owner_only"});
    info["combatRules"]["contactReactions"] = serde_json::json!({"configuration":"combat_rules.contact_reactions","when":["direct_contact","hp_damage"],"attacker":["source_only","any_hostile"],"sampling":"original_source_power_live","capture":"before_primary_damage","execute":"after_primary_before_hit_bindings","maximumPerHit":4,"defaultPerHit":1,"maximumPerStep":128,"defaultPerStep":32,"derivedTriggers":false,"event":"contact_reaction_resolved"});
    info["combatRules"]["finishers"] = serde_json::json!({"effect":"finish_other","comparison":["below","at_most"],"modes":["execute","power_damage"],"selector":"target_combat_profile","sampling":"post_consumption_source_power","executeBudget":"current_hp_plus_shield","nativeDeathGuard":true,"periodic":false,"recursive":false,"event":"finisher_resolved"});
    info["combatRules"]["finisherFollowups"] = serde_json::json!({"fact":"finisher_kill","selector":"finisher","stage":"synchronous_after_primary_rules","leafEffects":["heal_from_damage","restore_spirit","apply_self_status","reduce_skill_cooldowns","chain_damage"],"recursive":false,"delayedReplay":false,"spiritRatio":"restore_spirit.max_spirit_bps"});
    info["combatRules"]["actionEnhancement"] = serde_json::json!({"sampling":["contact","action"],"snapshot":"first_eligible_contact","key":"source_entity_generation_action_and_definition","snapshotBudget":[1,1024],"budgetProtection":"until_active_actions_drained","thresholdBonus":"finish_other.rule.consumed_status_bonus","resultFilter":"finisher.enhanced","wireEvents":"existing","recursive":false});
    info["combatRules"]["contactStatus"] = serde_json::json!({"configuration":"AttackRule.contact_status","subjects":["owner","other"],"source":"attacker_owned","sampledAt":"after_qualification_and_dodge_before_damage","periodic":false,"recursiveDamage":false,"penetrationCap":"AttackRule.armor_penetration_cap_bps","rawRounding":"u128_combined_floor_then_u32","consumption":"native_status_lifetime"});
    info["combatRules"]["hitStatus"] = serde_json::json!({"fact":"direct_hit","after":"damage_and_on_hit_bindings","zeroHpDamage":true,"periodic":false,"effect":"apply_other_status","target":"hostile_other","carriers":["timed_attribute_modifier","timed_compound_modifier","timed_control"],"failure":"final_preview_atomic","immunityFailureConsumesFirst":false,"movementSlowStack":{"maximumStacks":32,"aggregation":"strongest_per_instance_stack_product","minimumFactorBps":1}});
    info["combatRules"]["allySkillCommitted"] = serde_json::json!({"fact":"ally_skill_committed","owner":"caster","other":"committed_ally","amount":0,"after":"primary_effect","mechanisms":["ally_heal","ally_shield"],"periodicReplay":false,"primaryRollbackOnTriggerFailure":false});
    info["combatRules"]["statusOperations"]["lifecycleTransfer"] = serde_json::json!({"window":"preserve_record_and_maximum_snapshot","cleanse":"current_holder","sinceApplication":"original_applied_at_ms","schedule":"preserve","endOnTransfer":false,"privateObservers":"original_source_and_current_holder"});
    info["combatRules"]["statusOperations"]["controlTransfer"] = serde_json::json!({"duration":"preserve","hardControlRelation":"hostile_to_original_source","hardControlSource":"eligible","immunityPreflight":"final_preview_reject_without_consumption","onSuccess":"native_control_consequences","spiritLockTarget":"participant","immunityBenefit":"explicit_transfer_policy"});
    info["combatRules"]["deathGuard"] = serde_json::json!({"configuration":"combat_rules.death_guards","perEntityGeneration":1,"stage":"after_shields_before_hp","healing":"target_max_hp_then_healing_received","forcedTerminal":false});
    info["combatRules"]["deathGuard"]["statusWindows"] = serde_json::json!({"configuration":"combat_rules.status_death_guards","maximumDefinitions":16,"cooldown":"per_entity_generation_and_group","activeSelfStatus":true,"event":"death_guard_activated"});
    info["combatRules"]["skillCooldownsByMastery"] = serde_json::json!({"configuration":"combat_rules.skill_cooldowns_by_mastery","maximumRowsPerSkill":16,"timing":"commit","statusReduction":"multiplicative"});
    info["combatRules"]["allyRecoveryByMastery"] = serde_json::json!({"configuration":"combat_rules.ally_recovery_by_mastery","maximumRowsPerSkill":16,"target":"explicit_member","selfTargetConfiguration":"combat_rules.ally_self_target_skills","pendingSelfField":"ally_target_is_self","splashCenter":"primary_at_commit","hotBudget":"frozen_raw_heal","hotExpiryField":"tick_on_expiry","shieldBasis":"immediate_overheal","markOwners":["caster","primary_target"],"markAtCap":"consume_then_restore_eligible_members"});
    info["combatRules"]["triggerAggregation"] = serde_json::json!({"counterScopes":["owner","target"],"defaultTargetBudget":32,"maximumTargetBudget":128,"defaultPendingTotals":256,"maximumPendingTotals":1024,"maximumAmount":9007199254740991_u64,"completion":"last_native_plan_at_step_tail","periodic":false,"sourceGenerationIdentity":true});
    info["combatRules"]["periodicRecovery"] = serde_json::json!({"configuration":"combat_rules.periodic_recovery","maximumRules":16,"carrier":"hot","resources":["hp","spirit"],"conversionWhen":["full_before_tick","any_excess"],"order":["hp","base_spirit","converted_spirit","remaining_shield","cleanse"],"excessBudget":"single","lifetime":"native_hot"});
    info["combatRules"]["statusCleanses"] = serde_json::json!({"configuration":"combat_rules.status_cleanses","maximumRules":16,"phases":["hot_tick","expired"],"scope":["all","since_application"],"target":"holder","maximumPerAction":16,"maximumPerStep":128,"lifetime":"native_status","transfer":true,"privateResult":"count_only"});
    info["combatRules"]["compoundFlags"] = serde_json::json!({"configuration":"pve.status_definitions[].flags","maximumFlags":3,"flags":["silence","root","stun","control_immunity","damage_immunity","first_control_immunity"],"lifetime":"single_status_instance","hostileAndImmunityMix":false,"transfer":true,"window":false});
    info["combatRules"]["statusWindows"] = serde_json::json!({"configuration":"combat_rules.status_windows","maximumDefinitions":16,"maximumReleasesPerStep":128,"recordBasis":["none","damage_dealt","damage_taken","shield_absorbed"],"maximumSampling":["snapshot","live"],"recover":["hp","spirit"],"lifetime":"status_instance","transfer":true,"derivedRecording":false});
    info["memberSkillPower"] = serde_json::json!({"minimumMechanismVersion":104,"schemaVersion":7,"protocolVersion":6,"mechanism":"timed_attribute_modifier","attribute":"skill_power_bonus","binding":"self_status_on_commit","sampledAt":"skill_budget_materialization","base":"effective_atk","maximumModifierBps":100000});
    info["memberOverhealShield"] = serde_json::json!({"minimumMechanismVersion":75,"schemaVersion":7,"protocolVersion":6,"configField":"overheal_shield","scope":"member_item_hp","maxConversionBps":10000});
    info["memberNpcSpiritRecoveryLock"] = serde_json::json!({"minimumMechanismVersion":74,"schemaVersion":7,"protocolVersion":6,"attribute":"spirit_recovery_lock","binding":"npc_projectile_on_hit_target_statuses"});
    info["memberSpiritRecoveryLock"] = serde_json::json!({"minimumMechanismVersion":73,"schemaVersion":7,"protocolVersion":6,"attribute":"spirit_recovery_lock","modifierBps":-10_000,"sources":["natural_regen","basic_hit","recovery_orb","recovery_item"]});
    info["memberRecoveryStrength"] = serde_json::json!({"minimumMechanismVersion":72,"schemaVersion":7,"protocolVersion":6,"attribute":"recovery_strength","scope":"member_natural_regen","duplicatePolicies":["refresh","ignore"],"aggregation":"sum_bps_floor_zero"});
    info["memberRecoveryOrb"] = serde_json::json!({"minimumMechanismVersion":71,"schemaVersion":7,"protocolVersion":6,"configField":"pve.additional_templates.<id>.recovery_orb","maxBps":10_000,"amountSnapshot":"kill","pickup":"owner_only"});
    info["memberItemHealingReceived"] = serde_json::json!({"minimumMechanismVersion":70,"schemaVersion":7,"protocolVersion":6,"configField":"apply_healing_received","default":false,"scope":"member_item_hp","rounding":"quality_then_floor_then_healing_received_then_floor_then_clamp_to_deficit"});
    info["memberPeriodicHealingReceived"] = serde_json::json!({"minimumMechanismVersion":69,"schemaVersion":7,"protocolVersion":6,"attribute":"healing_received","scope":"member_self_hot","clock":"executed_at_ms"});
    info["memberPeriodicDamageReduction"] = serde_json::json!({"minimumMechanismVersion":68,"schemaVersion":7,"protocolVersion":6,"attribute":"damage_reduction","scope":"npc_physical_dot_on_member","clock":"executed_at_ms","order":["defense","damage_reduction","shield","hp"]});
    info["castLifecycle"] = serde_json::json!({"minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "movementPhases":["preparation","active","recovery"],"interruptPolicy":"interrupt_on_new_move","movementThresholds":"per_skill_frozen_phase_anchor",
        "mobileWindupLimit":"cast_limits.mobile_fixed_windup_max_ms","earlyRelease":"queue_until_min","releaseWindup":"release_windup_ms","timing":"ceil_each_charge_phase_to_step_without_power_increase",
        "explicitPulseCount":[1,crate::config::MAX_CHANNEL_PULSES],"pulseDeadlines":"absolute_from_commit","recoveryBudgets":"commit_snapshot_equal_share","remainderPolicies":["earliest","final"],
        "recoveryPulseKind":"channel_recovery_pulse","channelKinds":["projectile","self_recovery"],"activeMovementDefault":"locked","recoveryCancellation":["by_move","by_explicit_action"],
        "resourceLimits":{"maximum":crate::config::MAX_BASE_RESOURCE,"maximumSpiritCost":crate::config::MAX_SPIRIT_COST,"legacyDefault":1000000,"fields":["max_hp","max_spirit","max_spirit_cost"]}});

    info["clientMethods"]
        .as_array_mut()
        .unwrap()
        .extend(["set_skill_loadout", "set_equipped_weapon"].map(serde_json::Value::from));
    info["weaponEquipment"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"combat_rules.weapon_catalog","trustedSeatField":"weapon",
        "command":"set_equipped_weapon","privateProjection":"self.equipment",
        "invalidMorph":"family_base","committedBasic":"wait_for_terminal",
        "cas":"per_member","processRecovery":"receipts_and_financial_only"
    });
    info["npcMovingEnterArea"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"combat_rules.npc_enter_areas","capacityField":"members.area_capacity",
        "publication":"areas","mechanism":"moving_enter","contacts":"relative_swept_enter",
        "damage":"target_max_hp_then_reduction_then_shield",
        "displacement":"swept_movement_collision","invalidSource":"configured_hits_keep_released_identity",
        "retainedStatusField":"retained_hit_statuses"
    });
    info["npcGroundField"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"combat_rules.npc_ground_fields","capacityField":"members.area_capacity",
        "publication":"areas","mechanism":"npc_ground_field","geometry":"fixed_circle",
        "entryStatuses":"npc_projectile_on_hit_target_statuses_once_per_target",
        "firstPulse":"after_interval","finalPulse":"at_expiry","invalidSource":"configured_hits_keep_released_identity"
    });
    info["npcGroundStrike"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"combat_rules.npc_ground_strikes","capacityField":"members.area_capacity",
        "publication":"areas","mechanism":"npc_ground_strike","geometry":"fixed_circle",
        "target":"nearest_eligible_member_at_commit",
        "hitStatusChanceField":"hit_status_chance_bps","invalidSource":"cancel_warning"
    });
    info["memberGroundTrap"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"combat_rules.member_ground_traps","capacityField":"members.area_capacity",
        "input":"cast_ground","publication":"areas","mechanism":"member_ground_trap",
        "activationDelayField":"warning_ms","contacts":"hostile_entry_at_fixed_step",
        "history":"per_target_generation","sourceInvalid":"end"
    });
    info["memberCastAnchor"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"combat_rules.member_cast_anchors","capacityField":"members.area_capacity",
        "createInput":"cast_ground","useInput":"cast_at_anchor","publication":"areas",
        "mechanism":"member_cast_anchor","delivery":["projectile_origin","circle_center"],
        "ownership":"same_live_member","replacement":"same_owner_and_producer_skill",
        "recastInput":"recast_anchor","recastConfigField":"recast",
        "returnInput":"return_anchor","endInput":"end_anchor",
        "endEvent":"anchor_recast_ended","recastBehaviorConfigField":"behaviors",
        "recastEvent":"anchor_relocated","recastPublicationField":"anchor_recast",
        "followingFieldsConfigField":"following_field_skill_ids",
        "attachedFieldPublicationField":"attached_anchor_id",
        "maxRecastCount":crate::config::MAX_ANCHOR_RECAST_COUNT
    });
    info["combatRules"]["unitHitStatusesByMastery"] =
        serde_json::json!("combat_rules.unit_hit_statuses_by_mastery");
    info["combatRules"]["statusDamageTakenBps"] =
        serde_json::json!("combat_rules.status_damage_taken_bps");
    info["checkpointZones"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"members.checkpoint_schedules",
        "maxSchedules":crate::config::checkpoint_zones::MAX_CHECKPOINT_SCHEDULES,
        "maxWindowsPerSchedule":crate::config::checkpoint_zones::MAX_CHECKPOINT_WINDOWS,
        "publicProjection":"checkpoint_zones","privateProjection":"self.checkpoint_progress",
        "privateEvent":"checkpoint_reached","geometry":"fixed_circle_at_step_end",
        "qualification":["reach","avoid"],"avoidProjection":"avoid","avoidEvent":"checkpoint_avoided",
        "counterActivation":["on_reach","manual","first_contact"],"firstContactProjection":"on_contact",
        "clock":"world_active_relative_end_exclusive","identity":"one_receipt_per_member_and_window"
    });
    info["matchMode"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"match_mode","maxStages":crate::config::match_mode::MAX_STAGES,
        "objectives":["wait","checkpoint","clear_targets","survive","waves","kill_quota","contest"],
        "maxWavesPerStage":crate::config::match_mode::MAX_WAVES,
        "checkpointInput":"complete_mode_checkpoint","privateProjection":"self.mode",
        "outcomes":["win","loss","draw"],"settlement":"result_settlement",
        "maxRespawnsPerMember":crate::config::match_mode::MAX_RESPAWNS
    });
    info["memberConflict"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"members.member_conflict","capacity":[2,8],
        "mode":"extraction","credit":"native_npc_only"
    });
    info["zeroEntryCost"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"params.entry_years","admission":"revisioned_asset_commit"
    });
    info["trainingTargets"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"members.training_targets","runtimeConfig":"members.training_runtime",
        "inputs":["set_owned_targets","clear_owned_targets","reset_training_metrics"],
        "maxPresetsPerSeat":crate::config::training_targets::MAX_PRESETS,
        "maxTargetsPerPreset":crate::config::training_targets::MAX_TARGETS_PER_PRESET,
        "ownerField":"training_owner_id","scope":"seat_owned_targets","privateView":"self.training"
    });
    info["encounterProgress"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"encounter_progress","view":"encounter_progress",
        "source":"credited_native_npc_defeat","scope":"world","maxTiers":8
    });
    info["zoneExposure"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"zone_exposure","action":"use_exposure_node",
        "privateView":"self.zone_exposure","attributes":["atk","def"],
        "maximumBands":crate::config::zone_exposure::MAX_BANDS,
        "maximumNodes":crate::config::zone_exposure::MAX_NODES,
        "maximumStacks":crate::config::zone_exposure::MAX_STACKS
    });
    info["contestedEquipment"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"contested_equipment","action":"claim_world_equipment",
        "publicView":"contested_equipment","privateProfile":"world_weapon_profile",
        "maximumNodes":crate::config::contested_equipment::MAX_CLAIMS,
        "settlement":"extraction_only_instance","reconnect":"same_world"
    });
    info["extractionLifecycle"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"extraction_lifecycle","clock":"world_activation_relative",
        "pressureView":"extraction_pressure","broadcastMemberView":"extraction_channel",
        "broadcastEvent":"extraction_channel_changed"
    });
    info["spectators"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"members.spectators","maxSlots":crate::config::two_members::MAX_SPECTATORS,
        "maxRecentEvents":crate::config::two_members::MAX_SPECTATOR_EVENTS,
        "hostCommand":"spectate","publication":"spectator_publication_v6",
        "terminalRetention":"terminal_retention_ms"
    });
    info["settlementProgression"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"settlement_progression","rewardConfigField":"settlement_rewards",
        "admissionField":"clock","summaryField":"progression",
        "damageMetric":"direct_member_to_native_npc_hp_loss",
        "damageRounding":["accumulated","per_contact"],
        "earningScale":"trusted_asset_bps_by_active_basic_profile",
        "rewardAdjustments":["scale","add_original","defer"],
        "rewardTarget":"trusted_equipped_weapon_instance",
        "thresholdProgress":"post_settlement_total_at_current_stage",
        "stackFlows":"picked_up_consumed_released_relic_lost",
        "storage":["stack_quantity","instance_count","shared_slots"],
        "conversion":"first_matching_post_reward_threshold",
        "conversionModes":["first","ordered"],
        "partialEntryCharge":"positive_balance_capped_by_standard_fee",
        "rewardEntryCostBases":["standard","charged"],
        "terminalClockCost":"paired_remaining_and_elapsed",
        "region":"compare_and_swap"
    });
    info["characterBuild"] = serde_json::json!({"minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,"trustedSeatField":"combat_build","configField":"combat_rules.build_rules","growthInputs":"zero_based_model_indices_and_raw_points","conversion":"member_true_points_only_no_realm_multiplier","growth":"rank_base_plus_quality_level_mastery_then_external","finalPower":"one_floor_skill_bonus_and_power_points","selectedStatusSources":"status_power_sources","selectedDerivedSource":"source_skill_id","privateProjection":"skill_powers","inMatchMutation":false,"processRecovery":"financial_only"});
    info["castLifecycle"]["immediateSelfCommit"] = serde_json::json!({
        "field":"commit_on_accept","mechanism":"self_status","windupMs":0,
        "channel":false,"charge":false,"default":false,"newPlayerInput":false
    });
    info["paidResourceEffects"] = serde_json::json!({"minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,"fact":"skill_cost_committed","effect":"restore_paid_spirit","basis":"actual_initial_spirit_cost_after_full_payment","ratioBps":[1,10000],"maximumInitialCost":8000000,"periodicAndRecastPayments":false,"recovery":"live_gap_cap_and_lock","event":"paid_spirit_restored","zeroPolicies":["keep","remove"],"defaultZeroPolicy":"keep","removeRequiresPositiveInitial":true,"exhaustion":"consumed_end_after_current_return","publicZeroPolicyOverride":false});
    info["resourceSteal"] = serde_json::json!({"minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,"field":"resource_steal","budget":"target_current_resources_before_effects","hpPolicies":["direct_transfer","damage"],"remainder":"final","triggers":["direct_hit","basic_hit"],"result":"private_resource_stolen","lifestealAttribute":"life_steal","lifestealCapDefault":10000,"statusBonus":{"modes":["multiply_base","add_current_bps"],"qualification":"live_equipped_and_trusted_mastery_closed_interval","source":"active_owned_status","sampling":"before_condition_consumption","channel":"frozen_total_final_remainder","rounding":"wide_integer_once","zeroBaseRemainsDisabled":true}});
    info["resourceCommit"] = serde_json::json!({"minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,"field":"resource_commit","reserveAt":["accept","none"],"consumeAt":"commit","cooldownStartsAt":"commit","refundBeforeCommit":"full","reservation":"private_pending_snapshot","queue":"reserve_on_real_preparation","externalChange":"revalidate_or_reject_atomically","processRecovery":"financial_only"});
    info["unitEffects"] = serde_json::json!({"minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,"mechanism":"unit_effect","binding":"strict_entity_identity","projectiles":false,"channelDamageDistribution":"total","remainder":"final","friendlyChannel":"ally_heal_with_recovery","successGates":4,"legalTick":"qualified_target_not_positive_result","effectOrder":"after_movement_and_control","targetPlanVisibility":"owner_only"});
    info["worldGeometry"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"pve.world_geometry","maxBlockers":64,"maxVertices":16,
        "isometricGrid":"pve.isometric_grid",
        "channels":["movement","projectiles","vision"],
        "activeWindow":"world_elapsed_start_inclusive_end_exclusive",
        "occupiedClosure":"deferred","publicationField":"active_blockers"
    });
    info["memberSummons"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"combat_rules.member_summons","maxActivePerMember":4,
        "skillMechanism":"self_status","attack":"stationary_melee",
        "publicationField":"monsters[].summon","owner":"participant_entity_ref",
        "skillCopy":{"field":"skill_copy","whitelist":["projectile","unit_projectile","unit_effect","ground_circle"],
                     "cooldown":"summon_skill_and_owner_generation","event":"summon_skill_copied"},
        "attributeSnapshot":["atk","def","max_hp"],"deathBurst":"lethal_only"
    });
    info["basicWave"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"basic.wave","requiredConfig":"basic.settle_on_end",
        "geometry":["directional","radial"],"maxTargets":16,"maxTravelMs":60000,
        "targetSelection":"at_release","publicationField":"basic_waves",
        "behaviors":["push","alternate"],"nextPhaseField":"self.basic_wave_phase"
    });
    info["basicWorldFields"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"basic.wave.field","capacityField":"members.basic_field_capacity","maxActive":12,
        "triggers":["primary_contact","landing"],"powerSampling":["live","frozen"],
        "publication":"areas","mechanism":"basic_field","expiryTick":"inclusive","actionReward":"detached"
    });
    info["basicBeam"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"basic.beam","publicationField":"basic_beams",
        "actions":["start_attack_beam","aim_attack_beam","end_attack_beam"],
        "geometry":"finite_rectangle","maxTargetsPerTick":8,"openEnded":true,
        "movementIntent":"retain_while_locked","disconnect":"hide_then_end_next_step"
    });
    info["basicProjectileFollowup"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"basic.projectile_followup","patterns":["companion","volley","seal_recall","ricochet"],
        "maxStored":8,"segmentProjection":"projectiles[].basic_segment",
        "progressProjection":"self.basic_projectile_progress","sealProjection":"members[].basic_seals",
        "capacity":"reserved_at_release","returnToOwner":"optional_noncombat_flight"
    });
    info["basicImpact"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"basic.impact","requiredConfig":"basic.settle_on_end",
        "timing":["hitscan","fixed_delay"],"maxDelayMs":60000,"maxRefractionTargets":1,
        "publicationField":"basic_impacts","healKind":"basic_attack",
        "hitscanFollowups":["alternating_refraction","target_burst"],"maxSegmentedContacts":8
    });
    info["basicProfiles"] = serde_json::json!({
        "minimumMechanismVersion":105,"schemaVersion":7,"protocolVersion":6,
        "configField":"combat_rules.basic_profiles","trustedSeatField":"basic_profile",
        "projectionField":"self.basic_profile","maxProfiles":crate::config::MAX_BASIC_PROFILES,"frozenAt":"create",
        "effectPhases":["on_release_self_status","on_end_damage_self_status","on_end_damage_target_status","on_end_damage_self_heal_max_hp_bps"],
        "maxTargetStatusChoices":8
    });
    info["memberSkills"]["hotSwap"] = serde_json::json!(true);
    info["memberSkills"]["trustedLoadouts"] = serde_json::json!({"minimumMechanismVersion":105,"library":"skill_library","maxDefinitions":crate::config::MAX_SKILL_DEFINITIONS,"maxOwned":64,"maxEquipped":8,"minimumEquipped":0,"grants":"trusted_host_only","control":"set_skill_loadout","revision":"compare_and_swap","maxOperations":128,"idempotency":"no_eviction","revocation":"cancel_uncommitted_keep_committed_and_debts","recovery":"financial_only"});

    info
}
