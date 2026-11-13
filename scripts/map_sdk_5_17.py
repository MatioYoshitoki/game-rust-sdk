#!/usr/bin/env python3
"""Offline, deterministic requirements routing. Does not execute game scenarios."""
import argparse
import hashlib
import json
import re
from collections import Counter
from functools import lru_cache
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = 'rust-match-kernel-v1.1/'
DEST = ROOT / 'docs/audits/sdk-5-17'
GENERATED = {}


@lru_cache(maxsize=None)
def read(path):
    return json.loads((ROOT / path).read_text())


def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                     separators=(',', ':')).encode()).hexdigest()


def esc(key):
    return str(key).replace('~', '~0').replace('/', '~1')


def pointer(value, path):
    for key in path.strip('/').split('/') if path else []:
        key = key.replace('~1', '/').replace('~0', '~')
        value = value[int(key)] if isinstance(value, list) else value[key]
    return value


def leaves(value, path=''):
    if isinstance(value, (dict, list)) and value:
        for k, v in (value.items() if isinstance(value, dict) else enumerate(value)):
            yield from leaves(v, path + '/' + esc(k))
    else:
        yield path, value


def emit(name, value):
    GENERATED[name] = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, indent=2) + '\n'


# Each capability names a bounded slice; no entry claims complete original-content execution.
EXISTING_DATA = [
 ('E01', '单成员准入/ready/冻结责任', 'src/engine.rs', 'docs/evidence/B1-completion-2026-11-02.md', 'verification::tests::refused_admission_never_injects_inventory', 'Create单participant；receipt/ready', '独立操作账本、单成员阶段', '业务请求/快照/终局', '每World仅1参与者；不含同局多人'),
 ('E02', '平面移动、阻挡与等距显示', 'src/world.rs', 'docs/evidence/B1-completion-2026-11-02.md', 'verification::tests::movement_is_normalized_and_one_step_is_exact', 'Move/Stop/平面地图', '位置/运动意图/路径', '权威位置', '矩形/格阻挡、等高且无地形DOT；不含完整装饰碰撞'),
 ('E03', '两槽提交事务及取消', 'src/world/pve.rs', 'docs/interfaces/player-skills-v1.md', 'verification::tests::cost_is_committed_once_and_cancel_before_commit_is_free', 'Cast/CancelAction及至多2个技能', '准备/提交/共享后摇/CD/SP', '动作结果/费用', '后摇不可缩短；不含HP成本/引导/蓄力/再施放'),
 ('E04', '严格敌方单位弹道', 'src/world/pve.rs', 'docs/interfaces/unit-targets-v1.md', None, '敌方NPC单位引用/代次', '锁定目标与提交重验', '弹道接触/失效', '非友方即时治疗、非任意单位效果'),
 ('E05', 'ATK/DEF状态、来源分组与共享期限叠层', 'src/config/status.rs', 'docs/interfaces/status-stack-v1.md', None, '有限状态定义及commit/hit挂钩', 'refresh/ignore/stack、来源/到期', '有界状态视图/实际ATK/DEF', '只允许已支持属性与策略；复合原状态不能拆成两个后宣称等价'),
 ('E06', '基础吸收盾', 'src/world/pve.rs', 'docs/interfaces/absorption-shields-v1.md', None, '配置grant/cap/期限', '按期限/应用顺序消费盾', '吸收量/HP/盾余量', '无记录反击/增益吸收/过量转盾/任意触发'),
 ('E07', '物理DOT', 'src/world/pve.rs', 'docs/interfaces/periodic-damage-v1.md', None, '有界间隔/期限/伤害系数', '冻结来源ATK及绝对deadline', '物理周期伤害/终结', '不是状态持有者ATK；不含五行DOT'),
 ('E08', '自身HOT', 'src/world/pve.rs', 'docs/interfaces/periodic-healing-v1.md', None, '固定值+基础maxHP系数', '单人周期计划', '实际HP/到期', '自身、不得过疗；不含友方或施法者ATK完整公式'),
 ('E09', '受疗倍率', 'src/world/pve.rs', 'docs/interfaces/healing-received-v1.md', None, '有界受疗状态', '倍率聚合', '治疗结算值', '可复用受疗乘区；不等于完整鼎焰/层数联动'),
 ('E10', '一次圆形地面效果', 'src/world/pve.rs', 'docs/interfaces/ground-targets-v1.md', None, '合法点/半径/预警', '固定落点计划', '一次伤害/状态', '严格合法点；无任意clamp/移动锚点'),
 ('E11', '固定圆形持续场', 'src/world/pve.rs', 'docs/interfaces/persistent-fields-v1.md', None, '点、期限/间隔', '场计划、绝对tick', '有界接触', '不含enter/pickup触发或移动附着'),
 ('E12', '统一接触预算', 'src/world/pve.rs', 'docs/interfaces/contact-policy-v1.md', None, '每周期/总量/重复间隔', '命中账本', '接受/预算结束', '每周期1..32、总1..128；0重复间隔按计划定义一次'),
 ('E13', '直线穿透与重复弹道', 'src/world/pve.rs', 'docs/interfaces/projectile-repeat-v1.md', None, '方向/直线弹道/命中策略', '扫掠接触及历史', '按进度排序的伤害/结束', '不含回旋/追踪/分裂/反弹/来源失效后继续'),
 ('E14', '固定线形持续场', 'src/world/pve.rs', 'docs/interfaces/line-fields-v1.md', None, '方向/全宽/长度/预警', '固定胶囊、裁墙、tick', '线端点和接触', '全宽0.2..256、长0.1..10000、tick1..128；非移动线'),
 ('E15', '有限NPC技能/牵引/支援/阶段', 'src/world/pve.rs', 'docs/evidence/B3-1-map-pve-2026-11-03.md', None, 'NPC模板及既有有类型技能', '有限AI/牵引/同群支援/Boss阶段', '真实NPC伤害/回复/掉落', '不能解释任意skillSlots条件、完整仇恨/风筝/生存NPC'),
 ('E16', '遭遇、目标、调谐及重置', 'src/world/pve.rs', 'docs/evidence/B3-13-encounter-reset-2026-11-04.md', None, '配置遭遇/目标/成本', '依赖/阶段/整组复位', '目标进度/奖励可达', '单参与者，无贡献优先期/三格加持'),
 ('E17', '节点条件/选择/预警/效果组合', 'src/config.rs', 'docs/evidence/B3-19-sword-trial-2026-11-04.md', None, '有限节点和条件/效果', '节点选择和冻结预警', '伤害/修正/推进', '只覆盖现有原生组合，不能按地图ID推断算法'),
 ('E18', '整数物品、容器、落地拾取与一次性掉落', 'src/world/pve.rs', 'docs/evidence/B3-2-pouch-2026-11-03.md', 'verification::tests::full_inventory_does_not_consume_ground_drop', '箱/怪掉落/容量/拾取', '整数堆叠/容器进度/TTL', '库存与掉落', '无实例装备、主动Drop、多人资源竞争/遗物'),
 ('E19', '单人撤离/死亡/有限成长与资产事务', 'src/engine.rs', 'docs/interfaces/assets-v2.md', 'verification::tests::damage_and_leaving_range_interrupt_extraction', '终局触发/原子回执', '核心提交及冻结责任', '金额/物品分解/终局', '有限积分公式；不含全部成长/附属事实/转世'),
 ('E20', '连接代次、发布回执、有界IO及关局', 'src/runtime.rs', 'docs/evidence/B2-4-flow-control-2026-11-03.md', 'verification::tests::worker_shutdown_releases_each_of_one_hundred_runtime_threads', '输入/重连/发送回执/receipt/close', '单接收者基线、连接代次、恢复账本', '全量快照/故障/closed', '单成员权限；不含原客户端full/delta全字段及多人慢发送隔离'),
]
MISSING_DATA = [
 ('N01','实体身份/归属/关系','R2.1',['M01'],'成员/实体引用、代次、owner/关系','统一身份表/资格','只针对合法目标的结果','有界实体；禁止把NPC或另一Runtime当第二玩家；可为M02/M06/M07/M10/M14提供底座，不等于每个引用者都缺这五类行为'),
 ('N02','多成员准入与生命周期','R2.2',['M01'],'冻结席位、独立事实/回执/ready','成员阶段/成本/关局责任','按成员激活和终局','容量按模式配置；双人切片不关闭完整M01'),
 ('N03','逐接收者权限/基线','R2.3',['M14'],'接收者/连接/full或delta回执','各自可见集合/基线','已裁剪发布','慢发送、观礼、重连不相互污染'),
 ('N04','目标/动作/成本/多槽','R3.1',['M02'],'有类型目标/HP成本/蓄力引导再施放/道具','动作事务与阶段','真实费用/效果/中断','无通用脚本或ModeAction tunnel'),
 ('N05','属性与复合状态/控制','R3.2',['M03'],'原生属性、状态/免疫/驱散/控制','单状态身份、层数/期限/来源','属性和资格变化','D-03先裁定；不空实现状态名；伤害/恢复控制组合另关联M04'),
 ('N06','事件/计数器/条件触发','R3.2',['M03'],'有类型事件、条件、优先级、预算','计数/消费/ICD/触发去重','有限原生效果','G-01未知special先补精确语义；不运行JSON解释器'),
 ('N07','完整伤害乘区','R3.3',['M04'],'五行/抗性/境界/命中/破甲/真伤等事实','冻结或实时取值策略','实际伤害/免疫/死亡','公式和取整逐向量验证；NPC已有有限暴击闪避不等价'),
 ('N08','完整恢复/DOT/HOT/护盾','R3.3',['M04'],'持有者或来源ATK、友方、过量、盾策略','周期/来源存续/记录值','HP/SP/盾/反击','先冻结时钟/顺序；复用E06..E09有限切片'),
 ('N09','原生形状/运动/锚点/位移','R3.4',['M05'],'扇形/环/链/陷阱/附着/位移等有限计划','轨迹/锚点/接触历史','接触/位移/回收','形状不由VFX决定；共享命中链和预算'),
 ('N10','来源失效后的世界效果','R3.4',['M05'],'冻结source引用和continue/end策略','来源失效而计划仍可执行','目标伤害及独立结束','死亡不复活/不重结算；仍保持归属'),
 ('N11','召唤/复制/参战','R3.5',['M06'],'模板、owner、寿命、数量、允许复制能力','真实召唤实体','造成/承受伤害和退场','非仅视觉；受实体和触发预算约束'),
 ('N12','生产几何/导航/地形','R4.1',['M08'],'边界/高度/装饰/动态阵法/地形DOT','统一碰撞与导航场','路径/视线/扫掠/合法落点','G-02完整输入先冻结；禁止运行时回调JS'),
 ('N13','完整AI/多目标仇恨/NPC','R4.2',['M07'],'资格、仇恨、技能优先条件/库存','目标/寻路/生存状态','实际技能/移动/搜刮','复用E15；不能为45模板写45处理器'),
 ('N14','环境资源生成/分布','R4.3',['M09'],'热点/池/补充/一次性/落点预算','来源及重生计划','合法资源实例','满预算/无落点不能吞物或无限重试'),
 ('N15','实例物品/竞争/贡献/遗物','R4.4',['M10'],'物品实例/修订、领取者、占用、贡献','容量/预留/优先期/遗物','唯一收益、私有内容','两人/NPC、失败回滚、数量守恒与权限共同验收'),
 ('N16','成长派生/养成/附属事实','R5.1',['M13'],'完整成长事实/公式、功法和装备绑定','派生/上限/跨区及事务快照','金额/属性/附属事实','D-05/G-03先冻结；具名DTO解耦，不让Node补算'),
 ('N17','临时加持','R5.2',['M11'],'有界槽位、获取/替换/强化','临时所有权/自动被动','战斗变化/退出清理','首款配置三格；不能只把调谐结果改名'),
 ('N18','模式阶段/胜负/复活/检查点','R5.3',['M12'],'有类型模式配置和有限动作','模式状态机/期限/责任','胜负/复活/观礼/终局','D-07连接60s不覆盖竞技30s判负'),
]
CATALOG = {'schemaVersion':1,'baseline':{'crate':'0.19.0','mechanismVersion':34},'existing':[], 'missing':[]}
BASE_EVIDENCE = 'artifacts/sdk-5-16/acceptance.json'
for cid,title,code,doc,test,inp,state,out,limits in EXISTING_DATA:
    CATALOG['existing'].append(dict(id=cid,title=title,input=inp,state=state,output=out,limits=limits,
        code=code,contractOrEvidence=doc,passedReport=BASE_EVIDENCE,test=test,
        evidenceScope='历史记录声明已签收受控切片；不代表原内容/原场景精确执行',
        historicalReportAvailability='cleaned_by_user_2026-11-05',
        evidenceLimitation='本轮采集初始摘要后，用户明确清理artifacts；原报告当前不可复核。代码/契约仍可定位，不能将历史声明计为本轮重新验证通过。'))
