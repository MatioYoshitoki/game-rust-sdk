"""Record evidence for the verified candidate; never close the wider work packages."""
import hashlib
import json
import os
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(os.environ.get('SDK88_EVIDENCE_DIR', ROOT / 'artifacts/sdk-5-88'))
PRIOR_CHECKPOINT = None


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical_hash(value):
    return digest(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode())


def pointer(value, path):
    for key in path.strip('/').split('/'):
        value = value[int(key)] if isinstance(value, list) else value[key]
    return value


def verify_source_rows(source):
    path = ROOT/source['file']
    assert digest(path.read_bytes()) == source['fileSha256']
    document = read(path)
    for row in source['rows']:
        assert canonical_hash(pointer(document, row['pointer'])) == row['sha256']
    return document


def verify_prior():
    directory = PRIOR_CHECKPOINT or OUT/'checkpoints/core-38'
    assert directory.is_dir(), 'core-38 evidence required (set SDK88_PRIOR_CHECKPOINT)'
    manifest = directory.parent/'core-38-sha256.json'
    records = read(manifest)
    files = {str(p.relative_to(directory)):digest(p.read_bytes()) for p in directory.rglob('*') if p.is_file()}
    assert records == files
    prior = read(directory/'acceptance.json')
    assert prior['crateVersion'] == '0.101.0' and prior['candidatePassed']
    return prior, manifest, len(records)



