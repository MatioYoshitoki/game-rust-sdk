"""Three real SIGKILL boundaries, two recoveries from the SAME frozen checkpoint."""
import argparse
import hashlib
import json
import os
import shutil
import signal
import subprocess
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]

def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,default=ROOT/'target/debug/kernel-host');p.add_argument('--out',type=Path,default=ROOT/'artifacts/sdk-5-28/crashes');a=p.parse_args();out=a.out.resolve();out.mkdir(parents=True,exist_ok=True);cases=[]
    for boundary in ['partial_admissions','last_terminal_pending','committed_lost_receipt']:
        folder=out/boundary;folder.mkdir(exist_ok=True);config=folder/'external-profile';shutil.copytree(ROOT/'examples/skill-hp-cost-host',config,dirs_exist_ok=True)
        database=folder/'account.sqlite';checkpoint=folder/'original-checkpoint.json'
        cmd=['node','adapters/node/skill-hp-cost-crash-fixture.mjs','--binary',str(a.binary.resolve()),'--configs',str(config/'catalog.json'),'--accounts',str(config/'accounts-b.json'),'--database',str(database),'--boundary',boundary,'--checkpoint',str(checkpoint)]
        process=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True,timeout=25)
        (folder/'crash.stdout').write_text(process.stdout);(folder/'crash.stderr').write_text(process.stderr)
        assert process.returncode==-signal.SIGKILL,(boundary,process.returncode,process.stderr)
        marker=json.loads(process.stdout.strip().splitlines()[-1]);assert marker['boundary']==boundary;assert boundary=='partial_admissions' or (marker['replication']['delta']>0 and marker['startedWithActivePeriodic'] and marker['startedWithActiveShields'] and marker['startedWithSlots'] and marker['startedWithHpCost'])
        # The old kernel's stdin closes when its owning Node process is killed. Wait
        # for that exact process before using local-process fencing for missing writes.
        deadline=time.monotonic()+5
        while True:
            try:os.kill(marker['kernelPid'],0)
            except ProcessLookupError:break
            assert time.monotonic()<deadline,'old kernel still alive';time.sleep(.05)
        original=json.loads(checkpoint.read_text());original_hash=hashlib.sha256(checkpoint.read_bytes()).hexdigest()
        # External tuning changes cannot alter already frozen liabilities.
        document=json.loads((config/'arena-b.json').read_text());document['params']['entry_years']=9;document['carried_quantity']=7;document['skill']['cost']=29;(config/'arena-b.json').write_text(json.dumps(document,indent=2)+'\n')
        recoveries=[]
        for generation in [1,2]:
            command=['node','adapters/node/skill-hp-cost-host.mjs','--binary',str(a.binary.resolve()),'--database',str(database),'--resume',marker['roomId'],'--frozen-checkpoint',str(checkpoint)]
            result=subprocess.run(command,cwd=ROOT,capture_output=True,text=True,timeout=25)
            (folder/f'recovery-{generation}.stdout').write_text(result.stdout);(folder/f'recovery-{generation}.stderr').write_text(result.stderr)
            assert result.returncode==0,(boundary,generation,result.stderr);report=json.loads(result.stdout.strip().splitlines()[-1]);assert report['phase']=='closed' and report['pending']==0;assert report['generation']==generation;assert report['replication']['full']==report['replication']['delta']==0;assert all(m['dotTicks']==m['hotTicks']==0 for m in report['periodic']);assert all(m['grants']==m['absorptions']==0 for m in report['shields']);assert all(m['unitCommits']==m['unitProjectiles']==0 for m in report['combat']);assert all(m['ownStarted']==m['ownContacts']==0 for m in report['flightStats']);assert all(m['total']==0 and m['commits']=={} for m in report['skillStats'])
            assert all(m=={'commits':0,'total':0,'bySkill':{}}for m in report['hpCostStats'])
            members=report['accounts']
            counts=[(v['admissions'],v['settlements'],v['compensations']) for v in members]
            if boundary=='partial_admissions':assert counts==[(1,0,1)]*3+[(0,0,0)]*5,counts
            elif boundary=='last_terminal_pending':assert counts==[(1,1,0)]*8,counts
            else:assert counts==[(1,1,0) if i==3 else (1,0,1) for i in range(8)],counts
            if recoveries:assert members==recoveries[0]['accounts'],'recovery changed committed balances'
            recoveries.append(report)
        assert hashlib.sha256(checkpoint.read_bytes()).hexdigest()==original_hash
        cases.append({'boundary':boundary,'sigkill':True,'oldKernelExited':True,'sameFrozenCheckpointSha256':original_hash,'externalValuesChanged':True,'originalConfigHash':marker['hash'],'recoveries':recoveries})
    report={'passed':True,'crate':'0.30.0','binarySha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'cases':cases};(out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'passed':True,'boundaries':3,'recoveries':6,'report':str(out/'report.json')}))

if __name__=='__main__':main()