for cid,title,wp,gaps,inp,state,out,limits in MISSING_DATA:
    CATALOG['missing'].append(dict(id=cid,title=title,workPackages=[wp],gapIds=gaps,input=inp,state=state,output=out,limits=limits,status='missing_or_needs_spec'))
CAPS = {x['id']:x for group in ['existing','missing'] for x in CATALOG[group]}
WP = read('docs/plans/sdk-delivery-roadmap.json')
GAP_WP = {x['id']: x['id'] for x in WP['workPackages']}
GAP_ROUTES = {f'P{i:02}': w for i,w in enumerate(['R4.1','R4.2','R3.4','R4.3','R4.4','R5.3','R5.3','R5.1','R6.3'],1)}
GAP_ROUTES.update({f'I{i:02}': w for i,w in enumerate(['R6.1','R6.1','R6.1','R6.2','R6.2','R6.3','R6.4','R6.5','R6.6'],1)})


class Item:
    def __init__(self, audit, file, ptr, raw):
        self.row = dict(kind=audit['kind'],id=audit['id'],name=audit['name'],source={'file':file,'pointer':ptr,'sha256':digest(raw)},
            sourceAudit=audit,requirements=[],references=[],fieldAudit=[],scenarioIds=audit['scenarioIds'],
            exactAcceptance='not_run',wholeItemStatus='not_accepted',wholeItemReason='需完成全部子行为、引用绑定与I04；局部P不表示整项可配表完成')
        self.raw=raw

    def add(self, path, meaning, existing=(), missing=(), gaps=(), gates=(), status=None):
        value=pointer(self.raw,path)
        gaps=set(gaps)
        for c in missing:gaps.update(CAPS[c]['gapIds'])
        # I04 is independent, never used to assert semantic success.
        gaps.add('I04')
        wps={GAP_ROUTES[g] for g in gaps if g in GAP_ROUTES}
        for c in missing:wps.update(CAPS[c]['workPackages'])
        req=dict(id=f"q{len(self.row['requirements'])+1}",pointer=path,sourceSha256=digest(value),meaning=meaning,
            classes=sorted(set(g[0] for g in gaps)),existingCapabilities=list(existing),missingCapabilities=list(missing),
            gapIds=sorted(gaps),workPackages=sorted(wps),ruleGates=sorted(set(gates)),
            status=status or ('needs_spec' if gates else 'missing' if missing else 'configuration_slice' if existing else 'consumer_data'),
            exactItemEvidence='not_run')
        req['primaryWorkPackage']=CAPS[missing[0]]['workPackages'][0] if missing else next((GAP_ROUTES[g] for g in sorted(gaps) if g in GAP_ROUTES and g!='I04'),'R6.2')
        self.row['requirements'].append(req)
        return req['id']

    def finish(self):
        def owners(path):
            applicable=[q for q in self.row['requirements'] if path==q['pointer'] or path.startswith(q['pointer']+'/') or q['pointer']=='']
            assert applicable, (self.row['id'],path)
            specificity=max(len(q['pointer']) for q in applicable)
            return [q['id'] for q in applicable if len(q['pointer'])==specificity]
        def cover(value,path=''):
            # Uniform subtrees (especially RLE and decorations) are stored once, with an
            # exact digest/count; validation expands them to every original leaf.
            fields=list(leaves(value,path))
            routes={tuple(owners(p)) for p,_ in fields}
            if len(routes)==1:
                self.row['fieldAudit'].append(dict(pointer=path,sha256=digest(value),leafCount=len(fields),coverage='entire_subtree',requirements=list(next(iter(routes)))))
            else:
                for k,v in (value.items() if isinstance(value,dict) else enumerate(value)):cover(v,path+'/'+esc(k))
        cover(self.raw)
        self.row['gapIds']=sorted(set(g for q in self.row['requirements'] for g in q['gapIds']))
        self.row['workPackages']=sorted(set(g for q in self.row['requirements'] for g in q['workPackages']))
        self.row['ruleGates']=sorted(set(g for q in self.row['requirements'] for g in q['ruleGates']))
        self.row['acceptanceOwnerWorkPackage']='R6.2'
        return self.row