def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def main():
    prior, prior_manifest, prior_count = verify_prior()
    checks = read(OUT / 'extended-checks.json')
    independent = read(OUT / 'delivery/independent-verification.json')
    package = read(OUT / 'delivery/package.json')
    assert package['version'] == '0.102.0'
    assert checks['passed'] and independent['passed']
    assert checks['runtimeSourceSha256'] == independent['runtimeSourceSha256']
    assert package['sha256'] == independent['archiveSha256']
    assert hashlib.sha256(Path(package['archive']).read_bytes()).hexdigest() == package['sha256']
    assert all(independent[k]['passed'] for k in ['combat', 'legacy104', 'browser'])
    assert all(row['attackIntervals']==[858,600] for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['guardProbe']['recorded']==(150 if row['scenario']=='four' else 180) and row['guardProbe']['rawCounter']==(90 if row['scenario']=='four' else 108) and row['guardProbe']['consumed']==1 for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['statusProbe']=={'transferred':3,'preservedInstance':True,'cleansed':1,'blocked':1,'deathGuards':1,'reconnected':True,'protectedHp':151 if row['scenario']=='four' else 181} for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['aggregateProbe']=={'totals':([1344,1344] if row['scenario']=='four' else [1296,1296]),'contacts':[6,6],'rewards':2,'targetRewards':2,'reconnected':True} for k in ['combat','browser'] for row in independent[k]['cases'])
    schema_report=read(OUT/'record-schema-check.json')
    assert schema_report['passed'] and schema_report['archiveSha256']==package['sha256']
    assert all(row['windowProbe']=={'recorded':2000 if row['scenario']=='four' else 2592,'contacts':12,'rawHp':200 if row['scenario']=='four' else 180,'rawSpirit':40 if row['scenario']=='four' else 25,'allySpiritRestored':1,'source':'P00','target':'P01'} for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['compoundProbe']=={'flags':['silence','root'] if row['scenario']=='four' else ['stun'],'modifierBps':-2000 if row['scenario']=='four' else -3000,'immunityFlags':['control_immunity'],'oneIdentity':True,'reconnected':True,'refreshed':True,'expired':True} for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['cleanseProbe']=={'ticks':[1,2,3],'removed':[1,0,0,1],'reconnected':True,'endBeforeRecovery':True,'privateCountOnly':True} for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['resourceProbe']=={'base':([1,0,0] if row['scenario']=='four' else [2,0,0]),'converted':([3,0,0] if row['scenario']=='four' else [2,0,0]),'spent':([6,0,0] if row['scenario']=='four' else [8,0,0]),'shield':([30,33,33] if row['scenario']=='four' else [28,32,32]),'conserved':True,'sourceOwnUnchanged':True} for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['overhealRecordProbe']=={'balance':96 if row['scenario']=='four' else 92,'basis':'incoming' if row['scenario']=='four' else 'own_absorbed','recorded':40 if row['scenario']=='four' else 50,'rawCounter':20 if row['scenario']=='four' else 30,'targetsHit':1,'reconnected':True,'privateRecipients':2} for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['shieldTransferProbe']=={'granted':200,'absorbed':1,'balance':199,'recorded':None if row['scenario']=='four' else 1,'recordCap':None if row['scenario']=='four' else 180,'reconnected':True,'expired':True,'endEvents':0 if row['scenario']=='four' else 1} for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['periodicTransferProbe']=={'source':'power-target','target':'P00','totalTicks':9 if row['scenario']=='four' else 7,'lastTick':9 if row['scenario']=='four' else 7,'sampling':'source_snapshot' if row['scenario']=='four' else 'target_live','reconnected':True,'expired':True,'privateRecipients':1} for k in ['combat','browser'] for row in independent[k]['cases'])
    assert all(row['controlTransferProbe']=={'kind':'scalar' if row['scenario']=='four' else 'compound','source':'power-target','target':'P00','reconnected':True,'expired':True,'privateRecipients':1,'duration':1500} for k in ['combat','browser'] for row in independent[k]['cases'])
    for feature in ['combat','browser']:
        rows=independent[feature]['allySupportCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'source':'P02','target':'P03','originalSource':'control-source','flags':['silence','root'] if seats==4 else ['stun'],'preserved':True,'privateRecipients':2,'blockedWithoutCost':True,'oldHolderLeft':True,'reconnected':True,'expired':True,'resumed':True,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['lifecycleCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'source':'P00','previous':'P01','target':'P03','ticksBefore':1,'ticksAfter':[2,3],'cleanseRemoved':[1,0,1],'windowSpirit':2,'transferRecipients':3,'subsequentRecipients':2,'oldShieldIsolated':True,'oldHolderLeft':True,'reconnected':True,'expired':True,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['controlPolicyCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'cap':1800,'duration':3600 if seats==4 else 4500,'movementSlow':-5000,'intervalDelay':1500,'interval':690 if seats==4 else 930,'refreshed':True,'reconnected':True,'expired':True,'privateRecipients':1,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['hitStatusCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='marks-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'layers':[1,2,3],'consumed':3,'effect':'compound-lock' if seats==4 else 'control-softened','duration':1200 if seats==4 else 1800,'source':'P00','privateRecipients':1,'reconnected':True,'expired':True,'sourceLeftWithMark':True,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['contactModifierCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='strikes-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'damage':[50,64,78,94,50] if seats==4 else [50,78,111,111,78,50],'consumed':[1,2,3] if seats==4 else [1,2,2,1],'partial':seats==8,'reconnected':True,'privateRecipients':1,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['chainDamageCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='chain-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'damage':[80,60,54] if seats==4 else [50,25,18],'targets':['chain-a','chain-b','chain-c'],'consumed':3,'privateRecipients':1,'reconnected':True,'noReplay':True,'expired':True,'statuses':['compound-lock','compound-lock','control-softened'],'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['postConsumptionCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='consumed-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'consumed':3,'raw':175 if seats==4 else 130,'pursuit':105 if seats==4 else 52,'target':'chain-a','skippedBehind':True,'remainingBps':3000 if seats==4 else 5000,'fixedMs':0 if seats==4 else 60,'privateRecipients':1,'reconnected':True,'noReplay':True,'readyBeforeOriginal':True,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['contactReactionCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='reaction-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'source':'P01','provoker':'P00','raw':25 if seats==4 else 67,'hpDamage':25 if seats==4 else 67,'consumed':1,'privateRecipients':1,'preexistingOnly':True,'reconnected':True,'noReplay':True,'once':True,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['finisherCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='finisher-four' else 8
            ordinary=400 if seats==4 else 200;replacement=100 if seats==4 else 150
            assert row=={'scenario':row['scenario'],'members':seats,'targets':['capped-target','replacement-target','guard-target'],'modes':['execute','power_damage','execute'],'raw':[ordinary,replacement,ordinary],'lethal':[True,False,False],'guardHp':1,'replacementHp':300 if seats==4 else 50,'consumed':3,'privateRecipients':1,'reconnected':True,'noReplay':True,'guardKilledAfterward':True,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['finisherFollowupCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='followup-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'provider':'threshold-finish','mode':'execute','postEffects':4,'spiritRestored':20 if seats==4 else 30,'spiritAfter':100 if seats==4 else 110,'maxSpirit':250 if seats==4 else 300,'chainRaw':320 if seats==4 else 120,'chainTarget':'follow-target','privateRecipients':1,'reconnected':True,'noReplay':True,'readyBeforeOriginal':True,'expired':True,'settled':seats}
    for feature in ['combat','browser']:
        rows=independent[feature]['actionEnhancementCases']
        assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='enhancement-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'targets':['line-a','line-b'],'thresholdBps':5500 if seats==4 else 6000,'primaryRaw':450 if seats==4 else 460,'consumed':1,'enhancedKills':2,'grantCount':2,'spiritAfterLine':156 if seats==4 else 160,'spiritRewards':1,'cooldownRewards':1,'privateRecipients':1,'reconnected':True,'inFlightReconnect':True,'noSelfRefresh':True,'newActionGranted':True,'expired':True,'settled':seats}
    counts = checks['checks']
    fixture = ROOT / 'src/verification/source_combat_vectors.json'
    vector_ids = [r['id'] for r in read(fixture)['vectors'] if r['op'] in ['realm_modifier', 'defense_damage', 'dodge_percent']]
    source_statuses = read(ROOT / 'src/verification/source_status_mappings.json')['statuses']
    followup_fixture=read(ROOT/'src/verification/source_finisher_followup_vectors.json')
    original=next(d for d in read(ROOT/'rust-match-kernel-v1.1/source-rules.json')['documents'] if d['path']==followup_fixture['sourceDocument'])
    assert hashlib.sha256(original['content'].encode()).hexdigest()==original['sha256']==followup_fixture['sourceSha256']
    assert followup_fixture['cooldownBps']==3000 and followup_fixture['spiritBps']==800 and followup_fixture['chainBps']==[8000,6000]
    assert schema_report['invalidFollowupFragmentsRejected']==26 and len(schema_report['nativeFollowupTimings'])==2
    enhancement_fixture=read(ROOT/'src/verification/source_action_enhancement_vectors.json')
    assert enhancement_fixture['sourceSha256']==original['sha256'] and enhancement_fixture['sourceDocument']==original['path']
    assert enhancement_fixture['bonusBps']==500 and enhancement_fixture['attributeBps']==2500 and enhancement_fixture['durationMs']==30000
    assert schema_report['invalidEnhancementFragmentsRejected']==36
    caster_fixture=read(ROOT/'src/verification/source_cast_lifecycle_vectors.json')
    split=caster_fixture['quantitySplitSource'];split_doc=next(d for d in read(ROOT/'rust-match-kernel-v1.1/source-rules.json')['documents'] if d['path']==split['document'])
    assert hashlib.sha256(split_doc['content'].encode()).hexdigest()==split_doc['sha256']==split['sha256'] and split['equalBudgetShareRemainder']=='final'
    for row in caster_fixture['vectors']:
        source_path=ROOT/row['source']['file'];assert hashlib.sha256(source_path.read_bytes()).hexdigest()==row['source']['fileSha256']
        value=read(source_path)
        for token in row['source']['pointer'].split('/')[1:]:value=value[token.replace('~1','/').replace('~0','~')]
        assert hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==row['source']['rowSha256']
        for field in ['spiritCost','castLifecycle','factors']:assert row[field]==value[field]
    for feature in ['combat','browser']:
        rows=independent[feature]['castLifecycleCases'];assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='casting-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'largeCost':8000000,'movementRecovery':True,'atomicSelfTakeover':True,'interruptedPreparationFree':True,'queuedReleaseOnce':True,'mobileChannel':True,'committedFlightPreserved':True,'reconnected':True,'recoveryPulses':3,'recoveryRawHp':17 if seats==4 else 19,'recoveryRawSpirit':23 if seats==4 else 29,'private':True,'settled':seats}
    composed_fixture=read(ROOT/'src/verification/source_composed_status_vectors.json')
    assert len(composed_fixture['statuses'])==7
    for row in composed_fixture['statuses']:
        source_path=ROOT/row['source']['file'];assert hashlib.sha256(source_path.read_bytes()).hexdigest()==row['source']['fileSha256']
        value=read(source_path)
        for token in row['source']['pointer'].split('/')[1:]:value=value[token.replace('~1','/').replace('~0','~')]
        assert hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==row['source']['rowSha256']
    for feature in ['combat','browser']:
        rows=independent[feature]['periodicCompositionCases'];assert len(rows)==2
        for row in rows:
            four=row['scenario']=='composed-four';seats=4 if four else 8;layers=row['layers'];assert 3<=layers<=10
            assert row=={'scenario':row['scenario'],'members':seats,'layers':layers,'sampling':'target_live' if four else 'target_snapshot','firstRaw':(120 if four else 100)*700*layers//10000,'transferredRaw':(240 if four else 100)*700*layers//10000,'source':'burn-source-1','target':'P01','transferRecipients':2,'subsequentRecipients':1,'oldHolderLeft':True,'reconnected':True,'cleanseRemoved':1,'noLateTick':True,'settled':seats}
    assert schema_report['periodicCompositionConfigFragments']==12
    for feature in ['combat','browser']:
        rows=independent[feature]['skillLoadoutCases'];assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='loadouts-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'revision':6,'privateRecipients':1,'preparedCancelledFree':True,'emptySlots':True,'slotOrder':['old-bolt','learned-bolt'],'oldCooldownPreserved':True,'committedFlightPreserved':True,'paidSpirit':4,'exactRetryOnce':True,'casRejected':True,'reconnected':True,'settled':seats}
    assert schema_report['skillLoadoutConfigFragments']==38 and schema_report['skillLoadoutRuntimeFragments']==40
    unit_fixture=read(ROOT/'src/verification/source_unit_channel_vectors.json');source_path=ROOT/unit_fixture['source']['file'];assert hashlib.sha256(source_path.read_bytes()).hexdigest()==unit_fixture['source']['fileSha256']
    value=read(source_path)
    for token in unit_fixture['source']['pointer'].split('/')[1:]:value=value[token.replace('~1','/').replace('~0','~')]
    assert hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==unit_fixture['source']['rowSha256']
    for field in ['castLifecycle','channelExecution','factors']:assert unit_fixture[field]==value[field]
    for feature in ['combat','browser']:
        rows=independent[feature]['unitEffectCases'];assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='units-four' else 8;shares=[2,2,4]if seats==4 else[3,3,5]
            assert row=={'scenario':row['scenario'],'members':seats,'noProjectile':True,'damageShares':shares,'healShares':shares,'restoredHp':8 if seats==4 else 11,'fullOnceGates':2,'healRecipients':2,'planRecipients':1,'reconnectStopped':True,'targetExitStopped':True,'paidSpirit':22,'settled':seats}
    assert schema_report['unitEffectConfigFragments']==18 and schema_report['unitEffectRuntimeFragments']==24
    reservation_fixture=read(ROOT/'src/verification/source_resource_reservation_vectors.json');groups={}
    for row in reservation_fixture['vectors']:
        path=ROOT/row['source']['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==row['source']['fileSha256'];value=read(path)
        for token in row['source']['pointer'].split('/')[1:]:value=value[token.replace('~1','/').replace('~0','~')]
        assert hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==row['source']['rowSha256']
        assert row['resources']==value['castLifecycle']['resources']
        for id,skill in read(path)['tables']['data/skills']['SKILL_REGISTRY'].items():groups.setdefault(skill.get('castLifecycle',{}).get('resources',{}).get('reserveAt','missing'),set()).add(id)
    assert {k:sorted(v) for k,v in groups.items()}=={k:sorted(v)for k,v in reservation_fixture['groups'].items()}
    assert {k:len(v)for k,v in groups.items()}=={'accept':58,'none':20,'missing':8}
    for feature in ['combat','browser']:
        rows=independent[feature]['reservationCases'];assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='reservations-four' else 8
            assert row=={'scenario':row['scenario'],'members':seats,'initialHp':960,'quotedHp':96 if seats==4 else 192,'liveHpCost':100 if seats==4 else 200,'paidSpirit':4,'noEarlyDebit':True,'reconnectReleased':True,'cancelReleased':True,'privateRecipients':1,'settled':seats}
    assert schema_report['reservationConfigFragments']==16 and schema_report['reservationRuntimeFragments']==12
    steal_fixture=read(ROOT/'src/verification/source_resource_steal_vectors.json')
    assert steal_fixture['originalMappingAccepted'] is False and steal_fixture['originalHpPolicy']=='awaiting_user_rule'
    for source in steal_fixture['sources']:
        source_path=ROOT/source['file'];assert hashlib.sha256(source_path.read_bytes()).hexdigest()==source['fileSha256'];value=read(source_path)
        for token in source['pointer'].split('/')[1:]:value=value[token.replace('~1','/').replace('~0','~')]
        assert hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==source['rowSha256']
    for feature in ['combat','browser']:
        rows=independent[feature]['resourceStealCases'];assert len(rows)==2
        for row in rows:
            four=row['scenario']=='steal-four';seats=4 if four else 8
            assert row=={'scenario':row['scenario'],'members':seats,'hpPolicy':'direct_transfer'if four else'damage','requestedHp':[33,33,34]if four else[50,50,50],'requestedSpirit':[4,4,4]if four else[4,4,5],'hpRemoved':100 if four else 91,'spiritRemoved':12 if four else 13,'hpRestored':40,'spiritRestored':6,'nthStatus':'life_steal','lifestealBps':1000 if four else 1500,'prefixCounts':[1,1,1],'cancelled':True,'reconnected':True,'sourceLeft':True,'noReplay':True,'privateRecipients':1,'settled':seats}
    assert schema_report['resourceStealConfigFragments']==20 and schema_report['resourceStealRuntimeFragments']==26
    power_fixture=read(ROOT/'src/verification/source_character_power_vectors.json')
    for source in power_fixture['sources']:
        source_path=ROOT/source['file'];assert hashlib.sha256(source_path.read_bytes()).hexdigest()==source['fileSha256'];value=read(source_path)
        for token in source['pointer'].split('/')[1:]:value=value[token.replace('~1','/').replace('~0','~')]
        assert hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==source['rowSha256']
    document=power_fixture['sourceDocument'];source_path=ROOT/document['file'];assert hashlib.sha256(source_path.read_bytes()).hexdigest()==document['fileSha256']
    row=next(d for d in read(source_path)['documents']if d['path']==document['path']);assert hashlib.sha256(row['content'].encode()).hexdigest()==document['contentSha256']
    for feature in ['combat','browser']:
        rows=independent[feature]['characterPowerCases'];assert len(rows)==2
        for row in rows:
            four=row['scenario']=='power-four';seats=4 if four else 8;power=99 if four else 162
            assert row=={'scenario':row['scenario'],'members':seats,'basic':150 if four else 200,'skill':power,'boosted':108 if four else 189,'peerSkill':112 if four else 202,'recordCap':power,'healRaw':power,'healActual':40,'hotRaw':[99,99,99,99]if four else[189,189,162,162],'sampling':'source_snapshot'if four else'source_live','derivedMaxHp':1000 if four else 1250,'rawGrowthPrivate':True,'memberIsolation':True,'reconnected':True,'expired':True,'settled':seats}
    assert schema_report['characterPowerConfigFragments']==28 and schema_report['characterPowerRuntimeFragments']==28
    derived_fixture=read(ROOT/'src/verification/source_derived_vectors.json')
    source=derived_fixture['sourceConfig'];path=ROOT/source['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['fileSha256'];row=read(path)
    for token in source['pointer'].split('/')[1:]:row=row[token]
    assert hashlib.sha256(json.dumps(row,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==source['rowSha256']
    source=derived_fixture['sourceRules'];path=ROOT/source['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['fileSha256'];documents=read(path)['documents']
    for d in source['documents']:
        content=next(row['content']for row in documents if row['path']==d['path']);assert hashlib.sha256(content.encode()).hexdigest()==d['contentSha256']
    assert 'derived=true' in next(row['content']for row in documents if row['path']==source['documents'][0]['path'])
    for feature in ['combat','browser']:
        rows=independent[feature]['derivedEffectsCases'];assert len(rows)==2
        for row in rows:
            four=row['scenario']=='derived-four';seats=4 if four else 8
            assert row=={'scenario':row['scenario'],'members':seats,'seedEnds':2,'recorded':40,'counter':20 if four else 30,'dot':[10,10]if four else[15,15],'delayed':58 if four else 100,'sampling':'source_snapshot'if four else'source_live','derivedTriggerCount':0,'ordinaryTriggerCount':2,'reconnected':True,'deadlinePreserved':True,'sourceExitCancelled':True,'privateRecipients':True,'settled':seats}
    assert schema_report['derivedConfigFragments']==8 and schema_report['derivedRuntimeFragments']==32
    source_gate_fixture=read(ROOT/'src/verification/source_gate_vectors.json')
    source=source_gate_fixture['sourceConfig'];path=ROOT/source['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['fileSha256'];tree=read(path)
    for record in source['rows']:
        row=tree
        for token in record['pointer'].split('/')[1:]:row=row[token]
        assert hashlib.sha256(json.dumps(row,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==record['rowSha256']
    source=source_gate_fixture['sourceRules'];path=ROOT/source['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['fileSha256'];content=next(d['content']for d in read(path)['documents']if d['path']==source['path']);assert hashlib.sha256(content.encode()).hexdigest()==source['contentSha256']
    for feature in ['combat','browser']:
        rows=independent[feature]['sourceGateCases'];assert len(rows)==2
        for row in rows:
            four=row['scenario']=='source-four';seats=4 if four else 8;quota=2 if four else 3
            assert row=={'scenario':row['scenario'],'members':seats,'initialMarkers':[1,0,0,1],'directAndPeriodicReturns':[3,0,0,3],'thirdPartySource':'P00','reactionRaw':25,'excludedOwnSkills':2,'basicQualified':True,'quota':quota,'afterRevocation':quota+1,'inFlightCompleted':True,'canonicalGrowthWithAlternateEquipped':True,'reconnected':True,'deadlinePreserved':True,'privateSourceMetadata':True,'sourceExitCleaned':True,'settled':seats}
    assert schema_report['sourceGateConfigFragments']==42 and schema_report['sourceGateRuntimeFragments']==55
    stack_fixture=read(ROOT/'src/verification/stack_window_vectors.json')
    doc=next(d for d in read(ROOT/'rust-match-kernel-v1.1/source-rules.json')['documents']if d['path']==stack_fixture['sourceDocument'])
    assert doc['sha256']==stack_fixture['sourceDocumentSha256'] and stack_fixture['row'] in doc['content'].splitlines()
    assert hashlib.sha256(stack_fixture['row'].encode()).hexdigest()==stack_fixture['rowSha256']
    assert schema_report['stackWindowConfigFragments']==32 and schema_report['stackWindowRuntimeFragments']==48
    for feature in ['combat','browser']:
        rows=independent[feature]['stackWindowCases'];assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='counter-four'else 8
            assert row=={'scenario':row['scenario'],'members':seats,'initial':0,'maximum':3,'fixedLifeSteal':1500,'layerLifeSteal':300,'fixedReduction':2000,'lifetime':12000 if seats==4 else 15000,'heals':[15,18,21,24,15,18,21],'cleanses':2,'consumedToZero':True,'recorded':700,'expiryHeal':70,'finalHp':802,'peerHp':625,'sameInstance':True,'sameDeadline':True,'reconnected':True,'private':True,'exitCleaned':True,'settled':seats}
    activation_fixture=read(ROOT/'src/verification/conditional_activation_vectors.json')
    source=activation_fixture['sourceConfig'];path=ROOT/source['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['fileSha256'];original_config=read(path)
    for record in source['rows']:
        value=original_config
        for key in record['pointer'].strip('/').split('/'):value=value[key]
        assert hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==record['sha256']
    source=activation_fixture['sourceRules'];path=ROOT/source['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['fileSha256'];doc=next(d for d in read(path)['documents']if d['path']==source['document']);assert hashlib.sha256(doc['content'].encode()).hexdigest()==source['documentSha256']==doc['sha256'];assert source['row'] in doc['content'].splitlines() and hashlib.sha256(source['row'].encode()).hexdigest()==source['rowSha256']
    assert not activation_fixture['originalHpPolicyAccepted'] and activation_fixture['originalStackModeAccepted'] and not activation_fixture['fullSourceSkillEffectsAccepted']
    assert activation_fixture['durationMs']==5000 and activation_fixture['consumeStacks']==3 and activation_fixture['masteredLifeStealBps']==2000 and activation_fixture['killRefreshMs']==10000
    assert activation_fixture['approvedSourceMode']=='multiply_base'
    assert activation_fixture['approvedLayerRatiosBps']==[1000,1100,1200,1300]
    assert activation_fixture['approvedLayerBudgets']==[[100,12],[110,13],[120,14],[130,15]]
    assert activation_fixture['alternateModeIsGenericOnly']
    assert counts['conditional-activation']['count']==32 and schema_report['conditionalActivationConfigFragments']==44 and schema_report['invalidConditionalActivationFragmentsRejected']==31
    for feature in ['combat','browser']:
        rows=independent[feature]['conditionalActivationCases'];assert len(rows)==2
        for row in rows:
            four=row['scenario']=='activation-four';seats=4 if four else 8
            assert row['members']==row['settled']==seats and row['mode']==('multiply_base'if four else'add_current_bps')
            assert row['pulses']==[33,33,34] and row['consumed']==[0,3,3,0] and row['marksAfter']==[3,1,1,3] and row['sourceDurations']==5000 and row['killRefreshMs']==10000
            assert all(row[k]for k in ['frozenBeforeConsumption','atomicPair','killClockRenewed','reconnected','expired','private']) and row['exitPrefix']==1
            assert [v['stage']for v in row['samples']]==([1,2,4,0]if four else[1,3,4,0])
            assert [v['requestedHp']for v in row['samples']]==([1300,1300,1118,860]if four else[4000,4000,2360,590])
            assert [v['requestedSpirit']for v in row['samples']]==([1300,1300,1131,870]if four else[4000,4000,2400,600])
    assert independent['browser']['rooms']==50 and schema_report['consumedSampleCopiesCompared']==86
    paid_fixture=read(ROOT/'src/verification/paid_resource_vectors.json')
    source=paid_fixture['sourceConfig'];path=ROOT/source['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['fileSha256'];original_config=read(path)
    for record in source['rows']:
        value=original_config
        for key in record['pointer'].strip('/').split('/'):value=value[key]
        assert hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()==record['sha256']
    source=paid_fixture['sourceRules'];path=ROOT/source['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==source['fileSha256'];doc=next(d for d in read(path)['documents']if d['path']==source['document']);assert hashlib.sha256(doc['content'].encode()).hexdigest()==source['documentSha256']==doc['sha256'];assert source['row']in doc['content'].splitlines()and hashlib.sha256(source['row'].encode()).hexdigest()==source['rowSha256']
    assert paid_fixture['paidSpiritBps']==3000 and paid_fixture['initialCharges']==3 and paid_fixture['carrierMs']==8000 and paid_fixture['exhaustionLockMs']==3000 and not paid_fixture['fullOriginalRealmTechniqueAccepted']
    assert counts['paid-resources']['count']==19 and schema_report['paidResourceConfigFragments']==24 and schema_report['paidResourceRuntimeFragments']==152 and schema_report['invalidPaidResourceFragmentsRejected']==20
    for feature in ['combat','browser']:
        rows=independent[feature]['paidResourceCases'];assert len(rows)==2
        for row in rows:
            four=row['scenario']=='paid-four';seats=4 if four else 8;cost=10 if four else 20;refund=3 if four else 10
            assert row['members']==row['settled']==seats and row['cost']==cost and row['refund']==refund and row['refundsPerActor']==3 and row['afterExpiryRecovery']==7
            assert all(row[k]for k in ['fullCostFirst','exhausted','lockRecoveryBlocked','reconnected','noReplay','private'])
            assert row['samples']==[{'pid':pid,'paid':cost*4,'refunded':refund*3,'spiritAfter':69 if four else 150,'chargesConsumed':3,'lockMs':3000 if four else 3600}for pid in ['P00','P01','P02','P03']]
    gate = read(ROOT/'src/verification/cast_interception_vectors.json')
    original = verify_source_rows(gate['sourceConfig'])
    documents = gate['sourceRules']
    path = ROOT/documents['file']
    assert digest(path.read_bytes()) == documents['fileSha256']
    rules = {d['path']:d for d in read(path)['documents']}
    for binding in documents['documents']:
        document = rules[binding['document']]
        assert digest(document['content'].encode()) == binding['documentSha256'] == document['sha256']
        assert binding['row'] in document['content'].splitlines()
        assert digest(binding['row'].encode()) == binding['rowSha256']
    original_tables = original['tables']
    assert original_tables['skills']['realm-divine-lock']['factors'][0]['params'][1] == gate['markMs'] == 5000
    assert original_tables['status-effects']['effect_realm_divine_lock']['attachedFactors'][0]['params'] == [gate['sealMs'],gate['bossSlowMs']] == [700,1200]
    assert original_tables['status-effects']['effect_realm_divine_lock_boss_slow']['attachedFactors'][0]['params'][1]*100 == gate['bossSlowBps'] == -2500
    assert gate['immunity']=={'consumeMark':True,'allowCommit':True,'applyFollowup':False}
    assert gate['sealAttribute']=='silence' and not gate['fullOriginalRealmTechniqueAccepted']
    assert counts['cast-interception']['count']==10
    for key, expected in [('ConfigFragments',18),('RuntimeFragments',32),('BrowserStates',24),('BrowserEvents',8)]:
        assert schema_report['castInterception'+key]==expected
    assert schema_report['invalidCastInterceptionFragmentsRejected']==20
    for feature in ('combat','browser'):
        rows=independent[feature]['castInterceptionCases'];assert len(rows)==2
        for row in rows:
            seats=4 if row['scenario']=='interception-four' else 8
            four=seats==4
            assert row=={'scenario':row['scenario'],'members':seats,'sealMs':700 if four else 900,
                         'slowMs':1200 if four else 1500,'slowBps':-2500 if four else -2000,
                         'cost':8 if four else 12,'outcomes':4,
                         ('duplicateFree' if feature=='combat' else 'silenceVisible'):True,
                         'reconnected':True,'private':True,'settled':seats}
    routes=read(ROOT/'src/verification/source_execution_routes.json')
    assert routes['tolerance']==1e-9
    vectors_path=ROOT/'rust-match-kernel-v1.1/numeric-vectors.json'
    scenarios_path=ROOT/'rust-match-kernel-v1.1/scenarios.json'
    assert digest(vectors_path.read_bytes())==routes['sourceFileSha256']
    assert digest(scenarios_path.read_bytes())==routes['sourceScenariosFileSha256']
    original_vectors,original_scenarios=read(vectors_path),read(scenarios_path)
    assert len(routes['vectors'])==len(original_vectors['vectors'])==50
    exact=[]
    for row in routes['vectors']:
        source=row['source'];assert ROOT/source['file']==vectors_path
        actual=pointer(original_vectors,source['pointer'])
        assert row['original']==actual and row['id']==actual['id']
        assert canonical_hash(actual)==source['rowSha256']
        if row['exactExecutionStatus']=='native_assertion_verified':exact.append(row)
        assert not row['fullWorkPackageAccepted']
    assert len(exact)==27 and len({row['id']for row in exact})==27
    for row in routes['r3Scenarios']:
        original_row=pointer(original_scenarios,row['sourcePointer'])
        assert row['original']==original_row and row['id']==original_row['id']
        assert canonical_hash(original_row)==row['sourceRowSha256']
    assert {row['id']:row['exactExecutionStatus']for row in routes['r3Scenarios']}=={
        'S08':'native_numeric_assertion_verified','S10':'native_domain_assertion_verified',
        'S11':'native_protocol_assertion_verified','S12':'native_domain_assertion_verified',
        **dict.fromkeys(['S20','S28'],'not_run')}
    assert not routes['wholeR6_2Accepted'] and not routes['wholeDelivery01Accepted']
    assert counts['source-execution']['count']==5
    source_cast=read(ROOT/'src/verification/source_cast_scenarios.json')
    for name,expected in source_cast['sourceFiles'].items():
        assert digest((ROOT/'rust-match-kernel-v1.1'/name).read_bytes())==expected
    resolved=read(ROOT/'rust-match-kernel-v1.1/resolved-config.json')['tables']['data/skills']['SKILL_REGISTRY']
    assert all(row==resolved[skill] for skill,row in source_cast['skills'].items())
    assert source_cast['fixture']==original_scenarios['fixtures']['plain-active']
    assert source_cast['fixtureContract']==original_scenarios['fixtureContract']
    original_scenario_rows={row['id']:row for row in original_scenarios['scenarios']}
    assert source_cast['scenarios']==[original_scenario_rows[k] for k in ('S10','S11','S12')]
    assert source_cast['standardTerminalScenario']==original_scenario_rows['S28']
    assert counts['source-cast-scenarios']['count']==4 and schema_report['immediateSelfCommitConfigFragments']==1
    terminal=read(ROOT/'src/verification/terminal_counter_vectors.json')
    terminal_original=verify_source_rows(terminal['sourceConfig'])
    source=terminal['sourceRules'];path=ROOT/source['file']
    assert digest(path.read_bytes())==source['fileSha256']
    document=next(d for d in read(path)['documents'] if d['path']==source['document'])
    assert digest(document['content'].encode())==source['documentSha256']==document['sha256']
    assert source['row'] in document['content'].splitlines() and digest(source['row'].encode())==source['rowSha256']
    parameters=terminal_original['tables']['status-effects']['effect_yishen_seed']['attachedFactors'][1]['params']
    assert [parameters[i] for i in (1,5,6,14,15)]==[terminal[k]for k in (
        'seedGain','seedOwner','seedPhase','seedExcludedSkill','seedMaximumPerAction')]
    assert [terminal[k]for k in ('seedGain','seedOwner','seedPhase','seedMaximumPerAction')]==[1,'caster','afterAttack',1]
    assert not terminal['targetDependentAggregateSourceRowFound'] and not terminal['fullOriginalStageBindingsAccepted']
    assert counts['terminal-counters']['count']==3
    assert schema_report['terminalCounterNodeEvents']==schema_report['terminalCounterBrowserEvents']==8
    timing=OUT/'ct-h5-timing-observation.json'

    acceptance = {
        'batch': 'SDK-5-88', 'status': 'in_progress', 'candidatePassed': True, 'wholeTaskAccepted': False,
        'crateVersion': '0.102.0', 'mechanismVersion': 105, 'schemaVersion': 7, 'protocolVersion': 6,
        'workPackages': ['R3.1', 'R3.2', 'R3.3'], 'runtimeSourceSha256': checks['runtimeSourceSha256'],
        'checks': {'rustDefault': counts['rust-default']['count'], 'rustAllFeatures': counts['rust-all-features']['count'],
                   'newRustCases': counts['rust-default']['count']-prior['checks']['rustDefault'], 'focusedRustCases': counts['focused']['count'], 'node': counts['node']['count'], 'clippy': True, 'format': True,
                   'offlineIndependentDelivery': True, 'publicNodeMembers': [4, 8], 'browserMembers': [4, 8],
                   'legacy104': True, 'comparedSourceFiles': independent['sourceFilesCompared'], 'consumedSampleCopiesCompared':schema_report['consumedSampleCopiesCompared'],'schemaRuntimeFragments':schema_report['validatedRuntimeFragments'],'schemaConfigFragments':schema_report['validatedConfigFragments'],'invalidFollowupFragmentsRejected':schema_report['invalidFollowupFragmentsRejected'],'invalidEnhancementFragmentsRejected':schema_report['invalidEnhancementFragmentsRejected']},
        'castInterceptionRuns':{
            'spec':'docs/interfaces/cast-interception-v1.md','publicNode':independent['combat']['castInterceptionCases'],
            'publicH5':independent['browser']['castInterceptionCases'],'focusedRustCases':counts['cast-interception']['count'],
            'source':gate,'schemaFragments':{'config':18,'node':32,'h5States':24,'h5Events':8,'invalid':20}},
        'sourceExecutionRuns':{
            'routes':'src/verification/source_execution_routes.json','numericRows':50,'exactNative':27,
            'numericScenario':['S08'],'nativeCastScenarios':['S10','S11','S12'],
            'openExactScenarios':['S20','S28'],'focusedRustCases':5,'castScenarioRustCases':4},
        'terminalCounterRuns':{'spec':'docs/batches/SDK-5-88-action-terminal-counters.md',
            'source':terminal,'focusedRustCases':3,'nativeThreePulseContacts':3,
            'publicNodeTerminalEvents':8,'publicH5TerminalEvents':8},
        'paidResourceRuns':{'spec':'docs/interfaces/paid-resource-effects-v1.md','publicNode':independent['combat']['paidResourceCases'],'publicH5':independent['browser']['paidResourceCases'],'focusedRustCases':counts['paid-resources']['count'],'configFragments':schema_report['paidResourceConfigFragments'],'runtimeFragments':schema_report['paidResourceRuntimeFragments'],'browserStates':schema_report['paidResourceBrowserStates'],'browserEvents':schema_report['paidResourceBrowserEvents'],'invalidFragmentsRejected':schema_report['invalidPaidResourceFragmentsRejected'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/paid_resource_vectors.json').read_bytes()).hexdigest(),'fullPaymentBeforeReturn':True,'originalInitialCostOnly':True,'privatePerInstanceZeroPolicy':True,'exhaustionAfterCurrentReturn':True,'fullOriginalRealmTechniqueAccepted':False},
        'conditionalActivationRuns':{'spec':'docs/interfaces/conditional-activation-v1.md','publicNode':independent['combat']['conditionalActivationCases'],'publicH5':independent['browser']['conditionalActivationCases'],'focusedRustCases':counts['conditional-activation']['count'],'configFragments':schema_report['conditionalActivationConfigFragments'],'runtimeFragments':schema_report['conditionalActivationRuntimeFragments'],'browserStates':schema_report['conditionalActivationBrowserStates'],'browserEvents':schema_report['conditionalActivationBrowserEvents'],'invalidFragmentsRejected':schema_report['invalidConditionalActivationFragmentsRejected'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/conditional_activation_vectors.json').read_bytes()).hexdigest(),'nativeUnitCommit':True,'atomicPairAfterConsumptionPreview':True,'resourceBudgetBeforeConsumption':True,'durationOnlyRefresh':True,'originalHpPolicyAccepted':False,'originalStackModeAccepted':False,'fullOriginalStageRecipesAccepted':False},
        'stackWindowRuns':{'spec':'docs/interfaces/status-counters-v1.md','publicNode':independent['combat']['stackWindowCases'],'publicH5':independent['browser']['stackWindowCases'],'focusedRustCases':counts['stack-windows']['count'],'configFragments':schema_report['stackWindowConfigFragments'],'runtimeFragments':schema_report['stackWindowRuntimeFragments'],'browserStates':schema_report['stackWindowBrowserStates'],'browserEvents':schema_report['stackWindowBrowserEvents'],'invalidFragmentsRejected':schema_report['invalidStackWindowFragmentsRejected'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/stack_window_vectors.json').read_bytes()).hexdigest(),'oneIdentityAndWindow':True,'realZeroCounter':True,'fixedAndLayeredValues':True,'finiteCrossingLeaves':True,'fullOriginalStageBindingsAccepted':False},
        'sourceGateRuns':{'spec':'docs/interfaces/source-gates-v1.md','publicNode':independent['combat']['sourceGateCases'],'publicH5':independent['browser']['sourceGateCases'],'focusedRustCases':counts['source-gates']['count'],'configFragments':schema_report['sourceGateConfigFragments'],'runtimeFragments':schema_report['sourceGateRuntimeFragments'],'browserStates':schema_report['sourceGateBrowserStates'],'browserEvents':schema_report['sourceGateBrowserEvents'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/source_gate_vectors.json').read_bytes()).hexdigest(),'privateActionClassification':True,'liveProviderQualification':True,'boundedAttemptsPerAction':True,'originalRawProficiencyDerivationAccepted':False,'fullOriginalStageBindingsAccepted':False},
        'derivedEffectsRuns':{'spec':'docs/interfaces/status-ends-v1.md','publicNode':independent['combat']['derivedEffectsCases'],'publicH5':independent['browser']['derivedEffectsCases'],'focusedRustCases':counts['derived-effects']['count'],'configFragments':schema_report['derivedConfigFragments'],'runtimeFragments':schema_report['derivedRuntimeFragments'],'browserStates':schema_report['derivedBrowserStates'],'browserEvents':schema_report['derivedBrowserEvents'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/source_derived_vectors.json').read_bytes()).hexdigest(),'privatePersistentLineage':True,'boundedSharedEnds':True,'nativeAbsoluteDeadline':True,'fullOriginalStageBindingsAccepted':False},
        'characterPowerRuns':{'spec':'docs/interfaces/character-build-v1.md','publicNode':independent['combat']['characterPowerCases'],'publicH5':independent['browser']['characterPowerCases'],'focusedRustCases':counts['character-build']['count'],'configFragments':schema_report['characterPowerConfigFragments'],'runtimeFragments':schema_report['characterPowerRuntimeFragments'],'browserStates':schema_report['characterPowerBrowserStates'],'browserEvents':schema_report['characterPowerBrowserEvents'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/source_character_power_vectors.json').read_bytes()).hexdigest(),'trustedRawBuilds':True,'convertedResourcesBeforeAdmission':True,'fullOriginalEquipmentAndGrowthAccepted':False},
        'resourceStealRuns':{'spec':'docs/interfaces/resource-steal-v1.md','publicNode':independent['combat']['resourceStealCases'],'publicH5':independent['browser']['resourceStealCases'],'focusedRustCases':counts['resource-steal']['count'],'configFragments':schema_report['resourceStealConfigFragments'],'runtimeFragments':schema_report['resourceStealRuntimeFragments'],'browserStates':schema_report['resourceStealBrowserStates'],'browserEvents':schema_report['resourceStealBrowserEvents'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/source_resource_steal_vectors.json').read_bytes()).hexdigest(),'originalHpPolicy':'awaiting_user_rule','originalMappingAccepted':False,'fullSourceSkillEffectsAccepted':False},
        'reservationRuns':{'spec':'docs/interfaces/resource-commit-v1.md','publicNode':independent['combat']['reservationCases'],'publicH5':independent['browser']['reservationCases'],'focusedRustCases':counts['resource-reservations']['count'],'configFragments':schema_report['reservationConfigFragments'],'runtimeFragments':schema_report['reservationRuntimeFragments'],'browserStates':schema_report['reservationBrowserStates'],'browserEvents':schema_report['reservationBrowserEvents'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/source_resource_reservation_vectors.json').read_bytes()).hexdigest(),'sourcePolicies':{'accept':58,'none':20,'missing':8},'fullSourceSkillEffectsAccepted':False},
        'unitEffectRuns':{'spec':'docs/interfaces/unit-effects-v1.md','publicNode':independent['combat']['unitEffectCases'],'publicH5':independent['browser']['unitEffectCases'],'focusedRustCases':counts['unit-effects']['count'],'configFragments':schema_report['unitEffectConfigFragments'],'runtimeFragments':schema_report['unitEffectRuntimeFragments'],'browserStates':schema_report['unitEffectBrowserStates'],'browserEvents':schema_report['unitEffectBrowserEvents'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/source_unit_channel_vectors.json').read_bytes()).hexdigest(),'fullOriginalResourceStealAccepted':False},
        'skillLoadoutRuns':{'spec':'docs/interfaces/skill-loadouts-v1.md','publicNode':independent['combat']['skillLoadoutCases'],'publicH5':independent['browser']['skillLoadoutCases'],'focusedRustCases':counts['skill-loadouts']['count'],'configFragments':schema_report['skillLoadoutConfigFragments'],'runtimeFragments':schema_report['skillLoadoutRuntimeFragments'],'browserStates':schema_report['skillLoadoutBrowserStates'],'auditsReplayed':schema_report['skillLoadoutAuditsReplayed'],'trustedOnly':True,'processRecovery':'financial_close_and_compensation','clientReconnect':'preserve_world_skills_cooldowns_and_plans','fullOriginalDynamicStagesAccepted':False},
        'periodicCompositionRuns':{'spec':'docs/batches/SDK-5-88-combat-and-casting.md','publicNode':independent['combat']['periodicCompositionCases'],'publicH5':independent['browser']['periodicCompositionCases'],'configFragments':schema_report['periodicCompositionConfigFragments'],'runtimeFragments':schema_report['periodicCompositionRuntimeFragments'],'browserStates':schema_report['periodicCompositionBrowserStates'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/source_composed_status_vectors.json').read_bytes()).hexdigest(),'decision':composed_fixture['decision'],'fullSourceContentAccepted':False},
        'castLifecycleRuns': {'spec':'docs/interfaces/cast-lifecycle-v1.md','publicNode':independent['combat']['castLifecycleCases'],'publicH5':independent['browser']['castLifecycleCases'],'sourceProjectionSha256':hashlib.sha256((ROOT/'src/verification/source_cast_lifecycle_vectors.json').read_bytes()).hexdigest(),'sourceProjections':['tiandi-quan:charge_clocks_and_power_curve','realm-small-cycle:five_spirit_budget_shares','duduan-wangu:spirit_cost_bound'],'schemaConfigFragments':schema_report['castingConfigFragments'],'schemaRuntimeFragments':schema_report['castingRuntimeFragments'],'nativeHighCosts':schema_report['castingNativeCosts'],'fullOriginalSkillEffectsAccepted':False},
        'sourceNumericVectorRuns': {'fixtureSha256': hashlib.sha256(fixture.read_bytes()).hexdigest(),
                                   'executedIds': vector_ids, 'fixtureSkippedCount': 50-len(vector_ids), 'wholeOriginalVectorSuiteAccepted': False},
        'sourceStatusMappingRuns': {'decision': 'D-03', 'executedIds': [r['id'] for r in source_statuses],
                                   'windQualityVariants': 4, 'allSourcePlacementsAccepted': False,
                                   'note': 'Three slow durations use an explicit 300ms harness input; this verifies the resolved attribute semantics, not original missing placement durations.'},
        'attackSpeedMappingRuns': {'decision':'OBS-05','id':'effect_zhenlong_suppress','interval1000At70Percent':1429,'partial300msAt70Percent':1090,'allDragonMightCombinationsAccepted':False},
        'recordShieldRuns': {'sourceGuardScalarVectors':3,'fullOriginalGuardSkillsAccepted':False,'publicRecordCaps':[150,180],'publicRawCounters':[90,108],'statusConsumptionsPerProbe':1,'derivedRecursion':False},
        'statusDispositionRuns': {'transferPreservesInstanceSourceLayersExpiry':True,'dispelDoesNotReleaseShield':True,'oncePerEntityDeathGuard':True,'publicReconnectDoesNotReset':True,'fullSourceSkillSetAccepted':False},
        'actionAggregationRuns': {'counterScopes':['owner','target'],'safeIntegerTotals':True,'nativeTermination':True,'sourceGenerationIdentity':True,'publicContactsPerAction':6,'publicActionsPerCase':2,'fullOriginalWeaponFamiliesAccepted':False},
        'windowRecoveryRuns': {'bases':['none','damage_dealt','damage_taken','shield_absorbed'],'safeIntegerRecords':True,'snapshotAndLiveMaxima':True,'refreshModes':['reset','preserve','ignore'],'onlyEnabledEndsRecoverOnce':True,'sourceAndHolderDistinct':True,'hotUsesOriginalSchedule':True,'publicContacts':12,'publicRecorded':[2000,2592],'publicRawHp':[200,180],'publicRawSpirit':[40,25],'allOriginalCultivationSkillsAccepted':False},
        'compoundFlagRuns': {'flags':['silence','root','stun','control_immunity','damage_immunity','first_control_immunity'],'singleIdentityAndLifetime':True,'wholeInstanceImmunityAndConsumption':True,'nativeMemberAndNpcGates':True,'sameIdentityRefreshAfterReconnect':True,'windowBindingRefused':True,'transferUsesExplicitPolicy':True,'fullOriginalControlSkillsAccepted':False},
        'cleanseLifecycleRuns': {'nativeHotAndExpiry':True,'selectionScopes':['all','since_application'],'wholeInstancesAndExplicitPolicies':True,'emptyOperationsSpendBudget':True,'endBeforeRecovery':True,'noDerivedRelease':True,'privateCountOnly':True,'publicRemovedPerPhase':[1,0,0,1],'publicAllySpiritRestored':1,'allOriginalTimeCurrentAndSealSkillsAccepted':False},
        'periodicRecoveryRuns': {'nativeHotLifetime':True,'samplingModes':['source_snapshot','target_snapshot','source_live','target_live'],'conversionWhen':['full_before_tick','any_excess'],'singleExcessBudget':True,'baseBeforeConversionBeforeShield':True,'lockAndLiveCaps':True,'publicFirstBase':[1,2],'publicFirstConverted':[3,2],'publicFirstSpent':[6,8],'publicShieldGrants':[[30,33,33],[28,32,32]],'allOriginalTimeCurrentPlacementsAccepted':False},
        'overhealRecordRuns': {'grantSources':['status_binding','instant_overheal','hot_overheal','item_overheal'],'positiveRefillsOnly':True,'sameNativeIdentity':True,'sourcePowerPreflight':True,'resetAndPreserve':True,'atomicItemFailure':True,'publicBalances':[96,92],'publicRecorded':[40,50],'publicRawCounters':[20,30],'fullOriginalGuardSkillsAccepted':False},
        'shieldTransferRuns': {'nativeShieldAndRecord':True,'overhealCreated':True,'preserveIdentitySourceBalanceRecordExpiry':True,'finalPreviewPool':True,'noPartialTransfer':True,'publicGrant':200,'publicAbsorbed':1,'publicRemaining':199,'publicRecordCap':180,'allOriginalTransferPlacementsAccepted':False},
        'periodicTransferRuns': {'nativeDotAndHot':True,'sourceAndSchedulePreserved':True,'sampling':['source_snapshot','target_snapshot','source_live','target_live'],'dotRelation':'hostile_to_original_source','hotPermission':'explicit_transfer_policy','oldGeneratedShieldStays':True,'publicTicks':[9,7],'privateRecipients':1,'allOriginalPeriodicPlacementsAccepted':False},
        'controlTransferRuns': {'scalarAndCompound':True,'nativeConsequencesShared':True,'originalSourceAndDeadline':True,'finalPreviewImmunity':True,'immunityRejectionDoesNotConsume':True,'firstImmunityConsumedByLaterNativeControl':True,'publicKinds':['scalar','compound'],'publicDurationMs':1500,'newPveFriendlyActionEntry':True,'allOriginalControlSkillsAccepted':False},
        'allyCommitRuns': {'fact':'ally_skill_committed','mechanisms':['ally_heal','ally_shield'],'primaryThenTrigger':True,'sharedAtomicConditions':True,'publicNpcControlTransfer':True,'publicNode':independent['combat']['allySupportCases'],'publicH5':independent['browser']['allySupportCases'],'fullOriginalControlSkillsAccepted':False},
        'lifecycleTransferRuns': {'nativeWindowAndCleanse':True,'preservedHistoryMaximumAndSchedule':True,'sinceApplication':'original_applied_at_ms','currentHolderOnly':True,'cleanseBeforeWindowRecovery':True,'noEndOnTransfer':True,'publicNode':independent['combat']['lifecycleCases'],'publicH5':independent['browser']['lifecycleCases'],'fullOriginalPlacementsAccepted':False},
        'controlPolicyRuns': {'targetProfileResolution':True,'ordinaryCapMs':1800,'originalDurationPreserved':True,'oneCompoundInstance':True,'movementSlowBps':-5000,'basicIntervalDelayBps':1500,'intervalDefinition':'basic_attack_interval_length_increase','interval1000Ms':1150,'withDoubleAttackSpeedMs':575,'fractionCreditPreserved':True,'transferPolicyRechecked':True,'publicNode':independent['combat']['controlPolicyCases'],'publicH5':independent['browser']['controlPolicyCases'],'fullOriginalColdStackShatterPlacementsAccepted':False},
        'hitStatusRuns': {'fact':'direct_hit','afterNativeBindings':True,'zeroHpAndFullAbsorption':True,'effect':'apply_other_status','atomicFinalPreview':True,'counterSourceAttribution':True,'slowStackMaximum':32,'stackConsumptionAndStrongestSource':True,'sourceCleanupAndTransfer':True,'publicNode':independent['combat']['hitStatusCases'],'publicH5':independent['browser']['hitStatusCases'],'fullOriginalShatterChainAndPlacementsAccepted':False},
        'contactModifierRuns': {'configuration':'AttackRule.contact_status','source':'attacker_owned','subjects':['owner','other'],'sample':'native_contact_after_dodge_before_damage','partialConsumptionPreservesExpiry':True,'rawRounding':'u128_once_then_u32','configuredPenetrationCap':True,'inFlightSamplingAndContention':True,'periodicAndDerivedExcluded':True,'sourceCoefficientVectors':3,'sourceFixtureSha256':hashlib.sha256((ROOT/'src/verification/source_contact_vectors.json').read_bytes()).hexdigest(),'publicNode':independent['combat']['contactModifierCases'],'publicH5':independent['browser']['contactModifierCases'],'fullOriginalSwordPursuitCooldownAndShatterAccepted':False},
        'chainDamageRuns': {'effect':'chain_damage','maximumHops':8,'globalAttemptBudget':[1,128],'sameNativeTime':True,'sourceSample':'post_consumption_once','selection':'nearest_unvisited_hostile_with_line_of_sight','stableIdentityOrdering':True,'decay':'per_attempt','finalBonus':'configured_ordinal_only','immunitySpendsAttempt':True,'nativeDamageAndControlPolicies':True,'noDerivedRecordingOrTriggers':True,'noDelayedReplay':True,'sourceCoefficientVectors':1,'sourceFixtureSha256':hashlib.sha256((ROOT/'src/verification/source_chain_vectors.json').read_bytes()).hexdigest(),'publicNode':independent['combat']['chainDamageCases'],'publicH5':independent['browser']['chainDamageCases'],'fullOriginalChainSwordAndShatterPlacementsAccepted':False},
        'postConsumptionRuns': {'fact':'contact_status_consumed','filter':'actual_definition_and_consumed_count','amount':'enhanced_pre_defense_raw','afterDirectHit':True,'forwardSelection':'positive_dot_product_per_segment','cooldownEffect':'reduce_skill_cooldowns','currentRemainingRatioAndFixed':True,'independentSiblingEffects':True,'ownerOnlySkillDeadlines':True,'noDelayedReplay':True,'sourceCoefficientVectors':1,'sourceFixtureSha256':hashlib.sha256((ROOT/'src/verification/source_post_consumption_vectors.json').read_bytes()).hexdigest(),'publicNode':independent['combat']['postConsumptionCases'],'publicH5':independent['browser']['postConsumptionCases'],'fullOriginalSwordStagesAndFlightDirectionAccepted':False},
        'contactReactionRuns': {'configuration':'combat_rules.contact_reactions','when':['direct_contact','hp_damage'],'attacker':['source_only','any_hostile'],'sampling':'original_source_power_live','captureBeforePrimary':True,'executionBeforeHitBindings':True,'originalSourceDamageOwnership':True,'consumeWholeInstanceOnce':True,'boundedSharedBudgets':True,'nativeDefenseShieldGuardPipeline':True,'transferPreservesSourceAndDeadline':True,'derivedTriggersAndRecords':False,'sourceCoefficientVectors':1,'sourceFixtureSha256':hashlib.sha256((ROOT/'src/verification/source_reaction_vectors.json').read_bytes()).hexdigest(),'publicNode':independent['combat']['contactReactionCases'],'publicH5':independent['browser']['contactReactionCases'],'fullOriginalShatterPowerSamplingAndPlacementsAccepted':False},
        'finisherRuns': {'effect':'finish_other','comparison':['below','at_most'],'policySelector':'target_combat_profile','modes':['execute','power_damage'],'executeBudget':'current_hp_plus_unexpired_shields','powerCap':'post_consumption_effective_source_power','atomicPreview':True,'nativeDeathGuard':True,'nativeKillCredit':True,'ordinaryDerivedTriggersAndRecords':False,'explicitSuccessResult':'finisher_kill','npcResultPrivacy':'target_member_only','sourceCoefficientVectors':1,'sourceFixtureSha256':hashlib.sha256((ROOT/'src/verification/source_finisher_vectors.json').read_bytes()).hexdigest(),'publicNode':independent['combat']['finisherCases'],'publicH5':independent['browser']['finisherCases'],'fullOriginalFinisherStagesRewardsAndPlacementsAccepted':False},
        'finisherFollowupRuns': {'fact':'finisher_kill','selector':'provider_trigger_id_and_explicit_mode','modes':['execute','power_damage','any'],'stage':'synchronous_after_all_primary_rules','localResultsOnly':True,'amount':'actual_finisher_hp_damage_excluding_shield','preservedActionCreator':True,'leaves':['heal_from_damage','restore_spirit','apply_self_status','reduce_skill_cooldowns','chain_damage'],'sharedTriggerAndChainBudgets':True,'independentLeafFailures':True,'noDerivedRecursionOrReplay':True,'ratioSpirit':'fixed_plus_floor_current_effective_max_spirit','publicNode':independent['combat']['finisherFollowupCases'],'publicH5':independent['browser']['finisherFollowupCases'],'nativeFollowupTimings':schema_report['nativeFollowupTimings'],'publicChainBasis':'finisher_hp_damage','sourceCoefficientChainBasis':'explicit_attack_harness_input_only','sourceFixtureSha256':hashlib.sha256((ROOT/'src/verification/source_finisher_followup_vectors.json').read_bytes()).hexdigest(),'fullOriginalStagesPowerBasisAndPlacementsAccepted':False},
        'actionEnhancementRuns': {'sampling':['contact','action'],'sourceScope':'owner','snapshot':'first_eligible_contact_including_zero','key':'source_kind_id_generation_action_and_definition','contactConsumptionFacts':'actual_first_consumption_only','capturedDamageAndPenetration':True,'statusChangesDoNotResample':True,'snapshotBudget':[1,1024],'overflowProtection':'until_all_active_native_actions_drained','unknownClassificationCannotReward':True,'thresholdBonus':'consumed_status_bonus.hp_threshold_bps_per_stack','resultSelector':'finisher.enhanced','sameActionCannotSelfRefresh':True,'inFlightReconnectPreservesSnapshot':True,'sourceFixtureSha256':hashlib.sha256((ROOT/'src/verification/source_action_enhancement_vectors.json').read_bytes()).hexdigest(),'publicNode':independent['combat']['actionEnhancementCases'],'publicH5':independent['browser']['actionEnhancementCases'],'fullOriginalStagesAttributesPowerAndPlacementsAccepted':False},
        'extendedScope': ['action-scoped first-contact consumption snapshots including zero','consumed-state finisher threshold bonus and enhanced result selection','bounded snapshot admission protection until native actions drain','explicit finisher success facts and bounded synchronous result leaves','fixed plus effective maximum-spirit trigger recovery','threshold finish attempts and profile-specific power-capped damage','existing-state direct contact reactions with original-source damage and bounded budgets','typed native contact consumption and forward pursuit','bounded current skill cooldown returns','bounded native chain damage and attached harmful statuses','source-owned status consumption modifying current contact damage and penetration','optional attack-level penetration aggregation cap','native contact facts and conditional other-status grants','source-owned mark thresholds and layered movement slow','target-profile control cap and complete numeric replacement','basic interval delay and compound movement slow','native control and immunity transfer','native DOT/HOT transfer','native shield and record transfer','recovery excess and native recorded shield composition','native HOT spirit recovery and excess budget partitioning','bounded native HOT/expiry dispels','compound control/immunity flags and numeric status lifecycle','status lifetime windows and once-only HP/spirit end recovery','per-target counters and action-total completion rewards','typed dispel, numeric transfer and disposition blocking','once-per-entity pre-HP death guard','single-instance shield recording and end release','typed conditions and atomic status consumption','shared Node/H5 recorded-shield decoder', 'live ordinary attack-speed work clocks', 'temporary max_hp/max_spirit/spd', 'attribute/probability/critical caps',
                          'harmful/control duration factors', 'source/target snapshot/live DOT/HOT',
                          'status-only items and compound quality overrides', 'per-action field/channel trigger counting'],
        'publicCases': independent['combat']['cases'], 'browserCases': independent['browser']['cases'],
        'archive': package['archive'], 'archiveSha256': package['sha256'], 'binarySha256': package['binarySha256'],
        'verificationDriversSha256': {str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [*[ROOT/'scripts'/s for s in ['verify_sdk_5_88_interception.mjs','verify_sdk_5_88_interception_browser.py','verify_sdk_5_88_interception_schema.py','verify_sdk_5_88_paid.mjs','verify_sdk_5_88_paid_browser.py','verify_sdk_5_88_paid_schema.py','verify_sdk_5_88_activation.mjs','verify_sdk_5_88_activation_browser.py','verify_sdk_5_88_activation_schema.py','verify_sdk_5_88_counter.mjs','verify_sdk_5_88_counter_browser.py','verify_sdk_5_88_counter_schema.py','verify_sdk_5_88_gates.mjs','verify_sdk_5_88_gates_browser.py','verify_sdk_5_88_gates_schema.py','verify_sdk_5_88_derived.mjs','verify_sdk_5_88_derived_browser.py','verify_sdk_5_88_derived_schema.py','verify_sdk_5_88_checks.py','verify_sdk_5_87_public.mjs','verify_sdk_5_88_public.mjs','verify_sdk_5_88_browser.py','verify_sdk_5_45_browser.py','verify_sdk_5_88_delivery.py','verify_sdk_5_88_record_schema.py','verify_sdk_5_88_ally.mjs','verify_sdk_5_88_ally_browser.py','verify_sdk_5_88_unit.mjs','verify_sdk_5_88_unit_browser.py','verify_sdk_5_88_reservation.mjs','verify_sdk_5_88_reservation_browser.py','verify_sdk_5_88_steal.mjs','verify_sdk_5_88_steal_browser.py','verify_sdk_5_88_browser_actions.py','verify_sdk_5_88_power.mjs','verify_sdk_5_88_power_browser.py']],ROOT/'adapters/node/replication.test.mjs']},
        'verificationHarness': {'h5DatabaseIsolation':'per_invocation','demoShieldLifetimesMs':{'record-guard':2000,'overflow-shield':8000},'timingObservation':str(timing) if timing.is_file() else None,'nativeExpirationChanged':False},
        'priorEvidence': {'previousCheckpointAvailable':True,'previousCheckpointManifest':str(prior_manifest),
                          'previousFilesChecked':prior_count,'previousManifestSha256':digest(prior_manifest.read_bytes()),
                          'earlierCleanupBaseline':'docs/evidence/SDK-5-88-cleanup-baseline.json','historicalFilesRecreated':False},
        'coverage': 'docs/batches/SDK-5-88-coverage.json',
        'remainingScope': 'N04–N08 remaining fields remain open; no R3.1–R3.3 or whole-task acceptance is implied.',
    }
    checkpoint = OUT / 'checkpoints/core-39'
    assert not checkpoint.exists(), 'immutable checkpoint already exists'
    write(OUT / 'acceptance.json', acceptance)
    checkpoint.mkdir(parents=True)
    for source in [OUT / 'acceptance.json', OUT / 'extended-checks.json', OUT/'record-schema-check.json', *([timing] if timing.is_file() else []), OUT / 'delivery/independent-verification.json', OUT / 'delivery/package.json', Path(package['archive'])]:
        shutil.copy2(source, checkpoint / source.name)
    coverage_path = ROOT / 'docs/batches/SDK-5-88-coverage.json'
    coverage = read(coverage_path)
    coverage['acceptanceStatus'].update({'GC05': 'candidate_verified_declared_sampling', 'GC06': 'partial_declared_forms_verified',
                                       'GC07': 'partial_declared_forms_verified', 'GC09': 'candidate_verified', 'GC10': 'candidate_verified'})
    coverage['attackSpeedAcceptance'].update({'AS07':'candidate_verified','AS08':'candidate_verified'})
    coverage['recordConsumptionAcceptance'].update({f'RC{i:02}':'candidate_verified_declared_forms' for i in range(1,10)})
    coverage['statusDispositionAcceptance'].update({f'SD{i:02}':'candidate_verified_declared_forms' for i in range(1,10)})
    coverage['actionAggregationAcceptance'].update({f'AT{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['windowRecoveryAcceptance'].update({f'WR{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['compoundFlagAcceptance'].update({f'CF{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['cleanseLifecycleAcceptance'].update({f'LC{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['periodicRecoveryAcceptance'].update({f'PR{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['overhealRecordAcceptance'].update({f'OR{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['shieldTransferAcceptance'].update({f'ST{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['periodicTransferAcceptance'].update({f'PT{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['controlTransferAcceptance'].update({f'CT{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['allyCommitAcceptance'].update({f'AC{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['lifecycleTransferAcceptance'].update({f'LT{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['controlPolicyAcceptance'].update({f'CR{i:02}':'candidate_verified_declared_forms' for i in range(1,11)})
    coverage['hitStatusAcceptance'].update({f'HC{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['contactModifierAcceptance'].update({f'MC{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['chainDamageAcceptance'].update({f'CH{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['postConsumptionAcceptance'].update({f'PC{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['contactReactionAcceptance'].update({f'SR{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['finisherAcceptance'].update({f'FN{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['finisherFollowupAcceptance'].update({f'FK{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['actionEnhancementAcceptance'].update({f'AE{i:02}':'candidate_verified_declared_forms' for i in range(1,15)})
    coverage['castLifecycleAcceptance']={key:'candidate_verified_declared_forms' for key in ['phase_movement','early_release_queue','release_windup','absolute_pulse_count','channel_recovery','expanded_recovery','resource_limits','mobile_windup_gate']}
    additions={
        'N04':'显式accept/none预留、接受时HP费用冻结、提交HP/SP/冷却原子重验、未提交终态释放、排队提升才预留、私有动作与费用防篡改；严格单位即时效果、敌友引导总预算/末跳余量/每跳重验、第N合法跳完整状态、移动控制后执行、重连和目标退场停止；可信成员技能归属与0～8槽CAS热换、撤销未提交计划与未来引导、保留债务/已发弹道、私有版本与财务恢复审计；准备/引导有效新移动打断和自由/减速引导、松手排队/独立前摇及固定步取整；明确跳数按绝对期限调度，全技能后摇权限和原子接管；显式资源与长移动前摇上限，8m费用及1b基础资源真实准入/扣费，原源阶段/恢复预算局部绑定',
        'N05':'contact_status显式sampling=action只消费一次，后续目标/重复命中复用层数，新授予/刷新/搬移不改旧快照；有界缓存与排空保护不误消费或补发',
        'N06':'DirectHit携带冻结消费上下文，实际消费事实仍只在首次支出时发一次；finisher.enhanced筛选实际成功结果，按动作授予下一次增益且增强多杀不自刷新',
        'N08':'自身恢复引导在提交时冻结HP/SP总预算、最早跳余数分配，实时受疗/回灵锁/缺口；原生逐跳费用、终态/重连清理及公开Node/H5，原每秒/5秒/5跳灵力原量绑定'
    }
    for row in coverage['families']:
        value=additions.get(row['sourceCapability'])
        if value and value not in row['implemented']:row['implemented'].append(value)
        if row['sourceCapability']=='N04':row['remaining']=[x.replace('OBS-03的原8m成本与工程profile、','OBS-03其余工程预算、') for x in row['remaining']]
        if row['sourceCapability']=='N07':row['remaining']=[x.replace('原终结完整功法威力/专用阶段与强化消费、完整源绑定','原终结完整功法威力/专用阶段逐项绑定、完整源绑定') for x in row['remaining']]
    for row in coverage['families']:
        if row['sourceCapability'] in ['N04','N05','N08']:
            value='当前资源冻结总预算与即时/引导/命中共用夺取、独立转移与伤害链显式策略、真实扣除与返还及余量、BasicHit与有界触发、来源/目标退场和反噬终止、life_steal实时叠层与最终上限；原HP策略和完整源功法阶段待绑定'
            if value not in row['implemented']:row['implemented'].append(value)
    for row in coverage['families']:
        if row['sourceCapability'] in ['N04','N05','N07','N08']:
            value='core-32: 可信入场原始成长索引/点数、常数转换与转换后准入边界、每技能本人成长威力和一次乘区、盾记录/接触反应/终结/链/HOT来源选择、快照/live、私有槽位有效值与恢复身份；原完整装备养成仍归R5.1'
            if value not in row['implemented']:row['implemented'].append(value)
    coverage['periodicCompositionAcceptance'].update({f'PB{i:02}':'candidate_verified_declared_forms' for i in range(1,15)})
    coverage['skillLoadoutAcceptance'].update({f'LS{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['unitEffectAcceptance'].update({f'UT{i:02}':'candidate_verified_declared_forms' for i in range(1,15)})
    coverage['reservationAcceptance'].update({f'RV{i:02}':'candidate_verified_declared_forms' for i in range(1,13)})
    coverage['resourceStealAcceptance']={f'RS{i:02}':('source_projection_verified_hp_policy_pending'if i==15 else'candidate_verified_declared_forms')for i in range(1,17)}
    coverage['attributePowerAcceptance']={f'AP{i:02}':('source_contract_verified_full_growth_open'if i==15 else'candidate_verified_declared_forms')for i in range(1,17)}
    coverage['derivedEndAcceptance']={f'DE{i:02}':('source_contract_verified_original_attachments_open'if i==19 else'candidate_verified_declared_forms')for i in range(1,21)}
    for row in coverage['families']:
        if row['sourceCapability']in ['N05','N06','N08']:
            value='core-33: 按真实实例结束原因投放有界DOT/HOT/非记录盾叶子、记录盾结束及反击命中叶子、私有派生分类跨Tick/转移保持与递归阻断、共享步预算/有界待处理量、单跳绝对期限的实时缺失HP公式和来源snapshot/live及profile上限、层数消费原子准入；原完整阶段投放仍开放'
            if value not in row['implemented']:row['implemented'].append(value)
    for row in coverage['families']:
        if row['sourceCapability']=='N08':row['remaining']=[x.replace('记录盾结束/命中再创建周期或护盾的派生链、完整高阶反击策略及未执行的原内容投放组合','原记录盾结束/反击叶子的完整高阶策略与尚未执行的原内容投放组合')for x in row['remaining']]
    coverage['sourceGateAcceptance']={f'SG{i:02}':('source_contract_verified_full_stage_bindings_open'if i in [17,20]else'candidate_verified_declared_forms')for i in range(1,21)}
    for row in coverage['families']:
        if row['sourceCapability']in ['N04','N05','N06','N07']:
            value='core-34: 原生普攻/技能来源随弹道/区域/引导/周期状态/汇总保持，any/only/except封闭筛选；真实归属者当前装备候选与规范可信mastery行门槛，第三方接触反应检查原来源；按动作owner/target共享1～8次尝试配额、失败不吞层数/ICD；原熟练度到阶段派生与完整投放开放'
            if value not in row['implemented']:row['implemented'].append(value)
    coverage['stackWindowAcceptance']={f'SQ{i:02}':('source_contract_verified_full_stage_bindings_open'if i in [18,20]else'candidate_verified_declared_forms')for i in range(1,21)}
    for row in coverage['families']:
        if row['sourceCapability']in ['N05','N06','N08']:
            value='core-35: 固定与按层效果同实例聚合、显式0层计数器、本人来源既有计数调整/期限策略、实际跨越门槛的有限叶子与共享预算、消费至0保留固定/窗口/截止、原道种15%吸血/20%减伤/每层3%/满层净化/12或15秒/伤害10%与最大HP20%上限及四八人重连退出；原完整阶段投放开放'
            if value not in row['implemented']:row['implemented'].append(value)
    coverage['conditionalActivationAcceptance']={f'CB{i:02}':('source_contract_verified_original_mapping_pending'if i==19 else'source_contract_verified_full_stage_recipes_open'if i in [24,29]else'candidate_verified_declared_forms')for i in range(1,31)}
    for row in coverage['families']:
        if row['sourceCapability']in ['N04','N05','N06','N08']:
            value='core-36: 可信阶段闭区间、严格单位提交事实、消费后双方状态组合原子预检、消费前按层资源倍率冻结与两种显式乘区、既有数值状态只续期不增层；四八人真实击杀续期/重连/退出与源参数行绑定；原HP策略/乘区裁定和完整成长投放开放'
            if value not in row['implemented']:row['implemented'].append(value)
    coverage['paidResourceAcceptance']={f'PA{i:02}':('source_contract_verified_full_realm_technique_open'if i==16 else'candidate_verified_declared_forms')for i in range(1,21)}
    for row in coverage['families']:
        if row['sourceCapability']in ['N04','N05','N06','N08']:
            value='core-37: 初次技能已付成本原生事实、先付全额再按实际灵力比例返还、私有原动作金额与余量、显式计数器耗尽移除/消费后槽位预览、末次返还后Consumed有限叶子封锁、三次/30%/8秒/3秒原值与四八人重连及恢复通过；原完整普通功法清单与境界技投放开放'
            if value not in row['implemented']:row['implemented'].append(value)
    coverage['candidate']['crate']='0.102.0'
    coverage['castInterceptionAcceptance']={f'CI{i:02}':(
        'source_values_verified_full_recipe_open' if i==16 else
        'remaining_source_work_open' if i==20 else 'candidate_verified_declared_forms')for i in range(1,21)}
    coverage['sourceExecutionAcceptance']={f'VE{i:02}':(
        'exact_scenario_open' if i==11 else 'candidate_verified_declared_forms')for i in range(1,15)}
    coverage['resourceStealStackModeDecision'].update(status='verified_source_mode',originalMappingAccepted=True)
    coverage['castInterceptionDecision'].update(status='verified_declared_source_rules',
        originalMappingAccepted=True,fullOriginalRealmTechniqueAccepted=False)
    coverage['conditionalActivationAcceptance']['CB19']='source_multiplier_verified_original_hp_policy_pending'
    for row in coverage['families']:
        if row['sourceCapability'] in ['N04','N05','N06','N08']:
            value='core-38: 原生最终提交拦截、状态封禁与免控、成员/NPC配置分支、来源/代次/缩时/退场；吞元原乘区10/11/12/13%；50项来源算术逐条登记，27项精确运行，原S08已运行'
            if value not in row['implemented']:row['implemented'].append(value)
    coverage['workingScope']={'status':'continuing_same_module','verified':'actual initial skill cost returns after full payment, remove-at-zero owned carriers and finite exhaustion ends; strict unit activation with atomic paired statuses and pre-consumption resource bonus; clock-only owned numeric status renewal; explicit zero counter, fixed plus layered contributions, finite threshold leaves and same-instance windows; native action classification, live provider qualification and bounded per-action attempts; finite native status-end leaves and persistent derived lineage; single delayed missing-HP deadline; trusted raw growth and true-point conversion with private slot powers; selected native skill sources for damage/healing/records/reactions/finishers/chains/periodic sampling; resource transfer/reservation','remaining':'original resource-steal HP policy decision, complete qualified stage recipes, raw proficiency derivation and applicable original bindings'}
    coverage['workingScope']['verified'] += '; final cast interception with native immunity/silence, confirmed relative resource multiplier, exact source numeric routes, source S10/S11/S12 and native action terminal owned counters'
    for row in coverage['families']:
        if row['sourceCapability']=='N04':
            row['remaining']=[x.replace('动态技能变更及尚未支持的施法变体','尚未支持的施法变体') for x in row['remaining']]
        if row['sourceCapability']=='N06':
            row['remaining']=[x.replace('动作汇总的目标依赖效果；','原目标依赖终态效果未找到可绑定行；') for x in row['remaining']]
        if row['sourceCapability'] in ('N04','N05','N06'):
            value='core-39: 原S10/S11/S12、零前摇自身显式同刻提交和真实协议重送、已完成直接伤害汇总给本人既有同源计数器加层；八人Node/H5终态事件与原计数来源行'
            if value not in row['implemented']:row['implemented'].append(value)
    coverage['workingScope']['remaining'] += '; exact drop placement and absolute terminal scenarios (S20/S28) and unexecuted cross-package vectors'
    coverage['candidateEvidence'] = str(OUT/'acceptance.json')
    coverage['lastVerifiedCheckpoint'] = str(checkpoint/'acceptance.json')
    write(coverage_path, coverage)
    roadmap_path = ROOT / 'docs/plans/sdk-delivery-roadmap.json'
    roadmap = read(roadmap_path)
    roadmap['runtimeCandidate'].update({'crate':'0.102.0','status': 'verified_declared_scope', 'wholeTaskAccepted': False,
                                      'lastVerifiedCheckpoint': coverage['lastVerifiedCheckpoint']})
    roadmap['runtimeCandidate'].pop('pendingDecisionApplied', None)
    roadmap['runtimeCandidate']['appliedDecisions']=['docs/decisions/SDK-5-88-movement-status-mapping.md','docs/decisions/SDK-5-88-attack-speed-mapping.md','docs/decisions/SDK-5-88-extra-movement-status-mapping.md']
    roadmap['runtimeCandidate']['runtimeSourceSha256']=checks['runtimeSourceSha256']
    roadmap['runtimeCandidate']['lastVerifiedCrate']='0.102.0'
    roadmap['runtimeCandidate']['lastVerifiedRuntimeSourceSha256']=checks['runtimeSourceSha256']
    roadmap['runtimeCandidate']['pendingMappings']=['docs/decisions/SDK-5-88-resource-steal-policy.md']
    roadmap['runtimeCandidate'].pop('confirmedPendingImplementation',None)
    roadmap['runtimeCandidate']['appliedDecisions'] += ['docs/decisions/SDK-5-88-resource-steal-stack-mode.md','docs/decisions/SDK-5-88-cast-interception-policy.md']
    write(roadmap_path, roadmap)
    shutil.copy2(coverage_path, checkpoint/'coverage.json')
    drivers=checkpoint/'drivers'
    drivers.mkdir(exist_ok=True)
    for relative, driver_sha in acceptance['verificationDriversSha256'].items():
        source=ROOT/relative
        assert hashlib.sha256(source.read_bytes()).hexdigest()==driver_sha
        shutil.copy2(source,drivers/source.name)
    for pattern in ['*.periodic-composition-facts.json','*.loadout-facts.json','*.unit-facts.json','*.reservation-facts.json','*.steal-facts.json','*.power-facts.json','*.derived-facts.json','*.source-gates-facts.json','*.counter-facts.json','*.activation-facts.json','*.paid-facts.json','*.interception-facts.json']:
        for source in (OUT/'delivery/independent-combat').glob(pattern):shutil.copy2(source,checkpoint/source.name)
    for trace in (OUT/'delivery/independent-browser').glob('*.interception-browser-facts.json'):
        shutil.copy2(trace,checkpoint/trace.name)
    assert verify_prior() == (prior, prior_manifest, prior_count)
    print(json.dumps(acceptance['checks'], indent=2))


if __name__ == '__main__':
    if os.environ.get('SDK88_PRIOR_CHECKPOINT'):
        PRIOR_CHECKPOINT = Path(os.environ['SDK88_PRIOR_CHECKPOINT'])
    main()
