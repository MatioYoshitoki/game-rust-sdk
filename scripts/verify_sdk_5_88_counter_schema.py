"""Validate same-archive explicit counters and actual Node/H5 zero/layer/window traces."""
import hashlib,json
from jsonschema import Draft202012Validator

def verify_counter_schema(root,out,package,tar):
    name='docs/interfaces/status-counters-v1.schema.json';raw=tar.extractfile(package['rootName']+'/'+name).read();assert raw==(root/name).read_bytes();schema=json.loads(raw);Draft202012Validator.check_schema(schema);v=Draft202012Validator(schema)
    common=json.loads((root/'docs/interfaces/combat-rules-v1.schema.json').read_text());c=Draft202012Validator(common);sg=Draft202012Validator(json.loads((root/'docs/interfaces/source-gates-v1.schema.json').read_text()));loadout=Draft202012Validator(json.loads((root/'docs/interfaces/skill-loadouts-v1.schema.json').read_text()));views=json.loads((root/'docs/interfaces/publications-v6.schema.json').read_text());own=Draft202012Validator({'$ref':'#/$defs/privateMemberV4','$defs':views['$defs']});config_count=0
    for label in ['a','b']:
        for file in [f'counter-{label}.json',f'counter-accounts-{label}.json']:
            data=tar.extractfile(package['rootName']+'/samples/combat/'+file).read();assert data==(root/'examples/combat-host'/file).read_bytes();x=json.loads(data)
            for p in x.get('participants',[]):loadout.validate(p['skills']);config_count+=1
            if 'combat_rules'in x:
                d=next(d for d in x['pve']['status_definitions']if'stack_counter'in d);v.validate(d);c.validate(d);config_count+=2
                for rule in x['combat_rules']['triggers']:
                    c.evolve(schema={'$ref':'#/$defs/trigger_rule','$defs':common['$defs']}).validate(rule);config_count+=1
                    if rule['effect']['kind']=='add_status_stacks':v.validate(rule['effect']);config_count+=1
                    if 'stack_change'in rule:v.validate(rule['stack_change']);config_count+=1
                for field in ['status_windows','status_policies']:
                    for rule in x['combat_rules'][field].values():c.validate(rule);config_count+=1
    def state(s):
        own.validate(s);count=1
        for status in s['statuses']:
            if 'stack_counter'in status:v.validate(status);c.validate(status);count+=1
            assert not any(k in status for k in ['derived','action_origin','stack_change'])
        return count
    def event(e):
        if e.get('kind')=='combat_trigger_fired':sg.validate(e);return True
        if e.get('kind')in ['status_consumed','window_record_ended']:c.validate(e);return True
        return False
    runtime=browser_states=browser_events=terminal_node=terminal_browser=0
    for scenario in ['counter-four','counter-eight']:
        data=json.loads((out/f'delivery/independent-combat/{scenario}.counter-facts.json').read_text());assert len(data['states'])==5
        for s in data['states']:runtime+=state(s)
        terminal=[]
        for row in data['events']:
            e=row['event']
            if event(e):runtime+=1
            if e.get('kind')=='combat_action_total'and e['trigger_id']=='gain-seed':
                c.validate(e);assert row['recipient']==e['source']['id']==e['action_source']['id']
                terminal.append(e)
        assert len(terminal)==(8 if scenario=='counter-eight'else 0)
        terminal_node+=len(terminal)
        data=json.loads((out/f'delivery/independent-browser/{scenario}.counter-browser-facts.json').read_text());assert len(data['states'])==5
        for s in data['states']:browser_states+=state(s)
        terminal=[]
        for member in data['eventsByMember']:
            for envelope in member['events']:
                e=envelope.get('event',{})if envelope.get('kind')=='private'else envelope
                if event(e):browser_events+=1
                if e.get('kind')=='combat_action_total'and e['trigger_id']=='gain-seed':
                    c.validate(e);assert envelope['kind']=='private'
                    assert member['participant']==e['source']['id']==e['action_source']['id']
                    terminal.append(e)
        assert len(terminal)==(8 if scenario=='counter-eight'else 0)
        terminal_browser+=len(terminal)
    base=json.loads((root/'examples/combat-host/counter-a.json').read_text())['pve']['status_definitions'][0];status=json.loads((out/'delivery/independent-combat/counter-four.counter-facts.json').read_text())['original'];bad=[]
    import copy
    for path,value in [('stack_counter',None),('stack_counter',True),('base_modifiers',[]),('base_modifiers',None),('duplicate_policy','stack'),('source_scoped',False),('flags',['root'])]:
        b=copy.deepcopy(base);b[path]=value;bad.append(b)
    for path,value in [('stack_counter',None),('stack_counter',False),('stack_counter',{}),('stacks',129),('max_stacks',0),('base_modifiers',[]),('action_origin','basic'),('derived',False)]:
        b=copy.deepcopy(status);b[path]=value;bad.append(b)
    bad.extend([{'kind':'add_status_stacks','subject':'owner','status_id':'seed-carrier','amount':0},{'kind':'add_status_stacks','subject':'owner','status_id':'seed-carrier','amount':1,'refresh_duration':None},{'definition_id':'seed-carrier','at_least':0},{'definition_id':'seed-carrier','at_least':129},{'definition_id':'seed-carrier','at_least':3,'extra':True}])
    for b in bad:assert not v.is_valid(b),b
    assert(config_count,runtime,browser_states,browser_events)==(32,48,18,30),(config_count,runtime,browser_states,browser_events)
    assert (terminal_node,terminal_browser)==(8,8)
    return {'terminalCounterNodeEvents':terminal_node,'terminalCounterBrowserEvents':terminal_browser,'stackWindowSchema':name,'stackWindowSchemaSha256':hashlib.sha256(raw).hexdigest(),'stackWindowConfigFragments':config_count,'stackWindowRuntimeFragments':runtime,'stackWindowBrowserStates':browser_states,'stackWindowBrowserEvents':browser_events,'invalidStackWindowFragmentsRejected':len(bad)}
