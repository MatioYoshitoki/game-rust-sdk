"""Sign MC01-MC10 against immutable prior evidence and the actual same-candidate delivery."""
import hashlib,json,re,tarfile
from pathlib import Path
from urllib.parse import unquote
from package_sdk import ROOT,digest
OUT=ROOT/'artifacts/sdk-5-19'
def load(p):return json.loads(p.read_text())
def names(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    report_names=['native-verification.json','public/report.json','browser/report.json','crashes/report.json','legacy-public/report.json','delivery/independent-verification.json','delivery/source-verification.json','local-preview.json']
    reports={n:load(OUT/n) for n in report_names};assert all(r['passed'] for r in reports.values());pkg=load(OUT/'delivery/package.json');assert pkg['version']=='0.21.0' and digest(pkg['archive'])==pkg['sha256']
    for n in report_names:
        if 'binarySha256' in reports[n]:assert reports[n]['binarySha256']==pkg['binarySha256'],n
    native=reports['native-verification.json'];actual=names(OUT/'default.log');old=names(ROOT/'artifacts/sdk-5-18/default.log');assert len(old)==1184 and old<=actual and len(actual)==1209 and actual==names(OUT/'all-features.log');added=actual-old;assert len(added)==25 and sorted(added)==native['newTests']
    for p,h in native['sourceHashes'].items():assert digest(ROOT/p)==h
    for p,h in native['logs'].items():assert digest(OUT/(p+'.log'))==h
    assert native['nodeTests']==native['retainedNodeTests']==30
    public=reports['public/report.json'];assert len(public['cases'])==20 and all(c['noPartialCreate'] for c in public['cases'][:13]);assert reports['legacy-public/report.json']['crate']=='0.21.0' and len(reports['legacy-public/report.json']['cases'])==16
    browser=reports['browser/report.json'];assert sorted(c['pages'] for c in browser['cases'])==[4,8]
    for c in browser['cases']:assert c['privateFieldsAbsent'] and c['reconnectPeerContinued'] and c['hostReleased'] and not c['errors'] and all(a['admissions']==a['settlements']==1 for a in c['final'])
    crashes=reports['crashes/report.json'];assert len(crashes['cases'])==3
    for c in crashes['cases']:assert c['sigkill'] and c['oldKernelExited'] and c['externalValuesChanged'] and len(c['recoveries'])==2 and c['recoveries'][0]['accounts']==c['recoveries'][1]['accounts'] and len(c['recoveries'][0]['accounts'])==8 and all(r['pending']==0 and r['phase']=='closed' for r in c['recoveries'])
    source=reports['delivery/source-verification.json'];ind=reports['delivery/independent-verification.json'];assert source['archiveSha256']==ind['archiveSha256']==pkg['sha256'] and source['testCount']==1209 and source['emptyCargoHome'] and source['offline'] and source['sameWorkspaceDefaultAndAllFeatureSet'];assert len(ind['cases'])==34 and ind['outsideCheckout'] and ind['removedSdkRegressionsBeforeConsumption']
    for n,h in source['checks'].items():assert digest(OUT/f'delivery/source-{n}.log')==h
    for key,n in [('members-four',4),('members-eight',8)]:
        c=ind['cases'][key];assert c['capacity']==n and c['worlds']==1 and c['phase']=='closed' and c['pending']==0 and len(c['refused'])==n-1;assert c['winner']==('P07' if n==8 else 'P00');assert c['admissionOrder']==([f'P{i:02}' for i in reversed(range(n))] if n==8 else [f'P{i:02}' for i in range(n)])
    assert reports['local-preview.json']['createdMatch'] is False and reports['local-preview.json']['port']==4193
    baseline=load(OUT/'baseline.json');protected={p:h for p,h in baseline.items() if p.startswith(('config/','rust-match-kernel-v1.1/','examples/','artifacts/'))}
    for p,h in protected.items():assert (ROOT/p).is_file() and digest(ROOT/p)==h,p
    mapping=load(ROOT/'docs/checklists/SDK-5-19-acceptance.json');assert set(mapping['criteria'])=={f'MC{i:02}' for i in range(1,11)};covered=set()
    for key,c in mapping['criteria'].items():assert set(c['rustTests'])<=actual,key;covered|=set(c['rustTests']);assert all(reports[n]['passed'] for n in c['reports'])
    assert covered==added
    copies={'SDK.md':'sdk/SDK.md','Cargo.lock':'sdk/Cargo.lock',**{p:'sdk/'+p for p in native['sourceHashes'] if p.startswith('src/')},**{'adapters/node/'+n:'hosts/node/'+n for n in ['asset-host.mjs','asset-store.mjs','controlled-store.mjs','two-member-host.mjs','member-host.mjs']},**{'adapters/h5/'+n:'hosts/h5/'+n for n in ['persistence.mjs','two-member-server.mjs','member-server.mjs']},**{'web/'+n:'web/'+n for n in ['two-member.html','two-member.js','member.html','member.js']}}
    copies.update({str(p.relative_to(ROOT)):'samples/members/'+p.name for p in (ROOT/'examples/member-host').glob('*.json')})
    with tarfile.open(pkg['archive']) as archive:
        for p,target in copies.items():assert hashlib.sha256(archive.extractfile(pkg['rootName']+'/'+target).read()).hexdigest()==digest(ROOT/p),p
    docs=['README.md','PRD-367-independent-rust-match-kernel.md','SDK.md','docs/batches/SDK-5-19-member-capacity.md','docs/interfaces/members-v5.md','docs/evidence/SDK-5-19-member-capacity-2026-11-05.md','docs/plans/sdk-delivery-roadmap.md','docs/guides/sdk-integration.md','docs/guides/sdk-delivery.md','docs/checklists/independent-match-kernel.CHECKLIST.md','docs/checklists/sdk-requirements-gap.md'];historical=set();links=0
    for name in docs:
        page=ROOT/name
        for m in re.finditer(r'\[[^\]]*\]\(([^\s)]+)(?:\s+[^)]*)?\)',page.read_text()):
            raw=m.group(1)
            if raw.startswith(('http:','https:','mailto:','#')):continue
            target=(page.parent/unquote(raw.split('#',1)[0])).resolve();links+=1
            if target.exists() or target==OUT/'acceptance.json':continue
            relative=str(target.relative_to(ROOT))
            if relative.startswith('artifacts/') and not relative.startswith('artifacts/sdk-5-19/'):historical.add(relative)
            else:raise AssertionError(('broken current link',name,relative))
    doc={'passed':True,'checkedLinks':links,'historicalUnavailable':sorted(historical),'hashes':{n:digest(ROOT/n) for n in docs}};(OUT/'documentation-check.json').write_text(json.dumps(doc,ensure_ascii=False,indent=2)+'\n')
    paths=[*ROOT.glob('src/**/*.rs'),ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'SDK.md',*ROOT.glob('adapters/node/*member*.mjs'),*ROOT.glob('adapters/h5/*member*.mjs'),*ROOT.glob('web/*member*'),*ROOT.glob('examples/member-host/*.json'),*ROOT.glob('scripts/verify_sdk_5_19*'),ROOT/'scripts/package_sdk.py',ROOT/'docs/interfaces/members-v5.md',ROOT/'docs/interfaces/members-v5.schema.json']
    result={'passed':True,'date':'2026-11-05','batch':'SDK-5-19','crate':'0.21.0','protocol':5,'schema':6,'mechanism':36,'scope':'fixed required roster, configurable 1-8 members, inherited bounded PVE','rustTests':1209,'retainedRustTests':1184,'newRustTests':25,'nodeTests':30,'sameDefaultAndAllFeatures':True,'publicCases':20,'legacyProtocol4Cases':16,'h5Pages':[4,8],'strongKillBoundaries':3,'recoveriesFromSameCheckpoint':6,'independentCases':34,'originalScenariosExecuted':0,'originalNumericVectorsExecuted':0,'protectedFiles':len(protected),'historicalArtifactException':'SDK-5-17 user cleanup retained; all prior evidence present at this batch start is protected','criteria':{k:{'status':'passed',**c} for k,c in mapping['criteria'].items()},'limitations':mapping['limitations'],'archiveSha256':pkg['sha256'],'sourceCrateSha256':pkg['sourceCrateSha256'],'binarySha256':pkg['binarySha256'],'reports':{n:digest(OUT/n) for n in report_names+['documentation-check.json','delivery/package.json']},'documentationHashes':doc['hashes'],'protectedHashes':protected,'sourceHashes':{str(p.relative_to(ROOT)):digest(p) for p in paths}}
    (OUT/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print('SDK-5-19 MC01-MC10 passed with identical candidate and preserved previous evidence')
if __name__=='__main__':main()