def factor_family(fid):
    exact={
      'factor_deal_damage':(['E13'],['N07','N06']), 'factor_dot_damage':(['E07'],['N08']),
      'factor_heal':(['E08','E09','E15'],['N08','N06']), 'factor_dynamic_heal':(['E08'],['N08','N06']),
      'factor_dynamic_mana':([],['N08','N06']), 'factor_fix_attribute':(['E05'],['N05']),
      'factor_apply_status_effect':(['E05'],['N05','N06']), 'factor_cultivation_guard':(['E06'],['N08','N06']),
      'factor_cultivation_state_recovery':(['E09'],['N06','N08']), 'factor_weapon_cauldron_heal_reduction':(['E09'],['N06','N08']),
      'factor_weapon_cauldron_burn':(['E07','E09'],['N06','N08']), 'factor_cultivation_area_heal':(['E08'],['N08','N09']),
      'factor_cultivation_overheal_shield':(['E06'],['N08']), 'factor_aoe_damage':(['E10','E11'],['N07','N06']),
      'factor_deal_true_damage':([],['N07','N09']),
      'factor_cultivation_state_add':(['E05'],['N06','N07','N09']),
      'factor_cultivation_state_consume':([],['N06','N04','N05','N07','N09']),
      'factor_cultivation_state_skill_modifier':([],['N06','N08']),
      'factor_cultivation_zone':(['E11'],['N06','N07','N09']),
      'factor_cultivation_state_spread':([],['N06','N09']),
      'factor_cultivation_state_tick':(['E07','E08'],['N06','N08']),
      'factor_cultivation_state_delayed_damage':([],['N06','N07','N08']),
      'factor_cultivation_window':([],['N04','N05','N06','N07']),
      'factor_cultivation_expire_linger':([],['N06','N07']),
      'factor_cultivation_expire_dispel':([],['N05','N06']),
      'factor_cultivation_retaliation_window':([],['N05','N06','N07']),
      'factor_cultivation_seed_window':([],['N06','N16']),
      'factor_cultivation_time_current':([],['N06','N16']),
      'factor_cultivation_value_consume':([],['N06','N07']),
      'factor_cultivation_value_kill_refresh':([],['N04','N06']),
      'factor_cultivation_kill_cooldown_refund':([],['N04','N06']),
      'factor_cultivation_cooldown_modifier':([],['N04','N06']),
      'factor_cultivation_shatter':([],['N06','N07','N16']),
      'factor_life_steal':([],['N07','N08','N06']),
      'factor_steal':([],['N07','N08','N06']),
      'factor_reset_cooldown':([],['N04','N06']),
      'factor_insight_split_damage':([],['N07','N09','N16']),
      'factor_insight_apply_status_on_hit':(['E05'],['N05','N06','N16']),
      'factor_insight_periodic_damage':(['E07'],['N07','N08','N16']),
      'factor_insight_trail_damage':(['E14'],['N07','N09','N16']),
      'factor_insight_delayed_vulnerability':([],['N06','N07','N16']),
      'factor_insight_attack_speed_reduction':([],['N05','N16']),
      'factor_weapon_tower_suppression':([],['N05','N06','N07']),
      'factor_teleport':([],['N09']), 'factor_knockback':([],['N09']),
      'factor_summon':([],['N11']), 'factor_summon_death_explosion':([],['N11','N07']),
      'factor_cultivation_summon_presence':([],['N11','N06']),
      'factor_mind_art_effect':([],['N16','N06']), 'factor_realm_core_turn':([],['N16','N06']),
    }
    if fid in exact:return exact[fid]
    if fid.startswith('cultivation_layer'):
        text=json.dumps(FACTORS.get(fid,{}).get('paramsDesc',[]),ensure_ascii=False)
        families=['N16','N06']
        if any(w in text for w in ['伤害','防御','暴击','闪避','攻击力']):families+=['N07']
        if any(w in text for w in ['吸血','治疗','回复','护盾','灵力']):families+=['N08']
        if any(w in text for w in ['速度','属性']):families+=['N05']
        return [],sorted(set(families))
    if fid.startswith('factor_insight'):return [],['N16','N06']
    if any(x in fid for x in ['attribute','invincible','cast_lock','attack_speed','suppression']):return ['E05'],['N05','N06']
    if any(x in fid for x in ['heal','recovery','guard','linger']):return ['E06','E09'],['N08','N06']
    if fid=='factor_cultivation_secondary_hit':return ['E12'],['N09','N06','N07']
    if any(x in fid for x in ['damage','pierce','crit','dodge','unavoidable','irreducible','execute','life_steal','counter','steal','vulnerability']):return [],['N07','N06']
    return [],['N06']


def factor_binding(item,path,binding):
    fid=binding.get('factorId','')
    es,ns=factor_family(fid)
    definition=FACTORS.get(fid)
    detail=''
    params=binding.get('params',[])
    if fid=='factor_fix_attribute' and isinstance(params,list) and params:
        attr={1:'ATK',2:'DEF',3:'maxHP',4:'strength',5:'SPD',6:'divineSense'}.get(params[0],str(params[0]))
        detail=f'attrType={params[0]}明确指{attr}；当前原生属性状态仅ATK/DEF百分比，strength不能当ATK，maxHP不能当SPD。'
        if len(params)>=3 and params[0] in [1,2] and params[2]==1:
            ns=[]
            detail+='此绑定的百分比属性可转换为bps配置，不因因子目录还支持其它属性就继承那些缺口。'
    if fid=='factor_deal_damage' and isinstance(params,list) and len(params)>=4:
        if params[0]==0 and params[1] in [1,2] and params[2]==0:
            ns=[]
            detail='此绑定仅零固定基数+ATK比例的物理/金伤公式切片；不继承目录中其它伤害类型/暴击参数缺口，实际递送/触发仍按父计划审查。'
    if fid=='factor_apply_status_effect' and isinstance(params,list) and 1<=len(params)<=6:
        trigger=params[3] if len(params)>3 else None
        if trigger in [None,'','onSkillCast','onProjectileHit','onRuntimeContact']:
            ns=[]
            detail='固定状态绑定的commit/hit子集可复用，具体状态仍沿引用继续审查；enter/tick/pickup由父runtime触发规则另登记。'
    if fid=='factor_dot_damage':detail='原式基于状态持有者ATK及伤害类型；现有E07冻结来源ATK且仅物理，二者不可直接替换。'
    item.add(path, f"因子绑定{fid}；参数={json.dumps(params,ensure_ascii=False)}。{detail}现有能力仅表示可复用乘区/计划；触发、取值对象、类型和优先序仍按原定义逐项实现。",es,ns,gaps=['P03'] if es and not ns else [],gates=['G-01'] if not definition or definition.get('type')=='special' or fid.startswith('cultivation_layer') else [])
    if definition:
        item.row['references'].append({'kind':'factor','id':fid,'fromPointer':path,'scope':'definition_reference_only; concrete binding requirements own the applicable slice','source':{'file':SOURCE+'source-config.json','pointer':'/tables/factors/'+esc(fid),'sha256':digest(definition)}})
    else:
        item.row['references'].append({'kind':'unresolved_factor','id':fid,'fromPointer':path,'gate':'G-01'})
    for param_path,param in leaves(binding.get('params',[])):
        if isinstance(param,str) and param in SC['tables']['status-effects']:
            status=SC['tables']['status-effects'][param]
            item.row['references'].append({'kind':'status','id':param,'fromPointer':path+'/params'+param_path,'source':{'file':SOURCE+'source-config.json','pointer':'/tables/status-effects/'+esc(param),'sha256':digest(status)}})
        if fid=='factor_summon' and isinstance(param,str) and param in RC['tables']['data/monsters']['MONSTER_TEMPLATES']:
            monster=RC['tables']['data/monsters']['MONSTER_TEMPLATES'][param]
            item.row['references'].append({'kind':'monster','id':param,'fromPointer':path+'/params'+param_path,'source':{'file':SOURCE+'resolved-config.json','pointer':'/tables/data~1monsters/MONSTER_TEMPLATES/'+esc(param),'sha256':digest(monster)}})


def bindings(item, value,path=''):
    if isinstance(value,dict):
        if 'factorId' in value:factor_binding(item,path,value)
        for k,v in value.items():bindings(item,v,path+'/'+esc(k))
    elif isinstance(value,list):
        for k,v in enumerate(value):bindings(item,v,path+'/'+str(k))


META={'id','name','icon','emoji','description','lore','summary','synergy','themeTags','codexActionArtworkId','codexPortraitId','vfxTemplate','vfxMotionGrammar','vfxTier','vfxElementOverride','rewardDescription'}
P_GAPS={'skill':'P03','cultivation':'P08','status':'P03','factor':'P03','weaponMorph':'P03','monster':'P02','mapPve':'P06','ruins':'P07','map':'P01'}


