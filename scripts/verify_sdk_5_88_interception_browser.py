"""Verify native cast interception through the served H5 controls and transport."""

import json
from pathlib import Path
import subprocess
import time
from urllib.parse import urlsplit
from uuid import uuid4

from playwright.sync_api import Error, sync_playwright


def verify_interception(browser, request, address, scenario, seats, output):
    response = request.post('/api/rooms', data={'scenario': scenario})
    assert response.status == 201
    links = response.json()['links']
    assert len(links) == seats
    contexts = [browser.new_context(viewport={'width': 1100, 'height': 950}) for _ in links]
    pages = [c.new_page() for c in contexts]
    errors, states = [], []
    seal_ms, slow_ms, slow_bps, cost = (700, 1200, -2500, 8) if seats == 4 else (900, 1500, -2000, 12)

    def snapshot(page):
        return page.evaluate('window.__lastSnapshot')

    def member(page):
        return snapshot(page)['self']

    def monster(page, id):
        return next(n for n in snapshot(page)['monsters'] if n['entity']['id'] == id)

    def ready_at(page):
        return next(s['ready_at_ms'] for s in member(page)['player_skills'] if s['skill_id'] == 'ordinary')

    def state(index, step):
        states.append({'pid': f'P{index:02}', 'step': step, 'snapshot': snapshot(pages[index])})

    def status(unit, id):
        return next((s for s in unit.get('statuses', []) if s['definition_id'] == id), None)

    def own_status(page, id):
        return status(member(page), id)

    def events(page, kind):
        return page.evaluate('kind => window.__probeEvents.filter(r => r.kind === "private" && r.event.kind === kind).map(r => r.event)', kind)

    def wait_status(page, status_id, npc_id=None, present=True):
        page.wait_for_function("""([id,npc,present])=>{
          const s=window.__lastSnapshot,u=npc?s?.monsters.find(n=>n.entity.id===npc):s?.self;
          return !!u && !!u.statuses?.some(x=>x.definition_id===id)===present;
        }""", arg=[status_id, npc_id, present])

    def wait_event(page, kind, key, value, expected=1):
        page.wait_for_function("""([kind,key,value,n])=>window.__probeEvents.filter(
          r=>r.kind==="private"&&r.event.kind===kind&&r.event[key]===value
        ).length===n""", arg=[kind, key, value, expected])

    def followup(page, entity, id):
        return next((e['status'] for e in events(page, 'status_applied')
                     if e['status']['target']['id'] == entity and e['status']['definition_id'] == id), None)

    def check_resolution(page, mark, source, target, outcome, next_id, skill):
        rows = events(page, 'cast_interception_resolved')
        assert len(rows) == 1
        e = rows[0]
        assert [e['source']['id'], e['target']['id'], e['outcome'], e['mark_instance_id'], e['mark_definition_id'], e.get('followup_status_id'), e['skill_id']] == [
            source, target, outcome, mark['instance_id'], 'cast-mark', next_id, skill]
        assert e['source']['generation'] == mark['source']['generation']
        assert e['target']['generation'] == mark['target']['generation']
        assert ('action_id' in e) == (skill == 'ordinary')
        consume = [row for row in events(page, 'status_consumed') if row['instance_id'] == mark['instance_id']]
        assert len(consume) == 1
        assert [consume[0]['consumed_stacks'], consume[0]['remaining_stacks']] == [1, 0]
        return e

    def point(page, destination, aim=False):
        if aim:
            page.locator('#selectGround').click()
        x, y = member(page)['position']
        dx, dy = destination[0]-x, destination[1]-y
        canvas = page.locator('#map')
        box = canvas.bounding_box()
        canvas.click(position={'x': box['width']/2+(dx-dy)/2, 'y': box['height']/2+(dx+dy)/4})

    def cast(page, skill, npc_id=None):
        if npc_id:
            point(page, monster(page, npc_id)['position'], aim=True)
        page.locator('#skill').select_option(skill)
        page.locator('#cast').click()

    def wait_shot(page, id):
        page.wait_for_function('id=>window.__lastSnapshot.projectiles.some(p=>p.source.id===id)', arg=id)

    try:
        for page, link in zip(pages, links):
            page.add_init_script("""(()=>{
              const key='interception-events';
              window.__probeEvents=JSON.parse(sessionStorage.getItem(key)||'[]');
              window.__silenceDisplayed=false;
              new MutationObserver(()=>{
                const own=document.querySelector('#own');
                if(own?.textContent.includes('沉默中')&&document.querySelector('#cast')?.disabled)
                  window.__silenceDisplayed=true;
              }).observe(document,{subtree:true,childList:true,characterData:true,attributes:true,attributeFilter:['disabled']});
              const Native=window.EventSource;
              window.EventSource=class extends Native{
                constructor(...args){
                  super(...args);
                  this.addEventListener('gameplay',e=>{
                    window.__probeEvents.push(JSON.parse(e.data));
                    sessionStorage.setItem(key,JSON.stringify(window.__probeEvents));
                  });
                }
              };
            })();""")
            page.on('pageerror', lambda e: errors.append(str(e)))
            page.on('console', lambda m: errors.append(m.text) if m.type == 'error' else None)
            page.goto(address+link['url'], wait_until='domcontentloaded')
        for page in pages:
            page.wait_for_function("window.__lastSnapshot?.phase==='active'&&window.__lastSnapshot.self.ready")
        p0, p1, p2, p3 = pages[:4]
        cast(p1, 'grant-immunity')
        wait_status(p0, 'cast-mark')
        wait_status(p1, 'cast-mark')
        wait_status(p1, 'full-immunity')
        m0, m1 = own_status(p0, 'cast-mark'), own_status(p1, 'cast-mark')
        assert all(m['expires_at_ms']-m['applied_at_ms'] == 5000 for m in (m0, m1))
        immunity = own_status(p1, 'full-immunity')
        before0 = [member(p0)['hp'], member(p0)['spirit'], ready_at(p0), monster(p0, 'mark-a')['hp']]
        before1 = [member(p1)['hp'], member(p1)['spirit'], monster(p1, 'mark-b')['hp']]
        state(0, 'marked')
        state(1, 'marked-and-immune')

        cast(p0, 'ordinary', 'mark-a')
        wait_event(p0, 'cast_interception_resolved', 'outcome', 'interrupted')
        wait_status(p0, 'cast-mark', present=False)
        check_resolution(p0, m0, 'mark-a', 'P00', 'interrupted', 'cast-seal', 'ordinary')
        assert not any(e['skill_id'] == 'ordinary' for e in events(p0, 'action_committed'))
        assert len(events(p0, 'action_interrupted')) == 1
        assert [member(p0)['hp'], member(p0)['spirit'], ready_at(p0), monster(p0, 'mark-a')['hp']] == before0
        seal = followup(p0, 'P00', 'cast-seal')
        assert [seal['attribute'], seal['source']['id'], seal['expires_at_ms']-seal['applied_at_ms']] == ['silence', 'mark-a', seal_ms]
        p0.wait_for_function('window.__silenceDisplayed')
        state(0, 'interrupted')

        cast(p1, 'ordinary', 'mark-b')
        wait_event(p1, 'action_committed', 'skill_id', 'ordinary')
        wait_status(p1, 'cast-mark', present=False)
        r = check_resolution(p1, m1, 'mark-b', 'P01', 'immune_allowed', None, 'ordinary')
        c = next(e for e in events(p1, 'action_committed') if e['skill_id'] == 'ordinary')
        assert [c['action_id'], c['spirit_cost'], c['hp_cost']] == [r['action_id'], cost, 3]
        assert [member(p1)['hp'], member(p1)['spirit'], monster(p1, 'mark-b')['hp']] == [before1[0]-3, before1[1]-cost, before1[2]-8]
        assert not followup(p1, 'P01', 'cast-seal') and own_status(p1, 'full-immunity') == immunity
        state(1, 'immune-allowed')

        for index, id in [(2, 'ordinary-caster'), (3, 'boss-caster')]:
            cast(pages[index], 'mark-caster', id)
            wait_status(pages[index], 'cast-mark', id)
            state(index, 'npc-marked')
        mark2 = status(monster(p2, 'ordinary-caster'), 'cast-mark')
        mark3 = status(monster(p3, 'boss-caster'), 'cast-mark')
        point(p2, [470, 680])
        point(p3, [470, 900])
        wait_event(p2, 'cast_interception_resolved', 'outcome', 'interrupted')
        wait_status(p2, 'cast-mark', 'ordinary-caster', False)
        check_resolution(p2, mark2, 'P02', 'ordinary-caster', 'interrupted', 'cast-seal', 'npc-ordinary')
        npc_seal = followup(p2, 'ordinary-caster', 'cast-seal')
        assert [npc_seal['attribute'], npc_seal['source']['id'], npc_seal['expires_at_ms']-npc_seal['applied_at_ms']] == ['silence', 'P02', seal_ms]
        s = snapshot(p2)
        assert s['logical_ms'] < npc_seal['expires_at_ms'] and not any(p['source']['id'] == 'ordinary-caster' for p in s['projectiles'])
        state(2, 'npc-interrupted')

        wait_event(p3, 'cast_interception_resolved', 'outcome', 'allowed_with_followup')
        wait_shot(p3, 'boss-caster')
        check_resolution(p3, mark3, 'P03', 'boss-caster', 'allowed_with_followup', 'cast-delay', 'npc-ordinary')
        slow = followup(p3, 'boss-caster', 'cast-delay')
        assert [slow['attribute'], slow['source']['id'], slow['expires_at_ms']-slow['applied_at_ms'], slow['modifier_bps']] == [
            'movement_slow', 'P03', slow_ms, slow_bps]
        assert not status(monster(p3, 'boss-caster'), 'cast-mark')
        state(3, 'npc-allowed-with-projectile')
        wait_shot(p2, 'ordinary-caster')
        assert snapshot(p2)['logical_ms'] >= npc_seal['expires_at_ms']
        state(2, 'npc-resumed')

        wait_status(p0, 'cast-seal', present=False)
        cast(p0, 'ordinary', 'mark-a')
        wait_event(p0, 'action_committed', 'skill_id', 'ordinary')
        p0.wait_for_function('!window.__lastSnapshot.self.pending_action')
        assert [member(p0)['hp'], member(p0)['spirit'], monster(p0, 'mark-a')['hp']] == [before0[0]-3, before0[1]-cost, before0[3]-8]
        state(0, 'resumed')

        p1.reload(wait_until='domcontentloaded')
        p1.wait_for_function('window.__lastSnapshot?.self.ready')
        assert own_status(p1, 'full-immunity') == immunity and not own_status(p1, 'cast-mark')
        assert len(events(p1, 'cast_interception_resolved')) == 1
        assert len([e for e in events(p1, 'action_committed') if e['skill_id'] == 'ordinary']) == 1
        state(1, 'reconnected')
        wait_status(p3, 'cast-delay', 'boss-caster', False)
        assert snapshot(p3)['logical_ms'] >= slow['expires_at_ms']
        state(3, 'npc-recovered')

        by_member = [{'participant': f'P{i:02}', 'events': p.evaluate('window.__probeEvents')} for i, p in enumerate(pages)]
        intercepted = [(m['participant'], r['kind']) for m in by_member for r in m['events']
                       if (r.get('event') or r).get('kind') == 'cast_interception_resolved']
        assert intercepted == [(f'P{i:02}', 'private') for i in range(4)]
        assert all(p.evaluate('!JSON.stringify(window.__lastSnapshot).includes("cast_interceptions")') for p in pages)
        Path(output, scenario+'.interception-browser-facts.json').write_text(json.dumps(
            {'states': states, 'eventsByMember': by_member}, ensure_ascii=False, indent=2)+'\n')
        for page in pages:
            page.locator('#leave').click()
        deadline = time.monotonic()+15
        while True:
            ledgers = [request.get('/api/ledger?'+urlsplit(link['url']).query).json() for link in links]
            if all(r['phase'] == 'closed' and not r['host_live'] for r in ledgers):
                break
            assert time.monotonic() < deadline
            time.sleep(.04)
        assert not errors and all(r['ledger']['admissions'] == r['ledger']['settlements'] == 1 and not r['errors'] for r in ledgers), errors
        return {'scenario':scenario, 'members':seats, 'sealMs':seal_ms, 'slowMs':slow_ms, 'slowBps':slow_bps,
                'cost':cost, 'outcomes':4, 'silenceVisible':True, 'reconnected':True, 'private':True, 'settled':seats}
    except Exception:
        diagnostics = []
        for page in pages:
            try:
                diagnostics.append(page.evaluate('({events:window.__probeEvents, snapshot:window.__lastSnapshot})'))
            except Error as error:
                diagnostics.append({'error':str(error)})
        Path(output, scenario+'.interception-browser-error.json').write_text(json.dumps(
            {'errors':errors, 'pages':diagnostics, 'states':states}, ensure_ascii=False, indent=2)+'\n')
        raise
    finally:
        for context in contexts:
            context.close()


def verify_interception_matrix(args):
    database = args.out/f'interception-rooms-{uuid4().hex}.sqlite'
    address = f'http://127.0.0.1:{args.port}'
    command = ['node', str(args.server), '--binary', str(args.binary),
               '--configs', str(args.configs/'interception-catalog.json'),
               '--host-config', str(args.configs/'interception-host.json'),
               '--database', str(database), '--port', str(args.port)]
    with (args.out/'interception-server.log').open('w') as log:
        server = subprocess.Popen(command, cwd=args.cwd, stdout=log, stderr=subprocess.STDOUT, text=True)
        try:
            with sync_playwright() as p:
                request = p.request.new_context(base_url=address)
                deadline = time.monotonic()+20
                while True:
                    try:
                        response = request.get('/api/config', timeout=1000)
                    except Error:
                        response = None
                    if response and response.ok:
                        break
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.1)
                browser = p.chromium.launch()
                cases = [verify_interception(browser, request, address, scenario, seats, args.out)
                         for scenario, seats in [('interception-four',4), ('interception-eight',8)]]
                browser.close()
                request.dispose()
                return cases
        finally:
            server.terminate()
            server.wait(timeout=10)
