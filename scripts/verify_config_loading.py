"""Startup map packages, immutable cache, per-match versions and old durable responsibilities."""
from contextlib import closing
import hashlib
import json
import re
import sqlite3
import subprocess
import tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_durability import Host
from verify_h5 import active, closed, info, wait
from verify_reconnect import hashes

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/config-loading'
BINARY = ROOT / 'target/debug/kernel-host'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    native = subprocess.run(['cargo','test','--locked','--all-features','config::catalog::tests','--','--nocapture'],
                            cwd=ROOT, capture_output=True, text=True, check=True)
    tests = re.findall(r'^test (config::catalog::tests::\S+) \.\.\. ok$', native.stdout, re.M)
    assert len(tests) == 93, native.stdout
    measure = json.loads(re.search(r'CONFIG_CACHE_MEASURE=(\{[^\n]+\})', native.stdout)[1])
    subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True)
    binary_hash = digest(BINARY)
    results = {}
    errors = []
    with tempfile.TemporaryDirectory(prefix='rust-config-') as directory, sync_playwright() as pw:
        package = Path(directory)/'config'; package.mkdir()
        for source_file, alias in [('b1.json', 'arena'), ('b3-1.json', 'outpost')]:
            raw = json.loads((ROOT/'config'/source_file).read_text())
            raw['id'] = f'consumer-{alias}-v1'
            (package/f'{alias}.json').write_text(json.dumps(raw))
        outpost = package/'outpost.json'
        manifest = package/'catalog.json'
        manifest.write_text(json.dumps({'schema_version':1,'default_scenario':'outpost',
                                        'active':{'arena':'arena.json','outpost':'outpost.json'},'archives':[]}))
        host = Host(directory, manifest)
        browser = pw.chromium.launch(headless=True)
        context = browser.new_context(viewport={'width':1440,'height':1100})
        page = context.new_page(); page.on('pageerror',lambda error:errors.append(str(error)))
        def config(session=None):
            response = page.request.get(host.url+'/api/config'+('?sessionId='+session if session else ''))
            assert response.ok,response.text()
            return response.json()
        def frozen(value):
            return next(request for request in value['io']['requests'] if request['body']['kind']=='commit_settlement')
        try:
            host.start(); page.goto(host.url); original = config()
            catalog = page.request.get(host.url+'/api/catalog').json()
            assert catalog['defaultScenario'] == original['scenario'] == 'outpost'
            assert {entry['scenario'] for entry in catalog['profiles']} == {'arena', 'outpost'}
            assert original['configId'] == 'consumer-outpost-v1'
            assert subprocess.check_output([str(BINARY),'--configs',str(manifest),'--config-hash'],
                                           cwd=ROOT,text=True).strip() == original['configHash']
            page.wait_for_function("document.querySelector('#scenario').value === 'outpost'")
            # Edit the source after service startup. Even a newly spawned Rust child must retain startup bytes.
            raw = json.loads(outpost.read_text())
            raw['pve']['map_id'] = 'edited-forest'
            raw['spawn'] = raw['pve']['monster_positions'][0]
            raw['pve']['palette'][0]['speedMultiplier'] = .95
            outpost.write_text(json.dumps(raw))
            assert config()['configHash'] == original['configHash']
            page.locator('#fault').select_option('refused'); page.locator('#verify').click()
            page.locator('#reconcile').wait_for(state='visible',timeout=20000)
            before = info(page,host.url); operation = frozen(before)
            assert before['configHash'] == original['configHash'] and before['ledger']['manual']==1
            assert config(before['id'])['spawn']==original['spawn']
            assert before['snapshot']['pve']['kills']==3
            results['consumer_catalog_runs_without_fixture_aliases'] = {'defaultScenario':'outpost',
                'configId':original['configId'],'configHash':original['configHash'],'kills':3}
            results['file_edit_does_not_change_running_service_or_new_child'] = {'configHash':before['configHash'],'kills':3}
            host.kill(); page.goto('about:blank'); host.start(); page.goto(host.url)
            page.locator('#reconcile').wait_for(state='visible',timeout=20000)
            current = config(); restored = info(page,host.url); pinned = config(restored['id'])
            assert current['configHash'] != original['configHash'] and current['pve']['map_id']=='edited-forest'
            assert current['spawn']==raw['spawn']
            assert pinned['configHash']==original['configHash'] and pinned['pve']['map_id']==original['pve']['map_id']
            assert restored['configHash']==original['configHash'] and frozen(restored)==operation
            assert restored['ledger']['warehouse']==95 and restored['ledger']['manual']==1
            assert page.locator('#world-live').inner_text()=='0'
            assert page.locator('#arena').get_attribute('data-config-hash')==original['configHash']
            page.screenshot(path=str(OUTPUT/'old-version-pending.png'),full_page=True)
            results['restart_loads_changed_map_without_rebuild'] = {'oldHash':original['configHash'],'newHash':current['configHash'],'binarySha256':binary_hash}
            results['old_pending_match_uses_archived_map_and_frozen_settlement'] = {'configHash':pinned['configHash'],'operation':operation,'ledger':restored['ledger']}
            outsider = browser.new_context()
            outsider.request.get(host.url+'/')
            assert outsider.request.get(host.url+'/api/config?sessionId='+restored['id']).status==404
            outsider.close(); results['pinned_config_is_owner_scoped']=True
            page.locator('#reconcile').click(); closed(page)
            settled = wait(lambda:v if (v:=info(page,host.url))['ledger']['settlements']==1 and not v['ledger']['pending'] else None)
            expected = settled['ledger']
            assert expected['warehouse']==95+operation['body']['kept'] and expected['cultivation']==45
            for _ in range(2):
                host.kill(); page.goto('about:blank'); host.start(); page.goto(host.url); closed(page)
                assert info(page,host.url)['ledger']==expected
                assert config(settled['id'])['configHash']==original['configHash']
            results['old_version_reconciliation_and_restarts_commit_once']={'ledger':expected}
            # Keep the old result current in this owner's DB; another owner starts a real new-map match.
            other = browser.new_context(); other_page=other.new_page(); other_page.goto(host.url)
            other_page.locator('#start').click(); active(other_page)
            new = info(other_page,host.url)
            assert new['configHash']==current['configHash'] and new['snapshot']['pve']
            assert new['snapshot']['position']==raw['spawn']
            assert other_page.locator('#arena').get_attribute('data-config-hash')==current['configHash']
            other_page.locator('#close').click(); closed(other_page); other.close()
            results['new_match_binds_new_hash']={'configHash':new['configHash'],'mapId':'edited-forest'}
            page.screenshot(path=str(OUTPUT/'old-version-restored.png'),full_page=True)
            host.stop()
            # Bad startup paths must not listen, reset balances, or reinterpret old requests.
            with closing(sqlite3.connect(host.path)) as db:
                stores = db.execute('SELECT session_id,state,digest FROM stores ORDER BY session_id').fetchall()
            def rejects(name, database, code):
                result=subprocess.run(['node','adapters/h5/server.mjs','--port','0','--state',str(database),'--configs',str(manifest)],
                                      cwd=ROOT,capture_output=True,text=True,timeout=20)
                assert result.returncode and '"url"' not in result.stdout and code in result.stderr,(result.stdout,result.stderr)
                with closing(sqlite3.connect(database)) as db:
                    assert db.execute('SELECT session_id,state,digest FROM stores ORDER BY session_id').fetchall()==stores
                results[name]={'error':code,'storedAssetsUnchanged':True}
            valid_raw=outpost.read_text()
            bad=json.loads(valid_raw);bad['pve']['cells_rle']=[[0,1]]
            outpost.write_text(json.dumps(bad))
            rejects('invalid_map_refuses_before_listening',host.path,'CONFIG_RLE_COUNT')
            outpost.write_text(valid_raw)
            for name,sql,code in [
                ('missing_old_version_refuses_recovery','DELETE FROM config_versions WHERE hash=?','CONFIG_VERSION_MISSING'),
                ('damaged_old_version_refuses_recovery',"UPDATE config_versions SET raw=raw||' ' WHERE hash=?",'CONFIG_VERSION_CORRUPT')]:
                database=Path(directory)/(name+'.sqlite')
                with closing(sqlite3.connect(host.path)) as source, closing(sqlite3.connect(database)) as destination:
                    source.backup(destination);destination.execute(sql,(original['configHash'],));destination.commit()
                rejects(name,database,code)
        finally:
            context.close();browser.close();host.stop();host.error_log.close()
    assert digest(BINARY)==binary_hash,'binary changed during external-map update'
    assert sources==hashes(),'sources changed during config verification'
    assert not errors,errors
    (OUTPUT/'verification.json').write_text(json.dumps({'batch':'startup-config-loading','rustTests':tests,'cacheMeasurement':measure,
        'results':results,'pageErrors':errors,'binarySha256':binary_hash,'sourceSha256':sources},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'rustTests':len(tests),'processAndBrowserChecks':len(results),'cacheMeasurement':measure}),flush=True)

if __name__=='__main__':main()
