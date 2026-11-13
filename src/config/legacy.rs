//! Frozen interpretation of the already shipped v1 profiles. New configurations use schema v2.
use super::catalog::rules_hash;
use serde_json::{Value, json};
pub(super) fn upgrade(mut c: Value) -> Result<Value, &'static str> {
    let accepted = [
        "5051f18fd5ef26a4e88df4348bd73811cf909733c0a2fbc1c1421fa535ad73cf",
        "27692ca979585bb64f3d08e8cf506e76edbcf5d79eed2bb1ebc419611c524bfc",
        "da0218174cf0e72a4b9623dc4b0cebdb91508447e16a921a052ea2802a131f48",
    ];
    if !rules_hash(c.clone()).is_some_and(|h| accepted.contains(&h.as_str())) {
        return Err("LEGACY_CONFIG_IMMUTABLE_USE_SCHEMA_V2");
    }
    c["schema_version"] = json!(2);
    c["mechanism_version"] = json!(1);
    c["params"] =
        serde_json::from_str(crate::test_data::fixture!("src/config/legacy-v1.json")).unwrap();
    if !c.get("pve").is_some_and(Value::is_object) {
        c["params"]["basic_spirit_on_hit"] = json!(0);
    }
    c["basic"]["mechanism"] = json!("projectile");
    c["skill"]["mechanism"] = json!("projectile");
    if c.get("pve").is_some_and(Value::is_object) {
        let p = c["pve"].as_object_mut().unwrap();
        let t = p.remove("monster_template").unwrap();
        let skills = p.remove("skills").unwrap();
        let a = &skills[t["skillSlots"][0]["skillId"].as_str().unwrap()];
        let h = &skills[t["skillSlots"][1]["skillId"].as_str().unwrap()];
        let r = &a["runtimes"][0];
        p.insert("template_id".into(), t["id"].clone());
        p.insert(
            "drop_table".into(),
            json!({"guaranteed":[],"random":t["dropTable"]["random"]}),
        );
        p.insert("combat".into(),json!({"radius":14.0,"attack_interval_ms":1200,"aggro_range":t["aiProfile"]["aggroRadius"],"melee_range":t["aiProfile"]["attackRange"],"cast_chance":t["aiProfile"]["skillCastChance"],"return_distance":32.0,"lifesteal_ratio":0.08,"mark_max":3,"mark_duration_ms":24000,
   "projectile":{"mechanism":"projectile","id":a["id"],"cost":a["spiritCost"],"windup_ms":a["castLifecycle"]["preparation"]["durationMs"],"cooldown_ms":(a["cooldown"].as_f64().unwrap()*1000.0).round() as u64,"recovery_ms":a["castLifecycle"]["recovery"]["durationMs"],"range":r["motion"]["maxDistance"],"speed":r["motion"]["speed"],"radius":r["shape"]["radius"],"ttl_ms":r["lifecycle"]["durationTicks"].as_u64().unwrap()*30,"damage_bps":10000},
   "heal":{"mechanism":"budget_heal_channel","id":h["id"],"cost":h["spiritCost"],"windup_ms":h["castLifecycle"]["preparation"]["durationMs"],"cooldown_ms":(h["cooldown"].as_f64().unwrap()*1000.0).round() as u64,"recovery_ms":h["castLifecycle"]["recovery"]["durationMs"],"hp_threshold":t["skillSlots"][1]["condition"]["value"],"heal_bps":h["factors"][0]["params"][0],"interval_ms":h["castLifecycle"]["active"]["tickIntervalMs"],"ticks":h["channelExecution"]["tickCount"]}}));
        p.get_mut("stats")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("hit");
        for item in p
            .get_mut("items")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            *item = json!({"name":item["name"],"stackLimit":item["stackLimit"]});
        }
        if let Some(t) = p
            .get_mut("treasure_rules")
            .and_then(|v| v.get_mut("template"))
        {
            *t = json!({"id":t["id"],"name":t["name"],"respawn_delay_ms":t["respawnDelayMs"],"mechanism":"immediate_ground_loot"});
        }
    }
    Ok(c)
}
