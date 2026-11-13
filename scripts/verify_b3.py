"""B3-1 source terrain/monster slice through real Rust, browser inputs and durable restart."""
import json,re,subprocess,tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from verify_h5 import active,closed,info,wait
from verify_reconnect import hashes
from verify_durability import Host
ROOT=Path(__file__).resolve().parents[1]
OUTPUT=ROOT/'artifacts/b3-1'

def main():
 OUTPUT.mkdir(parents=True,exist_ok=True);sources=hashes();results={};errors=[]
 subprocess.run(['python3','scripts/export_b3.py','--check'],cwd=ROOT,check=True)
 native=subprocess.run(['cargo','test','--locked','--all-features','world::pve::tests'],cwd=ROOT,capture_output=True,text=True,check=True)
 tests=re.findall(r'^test (world::pve::tests::\S+) \.\.\. ok$',native.stdout,re.M);assert len(tests)==17
 subprocess.run(['cargo','build','--locked','--features','replay','--bin','kernel-host'],cwd=ROOT,check=True)
 with sync_playwright() as pw:
  browser=pw.chromium.launch(headless=True)
  def automatic(page):
   page.locator('#scenario').select_option('b3-1');page.locator('#verify').click()
  def terminal(page):
   closed(page);page.wait_for_function("document.body.dataset.verification==='passed'",timeout=20000)
  def settled(page,url):
   closed(page)
   return wait(lambda:value if (value:=info(page,url))['settlement'] and not value['ledger']['pending'] else None)
  def frozen(value):
   return next(request for request in value['io']['requests'] if request['body']['kind']=='commit_settlement')
  def check_assets(value,growth=None):
   body=frozen(value)['body'];ledger=value['ledger'];assert ledger['warehouse']==95+body['kept'],value
   assert ledger['items']=={item['item_id']:item['kept'] for item in body.get('items',[])},value
   assert body['carried']+body['looted']==body['kept']+body['dropped']
   for item in body.get('items',[]):assert item['looted']==item['kept']+item['dropped']
   assert ledger['admissions']==ledger['settlements']==1 and ledger['pending']==[] and ledger['manual']==0,value
   if growth is not None:assert ledger['cultivation']==growth,value
   assert len([event for event in value['events'] if event['kind']=='settlement_applied'])==1
  def reload(host,page):
   host.start();page.goto(host.url);closed(page);return info(page,host.url)
  def scenario(name,work,point='',kind=''):
   with tempfile.TemporaryDirectory(prefix='rust-b3-') as directory:
    host=Host(directory);context=browser.new_context(viewport={'width':1440,'height':1100});page=context.new_page()
    page.on('pageerror',lambda error:errors.append(str(error)))
    try:
     host.start(point,kind);page.goto(host.url);work(host,page,context);value=info(page,host.url)
     assert value['phase']=='closed' and value['ledger']['pending']==[],value
     assert page.locator('#world-live').inner_text()=='0' and not errors,errors
     results[name]={'ledger':value['ledger'],'events':value['events'],'requests':value['io']['requests'],'scenario':value['scenario'],'snapshot':value['snapshot']}
     print(name+': passed',flush=True)
    except Exception:
     page.screenshot(path=str(OUTPUT/'failure.png'),full_page=True);host.error_log.seek(0);print(host.error_log.read(),flush=True);raise
    finally:context.close();host.stop();host.error_log.close()
  def extraction(host,page,context):
   automatic(page);terminal(page);value=info(page,host.url);check_assets(value,45)
   assert value['snapshot']['pve']['kills']==3 and value['snapshot']['resources']==[]
   assert page.locator('#progress-count').inner_text()=='5 / 5'
   page.screenshot(path=str(OUTPUT/'extraction.png'),full_page=True)
   original=value['ledger'];operation=frozen(value)
   for _ in range(2):
    host.kill();page.goto('about:blank');value=reload(host,page);assert value['ledger']==original and frozen(value)==operation
    assert value['scenario']=='b3-1' and page.locator('#scenario').input_value()=='b3-1';check_assets(value,45)
   page.screenshot(path=str(OUTPUT/'restored.png'),full_page=True)
  scenario('three_wolf_extraction_and_repeated_result_restart',extraction)
  def death(host,page,context):
   automatic(page);wait(lambda:(value:=info(page,host.url)) and value['snapshot'] and value['snapshot']['pve']['kills']==3 and not value['snapshot']['resources'])
   page.locator('#close').click();value=settled(page,host.url);check_assets(value,11)
   assert frozen(value)['body']['outcome']=='death';page.screenshot(path=str(OUTPUT/'death.png'),full_page=True)
  scenario('multi_item_death_conserves_each_item',death)
  def refresh(host,page,context):
   page.locator('#scenario').select_option('b3-1');page.locator('#start').click();active(page)
   page.locator('#attack').click();wait(lambda:info(page,host.url)['snapshot']['pve']['kills']==1)
   before=info(page,host.url);page.reload();active(page);after=info(page,host.url)
   assert before['id']==after['id'] and after['scenario']=='b3-1'
   assert after['connection']['generation']>before['connection']['generation']
   assert after['snapshot']['pve']['kills']==1 and after['ledger']['admissions']==1
   assert before['snapshot']['basic_ready_at_ms']==after['snapshot']['basic_ready_at_ms']
   page.screenshot(path=str(OUTPUT/'exploration.png'),full_page=True)
   page.locator('#close').click();check_assets(settled(page,host.url),3)
  scenario('refresh_retains_multiple_monsters_and_cooldown',refresh)
  for point in ['before_storage_commit','after_storage_commit']:
   def crash(host,page,context,point=point):
    automatic(page);host.killed();page.goto('about:blank');state=host.state()
    operation=next(request for _,request in state['requests'] if request['body']['kind']=='commit_settlement')
    value=reload(host,page);check_assets(value,45)
    recovered=frozen(value);assert recovered['operation_id']==operation['operation_id'] and recovered['body']==operation['body']
    assert state['settlements']==int(point=='after_storage_commit')
   scenario('frozen_pve_settlement_'+point,crash,point,'commit_settlement')
  def manual(host,page,context):
   page.locator('#fault').select_option('refused');automatic(page);page.locator('#reconcile').wait_for(state='visible',timeout=20000)
   before=info(page,host.url);operation=frozen(before);assert before['ledger']['warehouse']==95
   host.kill();page.goto('about:blank');host.start();page.goto(host.url);page.locator('#reconcile').wait_for(state='visible')
   value=info(page,host.url);assert value['phase']=='closed' and value['ledger']['manual']==1 and frozen(value)==operation
   page.locator('#reconcile').click();value=settled(page,host.url);check_assets(value,45);assert value['ledger']['reconciliations']==1
   host.kill();page.goto('about:blank');value=reload(host,page);check_assets(value,45)
  scenario('manual_multi_item_obligation_survives_restart',manual)
  def interrupted(host,page,context):
   page.locator('#scenario').select_option('b3-1');page.locator('#start').click();active(page);host.kill();page.goto('about:blank')
   value=reload(host,page);assert value['ledger']['warehouse']==100 and value['ledger']['items']=={} and value['ledger']['compensations']==1 and value['ledger']['entryYears']==0 and value['ledger']['settlements']==0
  scenario('unfrozen_pve_interrupt_refunds_only_original_entry',interrupted)
  browser.close()
 assert sources==hashes(),'Sources changed during B3 verification'
 (OUTPUT/'verification.json').write_text(json.dumps({'batch':'B3-1','rustTests':tests,'results':results,'pageErrors':errors,'sourceSha256':sources},ensure_ascii=False,indent=2)+'\n')
 print(json.dumps({'rustTests':len(tests),'browserAndRestartChecks':len(results)}),flush=True)
if __name__=='__main__':main()
