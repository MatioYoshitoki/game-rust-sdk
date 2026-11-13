"""Close LF01-LF09 against the same native, browser and delivered candidate."""
import hashlib,json,re,tarfile
from pathlib import Path
from verify_sdk_5_16 import ROOT,OUTPUT,sources
from package_sdk import digest

def load(p):return json.loads(p.read_text())
def names(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    files=['native-verification.json','verification.json','baseline-verification.json','local-preview.json','documentation-check.json','delivery/independent-verification.json','delivery/source-verification.json'];reports={n:load(OUTPUT/n) for n in files};assert all(d['passed'] for d in reports.values())
    n=reports[files[0]];h=reports[files[1]];b=reports[files[2]];preview=reports[files[3]];docs=reports[files[4]];delivery=reports[files[5]];source=reports[files[6]];package=load(OUTPUT/'delivery/package.json');mapping=load(ROOT/'docs/checklists/SDK-5-16-acceptance.json')
    assert sources()==h['sourceHashes']==n['sourceHashes'];assert digest(ROOT/'target/debug/kernel-host')==h['binarySha256']==preview['binarySha256'];assert not preview['createdMatch'] and preview['configId']=='line-field-arena-v1'
    for revision in h.get('harnessProvenance',[]):
        assert revision['unchangedRuntimeAndData'] and revision['binarySha256']==h['binarySha256'] and digest(ROOT/revision['revision'])==revision['sha256']
    for revision in [n.get('harnessRevision'),*n.get('harnessRevisions',[])]:
        if revision:assert digest(ROOT/revision['revision'])==revision['previousSha256']
    for k,v in n['checks'].items():assert digest(OUTPUT/(k+'.log'))==v
    a=names(OUTPUT/'default.log');assert a==names(OUTPUT/'all-features.log');old=names(ROOT/'artifacts/sdk-5-15/default.log');assert len(a)==1146 and len(old)==1115 and old<=a and len(a-old)==31 and sorted(a-old)==b['added']==n['newTests'];assert '# tests 22' in (OUTPUT/'node-storage.log').read_text()
    assert len(h['cases'])==4 and len(h['touchCases'])==2 and len(h['legacy'])==4 and len(h['crashes'])==3
    for family in ['cases','touchCases','legacy']:
        for name,v in h[family].items():
            assert v['closed']['live_worlds']==0 and not v['ledger']['pending'] and v['ledger']['admissions']==v['ledger']['settlements']==1 and not v['pageErrors']
            if family=='legacy':continue
            p=v['skills'];f=p['field'];line=f['line'];policy=f['contact_policy'];remaining=policy['max_total_hits'];seen={};hits=[]
            assert line['start']==f['center'] and line['width']==2*f['radius'] and line['end'][0]>line['start'][0] and abs(line['end'][1]-line['start'][1])<.1;assert policy['used_hits']==0 and not policy['contacts'];assert p['switchedSelectionDuringPreparation'] and p['actualHealing']['hp_restored']>0
            for t in p['ticks']:
                assert t['hits']==len(t['hit_targets'])<=policy['max_targets_per_tick'];remaining-=t['hits'];assert t['remaining_hits']==remaining>=0
                for target in t['hit_targets']:
                    key=(target['id'],target['generation'])
                    if key in seen:assert policy['repeat_interval_ms']>0 and t['scheduled_at_ms']-seen[key]>=policy['repeat_interval_ms']
                    seen[key]=t['scheduled_at_ms'];hits.append(key)
            if policy['repeat_interval_ms']:
                assert len(hits)==4 and len(seen)==1 and p['ended']['reason']=='hit_budget_exhausted';assert p['afterField']['pve']['monsters'][0]['hp']==0 and all(m['hp']==180 for m in p['afterField']['pve']['monsters'][1:])
            else:
                assert len(hits)==3 and len(seen)==3 and p['ended']['reason']=='expired'
                assert all(any(s['definition_id']=='exposed' for s in m.get('statuses',[])) for m in p['afterField']['pve']['monsters'])
            assert v['fight']['kills']==3 and v['fight']['pickedUp']
    p=h['cases']['b-extraction']['skills'];assert p['reconnected'] and p['fieldIndex']==1 and p['progress']['completed_ticks']>=3 and p['progress']['contact_policy']['used_hits']>=2 and len(p['progress']['contact_policy']['contacts'])==1
    c=h['capacity'];assert c['rejected']['reason']=='CAPACITY' and c['selfStillUsable'] and c['before']['spirit']==c['after']['spirit']==176 and c['before']['player_skills']==c['after']['player_skills'] and c['closed']['live_worlds']==0 and not c['ledger']['pending'] and not c['pageErrors']
    rejection=h['rejections'];assert rejection['resource']['reason']=='NO_RESOURCE';assert set(rejection['invalid'])=={'wrong-intent','zero-direction'} and all(v['reason']=='INVALID_MESSAGE' for v in rejection['invalid'].values());assert not rejection['pageErrors'] and not rejection['ledger']['pending'];assert rejection['invalidClosed']['closed']['live_worlds']==0 and not rejection['invalidClosed']['ledger']['pending']
    for point,v in h['crashes'].items():
        assert len(v['recoveries'])==2 and v['recoveries'][0]==v['recoveries'][1] and v['recoveries'][0]['closed']['live_worlds']==0
        if point=='active':assert v['before']['pending_action'] and any(f['contact_policy']['used_hits']>=2 and f['line'] for f in v['before']['pve']['player_fields'])
    assert package['version']=='0.19.0' and digest(Path(package['archive']))==package['sha256']==delivery['archiveSha256']==source['archiveSha256'];assert source['testCount']==1146 and source['sameWorkspaceDefaultAndAllFeatureSet'] and source['offline'] and source['emptyCargoHome']
    for k,v in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+k+'.log'))==v
    assert len(delivery['nodeCases'])==40 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14 and delivery['removedSdkRegressionsBeforeConsumption'] and delivery['offline'] and delivery['emptyCargoHome'];assert delivery['support']['crateVersion']=='0.19.0' and delivery['support']['mechanismVersions']['max']==34 and not delivery['support']['clocks']['replay']
    for p,v in h['sourceHashes'].items():
        if p.startswith('src/'):assert delivery['rustSourceHashes'][p]==v
    copied={'SDK.md':'sdk/SDK.md','Cargo.lock':'sdk/Cargo.lock','adapters/python/asset_host.py':'hosts/asset_host.py',**{'adapters/node/'+n:'hosts/node/'+n for n in ['asset-host.mjs','asset-store.mjs','controlled-store.mjs']}}
    with tarfile.open(package['archive']) as archive:
        for p,v in copied.items():assert hashlib.sha256(archive.extractfile(package['rootName']+'/'+v).read()).hexdigest()==digest(ROOT/p),p
    assert b['protectedCount']==len(b['protectedHashes'])==109
    for group in ['protectedHashes','oldEvidenceHashes']:
        for p,v in b[group].items():assert digest(ROOT/p)==v,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json';assert digest(manifest)==b['sourcePackageManifestHash'];entries=load(manifest)['files']
    for e in entries:assert digest(manifest.parent/e['path'])==e['sha256']
    criteria=mapping['criteria'];assert set(criteria)=={f'LF{i:02}' for i in range(1,10)} and set().union(*(set(v['rustTests']) for v in criteria.values()))>=a-old
    for v in criteria.values():
        assert set(v['rustTests'])<=a
        for p in v['reports']:assert load(ROOT/p)['passed']
    assert not docs['broken']
    final={'passed':True,'date':'2026-11-05','crate':'0.19.0','mechanismVersion':34,'scope':'fixed capsule fields only; not complete M05','rustTests':1146,'newRustTests':31,'retainedOriginalRustTests':1115,'sameDefaultAndAllFeatures':True,'nodeTests':22,'ordinaryH5Cases':4,'touchCases':2,'legacyH5Cases':4,'capacityRejectionCases':1,'resourceRejectionCases':1,'invalidIntentCases':2,'crashBoundaries':3,'recoveriesPerBoundary':2,'independentNodeCases':40,'protocol1RustModes':6,'protocol2And3Cases':14,'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':1146,'protectedCount':109,'sourcePackageFilesVerified':len(entries),'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},'sourceHashes':h['sourceHashes'],'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],'binarySha256':{'ordinaryDelivery':package['binarySha256'],'replayAcceptance':h['binarySha256']},'reports':{str((OUTPUT/f).relative_to(ROOT)):digest(OUTPUT/f) for f in files},'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in (ROOT/'scripts').glob('verify_sdk_5_16*.py')}}
    (OUTPUT/'acceptance.json').write_text(json.dumps(final,ensure_ascii=False,indent=2)+'\n');print('SDK-5-16 LF01-LF09: all same-candidate checks passed')
if __name__=='__main__':main()
