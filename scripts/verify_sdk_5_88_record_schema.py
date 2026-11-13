from verify_sdk_5_88_interception_schema import verify_interception_schema
from verify_sdk_5_88_paid_schema import verify_paid_schema
from verify_sdk_5_88_activation_schema import verify_activation_schema
"""Validate archived record fragments and exact configs consumed by public tests.

Verification-only dependency: Python jsonschema. The SDK runtime does not use it.
"""
from verify_sdk_5_88_derived_schema import verify_derived_schema
from verify_sdk_5_88_gates_schema import verify_gates_schema
from verify_sdk_5_88_counter_schema import verify_counter_schema
import hashlib
import copy
import json
from pathlib import Path
import tarfile
import sqlite3
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'artifacts/sdk-5-88'


def main():
    package = json.loads((OUT / 'delivery/package.json').read_text())
    archive = Path(package['archive'])
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == package['sha256']
    root = package['rootName']
    power_config_count=0
    steal_config_count=0
    reservation_config_count=0
    unit_config_count=0
    loadout_config_count=0
    periodic_config_count=0
    sample_count = config_count = runtime_count = rejected_followup_fragments = rejected_enhancement_fragments = casting_config_count = 0
    with tarfile.open(archive) as tar:
        schema_bytes = tar.extractfile(root + '/docs/interfaces/combat-rules-v1.schema.json').read()
        assert schema_bytes == (ROOT / 'docs/interfaces/combat-rules-v1.schema.json').read_bytes()
        schema = json.loads(schema_bytes)
        Draft202012Validator.check_schema(schema)
        validator = Draft202012Validator(schema)
        casting_schema_bytes=tar.extractfile(root+'/docs/interfaces/cast-lifecycle-v1.schema.json').read()
        assert casting_schema_bytes==(ROOT/'docs/interfaces/cast-lifecycle-v1.schema.json').read_bytes()
        casting_schema=json.loads(casting_schema_bytes);Draft202012Validator.check_schema(casting_schema)
        casting_validator=Draft202012Validator(casting_schema)
        loadout_schema_bytes=tar.extractfile(root+'/docs/interfaces/skill-loadouts-v1.schema.json').read()
        assert loadout_schema_bytes==(ROOT/'docs/interfaces/skill-loadouts-v1.schema.json').read_bytes()
        loadout_schema=json.loads(loadout_schema_bytes);Draft202012Validator.check_schema(loadout_schema)
        loadout_validator=Draft202012Validator(loadout_schema)
        unit_schema_bytes=tar.extractfile(root+'/docs/interfaces/unit-effects-v1.schema.json').read()
        assert unit_schema_bytes==(ROOT/'docs/interfaces/unit-effects-v1.schema.json').read_bytes()
        unit_schema=json.loads(unit_schema_bytes);Draft202012Validator.check_schema(unit_schema);unit_validator=Draft202012Validator(unit_schema)
        reservation_schema_bytes=tar.extractfile(root+'/docs/interfaces/resource-commit-v1.schema.json').read()
        assert reservation_schema_bytes==(ROOT/'docs/interfaces/resource-commit-v1.schema.json').read_bytes()
        reservation_schema=json.loads(reservation_schema_bytes);Draft202012Validator.check_schema(reservation_schema);reservation_validator=Draft202012Validator(reservation_schema)
        steal_schema_bytes=tar.extractfile(root+'/docs/interfaces/resource-steal-v1.schema.json').read()
        assert steal_schema_bytes==(ROOT/'docs/interfaces/resource-steal-v1.schema.json').read_bytes()
        steal_schema=json.loads(steal_schema_bytes);Draft202012Validator.check_schema(steal_schema);steal_validator=Draft202012Validator(steal_schema)
        power_schema_bytes=tar.extractfile(root+'/docs/interfaces/character-build-v1.schema.json').read()
        assert power_schema_bytes==(ROOT/'docs/interfaces/character-build-v1.schema.json').read_bytes()
        power_schema=json.loads(power_schema_bytes);Draft202012Validator.check_schema(power_schema);power_validator=Draft202012Validator(power_schema)
        view_schema_bytes=tar.extractfile(root+'/docs/interfaces/publications-v6.schema.json').read()
        assert view_schema_bytes==(ROOT/'docs/interfaces/publications-v6.schema.json').read_bytes()
        view_schema=json.loads(view_schema_bytes);Draft202012Validator.check_schema(view_schema);own_validator=Draft202012Validator({'$ref':'#/$defs/privateMemberV4','$defs':view_schema['$defs']})
        for source in (ROOT / 'examples/combat-host').glob('*.json'):
            packed = tar.extractfile(root + '/samples/combat/' + source.name).read()
            assert packed == source.read_bytes()
            sample_count += 1
            config = json.loads(packed)
            if source.name.startswith(('counter-','activation-','paid-','interception-')):continue # Validated below against the dedicated same-archive counter fragments.
            if source.name.startswith('power-'):
                for participant in config.get('participants',[]):
                    loadout_validator.validate(participant['skills']);power_config_count+=1
                    power_validator.validate(participant['combat_build']);power_config_count+=1
                if 'combat_rules' in config:
                    power_validator.validate(config['combat_rules']['build_rules']);power_config_count+=1
                    power_validator.validate({'status_power_sources':config['combat_rules']['status_power_sources']});power_config_count+=1
                continue
            if source.name.startswith('steal-'):
                for participant in config.get('participants',[]):loadout_validator.validate(participant['skills']);steal_config_count+=1
                if 'skill' in config:
                    unit_validator.validate(config['skill']);steal_config_count+=1
                    steal_validator.validate(config['skill']['resource_steal']);steal_config_count+=1
                    steal_validator.validate({'life_steal_cap_bps':config['combat_rules']['life_steal_cap_bps']});steal_config_count+=1
                    for definition in config['pve']['status_definitions']:
                        if definition.get('attribute')=='life_steal':steal_validator.validate(definition);steal_config_count+=1
                continue
            if source.name.startswith('reservations-'):
                for participant in config.get('participants',[]):
                    loadout_validator.validate(participant['skills']);reservation_config_count+=1
                for skill in [config.get('skill',{})]+config.get('additional_skills',[]):
                    if skill.get('resource_commit'):reservation_validator.validate(skill['resource_commit']);reservation_config_count+=1
                continue
            if source.name.startswith('units-'):
                for participant in config.get('participants',[]):
                    loadout_validator.validate(participant['skills']);unit_config_count+=1
                for skill in [config.get('skill',{})]+config.get('additional_skills',[]):
                    if skill:unit_validator.validate(skill);unit_config_count+=1
                continue
            for participant in config.get('participants',[]):
                if 'skills' in participant:loadout_validator.validate(participant['skills']);loadout_config_count+=1
            if 'skill_library' in config:
                loadout_validator.validate({'skill_library':config['skill_library']});loadout_config_count+=1
            if config.get('id','').startswith('loadouts-'):continue
            if config.get('id','').startswith('composed-status-'):
                rules=config['combat_rules']
                for definition in config['pve']['status_definitions']:
                    if definition['mechanism'].startswith('periodic_'):
                        validator.validate(definition);periodic_config_count+=1
                for field in ['status_policies','status_cleanses']:
                    for rule in rules[field].values():validator.validate(rule);periodic_config_count+=1
                for t in rules['triggers']:validator.validate(t['effect']);periodic_config_count+=1
                continue
            for field in ['resource_limits','cast_limits']:
                if field in config:casting_validator.validate(config[field]);casting_config_count+=1
            for skill in [config.get('skill',{})]+config.get('additional_skills',[]):
                for field in ['cast_movement','recovery_cancel','charge','channel']:
                    if field in skill:casting_validator.validate(skill[field]);casting_config_count+=1
            combat=config.get('combat_rules',{})
            attacks=([combat['basic_attack']] if combat else [])+[r for field in ['skills','npc_attacks','periodic_damage'] for r in combat.get(field,{}).values()]
            for attack in attacks:
                fields={key:attack[key] for key in ['contact_status','armor_penetration_cap_bps'] if key in attack}
                if fields:
                    validator.validate(fields);config_count+=1
            for rule in config.get('combat_rules', {}).get('shield_records', {}).values():
                validator.validate(rule)
                config_count += 1
            for rule in config.get('combat_rules', {}).get('triggers', []):
                if rule['effect']['kind']=='restore_spirit' and 'max_spirit_bps' in rule['effect']:
                    validator.evolve(schema={'$ref':'#/$defs/restore_spirit','$defs':schema['$defs']}).validate(rule['effect']);config_count+=1
                if 'conditions' in rule:
                    validator.validate(rule['conditions'])
                    config_count += 1
            for policy in config.get('combat_rules', {}).get('status_policies', {}).values():
                validator.validate(policy)
                config_count += 1
            for policy in config.get('combat_rules',{}).get('control_policies',{}).values():
                validator.validate(policy)
                config_count += 1
            for guard in config.get('combat_rules', {}).get('death_guards', {}).values():
                validator.validate(guard)
                config_count += 1
            for trigger in config.get('combat_rules', {}).get('triggers', []):
                if trigger['effect']['kind'] in ['dispel_statuses','transfer_statuses','chain_damage','reduce_skill_cooldowns','finish_other']:
                    validator.validate(trigger['effect'])
                    config_count += 1
            for rule in config.get('combat_rules', {}).get('status_windows', {}).values():
                validator.validate(rule)
                config_count += 1
            for rule in config.get('combat_rules',{}).get('periodic_recovery',{}).values():
                validator.validate(rule)
                config_count += 1
            for rule in config.get('combat_rules',{}).get('status_cleanses',{}).values():
                validator.validate(rule)
                config_count += 1
            for definition in config.get('pve',{}).get('status_definitions',[]):
                if 'flags' in definition or definition.get('attribute')=='movement_slow' and definition.get('duplicate_policy')=='stack':
                    validator.validate(definition)
                    config_count += 1
            rules=config.get('combat_rules')
            if rules:
                for reaction in rules.get('contact_reactions',{}).values():
                    validator.validate(reaction);config_count+=1
                for t in rules['triggers']:
                    if t['when']=='finisher_kill' and t['effect']['kind']=='restore_spirit' and 'enhanced' not in t['finisher']:
                        for n in range(13):
                            b=copy.deepcopy(t)
                            if n==0: b.pop('finisher')
                            elif n==1: b['finisher']=None
                            elif n==2: b['finisher']['mode']='missing'
                            elif n==3: b['finisher']['mode']=None
                            elif n==4: b['when']='kill'
                            elif n==5: b['include_periodic']=True
                            elif n==6: b['counting']='action_total'
                            elif n==7: b['counter_scope']='target'
                            elif n==8: b['conditions']={'other_hp_at_most_bps':10000}
                            elif n==9: b['effect']['max_spirit_bps']=10001
                            elif n==10: b['effect']['max_spirit_bps']=None
                            elif n==11: b['effect']['max_spirit_bps']=0
                            else: b['finisher']['mode']={'execute':None}
                            assert not validator.is_valid(b), (source.name,n)
                            rejected_followup_fragments+=1
                if 'max_contact_action_snapshots' in rules:
                    validator.validate({'max_contact_action_snapshots':rules['max_contact_action_snapshots']});config_count+=1
                    contact=rules['skills']['surge-line']['contact_status']
                    provider=rules['triggers'][0]
                    selector=rules['triggers'][1]['finisher']
                    for n in range(18):
                        if n in [0,1,2,3,15]:
                            b=copy.deepcopy(contact);probe=validator.evolve(schema={'$ref':'#/$defs/contact_status','$defs':schema['$defs']})
                            if n==0:b['sampling']='missing'
                            elif n==1:b['sampling']=None
                            elif n==2:b['sampling']={'action':None}
                            elif n==3:b['subject']='other'
                            else:b['sampling']='contact'
                        elif n in [4,5,6]:
                            b={'max_contact_action_snapshots':{4:0,5:1025,6:None}[n]};probe=validator
                        elif n in [12,13,14]:
                            b=copy.deepcopy(selector);b['enhanced']={12:None,13:'false',14:{'false':None}}[n];probe=validator.evolve(schema={'$ref':'#/$defs/finisher_selector','$defs':schema['$defs']})
                        else:
                            b=copy.deepcopy(provider);probe=validator
                            bonus=b['effect']['rule']['consumed_status_bonus']
                            if n==7:b['effect']['rule']['consumed_status_bonus']=None
                            elif n==8:bonus['hp_threshold_bps_per_stack']=0
                            elif n==9:bonus['hp_threshold_bps_per_stack']=10001
                            elif n==10:bonus['typo']=1
                            elif n==11:b['when']='damage_dealt'
                            elif n==16:bonus.pop('definition_id')
                            else:bonus['hp_threshold_bps_per_stack']=1.5
                        assert not probe.is_valid(b), (source.name,n)
                        rejected_enhancement_fragments+=1
                if 'max_contact_reactions_per_hit' in rules or 'max_contact_reactions_per_step' in rules:
                    validator.validate({'max_contact_reactions_per_hit':rules.get('max_contact_reactions_per_hit',1),'max_contact_reactions_per_step':rules.get('max_contact_reactions_per_step',32)});config_count+=1
                if 'max_chain_hits_per_step' in rules:
                    validator.validate({'max_chain_hits_per_step':rules['max_chain_hits_per_step']})
                    config_count += 1
                validator.validate({'max_cleanse_operations_per_step':rules['max_cleanse_operations_per_step']})
                config_count += 1
                validator.validate({'max_window_releases_per_step':rules['max_window_releases_per_step']})
                config_count += 1
                for trigger in rules['triggers']:
                    validator.validate(trigger)
                    config_count += 1
                validator.validate({k:rules[k] for k in ['max_trigger_targets_per_rule','max_pending_action_totals']})
                config_count += 1
        independent = json.loads((OUT / 'delivery/independent-verification.json').read_text())
        assert independent['archiveSha256'] == package['sha256']
        assert independent['binarySha256'] == package['binarySha256']
        for path in (OUT / 'delivery/independent-combat').glob('*.record-facts.json'):
            data = json.loads(path.read_text())
            validator.validate(data['initialShield'])
            runtime_count += 1
            for row in data['events']:
                validator.validate(row['event'])
                runtime_count += 1
        for path in (OUT / 'delivery/independent-combat').glob('*.status-facts.json'):
            data = json.loads(path.read_text())
            assert {k:v for k,v in data['taken'].items() if k not in ['target','shield']} == {k:v for k,v in data['offer'].items() if k not in ['target','shield']}
            assert data['taken']['target'] != data['offer']['target']
            for row in data['events']:
                validator.validate(row['event'])
                runtime_count += 1
        for path in (OUT / 'delivery/independent-combat').glob('*.total-facts.json'):
            data=json.loads(path.read_text())
            for row in data['events']:
                validator.validate(row['event'])
                runtime_count += 1
        for path in (OUT / 'delivery/independent-combat').glob('*.window-facts.json'):
            data=json.loads(path.read_text())
            for window in data['windows']:
                validator.validate(window)
                runtime_count += 1
            for row in data['events']:
                validator.validate(row['event'])
                runtime_count += 1
        for path in (OUT / 'delivery/independent-combat').glob('*.compound-facts.json'):
            for status in json.loads(path.read_text())['statuses']:
                validator.validate(status)
                runtime_count += 1
        for path in (OUT / 'delivery/independent-combat').glob('*.cleanse-facts.json'):
            data=json.loads(path.read_text())
            validator.validate(data['capability'])
            runtime_count += 1
            for row in data['events']:
                validator.validate(row['event'])
                runtime_count += 1
        for path in (OUT / 'delivery/independent-combat').glob('*.resource-facts.json'):
            data=json.loads(path.read_text())
            validator.validate(data['periodic'])
            runtime_count += 1
            for row in data['events']:
                validator.validate(row['event'])
                runtime_count += 1
        for path in (OUT / 'delivery/independent-combat').glob('*.overheal-record-facts.json'):
            data=json.loads(path.read_text())
            assert data['initialShield']==data['restoredShield']
            for key in ['initialShield','restoredShield']:
                validator.validate(data[key])
                runtime_count += 1
            for row in data['events']:
                validator.validate(row['event'])
                runtime_count += 1
        for path in (OUT/'delivery/independent-combat').glob('*.shield-transfer-facts.json'):
            data=json.loads(path.read_text())
            assert data['before']['remaining']-data['consumption']['absorbed']==data['after']['remaining']==data['consumption']['remaining']
            assert data['before']['granted']==data['after']['granted']
            for key in ['before','after']:
                validator.validate(data[key]);runtime_count+=1
            for row in data['events']:
                validator.validate(row['event']);runtime_count+=1
        periodic_validator=validator.evolve(schema={'$ref':'#/$defs/periodic_transfer_view','$defs':schema['$defs']})
        for path in (OUT/'delivery/independent-combat').glob('*.periodic-transfer-facts.json'):
            data=json.loads(path.read_text());assert data['before']==data['after']
            for key in ['before','after']:
                periodic_validator.validate(data[key]);runtime_count+=1
            assert len(data['events'])==data['after']['total_ticks']
            for row in data['events']:
                validator.validate(row['event']);runtime_count+=1
        marker_validator=validator.evolve(schema={'$ref':'#/$defs/status_transferred/properties/status','$defs':schema['$defs']})
        for path in (OUT/'delivery/independent-combat').glob('*.control-transfer-facts.json'):
            data=json.loads(path.read_text());assert data['after']=={**data['before'],'target':data['after']['target']}
            for key in ['before','after']:
                marker_validator.validate(data[key]);runtime_count+=1
        for path in (OUT/'delivery/independent-combat').glob('*.ally-commit-facts.json'):
            data=json.loads(path.read_text());assert data['after']=={**data['before'],'target':data['after']['target']}
            for key in ['before','after']:
                marker_validator.validate(data[key]);runtime_count+=1
            for row in data['events']:
                validator.validate(row['event']);runtime_count+=1
        for path in (OUT/'delivery/independent-combat').glob('*.lifecycle-transfer-facts.json'):
            data=json.loads(path.read_text());assert data['after']=={**data['before'],'target':data['after']['target']}
            for key in ['before','after']:
                marker_validator.validate(data[key]);runtime_count+=1
            for row in data['events']:
                validator.validate(row['event']);runtime_count+=1
        for path in (OUT/'delivery/independent-combat').glob('*.control-policy-facts.json'):
            data=json.loads(path.read_text())
            for key in ['capped','first','refreshed']:
                marker_validator.validate(data[key]);runtime_count+=1
            assert data['capped']['expires_at_ms']-data['capped']['applied_at_ms']==1800
            assert data['first']['instance_id']==data['refreshed']['instance_id']
            assert data['attackTimes'][1]-data['attackTimes'][0]==(690 if path.name.startswith('four.') else 930)
        for path in (OUT/'delivery/independent-combat').glob('*.hit-status-facts.json'):
            data=json.loads(path.read_text())
            for status in [*data['marks'],data['converted']]:
                marker_validator.validate(status);runtime_count+=1
            for row in data['consumed']:
                validator.validate(row['event']);runtime_count+=1
            assert [s['stacks'] for s in data['marks']]==[1,2]
            assert data['marks'][0]['instance_id']==data['marks'][1]['instance_id']
            assert data['consumed'][0]['event']['consumed_stacks']==3
        for path in (OUT/'delivery/independent-combat').glob('*.contact-modifier-facts.json'):
            data=json.loads(path.read_text())
            for status in data['marks']:
                marker_validator.validate(status);runtime_count+=1
            for row in data['consumed']:
                validator.validate(row['event']);runtime_count+=1
            assert [s['stacks'] for s in data['marks']]==[1,2,3]
            assert [r['event']['consumed_stacks'] for r in data['consumed']]==([1,2,3] if path.name.startswith('strikes-four.') else [1,2,2,1])
        for path in (OUT/'delivery/independent-combat').glob('*.chain-facts.json'):
            data=json.loads(path.read_text())
            for status in [*data['marks'],*data['statuses']]:
                marker_validator.validate(status);runtime_count+=1
            for row in [*data['consumed'],*data['resolved']]:
                validator.validate(row['event']);runtime_count+=1
            assert len(data['resolved'])==1 and data['resolved'][0]['event']['reason']=='complete'
            assert [s['stacks'] for s in data['marks']]==[1,2]
            assert data['consumed'][0]['event']['consumed_stacks']==3
        for path in (OUT/'delivery/independent-combat').glob('*.post-consumption-facts.json'):
            data=json.loads(path.read_text())
            for status in [*data['marks'],data['status']]:
                marker_validator.validate(status);runtime_count+=1
            for row in [*data['consumed'],*data['resolved'],*data['refunds']]:
                validator.validate(row['event']);runtime_count+=1
            refund=data['refunds'][0]['event'];change=refund['changes'][0]
            assert change['previous_ready_at_ms']-change['ready_at_ms']==change['reduced_ms']
            assert change['ready_at_ms']>=refund['at_ms'] and data['consumed'][0]['event']['consumed_stacks']==3
        for path in (OUT/'delivery/independent-combat').glob('*.contact-reaction-facts.json'):
            data=json.loads(path.read_text())
            for status in [*data['marks'],data['frozen']]:
                marker_validator.validate(status);runtime_count+=1
            for row in [*data['consumed'],*data['reactions']]:
                validator.validate(row['event']);runtime_count+=1
            event=data['reactions'][0]['event'];assert event['source']['id']=='P01' and event['provoker']['id']=='P00'
            assert event['raw_damage']==event['hp_damage']==(25 if path.name.startswith('reaction-four.') else 67)
            assert data['consumed'][0]['event']['consumed_stacks']==1
        for path in (OUT/'delivery/independent-combat').glob('*.finisher-facts.json'):
            data=json.loads(path.read_text())
            for row in [*data['consumed'],*data['resolved']]:
                validator.validate(row['event']);runtime_count+=1
            assert [r['event']['lethal'] for r in data['resolved']]==[True,False,False]
            assert [r['event']['mode'] for r in data['resolved']]==['execute','power_damage','execute']
            assert all(r['event']['consumed_stacks']==1 for r in data['consumed'])
        for path in (OUT/'delivery/independent-combat').glob('*.finisher-followup-facts.json'):
            data=json.loads(path.read_text())
            marker_validator.validate(data['status']);runtime_count+=1
            for row in [*data['refunds'],*data['chains'],*data['resolved']]:
                validator.validate(row['event']);runtime_count+=1
            assert [r['event']['lethal'] for r in data['resolved']]==[False,False,True]
            assert [r['event']['trigger_id'] for r in data['post']]==['finish-state','finish-spirit','finish-refund','finish-chain']
            assert all(r['recipient']=='P00' and r['event']['source']['id']=='P00' for r in data['post'])
        for path in (OUT/'delivery/independent-combat').glob('*.action-enhancement-facts.json'):
            data=json.loads(path.read_text())
            for status in [data['initial'],data['renewed']]:
                marker_validator.validate(status);runtime_count+=1
            for row in [*data['consumptions'],*data['finishers'],*data['refunds']]:
                validator.validate(row['event']);runtime_count+=1
            assert len(data['consumptions'])==1 and data['consumptions'][0]['event']['consumed_stacks']==1
            assert [r['event']['target']['id'] for r in data['finishers']]==['capped-target','line-a','line-b','renew-target']
            primary=450 if path.name.startswith('enhancement-four.') else 460
            assert [r['event']['amount'] for r in data['primaryHits']]==[primary,primary]
            assert data['flightBefore']['id']==data['flightAfter']['id']
            before,after=data['flightBefore']['piercing'],data['flightAfter']['piercing']
            assert {k:v for k,v in before.items() if k!='remaining_range'}=={k:v for k,v in after.items() if k!='remaining_range'}
            assert 0<after['remaining_range']<=before['remaining_range'] and before['used_hits']==1
    assert runtime_count == 236 and config_count == 352 and sample_count == 86, (runtime_count,config_count,sample_count)
    native_followup_timings=[]
    with sqlite3.connect('file:'+independent['browser']['database']+'?mode=ro',uri=True) as database:
        rooms=[json.loads(r[0]) for r in database.execute('SELECT record FROM duo_rooms')]
        for scenario in ['followup-four','followup-eight']:
            matches=[r for r in rooms if r['scenario']==scenario];assert len(matches)==1
            room=matches[0]
            envelopes=[json.loads(r[0]) for r in database.execute('SELECT body FROM duo_outputs WHERE room_id=?',(room['id'],))]
            events=[]
            for envelope in envelopes:
                body=envelope.get('body',{})
                if body.get('kind')!='event_v4':continue
                event=body['event']
                if event.get('kind')=='private':event=event['event']
                events.append((envelope['logical_ms'],event))
            casts=[(now,event) for now,event in events if event.get('kind')=='action_committed' and event.get('skill_id')=='threshold-bolt']
            refunds=[event for now,event in events if event.get('kind')=='skill_cooldowns_reduced' and event.get('trigger_id')=='finish-refund']
            assert len(casts)==4 and len(refunds)==1
            change=refunds[0]['changes'][0];committed,cast=casts[-1]
            assert cast['unit_target']['id']=='guard-target' and change['ready_at_ms']<=committed<change['previous_ready_at_ms']
            native_followup_timings.append({'scenario':scenario,'committedAtMs':committed,'readyAtMs':change['ready_at_ms'],'previousReadyAtMs':change['previous_ready_at_ms']})

    casting_runtime=set();casting_costs=[]
    with sqlite3.connect('file:'+independent['browser']['database']+'?mode=ro',uri=True) as database:
        rooms=[json.loads(row[0]) for row in database.execute('SELECT record FROM duo_rooms')]
        selected=[room for room in rooms if room['scenario'] in ['casting-four','casting-eight']];assert len(selected)==2
        for room in selected:
            costs=[]
            for raw, in database.execute('SELECT body FROM duo_outputs WHERE room_id=?',(room['id'],)):
                envelope=json.loads(raw);body=envelope.get('body',{})
                if body.get('kind')=='publication_v6':
                    payload=body['payload'];own=payload['snapshot']['self'] if payload['kind']=='full' else payload['delta'].get('own')
                    if own:
                        fragments=[own.get('active_channel'),(own.get('pending_action') or {}).get('charge')]
                        for fragment in fragments:
                            if fragment is not None:casting_validator.validate(fragment);casting_runtime.add(json.dumps(fragment,sort_keys=True))
                if body.get('kind')=='event_v4':
                    event=body['event'];event=event['event'] if event.get('kind')=='private' else event
                    if event.get('kind')=='channel_recovery_pulse':
                        casting_validator.validate(event);assert event['executed_at_ms']>=event['scheduled_at_ms']
                        assert event['executed_at_ms']-event['scheduled_at_ms']<room['config']['step_ms']
                        assert body['recipient_id']=='P00';casting_runtime.add(json.dumps(event,sort_keys=True))
                    if event.get('kind')=='action_committed' and event.get('skill_id')=='large-cost':
                        assert body['recipient_id']=='P01' and event['spirit_cost']==8000000 and event['hp_cost']==7999999;costs.append(event)
            assert len(costs)==1;casting_costs.append({'scenario':room['scenario'],'spiritCost':8000000,'hpCost':7999999,'commits':1})
    periodic_runtime=[]
    for path in (OUT/'delivery/independent-combat').glob('*.periodic-composition-facts.json'):
        data=json.loads(path.read_text())
        for status in [data['before'],data['moved']]:validator.validate(status);periodic_runtime.append(status)
        for row in data['events']:
            if row['event']['kind'] in ['status_transferred','periodic_tick','periodic_heal_tick','periodic_spirit_tick','status_cleanse_resolved']:
                validator.validate(row['event']);periodic_runtime.append(row['event'])
        layers=data['before']['stacks'];four=path.name.startswith('composed-four.')
        assert data['first']['raw_damage']==(120 if four else 100)*700*layers//10000
        assert data['late']['raw_damage']==(240 if four else 100)*700*layers//10000
    browser_periodic=[]
    with sqlite3.connect('file:'+independent['browser']['database']+'?mode=ro',uri=True) as database:
        for scenario in ['composed-four','composed-eight']:
            rows=[json.loads(r[0]) for r in database.execute('SELECT record FROM duo_rooms')]
            rows=[r for r in rows if r['scenario']==scenario];assert len(rows)==1
            outputs=[json.loads(r[0]) for r in database.execute('SELECT body FROM duo_outputs WHERE room_id=?',(rows[0]['id'],))]
            seen=set()
            for envelope in outputs:
                body=envelope.get('body',{});payload=body.get('payload',{})
                state=payload.get('snapshot',{}).get('self',{}) if payload.get('kind')=='full' else payload.get('delta',{}).get('own',{})
                for status in state.get('statuses',[]):
                    if 'periodic' in status:
                        encoded=json.dumps(status,sort_keys=True)
                        if encoded not in seen:validator.validate(status);browser_periodic.append(status);seen.add(encoded)
    assert periodic_config_count==12 and len(periodic_runtime)>=16 and len(browser_periodic)>=16,(periodic_config_count,len(periodic_runtime),len(browser_periodic))
    loadout_runtime=[]
    for path in (OUT/'delivery/independent-combat').glob('*.loadout-facts.json'):
        data=json.loads(path.read_text())
        for fragment in [*data['initial'],*data['receipts'],data['projection'],data['controlCheckpoint']]:
            loadout_validator.validate(fragment);loadout_runtime.append(fragment)
        for row in data['events']:
            if row['event']['kind']=='skill_loadout_changed':
                assert row['recipient']=='P00';loadout_validator.validate(row['event']);loadout_runtime.append(row['event'])
        assert [r['revision'] for r in data['receipts']]==list(range(1,7))
        assert data['projection']['player_skills'][0]['ready_at_ms']==data['oldReady']
        assert data['oldFlight']['id']==data['preserved']['id'] and data['preserved']['source']['id']=='P00'
    loadout_browser_states=[];loadout_audits=[]
    with sqlite3.connect('file:'+independent['browser']['database']+'?mode=ro',uri=True) as database:
        rooms=[json.loads(r[0]) for r in database.execute('SELECT record FROM duo_rooms')]
        for scenario in ['loadouts-four','loadouts-eight']:
            matches=[r for r in rooms if r['scenario']==scenario];assert len(matches)==1;room=matches[0]
            outputs=[json.loads(r[0]) for r in database.execute('SELECT body FROM duo_outputs WHERE room_id=?',(room['id'],))]
            states=set();audits=set();changes=[]
            configured={s['id']for s in [room['config']['skill'],*room['config'].get('additional_skills',[]),*room['config']['skill_library']]}
            for envelope in outputs:
                body=envelope.get('body',{});payload=body.get('payload',{})
                own=payload.get('snapshot',{}).get('self',{}) if payload.get('kind')=='full' else payload.get('delta',{}).get('own',{})
                if own and 'skill_loadout_revision' in own:
                    fragment={k:own[k]for k in ['skill_loadout_revision','player_skills']};encoded=json.dumps(fragment,sort_keys=True)
                    if encoded not in states:loadout_validator.validate(fragment);loadout_browser_states.append(fragment);states.add(encoded)
                    assert 'owned_skill_ids' not in own
                if body.get('kind')=='event_v4':
                    event=body.get('event',{});event=event.get('event',{}) if event.get('kind')=='private' else event
                    if event.get('kind')=='skill_loadout_changed':
                        assert body.get('recipient_id')=='P00';loadout_validator.validate(event);changes.append(event)
                cp=body.get('checkpoint',{});audit=cp.get('state',{}).get('skill_control')
                if audit:
                    encoded=json.dumps(audit,sort_keys=True)
                    if encoded in audits:continue
                    loadout_validator.validate(audit);audits.add(encoded);loadout_audits.append(audit)
                    initial={s['participant_id']:{'loadout':s['skills'],'revision':0,'declared':True}for s in room['originalCreate']['participants']}
                    operations=set()
                    for operation in audit['operations']:
                        q=operation['request'];r=operation['receipt'];pid=q['participant_id']
                        assert q['key']==room['originalCreate']['key'] and q['entity_generation']==1 and q['operation_id'] not in operations
                        operations.add(q['operation_id']);assert q['expected_revision']==initial[pid]['revision']
                        assert set(q['loadout']['owned_skill_ids'])<=configured and set(q['loadout']['equipped_skill_ids'])<=set(q['loadout']['owned_skill_ids'])
                        assert r=={'operation_id':q['operation_id'],'revision':initial[pid]['revision']+1,'equipped_skill_ids':q['loadout']['equipped_skill_ids']}
                        initial[pid]={'loadout':q['loadout'],'revision':r['revision'],'declared':True}
                    assert audit['members']==initial
            assert [e['revision']for e in changes]==list(range(1,7))
    unit_runtime=[]
    for path in (OUT/'delivery/independent-combat').glob('*.unit-facts.json'):
        data=json.loads(path.read_text());assert len(data['projections'])==4
        for own in data['projections']:unit_validator.validate(own['active_channel']);unit_runtime.append(own['active_channel'])
        for row in data['events']:
            if row['event']['kind'] in ['unit_channel_pulse','channel_recovery_pulse']:
                assert row['recipient']=='P00';unit_validator.validate(row['event']);unit_runtime.append(row['event'])
    unit_browser_states=[];unit_browser_events=[]
    with sqlite3.connect('file:'+independent['browser']['database']+'?mode=ro',uri=True) as database:
        rooms=[json.loads(row[0]) for row in database.execute('SELECT record FROM duo_rooms')]
        for scenario in ['units-four','units-eight']:
            selected=[r for r in rooms if r['scenario']==scenario];assert len(selected)==1;room=selected[0];states=set();events=[];heals=[];targets={}
            for raw, in database.execute('SELECT body FROM duo_outputs WHERE room_id=?',(room['id'],)):
                envelope=json.loads(raw);body=envelope.get('body',{});payload=body.get('payload',{})
                own=payload.get('snapshot',{}).get('self',{}) if payload.get('kind')=='full' else payload.get('delta',{}).get('own',{})
                channel=own.get('active_channel')
                if channel:
                    assert body['recipient_id']=='P00';unit_validator.validate(channel);targets[channel['action_id']]=channel['target'];encoded=json.dumps(channel,sort_keys=True)
                    if encoded not in states:states.add(encoded);unit_browser_states.append(channel)
                if body.get('kind')=='event_v4':
                    event=body.get('event',{});event=event.get('event',{}) if event.get('kind')=='private' else event
                    if event.get('kind') in ['unit_channel_pulse','channel_recovery_pulse']:
                        assert body['recipient_id']=='P00';unit_validator.validate(event);assert 0<=event['executed_at_ms']-event['scheduled_at_ms']<room['config']['step_ms'];events.append(event);unit_browser_events.append(event)
                    if event.get('kind')=='heal_resolved' and event.get('heal_kind')=='ally_skill':
                        assert body['recipient_id'] in ['P00','P01'] and event['source']['id']=='P00' and event['target']['id']=='P01';heals.append(event)
            assert len(events)==8 and len(heals)==8 and len(targets)==4
    assert unit_config_count==18 and len(unit_runtime)==24 and len(unit_browser_states)>=16 and len(unit_browser_events)==16,(unit_config_count,len(unit_runtime),len(unit_browser_states),len(unit_browser_events))
    reservation_runtime=[]
    for path in (OUT/'delivery/independent-combat').glob('*.reservation-facts.json'):
        data=json.loads(path.read_text());assert len(data['projections'])==4 and len(data['commits'])==2
        assert sum('resource_reservation' in p for p in data['projections'])==3
        for fragment in [*data['projections'],*data['commits']]:reservation_validator.validate(fragment);reservation_runtime.append(fragment)
        assert data['commits'][0]['action_id']==data['projections'][0]['action_id'] and data['commits'][0]['hp_cost']==data['projections'][0]['resource_reservation']['hp']
    reservation_browser_states=[];reservation_browser_events=[]
    with sqlite3.connect('file:'+independent['browser']['database']+'?mode=ro',uri=True) as database:
        rooms=[json.loads(row[0]) for row in database.execute('SELECT record FROM duo_rooms')]
        for scenario in ['reservations-four','reservations-eight']:
            selected=[r for r in rooms if r['scenario']==scenario];assert len(selected)==1;room=selected[0];states=set();events=[];quotes={}
            for raw, in database.execute('SELECT body FROM duo_outputs WHERE room_id=?',(room['id'],)):
                envelope=json.loads(raw);body=envelope.get('body',{});payload=body.get('payload',{})
                own=payload.get('snapshot',{}).get('self',{}) if payload.get('kind')=='full' else payload.get('delta',{}).get('own',{})
                pending=own.get('pending_action')
                if pending and 'resource_reservation' in pending:assert body['recipient_id']=='P00'
                if pending and body.get('recipient_id')=='P00':
                    reservation_validator.validate(pending);encoded=json.dumps(pending,sort_keys=True)
                    if encoded not in states:states.add(encoded);reservation_browser_states.append(pending)
                    if 'resource_reservation' in pending:quotes[pending['action_id']]=pending['resource_reservation']
                if body.get('kind')=='event_v4':
                    event=body.get('event',{});event=event.get('event',{}) if event.get('kind')=='private' else event
                    if body.get('recipient_id')=='P00' and event.get('kind')=='action_committed' and event.get('skill_id') in ['reserved-bolt','live-bolt']:
                        reservation_validator.validate(event);events.append(event);reservation_browser_events.append(event)
            assert len(events)==2 and len(quotes)==3
            assert events[0]['hp_cost']==quotes[events[0]['action_id']]['hp'] and events[1]['hp_cost']==(100 if scenario=='reservations-four' else 200)
    assert reservation_config_count==16 and len(reservation_runtime)==12 and len(reservation_browser_states)>=8 and len(reservation_browser_events)==4,(reservation_config_count,len(reservation_runtime),len(reservation_browser_states),len(reservation_browser_events))
    steal_runtime=[];steal_invalid=0
    for path in (OUT/'delivery/independent-combat').glob('*.steal-facts.json'):
        data=json.loads(path.read_text());assert len(data['projections'])==4
        for projection in data['projections']:
            assert not any(k in projection['active_channel'] for k in ['steal_budget','resource_budget'])
            unit_validator.validate(projection['active_channel']);steal_runtime.append(projection['active_channel'])
            for status in projection['statuses']:
                if status.get('attribute')=='life_steal':steal_validator.validate(status);steal_runtime.append(status)
        transfers=[r['event']for r in data['events']if r['event']['kind']=='resource_stolen'];assert len(transfers)==6
        for event in transfers:
            steal_validator.validate(event);steal_runtime.append(event)
            assert event['adjusted_hp_return']==event['hp_restored']+event['hp_excess'] and event['spirit_removed']==event['spirit_restored']+event['spirit_excess']
        policy={'hp_current_bps':1000,'spirit_current_bps':1000,'hp_policy':'direct_transfer'}
        for bad in [None,{},dict(policy,hp_policy=None),dict(policy,hp_policy={'damage':None}),dict(policy,hp_policy='typo'),dict(policy,hp_current_bps=0),dict(policy,hp_current_bps=10001),dict(policy,spirit_current_bps=-1),dict(policy,extra=1),{'hp_current_bps':0,'spirit_current_bps':0}]:
            assert not steal_validator.is_valid(bad);steal_invalid+=1
    steal_browser_states=[];steal_browser_events=[]
    with sqlite3.connect('file:'+independent['browser']['database']+'?mode=ro',uri=True) as database:
        rooms=[json.loads(row[0])for row in database.execute('SELECT record FROM duo_rooms')]
        for scenario in ['steal-four','steal-eight']:
            selected=[r for r in rooms if r['scenario']==scenario];assert len(selected)==1;room=selected[0];states=set();events=[]
            for raw, in database.execute('SELECT body FROM duo_outputs WHERE room_id=?',(room['id'],)):
                body=json.loads(raw).get('body',{});payload=body.get('payload',{})
                own=payload.get('snapshot',{}).get('self',{})if payload.get('kind')=='full'else payload.get('delta',{}).get('own',{})
                channel=own.get('active_channel')
                if channel:
                    assert body['recipient_id']=='P00' and 'steal_budget'not in channel
                    unit_validator.validate(channel);encoded=json.dumps(channel,sort_keys=True)
                    if encoded not in states:states.add(encoded);steal_browser_states.append(channel)
                if body.get('kind')=='event_v4':
                    event=body.get('event',{});event=event.get('event',{})if event.get('kind')=='private'else event
                    if event.get('kind')=='resource_stolen':
                        assert body['recipient_id']=='P00';steal_validator.validate(event);events.append(event);steal_browser_events.append(event)
            assert len(events)==6 and len({e['action_id']for e in events})==4
    assert steal_config_count==20 and len(steal_runtime)==26 and len(steal_browser_states)>=8 and len(steal_browser_events)==12 and steal_invalid==20,(steal_config_count,len(steal_runtime),len(steal_browser_states),len(steal_browser_events),steal_invalid)
    power_runtime=[];power_invalid=0
    for path in (OUT/'delivery/independent-combat').glob('*.power-facts.json'):
        data=json.loads(path.read_text());assert len(data['projections'])==3
        validator.validate(data['shield']['shield']);power_runtime.append(data['shield']['shield'])
        for own in data['projections']:
            own_validator.validate(own)
            fragment={'skill_powers':own['skill_powers']};power_validator.validate(fragment);power_runtime.append(fragment)
            assert set(own['skill_powers'])=={s['skill_id']for s in own['player_skills']}
            assert 'model_id'not in json.dumps(own) and 'power_points'not in json.dumps(own)
        for row in data['events']:
            event=row['event']
            if event['kind']=='heal_resolved':
                assert row['recipient']in ['P00','P01'];power_validator.validate(event);power_runtime.append(event)
            if event['kind']=='periodic_heal_tick':
                assert row['recipient']in ['P00','P01'];validator.validate(event);power_runtime.append(event)
        for bad in [None,{}, {'skill_powers':None},{'skill_powers':{'s':-1}},{'skill_powers':{'s':4294967296}},{'skill_powers':{'s':1.5}},{'source_skill_id':None},{'source_skill_id':{}},{'status_power_sources':{'s':None}},{'model_id':'growth','rank':16,'quality':0,'level':0,'mastery':0}]:
            assert not power_validator.is_valid(bad);power_invalid+=1
    power_browser_states=[];power_browser_events=[]
    with sqlite3.connect('file:'+independent['browser']['database']+'?mode=ro',uri=True)as database:
        rooms=[json.loads(row[0])for row in database.execute('SELECT record FROM duo_rooms')]
        for scenario in ['power-four','power-eight']:
            selected=[r for r in rooms if r['scenario']==scenario];assert len(selected)==1;room=selected[0];states=set();events=[]
            for raw, in database.execute('SELECT body FROM duo_outputs WHERE room_id=?',(room['id'],)):
                body=json.loads(raw).get('body',{});payload=body.get('payload',{})
                own=payload.get('snapshot',{}).get('self',{})if payload.get('kind')=='full'else payload.get('delta',{}).get('own',{})
                if 'skill_powers'in own:
                    own_validator.validate(own);assert set(own['skill_powers'])=={s['skill_id']for s in own['player_skills']}
                    assert own['participant']['entity']['id']==body['recipient_id']
                    fragment={'skill_powers':own['skill_powers']};power_validator.validate(fragment);encoded=json.dumps([body['recipient_id'],fragment],sort_keys=True)
                    if encoded not in states:states.add(encoded);power_browser_states.append(fragment)
                if body.get('kind')=='event_v4':
                    event=body.get('event',{});event=event.get('event',{})if event.get('kind')=='private'else event
                    if event.get('kind')in ['heal_resolved','periodic_heal_tick']:
                        assert body['recipient_id']in ['P00','P01']
                        (power_validator if event['kind']=='heal_resolved'else validator).validate(event);events.append(event);power_browser_events.append(event)
            assert len(events)==10
    assert power_config_count==28 and len(power_runtime)==28 and len(power_browser_states)>=14 and len(power_browser_events)==20 and power_invalid==20,(power_config_count,len(power_runtime),len(power_browser_states),len(power_browser_events),power_invalid)
    assert loadout_config_count==38 and len(loadout_runtime)==40 and len(loadout_browser_states)>=18 and len(loadout_audits)>=14,(loadout_config_count,len(loadout_runtime),len(loadout_browser_states),len(loadout_audits))
    assert casting_config_count==31 and len(casting_runtime)>=16,(casting_config_count,len(casting_runtime))
    assert rejected_followup_fragments == 26 and rejected_enhancement_fragments == 36
    with tarfile.open(archive) as derived_tar:
        derived_report=verify_derived_schema(ROOT,OUT,package,derived_tar)
        source_gate_report=verify_gates_schema(ROOT,OUT,package,derived_tar)
        stack_window_report=verify_counter_schema(ROOT,OUT,package,derived_tar)
        conditional_activation_report=verify_activation_schema(ROOT,OUT,package,derived_tar)
        paid_resource_report=verify_paid_schema(ROOT,OUT,package,derived_tar)
        cast_interception_report=verify_interception_schema(ROOT,OUT,package,derived_tar)
    report = {
        **derived_report, **source_gate_report, **stack_window_report,**conditional_activation_report,**paid_resource_report, **cast_interception_report,
        'passed': True, 'archiveSha256': package['sha256'], 'binarySha256': package['binarySha256'],
        'schema': 'docs/interfaces/combat-rules-v1.schema.json',
        'characterPowerSchema':'docs/interfaces/character-build-v1.schema.json','characterPowerSchemaSha256':hashlib.sha256(power_schema_bytes).hexdigest(),'characterPowerConfigFragments':power_config_count,'characterPowerRuntimeFragments':len(power_runtime),'characterPowerBrowserStates':len(power_browser_states),'characterPowerBrowserEvents':len(power_browser_events),'invalidCharacterPowerFragmentsRejected':power_invalid,
        'resourceStealSchema':'docs/interfaces/resource-steal-v1.schema.json','resourceStealSchemaSha256':hashlib.sha256(steal_schema_bytes).hexdigest(),'resourceStealConfigFragments':steal_config_count,'resourceStealRuntimeFragments':len(steal_runtime),'resourceStealBrowserStates':len(steal_browser_states),'resourceStealBrowserEvents':len(steal_browser_events),'invalidResourceStealFragmentsRejected':steal_invalid,
        'reservationSchema':'docs/interfaces/resource-commit-v1.schema.json','reservationSchemaSha256':hashlib.sha256(reservation_schema_bytes).hexdigest(),'reservationConfigFragments':reservation_config_count,'reservationRuntimeFragments':len(reservation_runtime),'reservationBrowserStates':len(reservation_browser_states),'reservationBrowserEvents':len(reservation_browser_events),
        'unitEffectSchema':'docs/interfaces/unit-effects-v1.schema.json','unitEffectSchemaSha256':hashlib.sha256(unit_schema_bytes).hexdigest(),'unitEffectConfigFragments':unit_config_count,'unitEffectRuntimeFragments':len(unit_runtime),'unitEffectBrowserStates':len(unit_browser_states),'unitEffectBrowserEvents':len(unit_browser_events),
        'skillLoadoutSchema':'docs/interfaces/skill-loadouts-v1.schema.json','skillLoadoutSchemaSha256':hashlib.sha256(loadout_schema_bytes).hexdigest(),'skillLoadoutConfigFragments':loadout_config_count,'skillLoadoutRuntimeFragments':len(loadout_runtime),'skillLoadoutBrowserStates':len(loadout_browser_states),'skillLoadoutAuditsReplayed':len(loadout_audits),
        'periodicCompositionConfigFragments':periodic_config_count,'periodicCompositionRuntimeFragments':len(periodic_runtime),'periodicCompositionBrowserStates':len(browser_periodic),
        'castingSchema':'docs/interfaces/cast-lifecycle-v1.schema.json','castingSchemaSha256':hashlib.sha256(casting_schema_bytes).hexdigest(),'castingConfigFragments':casting_config_count,'castingRuntimeFragments':len(casting_runtime),'castingNativeCosts':casting_costs,
        'schemaSha256': hashlib.sha256(schema_bytes).hexdigest(),
        'consumedSampleCopiesCompared': sample_count, 'invalidFollowupFragmentsRejected':rejected_followup_fragments, 'invalidEnhancementFragmentsRejected':rejected_enhancement_fragments,
        'validatedRuntimeFragments': runtime_count, 'validatedConfigFragments': config_count, 'nativeFollowupTimings':native_followup_timings,
    }
    (OUT / 'record-schema-check.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
