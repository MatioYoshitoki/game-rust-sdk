"""Sign PP01-PP12 against immutable prior evidence and the actual same-candidate delivery."""
import hashlib,json,re,tarfile
from pathlib import Path
from urllib.parse import unquote
from package_sdk import ROOT,digest
OUT=ROOT/'artifacts/sdk-5-26'
def load(p):return json.loads(p.read_text())
def names(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    report_names=['native-verification.json','public/report.json','browser/report.json','interop-verification.json','crashes/report.json','legacy5/report.json','legacy4/report.json','legacy6/report.json','legacy38/report.json','legacy39/report.json','legacy40/report.json','legacy41/report.json','legacy42/report.json','delivery/independent-verification.json','delivery/source-verification.json','local-preview.json','visual-verification.json']
    reports={n:load(OUT/n) for n in report_names};assert all(r['passed'] for r in reports.values());pkg=load(OUT/'delivery/package.json');assert pkg['version']=='0.28.0' and digest(pkg['archive'])==pkg['sha256']
    for n in report_names:
        if 'binarySha256' in reports[n]:assert reports[n]['binarySha256']==pkg['binarySha256'],n
    for row in reports['visual-verification.json']['inspected']:assert digest(OUT/row['path'])==row['sha256']
    native=reports['native-verification.json'];actual=names(OUT/'default.log');old=names(ROOT/'artifacts/sdk-5-25/default.log');assert len(old)==1408 and old<=actual and len(actual)==1442 and actual==names(OUT/'all-features.log');added=actual-old;assert sorted(actual)==load(OUT/'expected-tests.json');assert len(added)==34 and sorted(added)==native['newTests']
    for p,h in native['sourceHashes'].items():assert digest(ROOT/p)==h
    for p,h in native['logs'].items():assert digest(OUT/(p+'.log'))==h
    assert native['nodeTests']==82 and native['retainedNodeTests']==74
    public=reports['public/report.json'];assert len(public['cases'])==64 and all(c['noPartialCreate'] for c in public['cases'][:13]);assert reports['legacy4/report.json']['crate']=='0.28.0' and len(reports['legacy4/report.json']['cases'])==16
    assert len(reports['legacy38/report.json']['cases'])==29
    assert len(reports['legacy39/report.json']['cases'])==33
    assert len(reports['legacy40/report.json']['cases'])==43
    assert len(reports['legacy41/report.json']['cases'])==59
    assert len(reports['legacy42/report.json']['cases'])==64
    browser=reports['browser/report.json'];assert sorted(c['pages'] for c in browser['cases'])==[4,8]
    for c in browser['cases']:
        assert c['privateProjection'] and c['reconnectPreserved'] and c['retiredSourceStopped'] and c['hostReleased'] and c['cacheLossRecovered'] and c['lostFrameRecovered'] and c['oldFrameRejected'] and not c['errors']
        assert all(a['admissions']==a['settlements']==1 for a in c['final'])
        assert all(any(m[k]>0 for m in c['periodic']) for k in ['outgoingDotTicks','incomingDotTicks','selfHotTicks','npcHotTicks','hpRestored'])
        assert all(m['ownGrants']>0 for m in c['shields']) and any(m['dotAbsorbed']>0 for m in c['shields']) and c['shieldReconnect']['preservedOrExpired']
        assert c['flightUiUsed'] and c['flightReconnect'] and all(m['ownStarted']>0 and m['ownContacts']>0 for m in c['flightStats'])
        if c['reconnectCadence']['expiredNormally']:assert c['reconnectCadence']['afterLogical']>=c['reconnectCadence']['expiresAt']
    interop=reports['interop-verification.json'];assert interop['exactSnapshotParity'] and interop['recipients']==8 and interop['delta']>0 and interop['rustVectorsSha256']==digest(OUT/'interop-default.json')==digest(OUT/'interop-all-features.json')
    assert len(reports['legacy5/report.json']['cases'])==20 and len(reports['legacy6/report.json']['cases'])==31
    for c in public['cases'][-2:]:
        assert c['peerDuringUnknown'] and c['retiredSourceStopped'] and c['reconnectPreserved'] and c['cacheResynced'] and c['pending']==0
        assert c['shieldReconnect'] and all(m['ownGrants']>0 for m in c['shields']) and any(m['dotAbsorbed']>0 for m in c['shields']) and c['wire']['privateSlices']>0
        assert c['flightValidated'] and c['flightReconnect'] and all(m['ownStarted']>0 and m['ownContacts']>0 for m in c['flightStats']) and c['wire']['flightPlans']>0 and c['wire']['flightPending']>0 and c['wire']['flightCommits']>0 and c['wire']['flightCleared']>0
        assert c['wire']['privateProjection'] and c['wire']['added']>0 and c['wire']['removed']>0 and c['wire']['clearedNpcLists']>0
        assert c['wire']['flightProgress']>0 and c['wire']['contactEvents']>0 and c['wire']['endedEvents']>0
    crashes=reports['crashes/report.json'];assert len(crashes['cases'])==3
    for c in crashes['cases']:assert c['sigkill'] and c['oldKernelExited'] and c['externalValuesChanged'] and len(c['recoveries'])==2 and c['recoveries'][0]['accounts']==c['recoveries'][1]['accounts'] and len(c['recoveries'][0]['accounts'])==8 and all(r['pending']==0 and r['phase']=='closed' for r in c['recoveries'])
    for c in crashes['cases']:
        marker=json.loads((OUT/('crashes/'+c['boundary']+'/crash.stdout')).read_text().strip().splitlines()[-1])
        assert c['boundary']=='partial_admissions' or marker['startedWithActivePeriodic'] and marker['startedWithActiveShields'] and marker['startedWithFlight']
        assert all(all(m['unitCommits']==m['unitProjectiles']==0 for m in r['combat']) for r in c['recoveries'])
        assert all(all(m['ownStarted']==m['ownContacts']==0 for m in r['flightStats']) for r in c['recoveries'])
        assert all(all(m['dotTicks']==m['hotTicks']==0 for m in r['periodic']) for r in c['recoveries'])
    source=reports['delivery/source-verification.json'];ind=reports['delivery/independent-verification.json'];assert source['archiveSha256']==ind['archiveSha256']==pkg['sha256'] and source['testCount']==1442 and source['emptyCargoHome'] and source['offline'] and source['sameWorkspaceDefaultAndAllFeatureSet'];assert ind['h5']['passed'] and ind['h5']['sourceFixturesHidden'] and not ind['h5']['browserErrors'];assert len(ind['cases'])==48 and ind['outsideCheckout'] and ind['removedSdkRegressionsBeforeConsumption']
    for n,h in source['checks'].items():assert digest(OUT/f'delivery/source-{n}.log')==h
    for key,n in [('members-four',4),('members-eight',8)]:
        c=ind['cases'][key];assert c['capacity']==n and c['worlds']==1 and c['phase']=='closed' and c['pending']==0 and len(c['refused'])==n-1;assert c['winner']==('P07' if n==8 else 'P00');assert c['admissionOrder']==([f'P{i:02}' for i in reversed(range(n))] if n==8 else [f'P{i:02}' for i in range(n)])
    assert reports['local-preview.json']['createdMatch'] is False and reports['local-preview.json']['port']==4213
    baseline=load(OUT/'baseline.json');protected={p:h for p,h in baseline.items() if p.startswith(('config/','rust-match-kernel-v1.1/','examples/','artifacts/'))}
    for p,h in protected.items():assert (ROOT/p).is_file() and digest(ROOT/p)==h,p
    mapping=load(ROOT/'docs/checklists/SDK-5-26-acceptance.json');assert set(mapping['criteria'])=={f'PP{i:02}' for i in range(1,13)};covered=set()
    for key,c in mapping['criteria'].items():assert set(c['rustTests'])<=actual,key;covered|=set(c['rustTests']);assert all(reports[n]['passed'] for n in c['reports'])
    assert covered==added
    copies={'SDK.md':'sdk/SDK.md','Cargo.lock':'sdk/Cargo.lock',**{p:'sdk/'+p for p in native['sourceHashes'] if p.startswith('src/')},**{'adapters/node/'+n:'hosts/node/'+n for n in ['asset-host.mjs','asset-store.mjs','controlled-store.mjs','two-member-host.mjs','member-host.mjs','delta-host.mjs','multiplayer-periodic-host.mjs','multiplayer-shield-host.mjs','multiplayer-combat-host.mjs','member-area-host.mjs','member-line-host.mjs','member-piercing-host.mjs']},**{'adapters/h5/'+n:'hosts/h5/'+n for n in ['persistence.mjs','two-member-server.mjs','member-server.mjs','delta-server.mjs','multiplayer-periodic-server.mjs','multiplayer-shield-server.mjs','multiplayer-combat-server.mjs','member-area-server.mjs','member-line-server.mjs','member-piercing-server.mjs']},**{'web/'+n:'web/'+n for n in ['two-member.html','two-member.js','member.html','member.js','delta.html','delta.js','replication.mjs','periodic.mjs','member-area.html','member-area.js','shields.mjs','multiplayer-periodic.html','multiplayer-periodic.js','multiplayer-shield.html','multiplayer-shield.js','combat.mjs','areas.mjs','multiplayer-combat.html','multiplayer-combat.js','member-line.html','member-line.js','member-piercing.html','member-piercing.js','flights.mjs']}}
    copies.update({str(p.relative_to(ROOT)):'samples/members/'+p.name for p in (ROOT/'examples/member-host').glob('*.json')})
    copies.update({str(p.relative_to(ROOT)):'samples/delta/'+p.name for p in (ROOT/'examples/delta-host').glob('*.json')})
    copies.update({str(p.relative_to(ROOT)):'samples/member-area/'+p.name for p in (ROOT/'examples/member-area-host').glob('*.json')})
    copies.update({'docs/interfaces/'+n:'docs/interfaces/'+n for n in ['publications-v6.md','publications-v6.schema.json','multiplayer-shields-v1.md','multiplayer-shields-v1.schema.json','unified-member-combat-v1.md','unified-member-combat-v1.schema.json','member-piercing-v1.md','member-piercing-v1.schema.json','member-circle-plans-v1.md','member-circle-plans-v1.schema.json','member-line-plans-v1.md','member-line-plans-v1.schema.json']})
    copies.update({'docs/interfaces/'+n:'docs/interfaces/'+n for n in ['multiplayer-periodic-v1.md','multiplayer-periodic-v1.schema.json']})
    copies.update({str(p.relative_to(ROOT)):'samples/member-line/'+p.name for p in (ROOT/'examples/member-line-host').glob('*.json')})
    copies.update({str(p.relative_to(ROOT)):'samples/member-piercing/'+p.name for p in (ROOT/'examples/member-piercing-host').glob('*.json')})
    copies['delivery/README.md']='README.md'
    copies['docs/batches/SDK-5-26-member-piercing.md']='docs/batches/SDK-5-26-member-piercing.md'
    with tarfile.open(pkg['archive']) as archive:
        for p,target in copies.items():
            expected=(ROOT/p).read_bytes()
            if p=='docs/interfaces/publications-v6.md':expected=expected.decode().replace('[DP01～DP12](../batches/SDK-5-20-member-delta.md)','仓库SDK-5-20规格DP01～DP12').encode()
            if p=='docs/interfaces/multiplayer-periodic-v1.md':expected=expected.decode().replace('(periodic-damage-v1.md)','(../../sdk/SDK.md)').replace('(periodic-healing-v1.md)','(../../sdk/SDK.md)').encode()
            if p=='docs/interfaces/multiplayer-shields-v1.md':expected=expected.decode().replace('(absorption-shields-v1.md)','(../../sdk/SDK.md)').encode()
            if p=='docs/interfaces/unified-member-combat-v1.md':expected=expected.decode().replace('(unit-targets-v1.md)','(../../sdk/SDK.md)').encode()
            assert archive.extractfile(pkg['rootName']+'/'+target).read()==expected,p
    docs=['README.md','PRD-367-independent-rust-match-kernel.md','SDK.md','docs/batches/SDK-5-26-member-piercing.md','docs/interfaces/member-piercing-v1.md','docs/evidence/SDK-5-26-member-piercing-2026-11-05.md','docs/plans/sdk-delivery-roadmap.md','docs/guides/sdk-integration.md','docs/guides/sdk-delivery.md','docs/checklists/independent-match-kernel.CHECKLIST.md','docs/checklists/sdk-requirements-gap.md'];historical=set();links=0
    for name in docs:
        page=ROOT/name
        for m in re.finditer(r'\[[^\]]*\]\(([^\s)]+)(?:\s+[^)]*)?\)',page.read_text()):
            raw=m.group(1)
            if raw.startswith(('http:','https:','mailto:','#')):continue
            target=(page.parent/unquote(raw.split('#',1)[0])).resolve();links+=1
            if target.exists() or target==OUT/'acceptance.json':continue
            relative=str(target.relative_to(ROOT))
            if relative.startswith('artifacts/') and not relative.startswith('artifacts/sdk-5-26/'):historical.add(relative)
            else:raise AssertionError(('broken current link',name,relative))
    doc={'passed':True,'checkedLinks':links,'historicalUnavailable':sorted(historical),'hashes':{n:digest(ROOT/n) for n in docs}};(OUT/'documentation-check.json').write_text(json.dumps(doc,ensure_ascii=False,indent=2)+'\n')
    paths=[*ROOT.glob('src/**/*.rs'),ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'SDK.md',*ROOT.glob('adapters/node/*member*.mjs'),*ROOT.glob('adapters/h5/*member*.mjs'),*ROOT.glob('web/*member*'),*ROOT.glob('examples/member-host/*.json'),*ROOT.glob('scripts/verify_sdk_5_26*'),ROOT/'web/replication.mjs',*ROOT.glob('web/delta*'),ROOT/'adapters/node/delta-host.mjs',ROOT/'adapters/h5/delta-server.mjs',*ROOT.glob('examples/delta-host/*.json'),ROOT/'scripts/package_sdk.py',ROOT/'docs/interfaces/publications-v6.md',ROOT/'docs/interfaces/publications-v6.schema.json',ROOT/'docs/interfaces/unified-member-combat-v1.md',ROOT/'docs/interfaces/unified-member-combat-v1.schema.json',ROOT/'adapters/node/member-area-host.mjs',ROOT/'adapters/node/member-area-crash-fixture.mjs',ROOT/'adapters/h5/member-area-server.mjs',ROOT/'web/periodic.mjs',*ROOT.glob('web/member-area.*'),*ROOT.glob('examples/member-area-host/*.json'),ROOT/'adapters/node/periodic.test.mjs',ROOT/'adapters/node/replication.test.mjs']
    paths += [ROOT/'web/member-line.js',ROOT/'web/member-line.html',ROOT/'adapters/node/member-line-host.mjs',ROOT/'adapters/h5/member-line-server.mjs',*ROOT.glob('examples/member-line-host/*.json')]
    paths += [ROOT/'web/shields.mjs',ROOT/'adapters/node/shields.test.mjs',ROOT/'adapters/node/multiplayer-periodic-host.mjs',ROOT/'docs/interfaces/unified-member-combat-v1.schema.json']
    paths += [ROOT/'web/combat.mjs',ROOT/'adapters/node/combat.test.mjs',ROOT/'docs/checklists/SDK-5-26-acceptance.json']
    paths += [ROOT/'web/areas.mjs',ROOT/'adapters/node/areas.test.mjs',ROOT/'docs/interfaces/member-piercing-v1.md',ROOT/'docs/interfaces/member-piercing-v1.schema.json']
    paths += [ROOT/'web/flights.mjs',ROOT/'adapters/node/flights.test.mjs',ROOT/'web/member-piercing.html',ROOT/'web/member-piercing.js',ROOT/'adapters/node/member-piercing-host.mjs',ROOT/'adapters/node/member-piercing-crash-fixture.mjs',ROOT/'adapters/h5/member-piercing-server.mjs',*ROOT.glob('examples/member-piercing-host/*.json')]
    result={'passed':True,'date':'2026-11-05','batch':'SDK-5-26','crate':'0.28.0','protocol':6,'schema':7,'mechanism':43,'scope':'fixed 1-8 required members, straight piercing flights with shared projectile capacity, hostile sweeps, contact budget and unified direct damage','rustTests':1442,'retainedRustTests':1408,'newRustTests':34,'nodeTests':82,'sameDefaultAndAllFeatures':True,'publicCases':64,'legacyProtocol4Cases':16,'legacyProtocol5Cases':20,'legacyProtocol6Cases':31,'h5Pages':[4,8],'strongKillBoundaries':3,'recoveriesFromSameCheckpoint':6,'independentCases':48,'originalScenariosExecuted':0,'originalNumericVectorsExecuted':0,'protectedFiles':len(protected),'historicalArtifactException':'SDK-5-17 user cleanup retained; all prior evidence present at this batch start is protected','criteria':{k:{'status':'passed',**c} for k,c in mapping['criteria'].items()},'limitations':mapping['limitations'],'archiveSha256':pkg['sha256'],'sourceCrateSha256':pkg['sourceCrateSha256'],'binarySha256':pkg['binarySha256'],'reports':{n:digest(OUT/n) for n in report_names+['documentation-check.json','delivery/package.json']},'documentationHashes':doc['hashes'],'protectedHashes':protected,'sourceHashes':{str(p.relative_to(ROOT)):digest(p) for p in paths}}
    (OUT/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print('SDK-5-26 PP01-PP12 passed with identical candidate and preserved previous evidence')
if __name__=='__main__':main()