def route(item):
    kind=item.row['kind']; obj=item.raw
    # Every top field gets an explicit owner. Deeper semantic plans override this coarse field owner below.
    for key,value in obj.items():
        path='/'+esc(key)
        if key in META:
            item.add(path,'来源身份/展示资料由配置及客户端保留；描述是核对依据，不由SDK按文字或VFX自动生成效果。',gaps=['I01'])
            continue
        if kind=='skill':
            if key in ['castTime','cooldown','range','spiritCost','hpCost','minHpAfterCost','hpCostMode']:
                extra=['N04'] if key in ['hpCost','hpCostMode','minHpAfterCost'] and value else []
                gates=['OBS-03'] if key=='spiritCost' and value>1000000 else []
                item.add(path,'费用/距离/时序配置值；castTime旧投影以castLifecycle为准。SP<=1000000等现有界限内可复用，HP成本不是SP参数。',['E03'],extra,['P03'],gates)
            elif key in ['targetType','targeting']:
                item.add(path,'目标、关系、瞄准/辅助与中心；方向/自身/严格敌NPC子集可复用，友方即时效果/瞄准策略须补。',['E03','E04','E10'],['N04'],['P03'])
            elif key=='runtimes':
                item.add(path,'世界效果计划集合；逐计划区分实际shape/motion/triggers，不依据VFX推断。集合本身不另算缺机制，子计划分别路由。',gaps=['I04'])
            elif key=='factors':item.add(path,'效果绑定集合；实际机制和参数逐绑定路由，不能仅识别factorId即算完成。',gaps=['I04'])
            elif key in ['aiPriority','aiCastPlan','bossTelegraph']:item.add(path,'NPC优先级/施放计划/预警；always可复用，新增条件或计划须由有限AI处理。',['E15'],[] if key=='aiPriority' and value.get('type')=='always' else ['N13'],['P02'])
            elif key in ['element','cultivationMethod']:item.add(path,'技能元素/修行方式作为配置标签及派生绑定保留；实际伤害由具体因子/成长公式决定，不因标签本身重复要求整套五行算法。',gaps=['P03'])
            elif key=='activationMode':item.add(path,'被动占槽/命中充能/器型强化；不等价SelfStatus主动施放。',missing=['N06','N09','N16'],gates=['G-01'])
            else:item.add(path,'施法阶段/移动/引导/蓄力/跟进及实际效果递送。',['E03'],['N04'],gates=['G-01'] if key not in ['castLifecycle','castMovementPolicy','effectDelivery','recoveryTime','recoveryCancelable','channelExecution','chargeEffectScaling'] else [])
        elif kind=='status':
            if key in ['duplicateType','maxStacks','sourceScoped']:item.add(path,'重复/来源策略可复用；完整状态必须保持同一状态身份、期限与槽位。',['E05'],gaps=['P03'])
            elif key=='attachedFactors':item.add(path,'状态内效果集合；逐绑定路由。空数组不证明无行为，需另跟踪cultivationStateId和控制/免疫规则。',['E05'],gaps=['P03'])
            elif key=='cultivationStateId':item.add(path,'专用计数/消费/自动触发状态标识；不能丢弃或仅展示。',missing=['N06'],gates=['G-01'])
            elif key=='qualityScaling':item.add(path,'品质派生状态系数/期限，与实例品质绑定。',missing=['N05','N15','N16'],gates=['G-01'])
            elif key in ['category','hidden']:item.add(path,'分类/隐藏的展示数据由接入方映射；不是凭buff/debuff标签新增一项SDK效果。实际控制/免疫或专用状态由行为要求另登记。',gaps=['I01'])
            else:item.add(path,'未解释的状态字段需确认语义，不能忽略或当no-op。',missing=['N05'],gates=['G-01'])
        elif kind=='factor':
            es,ns=factor_family(item.row['id'])
            gate=['G-01'] if obj.get('type')=='special' or item.row['id'].startswith('cultivation_layer') else []
            item.add(path,f"原生族候选：{obj.get('type')}，stateful={obj.get('stateful')}；{key}={json.dumps(value,ensure_ascii=False)}。参数含义、每个触发和优先序须一起实现；layer6/9未声明aliasOf，不能凭名称等同基础因子。",es,ns,gates=gate)
        elif kind=='cultivation':
            if key=='skillId':item.add(path,'解锁技能引用；须绑定对应技能全部语义。',missing=['N04','N16'])
            else:item.add(path,'修为门槛/品质/熟练度/分阶段效果、档位及派生绑定。',missing=['N16'],gates=['G-03'] if key=='proficiencyParams' else [])
        elif kind=='weaponMorph':
            if key=='combatStatus':item.add(path,'来源工程implemented标签仅为历史元数据，不是当前SDK通过证据。',gaps=['I04'])
            elif key in ['discovery','elements','minimumElementEnergy','minimumTotalEnergy','requiredEmergeSkillId']:item.add(path,'器型发现/能量/解锁条件是项目配置与派生事实。',missing=['N16'])
            else:item.add(path,f"{obj.get('familyId')} / {obj.get('pattern')} 的{key}：{json.dumps(value,ensure_ascii=False)}；需有限接触计划/计数/预算，非逐ID处理器。",['E12','E13'],['N09','N06','N07'],gates=['G-01'])
        elif kind=='monster':
            if key in ['statMultiplier','statMultipliers','rank','realm','element']:item.add(path,'基础属性配置乘数；全境界/五行/成长转化须由原生派生计算。',['E15'],['N07','N16'],['P02'])
            elif key=='dropTable':item.add(path,'既有普通掉落可复用；完整奖池/实例品质与合法落点另补。',['E18'],['N14','N15'],['P04'])
            elif key in ['normalAttackIntervalMs','skillWindupMs']:item.add(path,'有限NPC时序配置切片，须通过当前合法范围校验。',['E15'],gaps=['P02'])
            else:item.add(path,'多技能条件/优先级、组织类型、阶段变化或被动；不能把原skillSlots当既有有限NPC槽直接换名。',['E15'],['N13'],['P02'])
        elif kind=='mapPve':
            if key in ['contributionRatio','contributionInterrupts','mechanicContributionRequired','priorityDurationMs']:item.add(path,'多人贡献资格、机械贡献和领取优先窗口。',missing=['N02','N15'])
            elif key in ['treasureTypeId','rewardMarkerItemId','rewardMarkerItemIds','opportunityProfileId']:item.add(path,'奖励/加持池及领取绑定；普通标记物可复用，不能代替加持。',['E18'],['N15','N17'],['P06'])
            elif key in ['observePosition','placementRadius','exitClearance']:item.add(path,'原世界坐标/合法落点与退出净空，依赖完整生产碰撞。',['E02'],['N12'],['P01'])
            elif key=='finaleMechanic':item.add(path,f"终段原规则{value.get('kind')}；有界原生组合可复用的切片须核对节点时序/目标/乘区。",['E17'],['N07'] if value.get('kind')=='blood_tide' else [],['P07'],['G-01'])
            else:item.add(path,'遭遇、支援/削弱/定向重击、调谐成本、阶段与复位配置切片；source技能与生产坐标另有依赖。',['E15','E16','E17'],gaps=['P06'])
        elif kind=='ruins':
            ns=['N18']
            if any(x in key.lower() for x in ['wraith','template','hunt']):ns+=['N11','N13']
            if any(x in key.lower() for x in ['node','vein','relic','plant','loot']):ns+=['N14','N15']
            if 'cataclysm' in key.lower():ns+=['N07','N12']
            item.add(path,f"三遗迹{key}的真实模式规则：{json.dumps(value,ensure_ascii=False)}。来源全局窗口/容量同时生效；原生组合尚未完整冻结。",missing=ns,gaps=['P07'],gates=['G-01'])
        elif kind=='map':
            if key=='config':item.add(path,'地形RLE/调色盘/投影/原点/高度/DOT；平面格与投影仅是切片。',['E02'],['N12'],['P01'],['G-02'])
            elif key=='metadata':item.add(path,'出生/撤离/箱点/障碍/装饰/走廊；POI不等价资源分布算法。',['E02','E18'],['N12','N14'],['P01','P04'],['G-02'])
            else:item.add(path,'实际地图边界与坐标变换，统一约束移动/弹道/落点。',['E02'],['N12'],['P01'],['G-02'])
        else:raise AssertionError(kind)
    bindings(item,obj)
    if kind=='skill':
        life=obj.get('castLifecycle',{})
        for field in life:
            ns=['N04']; gates=[]
            meaning='完整施法'+field+'规则：'+json.dumps(life[field],ensure_ascii=False)
            if field=='recovery' and (life[field].get('cancelByMove') or life[field].get('cancelByExplicitAction')):
                gates=['OBS-01'];meaning+='；现有共享后摇不能被缩短。'
            item.add('/castLifecycle/'+esc(field),meaning,['E03'],ns,gates=gates)
        for i,rt in enumerate(obj.get('runtimes',[])):
            for field,value in rt.items():
                path=f'/runtimes/{i}/{esc(field)}'
                if field=='id':continue
                es=['E12'];ns=[];gates=[]
                if field in ['shape','motion']:
                    es+=['E10','E11','E13','E14']
                    if not (field=='motion' and value.get('type')=='linear') and not (field=='shape' and value.get('type') in ['circle','capsule']):ns=['N09']
                if field=='origin' and value!='caster':ns=['N09']
                if field=='attachment' and value!='world':ns=['N09']
                if field=='blocking' and (value.get('blocksProjectiles') or value.get('blocksMovement')):ns=['N09']
                if field=='triggers' and set(value)-{'contact'}:ns=['N06']
                if field=='lifecycle' and value.get('endOnSourceInvalid') is False:ns=['N10'];gates=['OBS-02']
                if field=='hitPolicy' and (value.get('maxTargets',1)>32 or value.get('maxTotalHits',1)>128):gates=['OBS-03'];ns=['N09']
                item.add(path,'实际计划'+field+'='+json.dumps(value,ensure_ascii=False)+'；范围/取值/时钟需与现有受控能力核对，不能忽略字段。',es,ns,['P03'],gates)
    if kind=='status':
        if item.row['id'] in D03:item.add('', '属性魔数3与减速/身法文案冲突；禁止默改成maxHP或SPD。',missing=['N05'],gates=['D-03'])
        if len(obj.get('attachedFactors',[]))>1:item.add('', '多个效果属于同一原状态身份；拆成两个状态会改变槽位/刷新/来源/清理语义。',['E05'],['N05'],gates=['OBS-04'])
        if obj.get('duplicateType')=='stack' and 'maxStacks' not in obj:item.add('/duplicateType','原状态stack未明确maxStacks，禁止自行选上限。',['E05'],['N05'],gates=['G-01'])
        if not obj.get('attachedFactors'):item.add('', '无附加因子仍可能有控制/免疫或状态计数行为；需按描述/父技能补精确语义，禁止no-op。',missing=['N05','N06'],gates=['G-01'])
        if item.row['id'] in {'effect_zhenlong_suppress','effect_leidi_field','effect_duduan_execute'}:
            item.add('', '描述与附加因子不完全一致；先追踪父技能，明确攻速/ATK或控制/斩杀由谁产生，禁止凭描述给空状态新增效果。',missing=['N05','N06','N07'],gates=['OBS-05'])
    if kind=='cultivation':
        for i,stage in enumerate(obj.get('stages',[])):
            for j,effect in enumerate(stage.get('effects',[])):
                item.add(f'/stages/{i}/effects/{j}', '原阶段效果绑定：'+json.dumps(effect,ensure_ascii=False)+'；熟练度区间='+str(stage.get('min'))+'..'+str(stage.get('max')),missing=['N16','N06'],gates=['G-01'] if effect.get('type')=='passive' else [])
    if kind=='map':
        cfg=obj['config'];meta=obj['metadata']
        for field,value in cfg.items():
            if field=='palette':
                dots=sorted(set(p['dotDamagePerSecond'] for p in value));heights=sorted(set(p['elevation'] for p in value))
                meaning=f'调色盘{len(value)}种；地形DOT={dots}，高度={heights}。当前等高/零DOT子集可复用，非零DOT与高度/通行语义须补。'
            elif field=='cellsRle':meaning=f'压缩地形{len(value)}段；{cfg["width"]}×{cfg["height"]}格，按原索引/顺序保留，不以新平面样例替代。'
            else:meaning=f'生产坐标/几何字段{field}={json.dumps(value,ensure_ascii=False)}；世界原点/菱形边界需统一变换。'
            item.add('/config/'+esc(field),meaning,['E02'],['N12'],['P01'],['G-02'])
        for field,value in meta.items():
            item.add('/metadata/'+esc(field),f'{field}共{len(value)}项；'+('原装饰类型/足迹碰撞待冻结。' if field=='decorations' else '点位/走廊保留原世界坐标；箱候选点不等价生成/刷新算法。'),['E02'],['N12']+(['N14'] if field=='chestSpawns' else []),['P01'],['G-02'])
    if kind=='ruins':
        item.add('', '三个region同时受epoch-ruins全局窗口、realmBrackets容量（24/16）和深度/兑换/奖励规则约束；不是仅加载region子对象即可运行。',missing=['N02','N18','N16'],gates=['D-08','G-01'])
        item.row['references'].append(dict(kind='context',id='epoch-ruins-global',fromPointer='',source={'file':SOURCE+'source-config.json','pointer':'/tables/epoch-ruins','sha256':digest(SC['tables']['epoch-ruins'])}))
    if kind=='skill' and item.row['id']=='liuyun-huifeng':
        item.add('/vfxMotionGrammar','VFX为orbit_then_pierce，实际runtime为linear+first；按实际计划映射，不自动增加回旋/穿透。',gaps=['I01'],gates=['OBS-06'])
    # Follow referenced skill/profile records with value hashes, so transitive dependencies are observable.
    for path,value in leaves(obj):
        key=path.rsplit('/',1)[-1]
        if isinstance(value,str) and (key.endswith('skillId') or key.endswith('SkillId') or '/skillIds/' in path):
            if value in SKILLS:item.row['references'].append(dict(kind='skill',id=value,fromPointer=path,source={'file':SOURCE+'resolved-config.json','pointer':'/tables/data~1skills/SKILL_REGISTRY/'+esc(value),'sha256':digest(SKILLS[value])}))
        if key=='profileId' and isinstance(value,str):
            profiles=SC['tables']['cultivation-combat-profiles']
            if value in profiles:item.row['references'].append(dict(kind='identity_profile',id=value,fromPointer=path,source={'file':SOURCE+'source-config.json','pointer':'/tables/cultivation-combat-profiles/'+esc(value),'sha256':digest(profiles[value])}))
            else:item.add(path,'无法解析identity_profile，需补充规格。',missing=['N16'],gates=['G-01'])
    return item.finish()


