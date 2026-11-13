"""Bind finite-end schemas to the exact archive and actual Node/H5 traces."""
import hashlib
import json
from pathlib import Path
from jsonschema import Draft202012Validator

def verify_derived_schema(root,out,package,tar):
    name='docs/interfaces/status-ends-v1.schema.json';raw=tar.extractfile(package['rootName']+'/'+name).read();assert raw==(root/name).read_bytes();schema=json.loads(raw);Draft202012Validator.check_schema(schema);v=Draft202012Validator(schema)
    common=json.loads((root/'docs/interfaces/combat-rules-v1.schema.json').read_text());c=Draft202012Validator(common)
    projection=json.loads((root/'docs/interfaces/publications-v6.schema.json').read_text());own=Draft202012Validator({'$ref':'#/$defs/privateMemberV4','$defs':projection['$defs']});monster=Draft202012Validator({'$ref':'#/$defs/monsterV4','$defs':projection['$defs']})
    def validate_state(state):
        (monster if 'entity'in state else own).validate(state)
        for status in state['statuses']:
            if 'periodic'in status:c.validate(status)
            if 'shield'in status:c.validate(status['shield'])
    config_count=0;runtime=[];browser_states=[];browser_events=[]
    for label in ['a','b']:
        path=root/f'examples/combat-host/derived-{label}.json';data=tar.extractfile(package['rootName']+'/samples/combat/'+path.name).read();assert data==path.read_bytes();rules=json.loads(data)['combat_rules']
        for fragment in [*rules['status_ends'].values(),*rules['delayed_damage'].values(),rules['max_status_end_releases_per_step']]:v.validate(fragment);config_count+=1
        for fragment in rules['shield_records'].values():c.validate(fragment);config_count+=1
    for scenario in ['derived-four','derived-eight']:
        data=json.loads((out/f'delivery/independent-combat/{scenario}.derived-facts.json').read_text())
        assert len(data['states'])==4
        for state in data['states']:
            validate_state(state);runtime.append(state)
        for row in data['events']:
            e=row['event'];kind=e['kind']
            if kind=='status_end_resolved':v.validate(e);runtime.append(e);assert row['recipient']in [e['source']['id'],e['target']['id']]
            if kind in ['periodic_tick','periodic_heal_tick','shield_record_ended']:c.validate(e);runtime.append(e)
        data=json.loads((out/f'delivery/independent-browser/{scenario}.derived-browser-facts.json').read_text());assert len(data['states'])==4
        for state in data['states']:validate_state(state);browser_states.append(state)
        ends=0
        for member in data['eventsByMember']:
            for envelope in member['events']:
                e=envelope['event']if envelope.get('kind')=='private'else envelope;kind=e.get('kind')
                if kind=='status_end_resolved':v.validate(e);ends+=1;browser_events.append(e);assert member['participant']in [e['source']['id'],e['target']['id']]
                if kind in ['periodic_tick','periodic_heal_tick','shield_record_ended']:c.validate(e);browser_events.append(e)
        assert ends==2
    examples=[None,{},0,129,{'on':[],'target_statuses':['a']},{'on':['reset'],'target_statuses':['a']},{'on':['expired','expired'],'target_statuses':['a']},{'on':['expired'],'target_statuses':[]},{'on':['expired'],'target_statuses':['a','a']},{'on':['expired'],'target_statuses':['a','b','c']},{'on':['expired'],'target_statuses':['a'],'derived':True},{'fixed':None,'source_power_bps':1,'target_missing_hp_bps':0,'caps_by_target_profile':{}},{'fixed':0,'source_power_bps':0,'target_missing_hp_bps':0,'caps_by_target_profile':{}},{'fixed':0,'source_power_bps':1,'target_missing_hp_bps':100001,'caps_by_target_profile':{}},{'fixed':0,'source_power_bps':1,'target_missing_hp_bps':0,'caps_by_target_profile':{'a':100001}},{'kind':'status_ends_suppressed','at_ms':0,'count':0},{'kind':'status_ends_suppressed','at_ms':0,'count':1,'source':'secret'}]
    for bad in examples:assert not v.is_valid(bad),bad
    v.validate({'kind':'status_ends_suppressed','at_ms':120,'count':76})
    assert config_count==8 and len(runtime)==32 and len(browser_states)==8 and len(browser_events)==24,(config_count,len(runtime),len(browser_states),len(browser_events))
    return {'derivedSchema':name,'derivedSchemaSha256':hashlib.sha256(raw).hexdigest(),'derivedConfigFragments':config_count,'derivedRuntimeFragments':len(runtime),'derivedBrowserStates':len(browser_states),'derivedBrowserEvents':len(browser_events),'invalidDerivedFragmentsRejected':len(examples)}
