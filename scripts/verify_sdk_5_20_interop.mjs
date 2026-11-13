import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {ReplicaV6} from '../web/replication.mjs';
const flag=(n,f)=>{const i=process.argv.indexOf(n);return i<0?f:process.argv[i+1]},input=resolve(flag('--vectors','artifacts/sdk-5-20/interop-default.json')),output=resolve(flag('--out','artifacts/sdk-5-20/interop-verification.json'));
const frames=JSON.parse(readFileSync(input,'utf8')),replicas=new Map();let full=0,delta=0;
for(const row of frames){const e=row.envelope;let r=replicas.get(e.recipient_id);if(!r){r=new ReplicaV6(e.key,e.recipient_id);replicas.set(e.recipient_id,r);}assert.deepEqual(r.apply(e),row.snapshot);assert.deepEqual(r.apply(structuredClone(e)),row.snapshot);if(e.payload.kind==='full')full++;else delta++;}
assert.equal(replicas.size,8);assert.ok(full>0&&delta>0);assert.ok(frames.some(r=>r.snapshot.projectiles.length>0));assert.ok(frames.some(r=>r.snapshot.resources.some(x=>x.item_id)));assert.ok(frames.some(r=>r.snapshot.self.inventory_revision>1));
const report={passed:true,rustVectorsSha256:createHash('sha256').update(readFileSync(input)).digest('hex'),frames:frames.length,recipients:replicas.size,full,delta,exactSnapshotParity:true,duplicateFramesIdempotent:true};mkdirSync(dirname(output),{recursive:true});writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report));
