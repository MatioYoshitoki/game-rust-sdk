"""Schema checks against the delivered source-gate configuration and actual traces."""
import hashlib,json
from jsonschema import Draft202012Validator

def verify_gates_schema(root,out,package,tar):
    name='docs/interfaces/source-gates-v1.schema.json';raw=tar.extractfile(package['rootName']+'/'+name).read();assert raw==(root/name).read_bytes();schema=json.loads(raw);Draft202012Validator.check_schema(schema);v=Draft202012Validator(schema)
    common=json.loads((root/'docs/interfaces/combat-rules-v1.schema.json').read_text());c=Draft202012Validator(common);build=Draft202012Validator(json.loads((root/'docs/interfaces/character-build-v1.schema.json').read_text()));loadout=Draft202012Validator(json.loads((root/'docs/interfaces/skill-loadouts-v1.schema.json').read_text()));views=json.loads((root/'docs/interfaces/publications-v6.schema.json').read_text());own=Draft202012Validator({'$ref':'#/$defs/privateMemberV4','$defs':views['$defs']});monster=Draft202012Validator({'$ref':'#/$defs/monsterV4','$defs':views['$defs']})
    config_count=0
    for label in ['a','b']:
        for file in [f'source-{label}.json',f'source-accounts-{label}.json']:
            data=tar.extractfile(package['rootName']+'/samples/combat/'+file).read();assert data==(root/'examples/combat-host'/file).read_bytes();x=json.loads(data)
            for p in x.get('participants',[]):build.validate(p['combat_build']);loadout.validate(p['skills']);config_count+=2
            if 'combat_rules'in x:
                rules=x['combat_rules'];build.validate(rules['build_rules']);config_count+=1
                for rule in rules['triggers']:
                    c.evolve(schema={'$ref':'#/$defs/trigger_rule','$defs':common['$defs']}).validate(rule);v.validate({k:rule[k]for k in ['action_filter','owner_skill','maximum_per_action']});config_count+=2
                for rule in rules['contact_reactions'].values():c.validate(rule);v.validate(rule['source_requirement']);config_count+=2
    def state(s):
        (monster if 'entity'in s else own).validate(s);assert not any(k in json.dumps(s)for k in ['action_origin','minimum_mastery','model_id'])
        for status in s['statuses']:
            if 'periodic'in status:c.validate(status)
    def event(e):
        if e.get('kind')=='combat_trigger_fired':v.validate(e);return True
        if e.get('kind')in ['periodic_tick','contact_reaction_resolved']:c.validate(e);return True
        return False
    runtime=[];browser_states=[];browser_events=[]
    for scenario in ['source-four','source-eight']:
        data=json.loads((out/f'delivery/independent-combat/{scenario}.source-gates-facts.json').read_text());assert len(data['states'])==3
        for s in data['states']:state(s);runtime.append(s)
        loadout.validate(data['receipt'])
        for row in data['events']:
            if event(row['event']):assert row['recipient']==row['event']['source']['id'];runtime.append(row['event'])
        data=json.loads((out/f'delivery/independent-browser/{scenario}.source-gates-browser-facts.json').read_text());assert len(data['states'])==3
        for s in data['states']:state(s);browser_states.append(s)
        loadout.validate(data['receipt']['receipt'])
        for member in data['eventsByMember']:
            for envelope in member['events']:
                e=envelope.get('event',{})if envelope.get('kind')=='private'else envelope
                if event(e):assert member['participant']==e['source']['id'];browser_events.append(e)
    invalid=[None,{}, {'basic':False,'skills':{'mode':'only','ids':[]}},{'basic':True,'skills':{'mode':'any','ids':['x']}},{'basic':True,'skills':{'mode':'only','ids':['x','x']}},{'basic':True,'skills':{'mode':'other'}},{'skill_id':'x','minimum_mastery':None},{'skill_id':'x','minimum_mastery':128},{'skill_id':'x','equipped_any_of':[]},{'skill_id':'x','equipped_any_of':['x','x']},{'skill_id':'x','equipped_any_of':None},{'maximum_per_action':0},{'maximum_per_action':9},{'maximum_per_action':None},{'owner_skill':None},{'source_requirement':None},{'action_filter':{'basic':False,'skills':{'mode':'any'}},'public_source_override':True},{'kind':'combat_trigger_fired','source':{'kind':'participant','id':'P00','generation':0},'trigger_id':'x'}]
    for bad in invalid:assert not v.is_valid(bad),bad
    assert (config_count,len(runtime),len(browser_states),len(browser_events))==(42,55,6,49),(config_count,len(runtime),len(browser_states),len(browser_events))
    return {'sourceGateSchema':name,'sourceGateSchemaSha256':hashlib.sha256(raw).hexdigest(),'sourceGateConfigFragments':config_count,'sourceGateRuntimeFragments':len(runtime),'sourceGateBrowserStates':len(browser_states),'sourceGateBrowserEvents':len(browser_events),'invalidSourceGateFragmentsRejected':len(invalid)}
