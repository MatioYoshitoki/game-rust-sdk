import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';

const key = (id,hash) => JSON.stringify([id,hash]);
const describe = (binary,manifest) => JSON.parse(execFileSync(binary,['--configs',manifest,'--catalog'], {encoding:'utf8',maxBuffer:134217728,timeout:15000}));
export function readSourceCatalog(binary,manifest) {return describe(binary,manifest);}
export function snapshotCatalog(database,binary,source) {
  // Keep exact old bytes for responsibility recovery, independent of later source-file edits.
  database.rememberConfigs(source.versions);
  const sourceVersions=new Map(source.versions.map(version=>[key(version.id,version.hash),version]));
  const versions = new Map(database.configs().map(version => [key(version.id,version.hash), {...version,config:JSON.parse(version.raw)}]));
  for (const create of database.requiredConfigs()) if (!versions.has(key(create.config_id,create.config_hash))) throw new Error('CONFIG_VERSION_MISSING');
  const catalog = new Map(source.active.map(entry => [entry.scenario,versions.get(key(entry.id,entry.hash))]));
  const root=join(dirname(database.path),`.catalog-${createHash('sha256').update(database.path).digest('hex').slice(0,16)}`);
  const marker=join(root,'OWNER');
  if (existsSync(root)) {
    if (!existsSync(marker) || readFileSync(marker,'utf8') !== 'match-kernel-catalog-v1') throw new Error('CONFIG_CACHE_OWNERSHIP');
    rmSync(root,{recursive:true});
  }
  mkdirSync(root,{mode:0o700});writeFileSync(marker,'match-kernel-catalog-v1',{mode:0o600});
  try {
    for (const version of versions.values()) {
      writeFileSync(join(root,version.hash+'.json'),version.raw,{mode:0o600});
      // A match worker needs its one immutable version, including an archived version on recovery.
      version.manifest=join(root,'match-'+version.hash+'.json');
      writeFileSync(version.manifest,JSON.stringify({schema_version:1,default_scenario:'match',
        active:{match:version.hash+'.json'},archives:[]}),{mode:0o600});
    }
    const active=Object.fromEntries(source.active.map(entry => [entry.scenario,entry.hash+'.json']));
    const activeHashes=new Set(source.active.map(entry=>entry.hash));
    const archives=[...versions.values()].filter(version=>!activeHashes.has(version.hash)).map(version=>version.hash+'.json');
    const manifest=join(root,'catalog.json');writeFileSync(manifest,JSON.stringify({schema_version:1,default_scenario:source.defaultScenario,active,archives}),{mode:0o600});
    // The source catalog has already been validated by Rust. Reuse its normalized
    // configs; only persisted versions absent from that catalog need another load.
    for(const [identity,version] of sourceVersions) versions.get(identity).config=version.config;
    const extra=[...versions.values()].filter(version=>!sourceVersions.has(key(version.id,version.hash)));
    const stats={...source.stats,parsedVersions:versions.size};
    if(extra.length) {
      const stored=join(root,'stored-catalog.json');
      writeFileSync(stored,JSON.stringify({schema_version:1,default_scenario:'stored',
        active:{stored:extra[0].hash+'.json'},archives:extra.slice(1).map(version=>version.hash+'.json')}),{mode:0o600});
      const validated=describe(binary,stored);
      for(const version of validated.versions) versions.get(key(version.id,version.hash)).config=version.config;
      stats.preparedMaps+=validated.stats.preparedMaps;stats.loadUs+=validated.stats.loadUs;
    }
    return {catalog,versions,manifest,defaultScenario:source.defaultScenario,stats,find:(id,hash)=>versions.get(key(id,hash)),dispose(){rmSync(root,{recursive:true,force:true});}};
  }catch(error){rmSync(root,{recursive:true,force:true});throw error;}
}
