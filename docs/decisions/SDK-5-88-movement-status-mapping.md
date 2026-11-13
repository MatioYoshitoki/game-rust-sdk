# D-03 移动状态的当前裁定

2026-11-07，用户明确选择“按文字含义映射为移速效果”。

`effect_tribulation_paralysis`、`effect_tribulation_windbound`及`effect_slow`采用明确的减速语义，`effect_pill_wind`采用明确的移速增益语义。离线导入使用有类型的移动效果和来源配置中的有效强度、期限；旧`attrType=3`不再被解释为这些条目的最大生命值修正。不得自行补造缺失数值。

不可变资料包及SDK-5-17历史映射保留原样，当前实现和验收引用本裁定。此裁定只解决这四项的属性语义，不代表相关内容或整个M03已经实现验收。


2026-11-07执行补记：`src/verification/source_status_mappings.json`记录明确属性映射，`combat_d03_source_mapping_changes_movement_and_keeps_maximum_hp_unchanged`执行四项移动与maxHP断言，并执行流云散四品质强度/期限。三个减速期限使用测试参数300ms，未伪造原投放期限。完整内容签收仍按原映射台账逐项完成。