SC=read(SOURCE+'source-config.json'); RC=read(SOURCE+'resolved-config.json')
FACTORS=SC['tables']['factors'];SKILLS=RC['tables']['data/skills']['SKILL_REGISTRY']
D03={'effect_tribulation_paralysis','effect_tribulation_windbound','effect_slow','effect_pill_wind'}


def main(check):
    audit=read(SOURCE+'content-audit.json')['items'];maps=read(SOURCE+'maps.json')['maps']
    morphs=RC['tables']['data/weapon-attack-morphs']['WEAPON_ATTACK_MORPHS']
    loc={
        'skill':('resolved-config.json','/tables/data~1skills/SKILL_REGISTRY'),
        'cultivation':('resolved-config.json','/tables/data~1cultivations/CULTIVATION_REGISTRY'),
        'monster':('resolved-config.json','/tables/data~1monsters/MONSTER_TEMPLATES'),
        'factor':('source-config.json','/tables/factors'),
        'status':('source-config.json','/tables/status-effects'),
        'mapPve':('source-config.json','/tables/map-pve-progression'),
        'ruins':('source-config.json','/tables/epoch-ruins/regions')}
    items=[]
    for a in audit:
        kind=a['kind']
        if kind=='map':file='maps.json';ptr='/maps/'+str(next(i for i,v in enumerate(maps) if v['id']==a['id']))
        elif kind=='weaponMorph':file='resolved-config.json';ptr='/tables/data~1weapon-attack-morphs/WEAPON_ATTACK_MORPHS/'+str(next(i for i,v in enumerate(morphs) if v['id']==a['id']))
        else:file,ptr=loc[kind];ptr+='/'+esc(a['id'])
        raw=pointer(read(SOURCE+file),ptr)
        items.append(route(Item(a,SOURCE+file,ptr,raw)))
    byid={(x['kind'],x['id']):x for x in items}
    for item in items:
        item['dependencyDetails']=[]
        profile_refs=[r for r in item['references'] if r['kind']=='identity_profile']
        for ref in profile_refs:
            profile=pointer(read(ref['source']['file']),ref['source']['pointer'])
            reviewed=[]
            for i,binding in enumerate(profile.get('factors',[])):
                fid=binding['factorId'];es,ns=factor_family(fid)
                route_info=routing(es,ns,['I04'],['G-01'])
                reviewed.append(dict(pointer='/factors/'+str(i),binding=binding,**route_info))
                if fid in FACTORS:
                    item['references'].append(dict(kind='factor',id=fid,fromContext=ref['source'],fromPointer='/factors/'+str(i),source={'file':SOURCE+'source-config.json','pointer':'/tables/factors/'+esc(fid),'sha256':digest(FACTORS[fid])}))
                for field in ['gapIds','workPackages','ruleGates']:item[field]=sorted(set(item[field]+route_info[field]))
            item['dependencyDetails'].append(dict(id=ref['id'],source=ref['source'],original=profile,reviewedBindings=reviewed,uninterpretedFields='其余身份/档位字段交N16/R5.1；未知special继续G-01，不把参数目录当算法'))
    # Carry transitive behavior routes; sources are not reinterpreted through their labels.
    for _ in range(len(items)):
        changed=False
        for item in items:
            # Factor definitions advertise a superset. A fixed binding must not inherit
            # every unsupported parameter/trigger in that catalog definition.
            deps=[byid[(ref['kind'],ref['id'])] for ref in item['references'] if ref['kind']!='factor' and (ref['kind'],ref['id']) in byid]
            for field in ['gapIds','workPackages','ruleGates']:
                merged=sorted(set(item[field]).union(*(set(d[field]) for d in deps)))
                if merged!=item[field]:item[field]=merged;changed=True
        if not changed:break
    for item in items:
        item['referencedSkillClosure']=[dict(id=r['id'],gapIds=byid[('skill',r['id'])]['gapIds'],workPackages=byid[('skill',r['id'])]['workPackages'],ruleGates=byid[('skill',r['id'])]['ruleGates']) for r in item['references'] if r['kind']=='skill']
    emit('content-mapping.json',dict(schemaVersion=1,baseline='0.19.0 / mechanism 34',execution='mapping_only',items=items))
    scenarios=map_scenarios();vectors=map_vectors()
    emit('scenario-mapping.json',scenarios);emit('numeric-mapping.json',vectors)
    emit('capability-catalog.json',CATALOG)
    emit('functional-domain-mapping.json',map_domains())
    decisions=map_decisions(items,scenarios,vectors);emit('rule-gates.json',decisions)
    human_indices(items,scenarios,vectors,decisions)
    verify(items,scenarios,vectors,decisions)
    for name,content in GENERATED.items():
        p=DEST/name
        if check:assert p.read_text()==content, f'generated file drift: {p}'
        else:p.parent.mkdir(parents=True,exist_ok=True);p.write_text(content)
    print(json.dumps({'mode':'check' if check else 'write','items':len(items),'scenarios':len(scenarios['scenarios']),'vectors':len(vectors['vectors']),'leafFields':sum(f['leafCount'] for x in items for f in x['fieldAudit']),'requirements':sum(len(x['requirements']) for x in items),'existingFamilies':len(CATALOG['existing']),'missingFamilies':len(CATALOG['missing']),'generatedFiles':len(GENERATED),'executedOriginalScenarios':0,'executedOriginalVectors':0},ensure_ascii=False))


