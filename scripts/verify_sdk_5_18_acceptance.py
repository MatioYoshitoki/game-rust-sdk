"""Resolve TM01-TM13 only from the frozen candidate's actual reports and protected baseline."""
import hashlib
import json
from pathlib import Path
import re
import tarfile
from urllib.parse import unquote
from package_sdk import ROOT,digest

OUT=ROOT/'artifacts/sdk-5-18'
def load(path):return json.loads(path.read_text())
def names(path):return set(re.findall(r'^test (\S+) \.\.\. ok$',path.read_text(),re.M))
def main():
    actual=names(OUT/'default.log');assert len(actual)==1184 and actual==names(OUT/'all-features.log')
    old=names(OUT/'baseline-rust.log');assert len(old)==1146 and old<=actual
    added=actual-old;assert len(added)==38 and all(s.startswith('two_members::tests::') for s in added)
    node=(OUT/'node-tests.log').read_text();assert '# tests 30' in node and '# pass 30' in node and '# fail 0' in node
    node_names=set(re.findall(r'^# Subtest: (.+)$',node,re.M));old_node_names=set(re.findall(r'^# Subtest: (.+)$',(OUT/'baseline-node.log').read_text(),re.M));assert len(old_node_names)==22 and old_node_names<=node_names and len(node_names-old_node_names)==8
    for log in ['clippy.log','fmt.log']:assert 'error:' not in (OUT/log).read_text()
    assert 'Finished' in (OUT/'clippy.log').read_text()
    report_names=['public/report.json','browser/report.json','crashes/report.json','delivery/independent-verification.json','delivery/source-verification.json','local-preview.json']
    reports={s:load(OUT/s) for s in report_names};assert all(r['passed'] for r in reports.values())
    pkg=load(OUT/'delivery/package.json');assert pkg['version']=='0.20.0' and digest(pkg['archive'])==pkg['sha256']
    for label in ['public/report.json','browser/report.json','crashes/report.json','delivery/independent-verification.json','local-preview.json']:assert reports[label]['binarySha256']==pkg['binarySha256'],label
    public=reports['public/report.json'];assert len(public['cases'])==16 and all(v.get('noPartialCreate') for v in public['cases'][:12])
    a,b=public['cases'][12:14];assert a['admissionOrder'][0]=='A' and b['admissionOrder'][0]=='south' and a['pickupWinner']=='A' and b['pickupWinner']=='south'
    assert all(m['admissions']==m['settlements']==1 and m['compensations']==0 for v in [a,b] for m in v['members'])
    browser=reports['browser/report.json'];assert len(browser['cases'])==2
    for c in browser['cases']:assert c['pages']==2 and c['privateFieldsAbsent'] and c['reconnectPeerContinued'] and c['hostReleased'] and not c['errors'] and c['raceLoser']=='INVALID_TARGET' and all(v['settlements']==v['admissions']==1 for v in c['final'])
    crashes=reports['crashes/report.json'];assert len(crashes['cases'])==3
    for c in crashes['cases']:assert c['sigkill'] and c['oldKernelExited'] and c['externalValuesChanged'] and len(c['recoveries'])==2 and c['recoveries'][0]['members']==c['recoveries'][1]['members'] and all(r['pending']==0 and r['phase']=='closed' for r in c['recoveries'])
    source=reports['delivery/source-verification.json'];independent=reports['delivery/independent-verification.json']
    assert source['archiveSha256']==independent['archiveSha256']==pkg['sha256'] and source['testCount']==1184 and source['emptyCargoHome'] and source['offline'] and source['sameWorkspaceDefaultAndAllFeatureSet']
    for k,h in source['checks'].items():assert digest(OUT/f'delivery/source-{k}.log')==h
    assert independent['outsideCheckout'] and independent['removedSdkRegressionsBeforeConsumption'] and independent['offline'] and independent['emptyCargoHome']
    assert len(independent['cases'])==32 and len([x for x in independent['cases'] if x.startswith('protocol1-')])==6
    for p,h in independent['rustSourceHashes'].items():assert digest(ROOT/p)==h
    assert reports['local-preview.json']['createdMatch'] is False and reports['local-preview.json']['port']==4190
    # Every retained old byte is checked; only explicitly migrated consumer/build/docs may differ.
    baseline=load(OUT/'baseline.json');allowed={'Cargo.toml','Cargo.lock','SDK.md','delivery/README.md','examples/minimal-host/Cargo.toml','examples/minimal-host/Cargo.lock','examples/minimal-host/src/main.rs'}
    protected={p:h for p,h in baseline.items() if p not in allowed}
    for p,h in protected.items():assert (ROOT/p).is_file() and digest(ROOT/p)==h,p
    mapping=load(ROOT/'docs/checklists/SDK-5-18-acceptance.json');assert set(mapping['criteria'])=={f'TM{i:02}' for i in range(1,14)}
    covered=set()
    for key,c in mapping['criteria'].items():
        selected={'two_members::tests::'+s for s in c['testSuffixes']};assert selected<=actual,key;covered|=selected
        for report in c['reports']:assert reports[report]['passed']
    assert covered==added,(added-covered,covered-added)
    copies={'SDK.md':'sdk/SDK.md','Cargo.lock':'sdk/Cargo.lock',**{p:'sdk/'+p for p in independent['rustSourceHashes']},**{'adapters/node/'+n:'hosts/node/'+n for n in ['asset-host.mjs','asset-store.mjs','controlled-store.mjs','two-member-host.mjs']},**{'adapters/h5/'+n:'hosts/h5/'+n for n in ['persistence.mjs','two-member-server.mjs']},**{'web/'+n:'web/'+n for n in ['two-member.html','two-member.js']}}
    copies.update({str(p.relative_to(ROOT)):'samples/two-members/'+p.name for p in (ROOT/'examples/two-member-host').glob('*.json')})
    with tarfile.open(pkg['archive']) as archive:
        for p,target in copies.items():assert hashlib.sha256(archive.extractfile(pkg['rootName']+'/'+target).read()).hexdigest()==digest(ROOT/p),p
    for p in ['PRD-367-independent-rust-match-kernel.md','docs/plans/sdk-delivery-roadmap.json','docs/batches/SDK-5-18-two-member-loop.md','README.md']:assert 'SDK-5-18' in (ROOT/p).read_text()
    documentation=['README.md','PRD-367-independent-rust-match-kernel.md','SDK.md','docs/batches/SDK-5-18-two-member-loop.md','docs/interfaces/two-members-v4.md','docs/evidence/SDK-5-18-two-member-loop-2026-11-05.md','docs/guides/sdk-integration.md','docs/guides/sdk-delivery.md','docs/plans/sdk-delivery-roadmap.md','docs/checklists/independent-match-kernel.CHECKLIST.md','docs/checklists/sdk-requirements-gap.md']
    historical=set();links=0
    for name in documentation:
        page=ROOT/name
        for match in re.finditer(r'\[[^\]]*\]\(([^\s)]+)(?:\s+[^)]*)?\)',page.read_text()):
            raw=match.group(1)
            if raw.startswith(('http:','https:','mailto:','#')):continue
            target=(page.parent/unquote(raw.split('#',1)[0])).resolve();links+=1
            if target.exists() or target==OUT/'acceptance.json':continue
            relative=str(target.relative_to(ROOT))
            if relative.startswith('artifacts/') and not relative.startswith('artifacts/sdk-5-18/'):historical.add(relative)
            else:raise AssertionError(('broken current documentation link',name,relative))
    doc_report={'passed':True,'checkedLinks':links,'unavailableHistoricalArtifactLinks':sorted(historical),'historicalException':'cleaned_by_user','hashes':{p:digest(ROOT/p) for p in documentation}}
    (OUT/'documentation-check.json').write_text(json.dumps(doc_report,ensure_ascii=False,indent=2)+'\n')
    evidence={str((OUT/p).relative_to(ROOT)):digest(OUT/p) for p in report_names+['baseline.json','baseline-rust.log','baseline-node.log','default.log','all-features.log','node-tests.log','clippy.log','fmt.log','delivery/package.json','documentation-check.json']}
    source_paths=[*ROOT.glob('src/**/*.rs'),ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'SDK.md',*ROOT.glob('adapters/node/two-member*.mjs'),ROOT/'adapters/h5/two-member-server.mjs',*ROOT.glob('web/two-member.*'),*ROOT.glob('examples/two-member-host/*.json'),*ROOT.glob('scripts/verify_sdk_5_18*'),ROOT/'scripts/package_sdk.py',ROOT/'docs/interfaces/two-members-v4.md',ROOT/'docs/interfaces/two-members-v4.schema.json']
    result={'passed':True,'date':'2026-11-05','batch':'SDK-5-18','crate':'0.20.0','protocol':4,'schema':5,'mechanism':35,'rustTests':1184,'retainedRustTests':1146,'newRustTests':38,'sameDefaultAllFeatures':True,'nodeTests':30,'retainedNodeTests':22,'newNodeTests':8,'retainedNodeTestNames':sorted(old_node_names),'newNodeTestNames':sorted(node_names-old_node_names),'publicCases':16,'twoPageH5Cases':2,'strongKillBoundaries':3,'recoveriesFromSameCheckpoint':6,'independentCases':32,'protectedFiles':len(protected),'historicalArtifactException':'Cleaned by user during SDK-5-17; missing archives not restored or claimed verified.','originalScenariosExecuted':0,'originalNumericVectorsExecuted':0,'criteria':{k:{'status':'passed',**c} for k,c in mapping['criteria'].items()},'limitations':mapping['limitations'],'archiveSha256':pkg['sha256'],'sourceCrateSha256':pkg['sourceCrateSha256'],'binarySha256':pkg['binarySha256'],'reports':evidence,'documentationHashes':doc_report['hashes'],'protectedHashes':protected,'sourceHashes':{str(p.relative_to(ROOT)):digest(p) for p in source_paths}}
    (OUT/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print('SDK-5-18 TM01-TM13 passed with same-candidate evidence')

if __name__=='__main__':main()
