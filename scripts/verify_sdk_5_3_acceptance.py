"""Aggregate AT01-AT10 using immutable logs, archive and current candidate hashes."""
from pathlib import Path
import json
import re

from verify_sdk_5_3 import ROOT, OUTPUT, sources
from package_sdk import digest


def load(path):return json.loads(path.read_text())


def tests(path):return set(re.findall(r'^test (\S+) \.\.\. ok$',path.read_text(),re.M))


def main():
    integration=load(OUTPUT/'verification.json')
    delivery=load(OUTPUT/'delivery/independent-verification.json')
    package=load(OUTPUT/'delivery/package.json')
    source=load(OUTPUT/'delivery/source-verification.json')
    baseline=load(OUTPUT/'baseline-verification.json')
    mapping=load(ROOT/'docs/checklists/SDK-5-3-acceptance.json')
    assert all(v['passed'] for v in [integration,delivery,baseline,source])
    assert sources()==integration['sourceHashes']
    assert digest(ROOT/'target/debug/kernel-host')==integration['binarySha256']
    core={p:h for p,h in integration['sourceHashes'].items() if p.startswith('src/') or p in ['Cargo.toml','Cargo.lock']}
    assert all(delivery['rustSourceHashes'][p]==h for p,h in core.items() if p.startswith('src/'))
    for name in ['default','all-features','clippy','node-storage']:
        assert digest(OUTPUT/(name+'.log'))==integration['checks'][name]
    a=tests(OUTPUT/'default.log');b=tests(OUTPUT/'all-features.log')
    old=set().union(*(tests(ROOT/p) for p in baseline['baselineSources']))
    assert a==b and len(a)==708 and len(old)==686 and old<=a
    assert source['testCount']==708 and source['sameWorkspaceDefaultAndAllFeatureSet']
    assert source['archiveSha256']==package['sha256'] and source['emptyCargoHome'] and source['offline']
    for name,h in source['checks'].items():assert digest(OUTPUT/('delivery/source-'+name+'.log'))==h
    assert len(a-old)==22 and sorted(a-old)==baseline['added']
    assert '# tests 22' in (OUTPUT/'node-storage.log').read_text()
    for p,h in baseline['protectedHashes'].items():assert digest(ROOT/p)==h,p
    manifest=ROOT/'rust-match-kernel-v1.1/manifest.json'
    assert digest(manifest)==baseline['sourcePackageManifestHash']
    assert digest(Path(package['archive']))==package['sha256']==delivery['archiveSha256']
    assert len(integration['legacy'])==4
    assert len(integration['cases'])==4 and len(integration['touchCases'])==2 and len(integration['crashes'])==3
    for family in ['cases','touchCases','legacy']:
        for value in integration[family].values():
            assert value['closed']['live_worlds']==0 and not value['ledger']['pending']
            assert value['ledger']['admissions']==value['ledger']['settlements']==1
    for value in integration['crashes'].values():
        assert value['multiLayerBeforeCrash'][0]['stacks']==2
        assert len(value['recoveries'])==2 and value['recoveries'][0]==value['recoveries'][1]
        assert value['recoveries'][0]['closed']['live_worlds']==0
    assert len(delivery['nodeCases'])==6 and len(delivery['rustConsumer'])==6 and len(delivery['protocol2And3'])==14
    assert delivery['emptyCargoHome'] and delivery['offline'] and delivery['removedSdkRegressionsBeforeConsumption']
    criteria=mapping['criteria']
    assert set(criteria)=={f'AT{i:02}' for i in range(1,11)}
    for value in criteria.values():
        assert set(value.get('rustTests',[]))<=a
        for p in value.get('reports',[]):assert load(ROOT/p)['passed']
    preview=load(OUTPUT/'local-preview.json');assert preview['passed'] and not preview['createdMatch']
    report={'passed':True,'date':'2026-11-04','crate':'0.6.0','mechanismVersion':21,
            'scope':'M03 finite ATK/DEF selectors and per-plan capture; not full original requirements',
            'rustTests':len(a),'newRustTests':22,'retainedOriginalRustTests':len(old),
            'sameDefaultAndAllFeatures':True,'clippy':'passed with -D warnings','nodeTests':22,
            'ordinaryNodeCases':4,'touchCases':2,'legacy19And20Cases':4,'crashBoundaries':3,'recoveriesPerBoundary':2,
            'independentNodeCases':6,'legacyRustModes':6,'protocol2And3Cases':14,
            'protocol2And3Restarts':14,'reconciliations':2,'unpackedSourceTests':708,
            'verificationScripts':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'scripts').glob('verify_sdk_5_3*.py'))},
            'criteria':{k:{'status':'passed',**v} for k,v in criteria.items()},
            'sourceHashes':integration['sourceHashes'],'nativeCoreSourceHashes':core,
            'protectedHashes':baseline['protectedHashes'],
            'archiveSha256':package['sha256'],'sourceCrateSha256':package['sourceCrateSha256'],
            'binarySha256':{'replayEnabledAcceptance':integration['binarySha256'],'ordinaryDelivery':package['binarySha256']},
            'reports':{str(p.relative_to(ROOT)):digest(p) for p in [OUTPUT/'verification.json',
                OUTPUT/'baseline-verification.json',
                OUTPUT/'delivery/independent-verification.json',OUTPUT/'delivery/package.json',
                OUTPUT/'delivery/source-verification.json',
                ROOT/'docs/checklists/SDK-5-3-acceptance.json']},
            'compatibilityNote':'Legacy 19/20 data and three historic SDK archives remain byte-identical; no broad map content signoff.'}
    (OUTPUT/'acceptance.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('SDK-5-3: AT01-AT10 passed; 708 identical native tests, 22 Node tests, closed-loop and independent delivery verified')


if __name__=='__main__':main()