SCENARIO_ROUTES = {
 1:(['E01'],['N02'],'两名必需席位就绪才共同激活；单成员激活不是等价证据'),
 2:(['E01'],[],'拒绝不注入库存/不进入可玩；精确fixture仍含第二成员'),
 3:(['E01'],['N02'],'缺席超时、席位释放与成本责任'),
 4:(['E02'],[],'归一化移动和一步时距可复用；不得删除原fixture的B'),
 5:(['E02','E20'],[],'NaN只由测试专用解码器构造；JSON字符串不是浮点NaN'),
 6:(['E20'],[],'前后台/保护/停止意图，不自动恢复旧运动'),
 7:(['E02'],['N12'],'原polygon与连续扫掠/边界规则，不以矩形替换'),
 8:(['E09','E15'],['N07','N08'],'原向量逐项贯穿实际公式，不能只校验Python期望'),
 9:(['E06'],['N01'],'来源B对A的原ResolvedDamage是测试域构造，不是公开输入'),
 10:(['E03'],['N05','N04'],'准备阶段Stun是控制效果；CancelAction不能替代'),
 11:(['E03'],['N08','N06'],'重复提交且原技能jinxia-buhuai护盾/减伤记录链真实存在'),
 12:(['E04'],['N01','N04','N08'],'友方严格单位治疗、目标死亡不重定向'),
 13:(['E13'],['N01','N10'],'来源死亡后弹道继续，目标伤害与runtime结束'),
 14:(['E13'],['N01'],'阻挡前后接触保留/裁掉；测试域构造需绑定真实统一接触链'),
 15:(['E12','E13'],['N01'],'重复间隔零只一次、原目标B非NPC替身'),
 16:(['E15'],[],'牵引外免伤、回内受击；直接域事件只能测试专用构造'),
 17:(['E15'],[],'同群最低HP比例且不能选外群/自己'),
 18:(['E18'],['N01','N15'],'两输入同刻竞争同一资源，恰一份收益'),
 19:(['E18'],[],'满包且不可堆叠时资源保留'),
 20:(['E18'],['N04','N15'],'主动Drop/expectedRevision、落点失败不扣物'),
 21:(['E18'],['N01','N15'],'遗物修订与重复/并发领取'),
 22:(['E18'],['N01','N03','N15'],'近距开启私有内容及公共禁字段'),
 23:(['E18'],[],'怪物一次性资源十分钟后不刷新'),
 24:(['E19'],[],'配置开放时刻179999ms拒绝'),
 25:(['E19'],[],'受伤中断撤离，不发终局核心提交'),
 26:(['E19','E20'],[],'核心not_committed不得成功；相同操作ID重试'),
 27:(['E19'],['N16'],'核心已提交、附属未知可靠重试，不回滚核心'),
 28:(['E19'],['N01','N05'],'绝对终局穿保护/无敌，A和B同时终局'),
 29:(['E19'],['N15'],'原死亡slot/quantity与遗物落地同时验证'),
 30:(['E16'],['N17'],'调谐不足成本拒绝切片已有；原加持语义不能以attuned计数替代'),
 31:([],['N17'],'临时加持退出清除'),
 32:([],['N01','N10','N18'],'训练只清本人效果，不删B的世界效果'),
 33:([],['N18'],'论剑终场HP比例同值判平'),
 34:(['E20'],['N18'],'30s竞技断线判负，40s重连只观礼（60s连接窗不延长判负）'),
 35:([],['N18'],'镇岳封印暴动同身份只执行一次'),
 36:([],['N18','N16'],'灾变宗门快照与功勋归属'),
 37:([],['N18'],'四劫检查点身份防串'),
 38:([],['N18'],'阶段失败不越阶且责任可收尾'),
 39:(['E20'],[],'旧连接fencing'),
 40:(['E20'],['N01','N03'],'逐接收者部分发送不相互推进基线'),
 41:(['E20'],[],'旧连接发布回执不推进基线'),
 42:(['E01','E20'],[],'未知准入查询原操作，不重扣'),
 43:(['E20'],[],'A外部IO挂起不阻塞另一个Match B；此处是跨World隔离'),
 44:(['E20'],[],'原输入队列容量1与当前最低16不兼容；先冻结合法容量/精确拒绝入口'),
 45:(['E19','E20'],[],'关局后迟到核心成功不重开World、冻结责任收尾'),
 46:(['E20'],[],'原TEST_FAULT注入需测试专用故障点，现有背压故障不能冒充同一断言'),
 47:(['E20'],['N01','N03','N15'],'重放S04/S18/S40；继承原场景机制依赖，非仅hash重复'),
 48:(['E20'],[],'100轮关闭核对World/任务/输出/待处理操作/线程全部归零；现有局部回收测试不足整体绑定'),
}


def routing(existing,missing,gaps=(),gates=()):
    gs=set(gaps)
    for c in missing:gs.update(CAPS[c]['gapIds'])
    ws={GAP_ROUTES[g] for g in gs if g in GAP_ROUTES}
    for c in missing:ws.update(CAPS[c]['workPackages'])
    return dict(existingCapabilities=existing,missingCapabilities=missing,gapIds=sorted(gs),workPackages=sorted(ws),ruleGates=list(gates),
                primaryWorkPackage=CAPS[missing[0]]['workPackages'][0] if missing else next((GAP_ROUTES[g] for g in sorted(gs) if g in GAP_ROUTES),'R6.2'))


def map_scenarios():
    src=read(SOURCE+'scenarios.json');out=[]
    for i,s in enumerate(src['scenarios']):
        n=int(s['id'][1:]);es,ns,meaning=SCENARIO_ROUTES[n]
        # plain-active includes A AND B. Keep the full fixture requirement, even for a single-actor assertion.
        if s['fixture']!='none' and 'N01' not in ns and 'N02' not in ns:ns=ns+['N01']
        gates=list(s.get('dependsOnDecision',[]))
        if n==34:gates+=['D-07']
        if n==44:gates+=['OBS-03']
        r=routing(es,ns,['I05']+(['I06'] if n in [43,44,48] else []),gates)
        row=dict(id=s['id'],title=s['title'],source={'file':SOURCE+'scenarios.json','pointer':f'/scenarios/{i}','sha256':digest(s)},original=s,
            fixturePreserved=True,exactExecutionStatus='not_run',explanation=meaning,
            acceptanceOwnerWorkPackage='R6.2',
            adapter='待R6.2测试专用typed-domain adapter：原fixture继承/override、逻辑毫秒、同刻列表顺序及domain event identity；不得向production create/input开放patch',**r)
        row['assertions']=[dict(index=j,original=a,sourcePointer=f'/scenarios/{i}/then/{j}',exactExecutionStatus='not_run',**r) for j,a in enumerate(s['then'])]
        row['similarEvidence']=[dict(capability=c,report=BASE_EVIDENCE,contract=CAPS[c]['contractOrEvidence'],test=CAPS[c]['test'],scope='受控相近切片，非本原断言已执行') for c in es]
        out.append(row)
    return dict(schemaVersion=1,fixtureContract=src['fixtureContract'],fixtures=src['fixtures'],fixtureSha256=digest(src['fixtures']),scenarios=out)


def map_vectors():
    src=read(SOURCE+'numeric-vectors.json');out=[]
    routes={
      'realm_modifier':([],['N07'],'完整境界等级差乘区'),
      'defense_damage':(['E15'],[],'守方境界ATK基准/防御帽/最小伤害已有受控乘区，原暴击/破甲/真伤分支另判'),
      'dodge_percent':(['E15'],['N07','N16'],'已有NPC概率闪避，不等价命中惩罚与派生闪避公式'),
      'movement_speed':(['E02'],[],'对数速度参数可配置；rank乘区另判'),
      'heal':(['E08','E09','E15'],[],'普通HP上限裁剪可复用；过量另判，不证明完整治疗来源公式'),
      'death_slots':(['E19'],[],'配置整数槽位损耗；需核对ceil与源slot语义'),
      'death_quantity':(['E19'],[],'整数物品quantity损耗/舍入及上限'),
      'hp_cost':([],['N04'],'HP成本与minHpAfterCost不是SP成本'),
      'pvp_multicast':([],['N09','N18'],'对玩家额外段折损/固有段规则'),
      'duration_ticks':(['E20'],[],'原毫秒ceil到逻辑tick，测试专用时钟适配后验证；不把当前deadline视为相同执行结果'),
      'orb':(['E15','E18'],[],'普通/Boss光球按基准属性比例烘焙、拾取才恢复'),
      'shield_grant':(['E06'],[],'基础maxHP grant/cap及已满盾零grant，不包含反击记录'),
      'move_step':(['E02'],[],'归一化方向×速度×步长，实际位置断言'),
      'session_cultivation':(['E19'],[],'有限积分/带入年限/撤离乘区可配置，完整成长仍M13')}
    for i,v in enumerate(src['vectors']):
        es,ns,meaning=routes[v['op']];ns=list(ns);inp=v['input']
        if v['op']=='defense_damage' and (inp.get('pierceBps',0) or inp.get('irreducible') or inp.get('critMultiplier',1)!=1):ns+=['N07']
        if v['op']=='movement_speed' and inp.get('rankMultiplier',1)!=1:ns+=['N16']
        if v['op']=='heal' and inp.get('allowOverheal'):ns+=['N08']
        gates=['D-02'] if v['op']=='defense_damage' else []
        if v['op']=='movement_speed' and inp.get('rankMultiplier',1)!=1:meaning+='；rankMultiplier=1.1需要完整派生/等级倍率，不能直接声称匹配当前速度函数。'
        if v['op']=='heal' and inp.get('allowOverheal'):meaning+='；allowOverheal=true缺机制。'
        if v['op']=='defense_damage' and ns:meaning+='；此行非平常伤害子集，必须补'+','.join(ns)+'对应乘区。'
        out.append(dict(id=v['id'],source={'file':SOURCE+'numeric-vectors.json','pointer':f'/vectors/{i}','sha256':digest(v)},original=v,exactExecutionStatus='not_run',explanation=meaning,
            acceptanceOwnerWorkPackage='R6.2',
            adapter='R6.2测试专用有类型公式/World执行绑定，按源tolerance核对原expected；不调用来源validate.py充当Rust执行',
            similarEvidence=[dict(capability=c,report=BASE_EVIDENCE,availability='cleaned_by_user',contract=CAPS[c]['contractOrEvidence'],test=CAPS[c]['test'],scope='历史相近乘区/算法引用，报告已清理；尚未绑定此向量') for c in es],**routing(es,ns,['I05'],gates)))
    return dict(schemaVersion=1,tolerance=src['tolerance'],vectors=out)


