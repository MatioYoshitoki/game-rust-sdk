# Raw character builds and selected skill power (schema 7 / mechanism 105 / protocol 6)

The trusted host may put combat_build on a Create participant seat. Node forwards saved raw points and growth indices. This field is never a player Action, and no in-match growth mutation RPC exists. Invalid models, indices, point limits or skill references fail before loading/admission. Create identity includes the full build, so recovery cannot replace it. Process recovery closes and compensates; it never resumes a battle.

~~~json
{"combat_build":{"power_points":20,"skill_power_points":10,"element_resistance_points":60,"powers":{"growth-strike":{"model_id":"growth","rank":0,"quality":1,"level":1,"mastery":1}}}}
~~~

Growth indices are zero based offsets into frozen configuration arrays. Models, conversions, coefficients and limits live in combat_rules.build_rules:

~~~json
{"conversion":{"hp_bps":10000,"spirit_bps":10000,"atk_bps":15000,"def_bps":5000,"spd_bps":10000,"element_resistance_bps":10000},"maximum_input_points":10000,"power_models":{"growth":{"rank_base":[30,90],"quality_add_bps":[0,2000],"level_add_bps":[0,3000],"mastery_add_bps":[0,5000],"external_bps":12500,"maximum_power":1000000000}}}
~~~

Player HP/SP/ATK/DEF/SPD use the existing trusted *_points load fields. With explicit build rules, the result is max(1, floor(points * conversion_bps / 10000)); player tier/stage does not multiply it again. HP/SP limits apply to the converted resource, including zero-point/minimum-one and large-raw/smaller-conversion cases. A rejected load produces no admission debit for that member. Without build rules, legacy point semantics and zero-HP rejection are preserved. NPC base stats remain independent. A configured NPC combat profile may have its own build; member builds must come from trusted seats.

A model first floors rank_base[rank] * max(0,10000+quality_add_bps[quality]+level_add_bps[level]+mastery_add_bps[mastery]) / 10000. It then applies the external multiplier, floors and clamps to the model maximum. Final skill power is floor(grown * skill_bonus_bps * (10000+power_points*100) / 100000000), clamped to u32. Permanent skill_power_points*100 and the live skill_power_bonus status share one aggregate cap and floor. A missing model entry falls back to the skill's static base or live ATK. An input entry together with an explicit attack base_power is refused. Basic attacks keep the ATK basis without skill-power multipliers.

The same path supplies instant healing and frozen recovery channels. The status_power_sources map selects a source skill for record caps, contact reactions and DOT/HOT sampling. Snapshot effects keep the application value; live effects resample the original source. Selected-source and target-sampling policies cannot be combined. A periodic attack's static base cannot override a selected-source binding. Chain and finisher rules may declare source_skill_id; preflight and execution use the same native status set. Omitted selectors retain the existing basis.

The private own view adds skill_powers, mapping exactly the current equipped slots to effective values. It contains no raw growth, model selection or another member's table. Full/delta consumers reject missing/extra slots, null, invalid amounts and public-member power fields atomically. Trusted skill changes recompute the slot table while retaining costs, cooldowns, raw build facts and committed budgets.

Engineering bounds: at most 64 models/skill inputs, rank arrays 1–16, bonus arrays 1–128, conversion 1–1000000 bps, model base/maximum at most 1 billion, signed addends -10000…1000000, external multiplier 0…1000000 bps. The configured input-point cap applies to the three build point fields; base load fields retain their wire integer and non-resource input bounds. No game, equipment, cultivation or map identifier is hardcoded.

The power-a/b samples exercise two tables, five trusted slots, four/eight members, ATK/skill separation, member isolation, healing caps, record snapshots, live/snapshot HOT, expiry, reconnect and one settlement. Source contracts are bound by source_character_power_vectors.json. Growth coefficients in the samples are explicit test values. Full original equipment sources, growth costs and cultivation-stage composition remain open under their original work packages.
