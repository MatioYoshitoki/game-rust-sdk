// Ordinary consumer: input intentions and actual Rust publication/periodic facts only.
import {readFileSync,realpathSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
import {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog,delay,requireHostOption} from './two-member-host.mjs';
export {TwoMemberDatabase,TwoMemberHost,newRecord,readCatalog};
import {runPeriodicJourney} from './multiplayer-periodic-host.mjs';
export const runCombatJourney=(host,options={})=>runPeriodicJourney(host,{...options,shieldValidation:true,unitValidation:true});
async function main(){const flag=(n,f)=>{const i=process.argv.indexOf(n);return i<0?f:process.argv[i+1]},binary=resolve(flag('--binary','target/debug/kernel-host'));const resume=process.argv.includes('--resume');
  let manifest=resume?undefined:resolve(requireHostOption(flag('--configs',process.env.MATCH_CONFIG_MANIFEST),'--configs'));
  const accounts=resume?undefined:JSON.parse(readFileSync(requireHostOption(flag('--accounts'),'--accounts'),'utf8'));const database=new TwoMemberDatabase(flag('--database','.local/combat-members-node.sqlite'));let record,recovery;
 if(resume){record=database.room(flag('--resume'));const path=flag('--frozen-checkpoint');recovery=database.recovery(record.id,path?JSON.parse(readFileSync(path,'utf8')):null);manifest=database.frozenManifest(record.id);}
 else{
    const catalog=readCatalog(binary,manifest);
    record=newRecord({catalog,scenario:flag('--scenario',catalog.defaultScenario),accounts,manifest});
    database.create(record);
  }
 const host=new TwoMemberHost({binary,manifest,database,record,autoAck:true});try{await host.start(recovery);let result;if(recovery){await host.until(()=>host.pending()===0);result={phase:host.phase,pending:host.pending(),accounts:database.views(record.id),generation:recovery.generation,replication:host.replicationStats,periodic:host.members.map(m=>m.periodic),shields:host.members.map(m=>m.shields),combat:host.members.map(m=>m.combat)};}else result=await runCombatJourney(host,{reverse:process.argv.includes('--reverse')});console.log(JSON.stringify({roomId:record.id,...result}));}finally{await host.dispose();database.close();}}
if(process.argv[1]&&import.meta.url===pathToFileURL(realpathSync(process.argv[1])).href)main().catch(e=>{console.error(e.stack);process.exitCode=1;});