def map_decisions(items,scenarios,vectors):
    definitions=[
      ('D-01','source_baseline','生命周期优先旧castTime投影','结构化castLifecycle/targeting优先；客户端不能按旧投影决定提交。',['R3.1']),
      ('D-02','draft_requires_confirmation','防御基准/帽/取整','来源草案采用守方境界ATK×2、70%帽；执行原向量前评审签收，不默改expected。',['R3.3','R6.2']),
      ('D-03','open_product_decision','属性魔数冲突','四个原状态的attrType=3是maxHP，文字是移速。产品明确采用行为/原生属性和边界后才实现受影响效果。',['R3.2']),
      ('D-04','source_baseline','恢复以光球为准','不叠加旧击杀立即恢复；原向量及拾取链绑定。',['R3.3','R6.2']),
      ('D-05','open_product_decision','itemsGained经手或净额','产品明确统计口径；保留carried/looted/consumed/kept/dropped事实，不让不明字段影响金额。',['R5.1']),
      ('D-06','draft_requires_confirmation','核心与附属提交','核心失败不得成功；未知先查原操作，附属不得反转核心；完整服务契约冻结后签收。',['R5.1','R6.1']),
      ('D-07','source_baseline','竞技断线30s/连接60s','二者并存；竞技判负后重连仅观礼。',['R5.3']),
      ('D-08','source_baseline','模式席位与地图playerLimit','采用resolved-matchmaking/各模式容量；下一批两席是显式受控子集，不能闭合标准8席和模式容量。',['R2.2','R5.3']),
      ('D-09','draft_requires_confirmation','概率0/1','数学0必不发、1必发；不继承旧RNG消费轨迹，奖池验收前签收。',['R4.3']),
      ('D-10','source_baseline','elemRes缺省转化1','完整属性派生明确缺省及版本；不是丢弃抗性。',['R5.1']),
      ('D-11','source_baseline','重复策略枚举','仅refresh/stack/ignore；sourceScoped分组，不造independent。',['R3.2']),
      ('G-01','open_spec','special/器型/状态语义','每个未知有效绑定给输入、状态、时机、取值对象、排序、上限、正常/拒绝/边界精确预期；不能只补函数名。',['R3.1','R3.2','R3.3','R3.4','R3.5','R5.1','R5.2','R5.3']),
      ('G-02','open_spec','生产碰撞场','冻结装饰类型/程序化/动态阵法/高度/地形DOT及坐标规则，与正式地图代表可达性对照。',['R4.1']),
      ('G-03','open_spec','完整成长公式/事务','补正常撤离、死亡、寿尽转世、满仓/冲突的金额/前后快照；先冻结再由Rust实现。',['R5.1']),
      ('G-04','open_spec','ModeAction与客户端映射','有类型schema、逐字段表和真实解码；SDK-5-18只冻结新的双人有限契约，不宣称旧客户端全部适配。',['R2.3','R3.1','R5.3','R6.1']),
      ('G-05','open_evidence','正式验收环境','冻结真实客户端/DB/Socket、负载机器/队列/阈值/平台并运行；本批映射不是行为证据。',['R6.1','R6.2','R6.3','R6.4','R6.5','R6.6']),
      ('OBS-01','observed_implementation_gap','移动/显式动作取消后摇','原技能recovery.cancelByMove/ExplicitAction=true；现有SDK后摇不可缩短。R3.1补可取消阶段及成本/CD/共享动作边界。',['R3.1']),
      ('OBS-02','observed_implementation_gap','来源死亡后世界效果继续','原runtime.endOnSourceInvalid=false；现有持续场/穿透计划随来源退场。R3.4补独立存续与冻结归属，不得复活来源。',['R3.4']),
      ('OBS-03','open_capacity_contract','原数值与现有限额不兼容','原spiritCost最高8000000、maxTargets64/total1024超现1m/32/128；S44容量1低于当前输入队列16。先冻结合法预算/profile、拒绝和资源界限，不直接放大常量。',['R3.1','R3.4','R6.2','R6.3']),
      ('OBS-04','observed_implementation_gap','一个原状态的复合效果身份','effect_berserk同时ATK+30%/DEF-20%，要保持一个状态槽位/期限/来源；拆成两条不能当作等价。effect_shield则明确为5层DEF增益，不是吸收盾。',['R3.2']),
      ('OBS-05','open_spec','说明与效果结构不完全一致','effect_zhenlong_suppress说明攻速而fix_attribute[1,-30,1]指ATK；effect_leidi_field/execute说明需追踪父技能后再归属，不能凭说明新增状态效果。',['R3.2','R3.3']),
      ('OBS-06','source_precedence','VFX不决定运动','liuyun-huifeng VFX orbit_then_pierce但实际runtime linear+first；采用实际计划，不据美术补回旋穿透。',['R3.4']),
    ]
    result=[]
    for gid,status,title,gate,wps in definitions:
        affected=[f"{x['kind']}:{x['id']}" for x in items if gid in x['ruleGates']]
        if gid=='D-05':affected+=[f"{x['kind']}:{x['id']}" for x in items if x['kind']=='cultivation']
        if gid=='OBS-05':affected += ['status:effect_zhenlong_suppress','status:effect_leidi_field','status:effect_duduan_execute']
        if gid=='OBS-06':affected+=['skill:liuyun-huifeng']
        if gid=='D-08':affected+=['matchmaking:resolved-matchmaking','modes:training/tournament/ruins/cataclysm']
        if not affected:
            families={'D-01':['skill'],'D-02':['factor'],'D-04':['monster'],'D-06':['cultivation'],'D-07':['ruins'],'D-09':['monster'],'D-10':['cultivation'],'D-11':['status'],'G-03':['cultivation'],'G-04':['skill','ruins'],'G-05':[]}
            affected=[f"{x['kind']}:{x['id']}" for x in items if x['kind'] in families.get(gid,[])]
        result.append(dict(id=gid,status=status,title=title,entryGate=gate,owner='项目规则负责人裁定语义；SDK维护者落实有类型契约/边界/验收',workPackages=wps,
            evidence=[SOURCE+'decisions.md'] if gid.startswith(('D-','G-')) else [SOURCE+'resolved-config.json',SOURCE+'source-config.json', 'src/config/validate.rs','src/world/pve.rs'],
            affectedItems=sorted(set(affected)),affectedScenarios=[s['id'] for s in scenarios['scenarios'] if gid in s['ruleGates']],affectedVectors=[v['id'] for v in vectors['vectors'] if gid in v['ruleGates']],resolution=None,
            sourceRequirements=[{'item':x['kind']+':'+x['id'],'file':x['source']['file'],'pointer':x['source']['pointer']+q['pointer'],'sha256':q['sourceSha256']} for x in items for q in x['requirements'] if gid in q['ruleGates']] ))
    return dict(schemaVersion=1,precedence='显式决定 > behavior/boundary与已确认规则 > resolved definitions > raw data > 旧文字；未确认草案保留门槛；美术字段不是runtime计划',rules=result,
        contextualSources=[{'file':SOURCE+'source-config.json','pointer':'/tables/epoch-ruins','purpose':'三遗迹共同窗口/realmBrackets容量/key交换/深度/奖励；三条region不是完整模式'},
          {'file':SOURCE+'resolved-config.json','pointer':'/tables/resolved-matchmaking','purpose':'标准席位与NPC预留；下一批只做显式2席子集'}])


def cell(value):
    return str(value).replace('|','\\|').replace('\n',' ')


