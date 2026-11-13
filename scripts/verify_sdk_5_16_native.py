"""Native candidate checks, preserving the original test set."""
import json,re,subprocess
from pathlib import Path
from verify_sdk_5_16 import ROOT,OUTPUT,sources
from package_sdk import digest

def names(p):return set(re.findall(r'^test (\S+) \.\.\. ok$',p.read_text(),re.M))
def main():
    OUTPUT.mkdir(parents=True,exist_ok=True);frozen=sources()
    initial=Path('/tmp/sdk516-default-first.log');text=initial.read_text();assert 'test result: FAILED' not in text and len(names(initial))==1146 and 'Doc-tests match_kernel' in text
    (OUTPUT/'default.log').write_text(text)
    commands={'all-features':['cargo','test','--locked','--offline','--all-features'],'clippy':['cargo','clippy','--locked','--offline','--all-targets','--all-features','--','-D','warnings'],'node-storage':['node','--test','adapters/h5/persistence.test.mjs','adapters/h5/flow-control.test.mjs','adapters/h5/isometric.test.mjs','adapters/node/asset-store.test.mjs'],'build':['cargo','build','--locked','--offline','--features','replay','--bin','kernel-host']}
    for name,cmd in commands.items():
        p=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True,timeout=900);(OUTPUT/(name+'.log')).write_text(p.stdout+p.stderr);assert p.returncode==0,(name,p.stderr[-3000:],p.stdout[-1500:]);print(name+': passed',flush=True)
    old=names(ROOT/'artifacts/sdk-5-15/default.log');a=names(OUTPUT/'default.log');b=names(OUTPUT/'all-features.log');assert len(old)==1115 and old<=a and a==b and len(a-old)==31;assert '# tests 22' in (OUTPUT/'node-storage.log').read_text();assert frozen==sources()
    (OUTPUT/'native-verification.json').write_text(json.dumps({'passed':True,'sourceHashes':frozen,'checks':{n:digest(OUTPUT/(n+'.log')) for n in ['default',*commands]},'testCount':len(a),'newTests':sorted(a-old),'oldRetained':len(old),'nodeTests':22},indent=2)+'\n');print('SDK-5-16 native: 1146 tests, 31 added, 1115 retained, Clippy and 22 Node passed',flush=True)
if __name__=='__main__':main()
