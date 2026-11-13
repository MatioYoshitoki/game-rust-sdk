# 50个原算术向量路由

全部not_run；[机器表](numeric-mapping.json)逐字保留原input/expected及公差，后续由测试专用Rust公式/World适配执行；原validate.py不是Rust证据。

|ID / 运算|原input → expected|已有切片及缺失乘区|工作包|
|---|---|---|---|
|realm-0 / realm_modifier|{"levelDifference": 0} → 1|完整境界等级差乘区 / N07|R3.3,R6.2|
|realm-2 / realm_modifier|{"levelDifference": 2} → 1.2|完整境界等级差乘区 / N07|R3.3,R6.2|
|realm--2 / realm_modifier|{"levelDifference": -2} → 0.25|完整境界等级差乘区 / N07|R3.3,R6.2|
|defense-basic / defense_damage|{"raw": 100, "armor": 100, "defenderRealmBaseAtk": 100, "pierceBps": 0, "irreducible": false, "critMultiplier": 1} → 66|守方境界ATK基准/防御帽/最小伤害已有受控乘区，原暴击/破甲/真伤分支另判 / |R6.2|
|defense-pierce / defense_damage|{"raw": 100, "armor": 100, "defenderRealmBaseAtk": 100, "pierceBps": 5000, "irreducible": false, "critMultiplier": 1} → 80|守方境界ATK基准/防御帽/最小伤害已有受控乘区，原暴击/破甲/真伤分支另判；此行非平常伤害子集，必须补N07对应乘区。 / N07|R3.3,R6.2|
|defense-cap / defense_damage|{"raw": 100, "armor": 10000, "defenderRealmBaseAtk": 100, "pierceBps": 0, "irreducible": false, "critMultiplier": 1} → 30|守方境界ATK基准/防御帽/最小伤害已有受控乘区，原暴击/破甲/真伤分支另判 / |R6.2|
|defense-negative / defense_damage|{"raw": 100, "armor": -100, "defenderRealmBaseAtk": 100, "pierceBps": 0, "irreducible": false, "critMultiplier": 1} → 100|守方境界ATK基准/防御帽/最小伤害已有受控乘区，原暴击/破甲/真伤分支另判 / |R6.2|
|defense-true / defense_damage|{"raw": 100, "armor": 100, "defenderRealmBaseAtk": 100, "pierceBps": 0, "irreducible": true, "critMultiplier": 1} → 100|守方境界ATK基准/防御帽/最小伤害已有受控乘区，原暴击/破甲/真伤分支另判；此行非平常伤害子集，必须补N07对应乘区。 / N07|R3.3,R6.2|
|defense-crit / defense_damage|{"raw": 100, "armor": 100, "defenderRealmBaseAtk": 100, "pierceBps": 0, "irreducible": false, "critMultiplier": 1.5} → 100|守方境界ATK基准/防御帽/最小伤害已有受控乘区，原暴击/破甲/真伤分支另判；此行非平常伤害子集，必须补N07对应乘区。 / N07|R3.3,R6.2|
|defense-minimum / defense_damage|{"raw": 0, "armor": 100, "defenderRealmBaseAtk": 100, "pierceBps": 0, "irreducible": false, "critMultiplier": 1} → 1|守方境界ATK基准/防御帽/最小伤害已有受控乘区，原暴击/破甲/真伤分支另判 / |R6.2|
|dodge-half / dodge_percent|{"dodge": 60, "hit": 100, "penalty": 0} → 30|已有NPC概率闪避，不等价命中惩罚与派生闪避公式 / N07,N16|R3.3,R5.1,R6.2|
|dodge-cap / dodge_percent|{"dodge": 200, "hit": 0, "penalty": 0} → 50|已有NPC概率闪避，不等价命中惩罚与派生闪避公式 / N07,N16|R3.3,R5.1,R6.2|
|dodge-penalty / dodge_percent|{"dodge": 50, "hit": 0, "penalty": 40} → 10|已有NPC概率闪避，不等价命中惩罚与派生闪避公式 / N07,N16|R3.3,R5.1,R6.2|
|speed-0-1 / movement_speed|{"spd": 0, "rankMultiplier": 1} → 0|对数速度参数可配置；rank乘区另判 / |R6.2|
|speed-1-1 / movement_speed|{"spd": 1, "rankMultiplier": 1} → 180|对数速度参数可配置；rank乘区另判 / |R6.2|
|speed-8-1 / movement_speed|{"spd": 8, "rankMultiplier": 1} → 180|对数速度参数可配置；rank乘区另判 / |R6.2|
|speed-80-1 / movement_speed|{"spd": 80, "rankMultiplier": 1} → 240|对数速度参数可配置；rank乘区另判 / |R6.2|
|speed-800-1 / movement_speed|{"spd": 800, "rankMultiplier": 1} → 300|对数速度参数可配置；rank乘区另判 / |R6.2|
|speed-800-1.1 / movement_speed|{"spd": 800, "rankMultiplier": 1.1} → 330|对数速度参数可配置；rank乘区另判；rankMultiplier=1.1需要完整派生/等级倍率，不能直接声称匹配当前速度函数。 / N16|R5.1,R6.2|
|heal-cap / heal|{"hp": 950, "maxHp": 1000, "base": 100, "receivedPercent": 20, "allowOverheal": false} → 50|普通HP上限裁剪可复用；过量另判，不证明完整治疗来源公式 / |R6.2|
|heal-normal / heal|{"hp": 700, "maxHp": 1000, "base": 100, "receivedPercent": 20, "allowOverheal": false} → 120|普通HP上限裁剪可复用；过量另判，不证明完整治疗来源公式 / |R6.2|
|heal-full / heal|{"hp": 1000, "maxHp": 1000, "base": 100, "receivedPercent": 0, "allowOverheal": false} → 0|普通HP上限裁剪可复用；过量另判，不证明完整治疗来源公式 / |R6.2|
|heal-over / heal|{"hp": 950, "maxHp": 1000, "base": 100, "receivedPercent": 20, "allowOverheal": true} → 120|普通HP上限裁剪可复用；过量另判，不证明完整治疗来源公式；allowOverheal=true缺机制。 / N08|R3.3,R6.2|
|death-slots-0 / death_slots|{"eligibleSlots": 0, "rate": 0.5} → 0|配置整数槽位损耗；需核对ceil与源slot语义 / |R6.2|
|death-slots-1 / death_slots|{"eligibleSlots": 1, "rate": 0.5} → 1|配置整数槽位损耗；需核对ceil与源slot语义 / |R6.2|
|death-slots-3 / death_slots|{"eligibleSlots": 3, "rate": 0.5} → 2|配置整数槽位损耗；需核对ceil与源slot语义 / |R6.2|
|death-slots-8 / death_slots|{"eligibleSlots": 8, "rate": 0.5} → 4|配置整数槽位损耗；需核对ceil与源slot语义 / |R6.2|
|death-quantity-1-99 / death_quantity|{"quantity": 1, "stackLimit": 99, "rate": 0.5} → 1|整数物品quantity损耗/舍入及上限 / |R6.2|
|death-quantity-5-99 / death_quantity|{"quantity": 5, "stackLimit": 99, "rate": 0.5} → 2|整数物品quantity损耗/舍入及上限 / |R6.2|
|death-quantity-1-1 / death_quantity|{"quantity": 1, "stackLimit": 1, "rate": 0.5} → 1|整数物品quantity损耗/舍入及上限 / |R6.2|
|hp-ratio / hp_cost|{"hp": 99, "cost": 0.05, "mode": "current_hp_ratio", "minimum": 1} → 4|HP成本与minHpAfterCost不是SP成本 / N04|R3.1,R6.2|
|hp-min / hp_cost|{"hp": 1, "cost": 0.05, "mode": "current_hp_ratio", "minimum": 1} → 0|HP成本与minHpAfterCost不是SP成本 / N04|R3.1,R6.2|
|hp-clamped / hp_cost|{"hp": 50, "cost": 100, "mode": "fixed", "minimum": 1} → 49|HP成本与minHpAfterCost不是SP成本 / N04|R3.1,R6.2|
|pvp-extra / pvp_multicast|{"total": 3, "inherent": 1} → 0.43333333333333335|对玩家额外段折损/固有段规则 / N09,N18|R3.4,R5.3,R6.2|
|pvp-inherent / pvp_multicast|{"total": 3, "inherent": 3} → 1|对玩家额外段折损/固有段规则 / N09,N18|R3.4,R5.3,R6.2|
|ticks-1-100 / duration_ticks|{"milliseconds": 1, "stepMs": 100} → 1|原毫秒ceil到逻辑tick，测试专用时钟适配后验证；不把当前deadline视为相同执行结果 / |R6.2|
|ticks-100-100 / duration_ticks|{"milliseconds": 100, "stepMs": 100} → 1|原毫秒ceil到逻辑tick，测试专用时钟适配后验证；不把当前deadline视为相同执行结果 / |R6.2|
|ticks-101-100 / duration_ticks|{"milliseconds": 101, "stepMs": 100} → 2|原毫秒ceil到逻辑tick，测试专用时钟适配后验证；不把当前deadline视为相同执行结果 / |R6.2|
|ticks-150-30 / duration_ticks|{"milliseconds": 150, "stepMs": 30} → 5|原毫秒ceil到逻辑tick，测试专用时钟适配后验证；不把当前deadline视为相同执行结果 / |R6.2|
|ticks-151-30 / duration_ticks|{"milliseconds": 151, "stepMs": 30} → 6|原毫秒ceil到逻辑tick，测试专用时钟适配后验证；不把当前deadline视为相同执行结果 / |R6.2|
|orb-normal / orb|{"maxHp": 1000, "maxSpirit": 200, "hpRatio": 0.03, "spiritRatio": 0.15} → {'hp': 30, 'spirit': 30}|普通/Boss光球按基准属性比例烘焙、拾取才恢复 / |R6.2|
|orb-boss / orb|{"maxHp": 1000, "maxSpirit": 200, "hpRatio": 0.15, "spiritRatio": 0.6} → {'hp': 150, 'spirit': 120}|普通/Boss光球按基准属性比例烘焙、拾取才恢复 / |R6.2|
|shield-cap / shield_grant|{"maxHp": 1000, "otherShields": 250, "requested": 100} → 50|基础maxHP grant/cap及已满盾零grant，不包含反击记录 / |R6.2|
|shield-full / shield_grant|{"maxHp": 1000, "otherShields": 300, "requested": 100} → 0|基础maxHP grant/cap及已满盾零grant，不包含反击记录 / |R6.2|
|ordinary-step / move_step|{"speed": 180, "dtMs": 30, "direction": [1, 0], "start": [0, 0]} → [5.4, 0]|归一化方向×速度×步长，实际位置断言 / |R6.2|
|diagonal-step / move_step|{"speed": 180, "dtMs": 30, "direction": [3, 4], "start": [0, 0]} → [3.24, 4.32]|归一化方向×速度×步长，实际位置断言 / |R6.2|
|cultivation-qi-extract / session_cultivation|{"killScore": 140, "extraction": true, "entryYears": 1, "realmCoefficient": 0.75, "extractionBase": 30, "multiplier": 1} → 127|有限积分/带入年限/撤离乘区可配置，完整成长仍M13 / |R6.2|
|cultivation-qi-death / session_cultivation|{"killScore": 140, "extraction": false, "entryYears": 1, "realmCoefficient": 0.75, "extractionBase": 30, "multiplier": 0.5} → 52|有限积分/带入年限/撤离乘区可配置，完整成长仍M13 / |R6.2|
|cultivation-foundation-extract / session_cultivation|{"killScore": 40, "extraction": true, "entryYears": 2, "realmCoefficient": 1.5, "extractionBase": 30, "multiplier": 1} → 210|有限积分/带入年限/撤离乘区可配置，完整成长仍M13 / |R6.2|
|cultivation-empty-extract / session_cultivation|{"killScore": 0, "extraction": true, "entryYears": 1, "realmCoefficient": 0.75, "extractionBase": 30, "multiplier": 1} → 22|有限积分/带入年限/撤离乘区可配置，完整成长仍M13 / |R6.2|
