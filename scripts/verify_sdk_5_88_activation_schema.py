"""Check exact archived conditional configs and the actual Node/H5 lifecycle traces."""
import copy,json
from jsonschema import Draft202012Validator


def verify_activation_schema(root,out,package,tar):
    def load(name):
        path='docs/interfaces/'+name+'.schema.json'
        raw=tar.extractfile(package['rootName']+'/'+path).read()
        assert raw==(root/path).read_bytes(),path
        schema=json.loads(raw);Draft202012Validator.check_schema(schema)
        return schema,Draft202012Validator(schema)
    schema,v=load('conditional-activation-v1');common,c=load('combat-rules-v1');_,sg=load('source-gates-v1');_,steal=load('resource-steal-v1');_,unit=load('unit-effects-v1');views,_=load('publications-v6')
    own=Draft202012Validator({'$ref':'#/$defs/privateMemberV4','$defs':views['$defs']})
    trigger=c.evolve(schema={'$ref':'#/$defs/trigger_rule','$defs':common['$defs']})
    config_count=runtime_count=browser_states=browser_events=0
    fixtures=[]
    for label in ['a','b']:
        name=f'activation-{label}.json';raw=tar.extractfile(package['rootName']+'/samples/combat/'+name).read();assert raw==(root/'examples/combat-host'/name).read_bytes();x=json.loads(raw);fixtures.append(x)
        for d in x['pve']['status_definitions']:v.validate(d);config_count+=1
        skill=x['skill'];unit.validate(skill);v.validate(skill['resource_steal']);steal.validate(skill['resource_steal']);config_count+=3
        bonus=skill['resource_steal']['status_bonus'];v.validate(bonus);sg.validate(bonus['qualification']);config_count+=2
        for rule in x['combat_rules']['triggers']:
            trigger.validate(rule);v.validate(rule['effect']);sg.validate(rule['owner_skill']);config_count+=3
        for rule in x['combat_rules']['status_policies'].values():c.validate(rule);config_count+=1
    def status(s):
        v.validate(s)
        assert not any(k in s for k in ['qualification','maximum_mastery','action_origin','resource_budget','model_id','derived'])
    def state(row):
        own.validate(row['own']);n=1
        for s in row['own']['statuses']+row['target']['statuses']:status(s);n+=1
        return n
    def event(e,recipient):
        n=0
        if e['kind']=='resource_stolen':v.validate(e);n=1
        elif e['kind']=='status_consumed':c.validate(e);n=1
        elif e['kind']=='unit_channel_pulse':unit.validate(e);n=1
        elif e['kind'] in ['status_applied','status_stacked','status_refreshed']:status(e['status']);n=1
        if e['kind'] in ['resource_stolen','status_consumed','combat_trigger_fired','status_applied','status_stacked','status_refreshed']:
            source=e.get('source',e.get('status',{}).get('source'))
            if source and source['kind']=='participant':assert recipient==source['id'],(recipient,e)
        return n
    for scenario in ['activation-four','activation-eight']:
        for browser in [False,True]:
            folder='independent-browser' if browser else 'independent-combat';suffix='activation-browser-facts'if browser else'activation-facts';data=json.loads((out/f'delivery/{folder}/{scenario}.{suffix}.json').read_text());assert len(data['states'])==6
            n=sum(state(s)for s in data['states'])
            if browser:browser_states+=n
            else:runtime_count+=n
            for key in ['clockBefore','clockAfter']:status(data[key])
            assert data['clockBefore']['instance_id']==data['clockAfter']['instance_id'] and data['clockBefore']['stacks']==data['clockAfter']['stacks']==1 and data['clockBefore']['expires_at_ms']<data['clockAfter']['expires_at_ms']
            if browser:rows=[{'recipient':m['participant'],'event':r['event']}for m in data['eventsByMember']for r in m['events']if 'event'in r]
            else:rows=data['events']
            count=sum(event(r['event'],r['recipient'])for r in rows)
            if browser:browser_events+=count
            else:runtime_count+=count+2
    # Invalid local shapes; cross-reference, reachable caps and stage ordering are native checks.
    bad=[];bundle=fixtures[0]['combat_rules']['triggers'][0]['effect'];refresh=fixtures[0]['combat_rules']['triggers'][2]['effect'];bonus=fixtures[0]['skill']['resource_steal']['status_bonus'];requirement=bonus['qualification']
    def changed(base,key,value):
        b=copy.deepcopy(base)
        if value is ...:b.pop(key,None)
        else:b[key]=value
        bad.append(b)
    for key,value in [('owner_statuses',None),('owner_statuses',[]),('other_statuses',None),('owner_statuses',['a','b','c']),('other_statuses',['a','a']),('private_budget',1)]:
        if key=='owner_statuses'and value==[]:
            b=copy.deepcopy(bundle);b['owner_statuses']=[];b['other_statuses']=[];bad.append(b)
        else:changed(bundle,key,value)
    for key,value in [('mode',...),('mode',None),('mode','implicit'),('qualification',None),('per_stack_bps',0),('maximum_bonus_bps',100001),('subject','target'),('definition_id',''),('private_budget',1)]:changed(bonus,key,value)
    for key,value in [('maximum_mastery',None),('maximum_mastery',128),('maximum_mastery',-1),('minimum_mastery',1.5),('trusted',True)]:changed(requirement,key,value)
    for key,value in [('duration_ms',None),('duration_ms',0),('duration_ms',3600001),('subject','ally'),('status_id',''),('add_stacks',1)]:changed(refresh,key,value)
    numeric=next(d for d in fixtures[0]['pve']['status_definitions']if'modifiers'in d)
    for key,value in [('flags',['root']),('source_scoped',False),('modifiers',[]),('qualification',{}),('mechanism','periodic_damage')]:changed(numeric,key,value)
    for b in bad:assert not v.is_valid(b),b
    assert config_count==44 and runtime_count>100 and browser_states>=24 and browser_events>100,(config_count,runtime_count,browser_states,browser_events)
    return {'conditionalActivationConfigFragments':config_count,'conditionalActivationRuntimeFragments':runtime_count,'conditionalActivationBrowserStates':browser_states,'conditionalActivationBrowserEvents':browser_events,'invalidConditionalActivationFragmentsRejected':len(bad)}
