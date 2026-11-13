"""Validate same-archive paid cost effects and actual counter/exhaustion traces."""
import copy,json
from jsonschema import Draft202012Validator

def verify_paid_schema(root,out,package,tar):
    def load(name):
        path='docs/interfaces/'+name+'.schema.json';raw=tar.extractfile(package['rootName']+'/'+path).read();assert raw==(root/path).read_bytes();x=json.loads(raw);Draft202012Validator.check_schema(x);return x,Draft202012Validator(x)
    schema,v=load('paid-resource-effects-v1');common,c=load('combat-rules-v1');_,counter=load('status-counters-v1');_,ends=load('status-ends-v1');_,loadout=load('skill-loadouts-v1');views,_=load('publications-v6');own=Draft202012Validator({'$ref':'#/$defs/privateMemberV4','$defs':views['$defs']});trigger=c.evolve(schema={'$ref':'#/$defs/trigger_rule','$defs':common['$defs']})
    cfg=Draft202012Validator({'$ref':'#/$defs/counter_config','$defs':schema['$defs']});configs=runtime=browser_states=browser_events=0;fixtures=[]
    for label in ['a','b']:
        for file in [f'paid-{label}.json',f'paid-accounts-{label}.json']:
            raw=tar.extractfile(package['rootName']+'/samples/combat/'+file).read();assert raw==(root/'examples/combat-host'/file).read_bytes();x=json.loads(raw)
            for a in x.get('participants',[]):loadout.validate(a['skills']);configs+=1
            if 'combat_rules'in x:
                fixtures.append(x)
                for d in x['pve']['status_definitions']:v.validate(d);configs+=1
                cfg.validate(x['pve']['status_definitions'][0]['stack_counter']);configs+=1
                t=x['combat_rules']['triggers'][0];trigger.validate(t);v.validate(t['effect']);configs+=2
                for rule in x['combat_rules']['status_ends'].values():ends.validate(rule);configs+=1
    def state(s):
        own.validate(s);n=1
        for status in s['statuses']:v.validate(status);n+=1;assert 'zero_policy'not in status
        return n
    def event(e,recipient):
        if e['kind']=='paid_spirit_restored':
            v.validate(e);assert recipient==e['source']['id']and e['requested']==e['restored']+e['excess'];return 1
        if e['kind']=='status_consumed':c.validate(e);return 1
        if e['kind']in['status_applied','status_refreshed','status_stacked']:v.validate(e['status']);return 1
        return 0
    for scenario in ['paid-four','paid-eight']:
        for browser in[False,True]:
            folder='independent-browser'if browser else'independent-combat';suffix='paid-browser-facts'if browser else'paid-facts';x=json.loads((out/f'delivery/{folder}/{scenario}.{suffix}.json').read_text());assert len(x['states'])==24;count=sum(state(s['own'])for s in x['states'])
            if browser:browser_states+=count;rows=[{'recipient':m['participant'],'event':r['event']}for m in x['eventsByMember']for r in m['events']if'event'in r]
            else:runtime+=count;rows=x['events']
            count=sum(event(r['event'],r['recipient'])for r in rows)
            if browser:browser_events+=count
            else:runtime+=count
            refunds=[r['event']for r in rows if r['event']['kind']=='paid_spirit_restored'];assert len(refunds)==12
            for pid in ['P00','P01','P02','P03']:
                sequence=[r['own']for r in x['states']if r['pid']==pid];assert [next((s.get('stacks')for s in r['statuses']if s['definition_id']=='charges'),0)for r in sequence]==[3,2,1,0,0,0]
    bad=[]
    def change(base,key,value):b=copy.deepcopy(base);b[key]=value;bad.append((v,b))
    effect=fixtures[0]['combat_rules']['triggers'][0]['effect'];definition=fixtures[0]['pve']['status_definitions'][0]
    for value in [0,10001,None,1.5]:change(effect,'bps',value)
    change(effect,'paid_spirit',1000)
    for value in ['unknown',None]:b=copy.deepcopy(definition);b['stack_counter']['zero_policy']=value;bad.append((v,b))
    b=copy.deepcopy(definition);b['stack_counter']['initial']=0;bad.append((v,b))
    change(definition,'zero_policy','remove')
    original=fixtures[0]['combat_rules']['triggers'][0]
    for key,value in [('when','skill_committed'),('include_periodic',True),('counting','action_total'),('counter_scope','target')]:b=copy.deepcopy(original);b[key]=value;bad.append((trigger,b))
    b=copy.deepcopy(original);b['conditions']['status']['subject']='other';bad.append((trigger,b));b=copy.deepcopy(original);b['conditions']['other_hp_at_most_bps']=5000;bad.append((trigger,b))
    native=json.loads((out/'delivery/independent-combat/paid-four.paid-facts.json').read_text());event_sample=next(r['event']for r in native['events']if r['event']['kind']=='paid_spirit_restored')
    for key,value in [('paid_spirit',8000001),('requested',-1),('restored',None),('action_id',None),('zero_policy','remove')]:change(event_sample,key,value)
    for probe,b in bad:assert not probe.is_valid(b),b
    assert configs==24 and runtime==152 and browser_states==88 and browser_events==64 and len(bad)==20,(configs,runtime,browser_states,browser_events,len(bad))
    return {'paidResourceConfigFragments':configs,'paidResourceRuntimeFragments':runtime,'paidResourceBrowserStates':browser_states,'paidResourceBrowserEvents':browser_events,'invalidPaidResourceFragmentsRejected':len(bad)}
