"""Run public consumers against the one extracted ordinary release candidate."""
import concurrent.futures,json,subprocess,time
from pathlib import Path
from package_sdk import ROOT,digest,extract_crate
OUT=ROOT/'artifacts/sdk-5-22'
def run(label,command):
    with (OUT/(label+'-driver.log')).open('w') as log:
        p=subprocess.run(command,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,timeout=600)
    assert p.returncode==0,(label,p.returncode);print(label+': passed',flush=True)
def main():
    deadline=time.monotonic()+1200
    while not (OUT/'delivery/package.json').exists():
        assert time.monotonic()<deadline,'package not completed';time.sleep(1)
    pkg=json.loads((OUT/'delivery/package.json').read_text());assert digest(pkg['archive'])==pkg['sha256']
    candidate=OUT/'candidate';candidate.mkdir(exist_ok=True);extract_crate(pkg['archive'],candidate,pkg['rootName']);binary=str(candidate/'bin/kernel-host');assert digest(binary)==pkg['binarySha256']
    checks=[('legacy38',['node','scripts/verify_sdk_5_22_legacy38.mjs','--binary',binary]),('public',['node','scripts/verify_sdk_5_22_public.mjs','--binary',binary]),('legacy4',['node','scripts/verify_sdk_5_22_legacy4.mjs','--binary',binary]),('legacy6',['node','scripts/verify_sdk_5_22_legacy6.mjs','--binary',binary]),('legacy5',['node','scripts/verify_sdk_5_22_legacy5.mjs','--binary',binary]),('crashes',['python3','scripts/verify_sdk_5_22_crashes.py','--binary',binary])]
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:list(pool.map(lambda c:run(*c),checks))
    run('browser-recheck',['python3','scripts/verify_sdk_5_22_browser.py','--binary',binary,'--out',str(OUT/'browser-recheck')])
if __name__=='__main__':main()
