"""Check same-archive cast-interception rules against actual Node and H5 facts."""

import copy
import json

from jsonschema import Draft202012Validator


def verify_interception_schema(root, out, package, tar):
    def same_archive(path, local=None):
        raw = tar.extractfile(package['rootName']+'/'+path).read()
        assert raw == (root/(local or path)).read_bytes()
        return json.loads(raw)

    def schema(name):
        x = same_archive('docs/interfaces/'+name+'.schema.json')
        Draft202012Validator.check_schema(x)
        return x

    def part(x, name):
        return Draft202012Validator({'$ref':'#/$defs/'+name,'$defs':x['$defs']})

    local = schema('cast-interception-v1')
    common = Draft202012Validator(schema('combat-rules-v1'))
    validator = Draft202012Validator(local)
    publications = schema('publications-v6')
    own = part(publications, 'privateMemberV4')
    monster = part(publications, 'monsterV4')
    config = part(local, 'interception_config')
    loadout = Draft202012Validator(schema('skill-loadouts-v1'))
    immediate = part(schema('cast-lifecycle-v1'), 'immediate_self_commit')
    configs = runtime = browser_states = browser_events = immediate_config = 0
    for label in ('a','b'):
        def sample(name):
            return same_archive('samples/combat/'+name, 'examples/combat-host/'+name)
        configured=sample(f'interception-{label}.json')
        skill=next(s for s in configured['additional_skills'] if s['id']=='grant-immunity')
        if label=='b':
            immediate.validate({k:skill[k] for k in ('mechanism','windup_ms','commit_on_accept')})
            assert 'channel' not in skill and 'charge' not in skill
            immediate_config+=1
        else:
            assert 'commit_on_accept' not in skill
        rules = configured['combat_rules']['cast_interceptions']
        config.validate(rules)
        configs += 1
        for rule in rules.values():
            common.validate(rule)
            validator.validate(rule)
            configs += 1
        for participant in sample(f'interception-accounts-{label}.json')['participants']:
            loadout.validate(participant['skills'])
            configs += 1
    for name in ('catalog','host'):
        sample(f'interception-{name}.json')
        configs += 1

    events = {}
    for scenario in ('interception-four','interception-eight'):
        for browser in (False,True):
            folder = 'independent-browser' if browser else 'independent-combat'
            suffix = 'interception-browser-facts' if browser else 'interception-facts'
            x = json.loads((out/f'delivery/{folder}/{scenario}.{suffix}.json').read_text())
            assert len(x['states']) == 12
            for state in x['states']:
                own.validate(state['snapshot']['self'])
                for view in state['snapshot']['monsters']:
                    monster.validate(view)
                assert 'cast_interceptions' not in state['snapshot']['self']
                assert all('cast_interceptions' not in view for view in state['snapshot']['monsters'])
            if browser:
                browser_states += len(x['states'])
                rows = [(m['participant'], r) for m in x['eventsByMember'] for r in m['events']]
            else:
                runtime += len(x['states'])
                rows = [(r['recipient'],r['wrapped']) for r in x['events']]
            resolution = {}
            for recipient, wrapped in rows:
                event = wrapped.get('event',wrapped)
                if event['kind'] != 'cast_interception_resolved':
                    continue
                assert wrapped['kind'] == 'private'
                validator.validate(event)
                common.validate(event)
                assert event['source']['kind'] != event['target']['kind']
                assert recipient == next(e['id'] for e in (event['source'],event['target']) if e['kind']=='participant')
                assert recipient not in resolution
                resolution[recipient] = event
            assert [resolution[f'P{i:02}']['outcome'] for i in range(4)] == ['interrupted','immune_allowed','interrupted','allowed_with_followup']
            if browser:
                browser_events += len(resolution)
            else:
                runtime += len(resolution)
                events = resolution

    rule = rules['cast-mark']
    rejected = []
    def change(base, key, value, probe=validator):
        other = copy.deepcopy(base)
        other[key] = value
        rejected.append((probe,other))
    change(rule,'action_filter',{'basic':True,'skills':{'mode':'any'}})
    change(rule,'action_filter',{'basic':False,'skills':{'mode':'only','ids':[]}})
    change(rule,'action_filter',{'basic':False,'skills':{'mode':'except','ids':['skill','skill']}})
    change(rule,'action_filter',{'basic':False,'skills':{'mode':'any','ids':[]}})
    change(rule,'action_filter',{'basic':False,'skills':{'mode':'only','ids':[f's{i}' for i in range(65)]}})
    change(rule,'default',{'kind':'invalid','status_id':'cast-seal'})
    change(rule,'default',{'kind':'interrupt','status_id':None})
    change(rule,'default',{'kind':'interrupt','status_id':'seal','duration_ms':1})
    change(rule,'profile_overrides',None)
    change(rule,'profile_overrides',{str(i):rule['default'] for i in range(17)})
    change(rule,'source',{'id':'P00','generation':1})
    rejected += [(config,None),(config,{str(i):rule for i in range(17)})]
    change(events['P01'],'followup_status_id','cast-seal')
    change(events['P00'],'action_id',None)
    change(events['P00'],'mark_instance_id','')
    change(events['P00'],'source',{'kind':'monster','id':'m','generation':0})
    change(events['P02'],'action_id','forged')
    for participant, key in [('P00','followup_status_id'),('P01','action_id')]:
        other = copy.deepcopy(events[participant])
        del other[key]
        rejected.append((validator,other))
    for probe, value in rejected:
        assert not probe.is_valid(value), value
    assert (configs,runtime,browser_states,browser_events,len(rejected)) == (18,32,24,8,20)
    assert immediate_config==1
    return {'immediateSelfCommitConfigFragments':immediate_config,'castInterceptionConfigFragments':configs,'castInterceptionRuntimeFragments':runtime,
            'castInterceptionBrowserStates':browser_states,'castInterceptionBrowserEvents':browser_events,
            'invalidCastInterceptionFragmentsRejected':len(rejected)}