def human_indices(items,scenarios,vectors,decisions):
    counts=Counter(x['kind'] for x in items)
    intro=['# SDK-5-17 原需求逐项映射','', '2026-11-05；基线0.19.0 / 机制34。本文和JSON由 `scripts/map_sdk_5_17.py` 离线生成。只完成路由审查，原场景/向量执行数均为0，498项没有因映射而签收。','',
      '每项JSON保留原位置/摘要、原审计记录、全部叶字段审查去向、子行为的P/M/I、现有能力切片与边界、缺失能力/工作包/门槛及引用闭包。原sourceAudit.not-started是历史原件，不作当前实现状态。P/M/I在子行为上可并存，I04始终开放。','',
      '20个已有能力族 + 18个剩余原生能力族是本次排程粒度，不是声称底层只有38个函数或18个开发批次。未知special归needs_spec，后续补规格可以细分族；25工作包及32类缺口总账保持。','',
      '历史证据例外：本轮采集基线摘要后，用户明确清理artifacts目录。能力表保留历史报告身份并标cleaned_by_user；源码/契约与原资料可核对，历史1146/22等只保留已报告口径，当前无法复核原日志。不得把清理前摘要还原为原报告或把文档核对算行为通过。','',
      '- [F-01～F-14原功能域与工作包映射](functional-domain-mapping.json)',
      '- [完整498项机器映射](content-mapping.json)', '- [48场景与122条原断言](scenario-mapping.json) / [50算术向量](numeric-mapping.json)',
      '- [能力族输入/状态/输出/限制](capability-catalog.json) / [规则门槛与影响ID](rule-gates.json)',
      '- [本批范围和验收](../../batches/SDK-5-17-requirements-mapping.md) / [下一批双成员闭环](../../batches/SDK-5-18-two-member-loop.md)',
      '- [路线及25工作包](../../plans/sdk-delivery-roadmap.md) / [三类缺口](../../checklists/sdk-requirements-gap.md) / [本批校验](../../../artifacts/sdk-5-17/verification.json)','',
      '|目录|项数|逐项人读索引|','|---|---:|---|']
    for kind,count in counts.items():
        intro.append(f'|{kind}|{count}|[{kind}.md]({kind}.md)|')
        lines=[f'# {kind}：{count}项逐项路由','', '[索引/口径](README.md)；精确字段、参数、摘要和引用闭包以 [content-mapping.json](content-mapping.json) 为准。所有整项验收均not_run。','',
          '|来源ID / 名称|此项实际字段/参数与能力边界|缺口 / 工作包|规则门槛|','|---|---|---|---|']
        for x in [x for x in items if x['kind']==kind]:
            semantic=[q for q in x['requirements'] if q['status'] not in ['consumer_data'] and q['pointer'] not in ['/id','/name']]
            text='；'.join(q['pointer']+': '+q['meaning'][:200] for q in semantic)
            lines.append('|'+ '|'.join(map(cell,[x['id']+' / '+x['name'],text,','.join(x['gapIds'])+' → '+','.join(x['workPackages']),','.join(x['ruleGates']) or '—']))+'|')
        emit(kind+'.md','\n'.join(lines)+'\n')
    intro+=['','|场景与向量|数量|人读索引|','|---|---:|---|','|场景|48|[scenarios.md](scenarios.md)|','|向量|50|[vectors.md](vectors.md)|','',
      '最先补N01/N02，并在有限双人切片提供N03必要的私有全量发布。两名参与者必须在同一World真实交互；现有“两个Runtime互不阻塞”的证据仅支持跨对局隔离。标准8席/各模式容量、delta/观礼、完整PvP和实例遗物仍在后续，M01/M14保持开放。']
    emit('README.md','\n'.join(intro)+'\n')
    lines=['# 48个原场景路由','', '全部exactExecutionStatus=not_run。原fixture/全部when/每个then精确保留在 [机器表](scenario-mapping.json)；这里列路由，不把相近测试变成原场景通过。plain-active含A/B，两人fixture不裁掉B。','', '|ID / 标题|逐断言数|差异与执行入口|缺口 / 工作包|','|---|---:|---|---|']
    for x in scenarios['scenarios']:lines.append('|'+ '|'.join(map(cell,[x['id']+' '+x['title'],len(x['assertions']),x['explanation'],','.join(x['gapIds'])+' → '+','.join(x['workPackages'])]))+'|')
    emit('scenarios.md','\n'.join(lines)+'\n')
    lines=['# 50个原算术向量路由','', '全部not_run；[机器表](numeric-mapping.json)逐字保留原input/expected及公差，后续由测试专用Rust公式/World适配执行；原validate.py不是Rust证据。','', '|ID / 运算|原input → expected|已有切片及缺失乘区|工作包|','|---|---|---|---|']
    for x in vectors['vectors']:lines.append('|'+ '|'.join(map(cell,[x['id']+' / '+x['original']['op'],json.dumps(x['original']['input'],ensure_ascii=False)+' → '+str(x['original']['expected']),x['explanation']+' / '+','.join(x['missingCapabilities']),','.join(x['workPackages'])]))+'|')
    emit('vectors.md','\n'.join(lines)+'\n')
    lines=['# 规则门槛、差异和准入条件','', '[机器台账](rule-gates.json)保留每条影响ID和工作包。D-03/D-05待产品裁定；草案不冒充已确认。OBS表示本轮观察，不是新增产品决定或新的M大类。','', '|ID / 状态|问题 / 进入实现前条件|影响条目数 / 工作包|','|---|---|---|']
    for r in decisions['rules']:lines.append('|'+ '|'.join(map(cell,[r['id']+' / '+r['status'],r['title']+'：'+r['entryGate'],str(len(r['affectedItems']))+' / '+','.join(r['workPackages'])]))+'|')
    emit('rules.md','\n'.join(lines)+'\n')


def verify(items,scenarios,vectors,decisions):
    assert len(items)==498 and len({(x['kind'],x['id']) for x in items})==498
    assert len(scenarios['scenarios'])==48 and len(vectors['vectors'])==50
    validwps=set(GAP_WP);validgates={r['id'] for r in decisions['rules']}
    allrows=items+scenarios['scenarios']+vectors['vectors']
    for x in allrows:
        assert set(x['workPackages'])<=validwps
        assert set(x.get('ruleGates',[]))<=validgates
        assert digest(pointer(read(x['source']['file']),x['source']['pointer']))==x['source']['sha256']
        assert x.get('exactAcceptance',x.get('exactExecutionStatus'))=='not_run'
    for x in items:
        raw=pointer(read(x['source']['file']),x['source']['pointer'])
        covered=[]
        for f in x['fieldAudit']:
            value=pointer(raw,f['pointer'])
            assert digest(value)==f['sha256']
            subfields=list(leaves(value,f['pointer']))
            assert len(subfields)==f['leafCount'] and f['requirements']
            covered += [p for p,_ in subfields]
        assert len(covered)==len(set(covered)) and set(p for p,_ in leaves(raw))==set(covered)
        for r in x['requirements']:
            assert set(r['workPackages'])<=validwps and set(r['ruleGates'])<=validgates
            assert set(r['existingCapabilities']+r['missingCapabilities'])<=set(CAPS)
            assert r['primaryWorkPackage'] in r['workPackages']
        for ref in x['references']:
            if 'source' in ref:assert digest(pointer(read(ref['source']['file']),ref['source']['pointer']))==ref['source']['sha256']
    src=read(SOURCE+'scenarios.json');num=read(SOURCE+'numeric-vectors.json')
    assert scenarios['fixtures']==src['fixtures'] and scenarios['fixtureContract']==src['fixtureContract']
    for x,s in zip(scenarios['scenarios'],src['scenarios']):
        assert x['original']==s and [a['original'] for a in x['assertions']]==s['then']
    assert [x['original'] for x in vectors['vectors']]==num['vectors']
    assert vectors['tolerance']==num['tolerance']
    for c in CATALOG['existing']:
        for key in ['code','contractOrEvidence']:assert (ROOT/c[key]).is_file(),c
        assert c['historicalReportAvailability']=='cleaned_by_user_2026-11-05'
        if c['test']:
            name=c['test'].rsplit('::',1)[-1]
            assert any('fn '+name+'(' in p.read_text() for p in (ROOT/'src').rglob('*.rs')),name
    for c in CATALOG['missing']:assert set(c['workPackages'])<=validwps


def map_domains():
    source=(ROOT/(SOURCE+'acceptance.md')).read_text().splitlines()
    current=(ROOT/'docs/checklists/sdk-requirements-gap.md').read_text().splitlines()
    families=[['E01'],['E02','E03','E04'],['E05','E06','E07','E08','E09','E15'],['E03','E04','E10','E11','E12','E13','E14'],['E02','E15'],['E18'],['E19'],['E16','E17'],[],['E19'],['E20'],['E20'],['E01','E19','E20'],['E20']]
    missing=[['N01','N02'],['N04','N09','N12'],['N05','N06','N07','N08'],['N04','N06','N09','N10','N11'],['N12','N13'],['N14','N15'],['N15','N16'],['N15','N17'],['N18'],['N05','N07','N16','N18'],['N02','N18'],['N03'],['N16'],[]]
    rows=[]
    for n in range(1,15):
        fid=f'F-{n:02}'; original=next(l for l in source if l.startswith('| '+fid+' '));now=next(l for l in current if l.startswith('| '+fid+' '))
        gaps=sorted(set(re.findall(r'\b(?:P|M|I)\d{2}\b',now)))
        if n in [13,14]:gaps+=['I02','I03'] if n==13 else ['P09','I06','I07','I08','I09']
        gates={1:['D-08','G-04'],2:['G-02','G-04'],3:['D-02','D-03','G-01'],4:['D-01','G-01'],5:['G-02'],6:['D-09','G-01'],7:['D-05','D-06','G-03'],8:['G-01'],9:['D-07','D-08','G-01','G-04'],10:['D-03','G-01','G-03'],11:['D-07'],12:['G-04'],13:['D-05','D-06','G-03','G-04'],14:['G-05']}
        rows.append(dict(id=fid,sourceFile=SOURCE+'acceptance.md',originalRow=original,sourceSha256=digest(original),currentMatrixFile='docs/checklists/sdk-requirements-gap.md',currentRow=now,
            wholeDomainStatus='open',exactOriginalAcceptance='not_run',**routing(families[n-1],missing[n-1],gaps,gates[n])))
    assert len(rows)==14 and len({x['id'] for x in rows})==14
    return dict(schemaVersion=1,domains=rows)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true')
    main(parser.parse_args().check)
