"""Consume the exact archived candidate outside the checkout, with ordinary clocks."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'artifacts/sdk-5-88/delivery'

def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def run(name,command,cwd,env=None):
    result=subprocess.run(command,cwd=cwd,env={**os.environ,**(env or {})},capture_output=True,text=True,timeout=900)
    (OUT/(name+'.log')).write_text(result.stdout+result.stderr)
    if result.returncode: raise RuntimeError(name+': '+(result.stdout+result.stderr)[-2500:])
    print(name+': passed',flush=True)

def main():
    package=json.loads((OUT/'package.json').read_text())
    archive=Path(package['archive'])
    assert sha(archive)==package['sha256']
    with tempfile.TemporaryDirectory(prefix='match-sdk-5-88-independent-') as directory:
        directory=Path(directory)
        with tarfile.open(archive) as tar:
            tar.extractall(directory,filter='data')
        root=directory/package['rootName']
        binary=root/'bin/kernel-host'
        assert sha(binary)==package['binarySha256']
        source_files=[p for p in (ROOT/'src').rglob('*') if p.is_file() and p.suffix in ['.rs','.json']]
        source_files += list((ROOT/'examples/combat-host').glob('*.json'))+[ROOT/'SDK.md',ROOT/'web/replication.mjs',ROOT/'web/skill-hp-cost.js']
        for source in source_files:
            packaged=(root/source.relative_to(ROOT)) if source.parent==ROOT/'web' else (root/'sdk'/source.relative_to(ROOT))
            assert packaged.is_file() and sha(packaged)==sha(source),str(source.relative_to(ROOT))
        source_identity=hashlib.sha256(json.dumps({str(p.relative_to(ROOT)):sha(p) for p in sorted(source_files)},sort_keys=True).encode()).hexdigest()

        assert sha(root/'web/skill-slots.mjs')==sha(ROOT/'web/skill-slots.mjs')
        home=directory/'empty-cargo-home';home.mkdir()
        run('independent-bundle-check',[sys.executable,str(root/'verify_bundle.py')],root,
            {'CARGO_HOME':str(home),'CARGO_NET_OFFLINE':'true','CARGO_TARGET_DIR':str(directory/'target')})
        for feature,script in [('combat','verify_sdk_5_88_public.mjs'),('member-skill-power','verify_sdk_5_87_public.mjs')]:
            run('independent-'+feature,['node',str(ROOT/'scripts'/script),str(binary),
                str(root/'hosts/node/two-member-host.mjs'),str(root/'samples'/feature),
                str(OUT/('independent-'+feature))],directory)
        run('independent-browser',[sys.executable,str(ROOT/'scripts/verify_sdk_5_88_browser.py'),
            '--binary',str(binary),'--server',str(root/'hosts/h5/skill-hp-cost-server.mjs'),
            '--configs',str(root/'samples/combat'),'--cwd',str(root),'--out',str(OUT/'independent-browser'),'--port','4362'],directory)
        report={'passed':True,'archiveSha256':sha(archive),'binarySha256':sha(binary),
                'ordinaryClock':True,'offlineEmptyCache':True,'sourceFilesCompared':len(source_files),'runtimeSourceSha256':source_identity,
                'combat':json.loads((OUT/'independent-combat/report.json').read_text()),
                'legacy104':json.loads((OUT/'independent-member-skill-power/report.json').read_text()),
                'browser':json.loads((OUT/'independent-browser/report.json').read_text())}
        (OUT/'independent-verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')

if __name__=='__main__':main()
