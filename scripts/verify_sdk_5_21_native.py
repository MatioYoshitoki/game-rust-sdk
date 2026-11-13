"""Retain the full SDK-5-20 test sets and verify the current capacity implementation."""
import hashlib,json,os,re,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'artifacts/sdk-5-21'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def names(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    paths=[*ROOT.glob('src/**/*.rs'),ROOT/'Cargo.toml',ROOT/'Cargo.lock'];before={str(p.relative_to(ROOT)):digest(p) for p in paths}
    for label,cmd in [('default',['cargo','test','--locked']),('all-features',['cargo','test','--locked','--all-features']),('clippy',['cargo','clippy','--locked','--all-targets','--all-features','--','-D','warnings']),('fmt',['cargo','fmt','--check']),('node-tests',['node','--test',*[str(p.relative_to(ROOT)) for p in sorted((ROOT/'adapters').glob('*/*.test.mjs'))]])]:
        with (OUT/(label+'.log')).open('w') as log:p=subprocess.run(cmd,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,timeout=1200,env={**os.environ,'RUST_TEST_THREADS':'16',**({'SDK521_INTEROP_OUTPUT':str(OUT/('interop-'+label+'.json'))} if label in ['default','all-features'] else {})})
        assert p.returncode==0,(label,p.returncode);print(label+': passed',flush=True)
    actual=names(OUT/'default.log');previous=names(ROOT/'artifacts/sdk-5-20/default.log');assert len(previous)==1240 and previous<=actual and actual==names(OUT/'all-features.log');assert len(actual-previous)==35
    nodes=set(re.findall(r'^# Subtest: (.+)$',(OUT/'node-tests.log').read_text(),re.M));old=set(re.findall(r'^# Subtest: (.+)$',(ROOT/'artifacts/sdk-5-20/node-tests.log').read_text(),re.M));assert old<=nodes and len(nodes)==48
    assert digest(OUT/'interop-default.json')==digest(OUT/'interop-all-features.json')
    subprocess.run(['node','scripts/verify_sdk_5_21_interop.mjs'],cwd=ROOT,check=True)
    assert all(digest(ROOT/p)==h for p,h in before.items())
    report={'passed':True,'crate':'0.23.0','rustTests':len(actual),'retainedRustTests':1240,'newTests':sorted(actual-previous),'sameDefaultAndAllFeatures':True,'nodeTests':len(nodes),'retainedNodeTests':len(old),'sourceHashes':before,'logs':{n:digest(OUT/(n+'.log')) for n in ['default','all-features','clippy','fmt','node-tests']}}
    (OUT/'native-verification.json').write_text(json.dumps(report,indent=2)+'\n')
if __name__=='__main__':main()
