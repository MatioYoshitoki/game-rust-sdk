import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { randomUUID } from 'node:crypto';

export async function verifyCastInterception({ binary, hostPath, configs, output, scenario }) {
  const { TwoMemberDatabase, TwoMemberHost, newRecord, readCatalog } = await import(pathToFileURL(hostPath).href);
  const four = scenario === 'interception-four';
  const manifest = join(configs, 'interception-catalog.json');
  const catalog = readCatalog(binary, manifest);
  const accounts = JSON.parse(readFileSync(join(configs, four ? 'interception-accounts-a.json' : 'interception-accounts-b.json')));
  const record = newRecord({ catalog, scenario, accounts, manifest });
  const database = new TwoMemberDatabase(join(output, scenario + '-' + randomUUID() + '.sqlite'));
  database.create(record);
  const facts = [], states = [];
  const host = new TwoMemberHost({
    binary, manifest, database, record, autoAck: true,
    onEvent(recipient, wrapped) { facts.push({ recipient, wrapped }); },
  });
  const snap = pid => host.member(pid).snapshot;
  const own = pid => snap(pid).self;
  const npc = (pid, id) => snap(pid).monsters.find(n => n.entity.id === id);
  const status = (unit, id) => unit.statuses?.find(s => s.definition_id === id);
  const privateEvents = (pid, kind) => facts.filter(r => r.recipient === pid && r.wrapped.kind === 'private' && r.wrapped.event.kind === kind).map(r => r.wrapped.event);
  const resolutions = pid => privateEvents(pid, 'cast_interception_resolved');
  const commits = (pid, skill) => privateEvents(pid, 'action_committed').filter(e => e.skill_id === skill);
  const ready = pid => own(pid).player_skills.find(s => s.skill_id === 'ordinary').ready_at_ms;
  const followup = (pid, entity, id) => privateEvents(pid, 'status_applied').find(e => e.status.target.id === entity && e.status.definition_id === id)?.status;
  const shot = (pid, id) => snap(pid).projectiles.find(p => p.source.id === id);
  const state = (pid, step) => states.push({ pid, step, snapshot: structuredClone(snap(pid)) });
  const cast = async (pid, skill, id, repeat = false) => {
    const action = id ? { kind: 'cast_unit', skill_id: skill, target: {
      id, generation: npc(pid, id).entity.generation,
    } } : { kind: 'cast_self', skill_id: skill };
    await host.input(pid, action);
    if (repeat) {
      const m = host.member(pid);
      const response = await host.kernel.rpc({ type: 'submit_input', request: {
        key: host.originalCreate.key, participant_id: pid, entity_generation: 1, connection: m.connection,
        request_id: `host-${m.connection.generation}-${m.sequence}`, sequence: m.sequence, action,
      } });
      assert.ok(response.status);
    }
  };
  const checkResolution = (pid, mark, source, target, outcome, next, member) => {
    const rows = resolutions(pid);
    assert.equal(rows.length, 1);
    const r = rows[0];
    assert.deepEqual([r.source.id, r.target.id, r.outcome, r.mark_instance_id, r.mark_definition_id, r.followup_status_id],
      [source, target, outcome, mark.instance_id, 'cast-mark', next]);
    assert.equal(r.skill_id, member ? 'ordinary' : 'npc-ordinary');
    assert.equal(typeof r.action_id, member ? 'string' : 'undefined');
    assert.deepEqual([r.source.generation, r.target.generation], [mark.source.generation, mark.target.generation]);
    const consumed = privateEvents(pid, 'status_consumed').filter(e => e.instance_id === mark.instance_id);
    assert.equal(consumed.length, 1);
    assert.deepEqual([consumed[0].consumed_stacks, consumed[0].remaining_stacks], [1, 0]);
  };
  try {
    await host.start();
    await host.joinAndReady();
    const sealMs = four ? 700 : 900, slowMs = four ? 1200 : 1500, slowBps = four ? -2500 : -2000, cost = four ? 8 : 12;
    await cast('P01', 'grant-immunity');
    await host.until(() => status(own('P01'), 'full-immunity') && status(own('P00'), 'cast-mark') && status(own('P01'), 'cast-mark'));
    const mark0 = status(own('P00'), 'cast-mark'), mark1 = status(own('P01'), 'cast-mark');
    assert.equal(mark0.expires_at_ms - mark0.applied_at_ms, 5000);
    assert.equal(mark1.expires_at_ms - mark1.applied_at_ms, 5000);
    const before0 = [own('P00').hp, own('P00').spirit, ready('P00'), npc('P00', 'mark-a').hp];
    const before1 = [own('P01').hp, own('P01').spirit, npc('P01', 'mark-b').hp];
    const immunity = structuredClone(status(own('P01'), 'full-immunity'));
    state('P00', 'marked'); state('P01', 'marked-and-immune');

    await cast('P00', 'ordinary', 'mark-a', true);
    await host.until(() => resolutions('P00').length && !own('P00').pending_action && !status(own('P00'), 'cast-mark'));
    checkResolution('P00', mark0, 'mark-a', 'P00', 'interrupted', 'cast-seal', true);
    assert.equal(commits('P00', 'ordinary').length, 0);
    assert.equal(privateEvents('P00', 'action_interrupted').length, 1);
    assert.deepEqual([own('P00').hp, own('P00').spirit, ready('P00'), npc('P00', 'mark-a').hp], before0);
    const seal = followup('P00', 'P00', 'cast-seal');
    assert.deepEqual([seal.attribute, seal.source.id, seal.expires_at_ms - seal.applied_at_ms], ['silence', 'mark-a', sealMs]);
    state('P00', 'interrupted');

    await cast('P01', 'ordinary', 'mark-b', true);
    await host.until(() => commits('P01', 'ordinary').length === 1 && !own('P01').pending_action && !status(own('P01'), 'cast-mark'));
    checkResolution('P01', mark1, 'mark-b', 'P01', 'immune_allowed', undefined, true);
    assert.equal(commits('P01', 'ordinary')[0].action_id, resolutions('P01')[0].action_id);
    assert.deepEqual([commits('P01', 'ordinary')[0].spirit_cost, commits('P01', 'ordinary')[0].hp_cost], [cost, 3]);
    assert.deepEqual([own('P01').hp, own('P01').spirit, npc('P01', 'mark-b').hp], [before1[0]-3, before1[1]-cost, before1[2]-8]);
    assert.equal(followup('P01', 'P01', 'cast-seal'), undefined);
    assert.deepEqual(status(own('P01'), 'full-immunity'), immunity);
    state('P01', 'immune-allowed');

    for (const [pid, id] of [['P02','ordinary-caster'], ['P03','boss-caster']]) {
      await cast(pid, 'mark-caster', id);
      await host.until(() => status(npc(pid, id), 'cast-mark'));
      state(pid, 'npc-marked');
    }
    const mark2 = status(npc('P02', 'ordinary-caster'), 'cast-mark');
    const mark3 = status(npc('P03', 'boss-caster'), 'cast-mark');
    await host.input('P02', { kind: 'navigate', destination: [470,680] });
    await host.input('P03', { kind: 'navigate', destination: [470,900] });
    await host.until(() => resolutions('P02').length && !status(npc('P02', 'ordinary-caster'), 'cast-mark'));
    checkResolution('P02', mark2, 'P02', 'ordinary-caster', 'interrupted', 'cast-seal', false);
    const npcSeal = followup('P02', 'ordinary-caster', 'cast-seal');
    assert.deepEqual([npcSeal.attribute, npcSeal.source.id, npcSeal.expires_at_ms - npcSeal.applied_at_ms], ['silence', 'P02', sealMs]);
    assert.ok(snap('P02').logical_ms < npcSeal.expires_at_ms && !shot('P02', 'ordinary-caster'));
    state('P02', 'npc-interrupted');
    await host.until(() => resolutions('P03').length && shot('P03', 'boss-caster'));
    checkResolution('P03', mark3, 'P03', 'boss-caster', 'allowed_with_followup', 'cast-delay', false);
    const bossSlow = followup('P03', 'boss-caster', 'cast-delay');
    assert.deepEqual([bossSlow.attribute, bossSlow.source.id, bossSlow.expires_at_ms-bossSlow.applied_at_ms, bossSlow.modifier_bps],
      ['movement_slow', 'P03', slowMs, slowBps]);
    assert.equal(status(npc('P03', 'boss-caster'), 'cast-mark'), undefined);
    state('P03', 'npc-allowed-with-projectile');
    await host.until(() => shot('P02', 'ordinary-caster'));
    assert.ok(snap('P02').logical_ms >= npcSeal.expires_at_ms);
    state('P02', 'npc-resumed');

    await host.until(() => !status(own('P00'), 'cast-seal'));
    await cast('P00', 'ordinary', 'mark-a');
    await host.until(() => commits('P00', 'ordinary').length === 1 && !own('P00').pending_action);
    assert.deepEqual([own('P00').hp, own('P00').spirit, npc('P00', 'mark-a').hp], [before0[0]-3, before0[1]-cost, before0[3]-8]);
    state('P00', 'resumed');

    const m = host.member('P01');
    await host.reconnect('P01');
    await host.until(() => !own('P01').ready);
    await host.input('P01', { kind: 'foreground' });
    await host.input('P01', { kind: 'combat_ready', publication_id: m.readyPublication ?? m.publication });
    await host.until(() => own('P01').ready);
    assert.deepEqual(status(own('P01'), 'full-immunity'), immunity);
    assert.equal(status(own('P01'), 'cast-mark'), undefined);
    assert.equal(resolutions('P01').length, 1);
    assert.equal(commits('P01', 'ordinary').length, 1);
    state('P01', 'reconnected');
    await host.until(() => !status(npc('P03','boss-caster'), 'cast-delay'));
    assert.ok(snap('P03').logical_ms >= bossSlow.expires_at_ms);
    state('P03','npc-recovered');

    const intercepted = facts.filter(r => r.wrapped.event?.kind === 'cast_interception_resolved');
    assert.deepEqual(intercepted.map(r => r.recipient).sort(), ['P00','P01','P02','P03']);
    assert.ok(intercepted.every(r => r.wrapped.kind === 'private'));
    assert.ok(host.members.every(m => !JSON.stringify(m.snapshot).includes('cast_interceptions')));
    writeFileSync(join(output, scenario+'.interception-facts.json'), JSON.stringify({ states, events: facts }, null, 2)+'\n');
    for (const m of host.members) await host.input(m.account.participant_id, { kind: 'leave' });
    await host.until(() => host.phase === 'closed' && host.pending() === 0 && host.publicationsComplete());
    assert.ok(database.views(record.id).every(r => r.admissions === 1 && r.settlements === 1));
    assert.equal(host.errors.length, 0);
    return { scenario, members: host.members.length, sealMs, slowMs, slowBps, cost, outcomes:4, duplicateFree:true, reconnected:true, private:true, settled:host.members.length };
  } catch (error) {
    writeFileSync(join(output, scenario+'.interception-error.json'), JSON.stringify({ error:String(error), facts, states, snapshots:host.members.map(m=>m.snapshot), errors:host.errors }, null, 2));
    throw error;
  } finally {
    await host.dispose();
    database.close();
  }
}
