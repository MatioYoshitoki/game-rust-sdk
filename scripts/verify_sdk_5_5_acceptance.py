"""Aggregate PD01-PD10 using immutable logs, archive and current candidate hashes."""
from pathlib import Path
import json
import re

from verify_sdk_5_5 import ROOT, OUTPUT, sources
from package_sdk import digest


def load(path):return json.loads(path.read_text())


def tests(path):return set(re.findall(r'^test (\S+) \.\.\. ok$',path.read_text(),re.M))


def main():
    integration=load(OUTPUT/'verification.json')
    delivery=load(OUTPUT/'delivery/independent-verification.json')
    native=load(OUTPUT/'native-verification.json')
    package=load(OUTPUT/'delivery/package.json')
    source=load(OUTPUT/'delivery/source-verification.json')
    baseline=load(OUTPUT/'baseline-verification.json')
    mapping=load(ROOT/'docs/checklists/SDK-5-5-acceptance.json')
    assert all(v['passed'] for v in [integration,delivery,baseline,source,native])
    assert sources()==integration['sourceHashes']
    assert digest(ROOT/'target/debug/kernel-host')==integration['binarySha256']
    assert native['sourceHashes']=={p:h for p,h in integration['sourceHashes'].items() if not p.startswith('scripts/')}
    core={p:h for p,h in integration['sourceHashes'].items() if p.startswith('src/') or p in ['Cargo.toml','Cargo.lock']}
    assert all(delivery['rustSourceHashes'][p]==h for p,h in core.items() if p.startswith('src/'))
    for name in ['default','all-features','clippy','node-storage']:
        assert digest(OUTPUT/(name+'.log'))==integration['checks'][name]==native['checks'][name]
    a=tests(OUTPUT/'default.log');b=tests(OUTPUT/'all-features.log')
    old=set().union(*(tests(ROOT/p) for p in baseline['baselineSources']))
    assert a==b and len(a)==767 and len(old)==736 and old<=a
    assert source['testCount']==767 and source['sameWorkspaceDefaultAndAllFeatureSet']
    assert source['archiveSha256']==package['sha256'] and source['emptyCargoHome'] and source['offline']
    for name,h in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+name+'.log'))==h
    assert len(a-old)==31 and sorted(a-old)==baseline['added']
    assert '# tests 22' in (OUTPUT/'node-storage.log').read_text()
    for p,h in baseline['protectedHashes'].items():assert digest(ROOT/p)==h,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json'
    assert digest(manifest)==baseline['sourcePackageManifestHash']
    assert digest(Path(package['archive']))==package['sha256']==delivery['archiveSha256']
    assert len(integration['legacy'])==8
    assert len(integration['cases'])==4 and len(integration['touchCases'])==2 and len(integration['crashes'])==3
    for family in ['cases','touchCases','legacy']:
        for value in integration[family].values():
            assert value['closed']['live_worlds']==0 and not value['ledger']['pending']
            assert value['ledger']['admissions']==value['ledger']['settlements']==1
    for value in integration['crashes'].values():
        p=value['partialDotBeforeCrash']['periodic']
        assert p['ticks_done']>=1 and p['next_tick_at_ms'] is not None
        assert len(value['recoveries'])==2 and value['recoveries'][0]==value['recoveries'][1]
        assert value['recoveries'][0]['closed']['live_worlds']==0
    for family in ['cases','touchCases']:
        for v in integration[family].values():
            assert v['lethal']['kills']==1 and v['lethal']['pickedUp']
            assert v['lethal']['lethalTick']['hp_damage']>0
            assert v['status']['partial']['periodic']['ticks_done']>=1
    assert len(delivery['nodeCases'])==10 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14
    assert delivery['emptyCargoHome'] and delivery['offline'] and delivery['removedSdkRegressionsBeforeConsumption']
    criteria=mapping['criteria']
    assert set(criteria)=={f'PD{i:02}' for i in range(1,11)}
    for value in criteria.values():
        assert set(value.get('rustTests',[]))<=a
        for p in value.get('reports',[]):assert load(ROOT/p)['passed']
    preview=load(OUTPUT/'local-preview.json');assert preview['passed'] and not preview['createdMatch']
    documentation=load(OUTPUT/'documentation-check.json');assert documentation['passed'] and not documentation['broken']
    report={'passed':True,'date':'2026-11-04','crate':'0.8.0','mechanismVersion':23,
            'scope':'M04 finite periodic physical damage; not full original requirements',
            'rustTests':len(a),'newRustTests':31,'retainedOriginalRustTests':len(old),
            'sameDefaultAndAllFeatures':True,'clippy':'passed with -D warnings','nodeTests':22,
            'ordinaryNodeCases':4,'touchCases':2,'legacy19To22Cases':8,'crashBoundaries':3,'recoveriesPerBoundary':2,
            'independentNodeCases':10,'legacyRustModes':6,'protocol2And3Cases':14,
            'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':767,
            'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'scripts').glob('verify_sdk_5_5*.py'))},
            'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},
            'sourceHashes':integration['sourceHashes'],'nativeCoreSourceHashes':core,
            'protectedHashes':baseline['protectedHashes'],
            'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],
            'binarySha256':{'replayEnabledAcceptance':integration['binarySha256'],'ordinaryDelivery':package['binarySha256']},
            'reports':{str(p.relative_to(ROOT)):digest(p) for p in [OUTPUT/'verification.json',
                OUTPUT/'baseline-verification.json',OUTPUT/'native-verification.json',
                OUTPUT/'local-preview.json',OUTPUT/'documentation-check.json',
                OUTPUT/'delivery/independent-verification.json',OUTPUT/'delivery/package.json',
                OUTPUT/'delivery/source-verification.json',
                ROOT/'docs/checklists/SDK-5-5-acceptance.json']},
            'verificationNote':'First browser attempt reached legacy22 but the verifier read an optional statuses field after expiry. The verifier now checks the actual applied legacy view; complete browser journeys rerun against unchanged native/runtime inputs. See first-browser-attempt.log.',
            'compatibilityNote':'Legacy 19/20/21/22 data and historic SDK archives remain byte-identical; no broad map content signoff.'}
    (OUTPUT/'acceptance.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('SDK-5-5: PD01-PD10 passed; 767 identical native tests, 22 Node tests, closed-loop and independent delivery verified')


if __name__=='__main__':main()
