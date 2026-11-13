import sys,tempfile,json
from pathlib import Path
sys.path.insert(0,str(Path.cwd()/'scripts'))
import verify_sdk_5_14 as v
from playwright.sync_api import sync_playwright
from verify_h5 import wait
with tempfile.TemporaryDirectory(prefix='sdk-5-14-capacity-') as d,sync_playwright() as pw:
 folder=Path(d);cfg=v.package(folder/'outside');cfg['skill']['on_commit_self_statuses']=['ward'];(folder/'outside/arena.json').write_text(json.dumps(cfg));host=v.Host(folder,folder/'outside/catalog.json',folder/'outside/account.json');host.start();browser=pw.chromium.launch();j=v.PiercingJourney(browser,host,cfg)
 try:
  j.start();j.choose([400,700]);j.use(0);j.use(0);j.page.wait_for_function("!document.querySelector('#cast').disabled");before=j.current();assert len(before['snapshot']['pve']['piercing_projectiles'])==2
  with j.page.expect_response(lambda r:'/input' in r.url and r.request.method=='POST' and (r.request.post_data_json or {}).get('action',{}).get('skill_id')==cfg['skill']['id']) as result:j.page.locator('#cast').click()
  response=result.value;assert response.ok;payload=response.request.post_data_json;again=j.page.request.post(response.url,data=payload);assert again.ok and again.json()==response.json()
  rejected=wait(lambda:next((e for e in j.current()['events'] if e['kind']=='action_rejected' and e['action_id']==payload['requestId']),None));assert rejected['reason']=='CAPACITY'
  after=j.current();assert before['snapshot']['spirit']==after['snapshot']['spirit']==176;assert before['snapshot']['player_skills']==after['snapshot']['player_skills'];assert before['snapshot']['recovery_until_ms']==after['snapshot']['recovery_until_ms'];assert len(after['snapshot']['pve']['piercing_projectiles'])==2
  assert not any(e['kind']=='action_committed' and e['action_id']==payload['requestId'] for e in after['events'])
  # A full piercing flight budget must not block the self skill.
  j.use(1);assert j.current()['snapshot']['spirit']==168
  value=j.finish('death');host.stop();state,_=host.stored();v.assert_settlement(state,value,json.loads((folder/'outside/account.json').read_text()));assert not j.errors,j.errors
  report={'passed':True,'sourceHashes':v.sources(),'rejected':rejected,'before':before['snapshot'],'after':after['snapshot'],'selfSkillStillUsable':True,'closed':v.assert_closed(value),'ledger':value['ledger'],'pageErrors':j.errors}
  (v.OUTPUT/'capacity-verification.json').write_text(json.dumps(report,indent=2)+'\n');print('H5 two-flight capacity: third commit rejected atomically, self still usable, death assets closed')
 finally:host.stop();j.context.close();browser.close()
