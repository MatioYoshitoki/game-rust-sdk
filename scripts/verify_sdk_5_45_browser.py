"""Exercise both recovery cases in the packaged ordinary-clock H5."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import time
from urllib.parse import urlsplit

from playwright.sync_api import Error, expect, sync_playwright


def verify_mitigation(actor, pages, bps, reduced, shield=None):
    periodic = shield is not None
    actor.wait_for_function('periodic => window.__probeEvents.some(row => window.__probeHit(row, periodic))',
                            arg=periodic)
    before = actor.evaluate('window.__lastSnapshot.self')
    actor.locator('#useItemChoice').select_option('pill_guard')
    actor.locator('#useItem').click()
    actor.wait_for_function("""() => window.__lastItemUsed?.item_id === 'pill_guard' &&
        window.__lastSnapshot.self.statuses.some(row => row.definition_id === 'pill-guard')""")
    after = actor.evaluate('window.__lastSnapshot.self')
    state = next(row for row in after['statuses'] if row['definition_id'] == 'pill-guard')
    assert state['attribute'] == 'damage_reduction'
    assert state['modifier_bps'] == bps
    assert state['expires_at_ms'] - state['applied_at_ms'] == 3300
    assert after['inventory_revision'] == before['inventory_revision'] + 1
    assert next(row['quantity'] for row in after['inventory'] if row['item_id'] == 'pill_guard') == 1
    if periodic:
        assert after['shield_total'] == shield
    restored = actor.evaluate('window.__lastItemUsed.hp_restored')
    assert restored > 0
    expect(actor.locator('#lastItemUse')).to_have_text(f'恢复生命 {restored}')
    expect(actor.locator('#useItem')).to_be_disabled()
    assert all(page.evaluate("!window.__lastSnapshot.self.statuses.some(s => s.definition_id === 'pill-guard')")
               for page in pages[1:])
    actor.wait_for_function("""([id, periodic]) => {
        const events = window.__probeEvents;
        const end = events.findIndex(row => row.event?.kind === 'status_removed' && row.event.instance_ids.includes(id));
        return end >= 0 && events.slice(end + 1).some(row => window.__probeHit(row, periodic));
    }""", arg=[state['instance_id'], periodic])
    trace = actor.evaluate('window.__probeEvents')
    start = next(i for i, row in enumerate(trace) if row.get('event', {}).get('kind') == 'status_applied'
                 and row['event']['status']['instance_id'] == state['instance_id'])
    end = next(i for i, row in enumerate(trace) if row.get('event', {}).get('kind') == 'status_removed'
               and state['instance_id'] in row['event']['instance_ids'])
    assert trace[end]['event']['reason'] == 'expired'
    groups = (trace[:start], trace[start+1:end], trace[end+1:])
    if periodic:
        facts = [[row['event'] for row in group if row.get('event', {}).get('kind') == 'periodic_tick'
                  and row['event']['source']['id'] == 'damage-probe' and row['event']['target']['id'] == 'P00']
                 for group in groups]
        assert all(fact['raw_damage'] == 101 and fact['outcome'] == 'applied' for group in facts for fact in group)
        phases = [[[fact['absorbed'], fact['hp_damage']] for fact in group] for group in facts]
        baseline, during, expired = phases
        assert baseline and len(during) >= 2 and expired, phases
        assert during[0] == [shield, reduced - shield]
        assert all(row == [0, reduced] for row in during[1:]), phases
        assert all(row == [0, 100] for row in baseline + expired), phases
        expect(actor.locator('#own')).to_contain_text(f'DOT 实际吸收 {shield}')
    else:
        phases = [[row['amount'] for row in group if row['kind'] == 'damage'
                   and row['source']['id'] == 'damage-probe' and row['target']['id'] == 'P00'] for group in groups]
        assert all(rows and all(amount == expected for amount in rows)
                   for rows, expected in zip(phases, [100, reduced, 100])), phases
    return {'bps': bps, 'phases': phases, 'restoredHp': restored, 'expired': True,
            **({'shield': shield} if periodic else {})}


def verify_periodic_healing_received(actor, pages, item, bps, adjusted):
    actor.wait_for_function('() => window.__probeEvents.some(row => window.__probeHit(row, true))')
    before = actor.evaluate('window.__lastSnapshot.self.inventory_revision')
    actor.locator('#useItemChoice').select_option(item)
    actor.locator('#useItem').click()
    actor.wait_for_function("""id => window.__lastItemUsed?.item_id === id &&
        window.__lastSnapshot.self.statuses.some(row => row.definition_id === 'pill-care')""", arg=item)
    after = actor.evaluate('window.__lastSnapshot.self')
    state = next(row for row in after['statuses'] if row['definition_id'] == 'pill-care')
    assert state['attribute'] == 'healing_received' and state['modifier_bps'] == bps
    assert state['expires_at_ms'] - state['applied_at_ms'] == 3300
    assert after['inventory_revision'] == before + 1
    assert next(row['quantity'] for row in after['inventory'] if row['item_id'] == item) == 1
    assert actor.evaluate('window.__lastItemUsed.hp_restored') == 10
    expect(actor.locator('#lastItemUse')).to_have_text('恢复生命 10')
    expect(actor.locator('#useItem')).to_be_disabled()
    actor.wait_for_function("""deadline => window.__probeEvents.some(row =>
        row.event?.kind === 'periodic_heal_tick' && row.event.definition_id === 'pill-hot' &&
        row.event.executed_at_ms >= deadline)""", arg=state['expires_at_ms'])
    trace = actor.evaluate('window.__probeEvents')
    assert any(row.get('event', {}).get('kind') == 'status_removed'
               and row['event']['reason'] == 'expired'
               and state['instance_id'] in row['event']['instance_ids'] for row in trace)
    phases = [[], []]
    for row in trace:
        tick = row.get('event', {})
        if tick.get('kind') != 'periodic_heal_tick' or tick['definition_id'] != 'pill-hot':
            continue
        assert tick['source'] == tick['target'] and tick['target']['id'] == 'P00'
        assert tick['raw_heal'] == 51 and tick['outcome'] == 'applied'
        phase = 1 if tick['executed_at_ms'] >= state['expires_at_ms'] else 0
        phases[phase].append(tick['hp_restored'])
    assert len(phases[0]) >= 2 and phases[0] == [adjusted] * len(phases[0]), phases
    assert phases[1] and phases[1] == [51] * len(phases[1]), phases
    for page in pages[1:]:
        assert page.evaluate("""() => !window.__probeEvents.some(row =>
            row.event?.kind === 'periodic_heal_tick' && row.event.definition_id === 'pill-hot')
            && !window.__lastSnapshot.self.statuses.some(row => ['pill-care', 'pill-hot'].includes(row.definition_id))""")
    actor.wait_for_function("""() => {
        const ticks = window.__probeEvents.map(row => row.event).filter(e => e?.kind === 'periodic_heal_tick');
        const own = ticks.filter(e => e.definition_id === 'pill-hot').length;
        const restored = ticks.reduce((total, tick) => total + tick.hp_restored, 0);
        const label = document.querySelector('#periodicStats')?.textContent ?? '';
        return label.includes(`自身 ${own} / `) && label.endsWith(`实际治疗 ${restored}`);
    }""")
    return {'bps': bps, 'phases': phases, 'rawHeal': 51, 'immediateHeal': 10}


def verify_item_healing_received(actor, pages, pill, bps, adjusted):
    actor.wait_for_function("""() => window.__probeEvents.some(row =>
        window.__probeHit(row, true) && row.event.definition_id === 'probe-dot' && row.event.hp_damage === 100)""")

    def use(item, expected, remaining):
        revision = actor.evaluate('window.__lastSnapshot.self.inventory_revision')
        actor.locator('#useItemChoice').select_option(item)
        expect(actor.locator('#useItem')).to_be_enabled()
        actor.locator('#useItem').click()
        actor.wait_for_function("""([id, revision, quantity]) => {
            const own = window.__lastSnapshot.self;
            return window.__lastItemUsed?.item_id === id && own.inventory_revision > revision &&
                (own.inventory.find(row => row.item_id === id)?.quantity ?? 0) === quantity;
        }""", arg=[item, revision, remaining])
        assert actor.evaluate('window.__lastSnapshot.self.inventory_revision') == revision + 1
        restored = actor.evaluate('window.__lastItemUsed.hp_restored')
        assert restored == expected
        expect(actor.locator('#lastItemUse')).to_have_text(f'恢复生命 {restored}')
        return restored

    tonic_heal = use('care_tonic', 10, 0)
    state = actor.evaluate("window.__lastSnapshot.self.statuses.find(row => row.definition_id === 'pill-care')")
    assert state['attribute'] == 'healing_received' and state['modifier_bps'] == bps
    assert state['expires_at_ms'] - state['applied_at_ms'] == 3300
    active_heal = use(pill, adjusted, 2)
    assert actor.evaluate("window.__lastSnapshot.self.statuses.some(row => row.definition_id === 'pill-care')")
    expect(actor.locator('#useItem')).to_be_disabled()
    actor.wait_for_function("""([id, deadline]) => {
        const snapshot = window.__lastSnapshot;
        return snapshot.logical_ms >= deadline && !snapshot.self.statuses.some(row => row.instance_id === id);
    }""", arg=[state['instance_id'], state['expires_at_ms']])
    expired_heal = use(pill, 53, 1)
    for page in pages[1:]:
        assert page.evaluate("""() => !window.__probeEvents.some(row => row.event?.kind === 'item_used')
            && !window.__lastSnapshot.self.statuses.some(row => row.definition_id === 'pill-care')""")
    return {'bps': bps, 'tonicHeal': tonic_heal, 'activeHeal': active_heal, 'expiredHeal': expired_heal,
            'pillsUsed': 2}


def pickup_recovery_orb(actor, other, restored, basic_spirit, target=None):
    before_attack = actor.evaluate('window.__lastSnapshot.self')
    if target:
        generation = actor.evaluate("""id => window.__lastSnapshot.monsters.find(row =>
            row.entity.id === id).entity.generation""", target)
        actor.locator('#attackTarget').select_option(label=f'{target} #{generation}')
    actor.get_by_role('button', name='普攻', exact=True).click()
    actor.wait_for_function("""() => window.__lastSnapshot.resources.some(row =>
        !row.item_id && row.owner?.id === 'P00' && row.quantity === 0)""")
    orb = actor.evaluate("""window.__lastSnapshot.resources.find(row => !row.item_id && row.owner?.id === 'P00')""")
    before = actor.evaluate('window.__lastSnapshot.self')
    assert before['hp'] == before_attack['hp']
    basic_delta = before['spirit'] - before_attack['spirit']
    assert basic_delta == basic_spirit
    assert orb['owner']['kind'] == 'participant'
    other.wait_for_function('id => window.__lastSnapshot.resources.some(row => row.id === id)', arg=orb['id'])
    actor.locator('#pickup').click()
    actor.wait_for_function("""([id, revision]) => {
        const snapshot = window.__lastSnapshot;
        return snapshot.self.inventory_revision === revision + 1 && !snapshot.resources.some(row => row.id === id);
    }""", arg=[orb['id'], before['inventory_revision']])
    after = actor.evaluate('window.__lastSnapshot.self')
    change = [after['hp'] - before['hp'], after['spirit'] - before['spirit']]
    assert change == restored
    assert after['inventory'] == before['inventory']
    expect(actor.locator('#own')).to_contain_text(f"HP {after['hp']} / 1000")
    expect(actor.locator('#own')).to_contain_text(f"灵力 {after['spirit']} / 200")
    return change, basic_delta


def verify_recovery_orb(actor, peers, hp_deficit, restored, locked=False, npc_source=False):
    actor.locator('#selfCast').click()
    actor.wait_for_function("""([hp, npcSource]) => {
        const snapshot = window.__lastSnapshot;
        const own = snapshot.self;
        return (npcSource ? own.hp <= 1000 - hp : own.hp === 1000 - hp) &&
            own.spirit === 50 && snapshot.logical_ms >= own.recovery_until_ms;
    }""", arg=[hp_deficit, npc_source])
    if not locked:
        change, _ = pickup_recovery_orb(actor, peers[0], restored, 2)
        return {'restored': change, 'inventoryUnchanged': True}
    actor.wait_for_function("""() => window.__lastSnapshot.self.statuses.some(row =>
        row.definition_id === 'spirit-seal')""")
    state = actor.evaluate("window.__lastSnapshot.self.statuses.find(row => row.definition_id === 'spirit-seal')")
    assert state['attribute'] == 'spirit_recovery_lock' and state['modifier_bps'] == -10000
    assert state['expires_at_ms'] - state['applied_at_ms'] == 4200
    if npc_source:
        source = actor.evaluate("""window.__lastSnapshot.monsters.find(row => row.entity.id === 'seal-caster').entity""")
        assert state['source'] == source
        assert state['target'] == actor.evaluate('window.__lastSnapshot.self.participant.entity')
    assert all(peer.evaluate("""() => !window.__lastSnapshot.self.statuses.some(row =>
        row.definition_id === 'spirit-seal')""") for peer in peers)
    active, active_basic = pickup_recovery_orb(actor, peers[0], [restored[0], 0], 0, 'orb-target')
    assert actor.evaluate("window.__lastSnapshot.self.statuses.some(row => row.definition_id === 'spirit-seal')")
    actor.wait_for_function("""([id, deadline]) => {
        const snapshot = window.__lastSnapshot;
        return snapshot.logical_ms >= deadline && snapshot.logical_ms >= snapshot.self.basic_ready_at_ms &&
            !snapshot.self.statuses.some(row => row.instance_id === id);
    }""", arg=[state['instance_id'], state['expires_at_ms']])
    expired, expired_basic = pickup_recovery_orb(actor, peers[0], restored, 2, 'orb-target-after')
    result = {'locked': active, 'expired': expired, 'basicSpirit': [active_basic, expired_basic]}
    if npc_source:
        result['source'] = state['source']['id']
    return result


def verify_recovery_strength(actor, peers, hp_deficit, bps, active, expired):
    actor.locator('#selfCast').click()
    actor.wait_for_function("""() => window.__lastSnapshot.self.statuses.some(row =>
        row.definition_id === 'regen-strength')""")
    own = actor.evaluate('window.__lastSnapshot.self')
    state = next(row for row in own['statuses'] if row['definition_id'] == 'regen-strength')
    assert state['attribute'] == 'recovery_strength' and state['modifier_bps'] == bps
    assert state['expires_at_ms'] - state['applied_at_ms'] == 3300
    assert 1000 - hp_deficit <= own['hp'] <= 1000 - hp_deficit + active[0]
    assert 50 <= own['spirit'] <= 50 + active[1]

    def observe_tick(expected):
        before = actor.evaluate('window.__lastSnapshot.self')
        actor.wait_for_function('hp => window.__lastSnapshot.self.hp > hp', arg=before['hp'])
        after = actor.evaluate('window.__lastSnapshot.self')
        change = [after['hp'] - before['hp'], after['spirit'] - before['spirit']]
        assert change == expected
        assert after['inventory_revision'] == before['inventory_revision']
        expect(actor.locator('#own')).to_contain_text(f"HP {after['hp']} / 1000")
        expect(actor.locator('#own')).to_contain_text(f"灵力 {after['spirit']} / 200")
        return change

    active_tick = observe_tick(active)
    assert actor.evaluate("""id => window.__lastSnapshot.self.statuses.some(row => row.instance_id === id)""",
                          state['instance_id'])
    assert all(peer.evaluate("""() => !window.__lastSnapshot.self.statuses.some(row =>
        row.definition_id === 'regen-strength')""") for peer in peers)
    actor.wait_for_function("""([id, deadline]) => {
        const snapshot = window.__lastSnapshot;
        return snapshot.logical_ms >= deadline && !snapshot.self.statuses.some(row => row.instance_id === id);
    }""", arg=[state['instance_id'], state['expires_at_ms']])
    expired_tick = observe_tick(expired)
    return {'bps': bps, 'active': active_tick, 'expired': expired_tick}


def verify_overheal_shield(actor, peers, item, restored, requested):
    if restored > 0:
        actor.locator('#selfCast').click()
        actor.wait_for_function("""hp => window.__lastSnapshot.self.hp === window.__lastSnapshot.self.max_hp - hp""",
                                arg=restored)
    before = actor.evaluate('window.__lastSnapshot.self')
    assert before['hp'] == before['max_hp'] - restored and before['shield_total'] == 0
    actor.locator('#useItemChoice').select_option(item)
    actor.locator('#useItem').click()
    actor.wait_for_function("""id => window.__lastItemUsed?.item_id === id &&
        window.__lastSnapshot.self.statuses.some(row => row.definition_id === 'overflow-shield') &&
        window.__shieldStats?.last?.definition_id === 'overflow-shield'""", arg=item)
    after = actor.evaluate('window.__lastSnapshot.self')
    instance = next(row for row in after['statuses'] if row['definition_id'] == 'overflow-shield')
    assert after['hp'] - before['hp'] == restored
    assert after['inventory_revision'] == before['inventory_revision'] + 1
    assert next(row['quantity'] for row in after['inventory'] if row['item_id'] == item) == 1
    assert instance['expires_at_ms'] - instance['applied_at_ms'] == 3000
    assert after['shield_total'] == instance['shield']['remaining'] == 100
    fact = actor.evaluate('window.__shieldStats.last')
    assert [fact['requested'], fact['granted']] == [requested, 100]
    expect(actor.locator('#lastItemUse')).to_have_text(f'恢复生命 {restored}')
    expect(actor.locator('#own')).to_contain_text('本人护盾 100')
    assert all(peer.evaluate('window.__lastSnapshot.self.shield_total') == 0 for peer in peers)
    actor.wait_for_function("""([id, deadline]) => {
        const snapshot = window.__lastSnapshot;
        return snapshot.logical_ms >= deadline && snapshot.self.shield_total === 0 &&
            !snapshot.self.statuses.some(row => row.instance_id === id);
    }""", arg=[instance['instance_id'], instance['expires_at_ms']])
    return {'restored': restored, 'requested': fact['requested'], 'granted': fact['granted']}


def verify_periodic_overheal_shield(actor, peers, expected):
    actor.locator('#selfCast').click()
    total = sum(tick[1] for tick in expected)
    actor.wait_for_function("""total => window.__probeEvents.filter(row =>
        row.event?.kind === 'periodic_heal_tick' && row.event.definition_id === 'overflow-hot').length === 3
        && window.__lastSnapshot.self.shield_total === total""", arg=total)
    ticks = []
    for row in actor.evaluate('window.__probeEvents'):
        event = row.get('event', {})
        if event.get('kind') == 'periodic_heal_tick' and event['definition_id'] == 'overflow-hot':
            assert event['raw_heal'] == 100 and event['outcome'] == 'applied'
            ticks.append([event['hp_restored'], 0])
        elif event.get('kind') == 'shield_granted' and event['definition_id'] == 'overflow-shield':
            assert ticks[-1][1] == 0
            ticks[-1][1] = event['granted']
    assert ticks == expected
    own = actor.evaluate('window.__lastSnapshot.self')
    assert own['hp'] == own['max_hp']
    instance = next(row for row in own['statuses'] if row['definition_id'] == 'overflow-shield')
    expect(actor.locator('#own')).to_contain_text(f'本人护盾 {total}')
    assert all(peer.evaluate('window.__lastSnapshot.self.shield_total') == 0 for peer in peers)
    actor.wait_for_function("""([id, deadline]) => {
        const snapshot = window.__lastSnapshot;
        return snapshot.logical_ms >= deadline && snapshot.self.shield_total === 0 &&
            !snapshot.self.statuses.some(row => row.instance_id === id);
    }""", arg=[instance['instance_id'], instance['expires_at_ms']])
    return {'ticks': ticks, 'expired': True}


def verify_instant_self_heal(actor, peers, expected):
    hp_cost, raw, restored, granted = expected
    before = actor.evaluate('window.__lastSnapshot.self')
    assert before['hp'] == before['max_hp'] and before['shield_total'] == 0
    actor.locator('#selfCast').click()
    actor.wait_for_function("""total => window.__probeEvents.some(row =>
        row.event?.kind === 'heal_resolved' && row.event.heal_kind === 'self_skill') &&
        window.__lastSnapshot.self.shield_total === total""", arg=granted)
    own = actor.evaluate('window.__lastSnapshot.self')
    events = actor.evaluate("""window.__probeEvents.map(row => row.event).filter(event =>
        event?.kind === 'action_committed' || event?.kind === 'heal_resolved' ||
        event?.kind === 'shield_granted')""")
    assert [event['kind'] for event in events] == ['action_committed', 'heal_resolved', 'shield_granted']
    commit, heal, grant = events
    assert commit.get('hp_cost', 0) == hp_cost
    assert heal['heal_kind'] == 'self_skill'
    assert [heal[key] for key in ('raw_heal', 'multiplier_bps', 'adjusted_heal', 'hp_restored')] == [
        raw, 10000, raw, restored,
    ]
    assert [grant[key] for key in ('requested', 'granted', 'remaining')] == [granted] * 3
    assert all(event['source'] == event['target'] == own['participant']['entity'] for event in (heal, grant))
    assert own['hp'] == own['max_hp']
    instance = next(row for row in own['statuses'] if row['definition_id'] == 'overflow-shield')
    expect(actor.locator('#lastSelfHeal')).to_have_text(f'自身治疗 · 基础 {raw} · 实回 {restored}')
    expect(actor.locator('#own')).to_contain_text(f'本人护盾 {granted}')
    for peer in peers:
        assert peer.evaluate('window.__lastSnapshot.self.shield_total') == 0
        expect(peer.locator('#lastSelfHeal')).to_be_hidden()
    actor.wait_for_function("""([id, deadline]) => {
        const snapshot = window.__lastSnapshot;
        return snapshot.logical_ms >= deadline && snapshot.self.shield_total === 0 &&
            !snapshot.self.statuses.some(row => row.instance_id === id);
    }""", arg=[instance['instance_id'], instance['expires_at_ms']])
    return {'hpCost': hp_cost, 'raw': raw, 'restored': restored, 'granted': granted, 'expired': True}


def verify_ally_heal(actor, target, peers, expected, conversion=None):
    missing, restored = expected
    before = actor.evaluate('window.__lastSnapshot.self')
    if missing:
        target.locator('#selfCast').click()
    target.wait_for_function("""missing => {
        const own = window.__lastSnapshot.self;
        return own.hp === own.max_hp - missing;
    }""", arg=missing)
    injured = target.evaluate('window.__lastSnapshot.self')
    source_entity = before['participant']['entity']
    target_entity = injured['participant']['entity']
    actor.locator('#skill').select_option('ally-restore')
    expect(actor.locator('#cast')).to_be_disabled()
    actor.locator('#allyTarget').select_option(label=target_entity['id'])
    expect(actor.locator('#cast')).to_be_enabled()
    actor.locator('#cast').click()
    target.wait_for_function('hp => window.__lastSnapshot.self.hp === hp', arg=injured['hp'] + restored)
    label = f"友方治疗 · {source_entity['id']} → {target_entity['id']} · 实回 {restored}"
    expect(actor.locator('#lastAllyHeal')).to_have_text(label)
    expect(target.locator('#lastAllyHeal')).to_have_text(label)
    for peer in peers:
        expect(peer.locator('#lastAllyHeal')).to_be_hidden()
    observe = """window.__probeEvents.map(row => row.event).filter(event =>
        (event?.kind === 'action_committed' && event.skill_id === 'ally-restore') ||
        (event?.kind === 'heal_resolved' && event.heal_kind === 'ally_skill') ||
        (event?.kind === 'shield_granted' && event.definition_id === 'overflow-shield'))"""
    if conversion:
        raw, leaving_role, _, granted = conversion
        shield_label = f"友方护盾 · {source_entity['id']} → {target_entity['id']} · 实授 {granted}"
        expect(actor.locator('#lastAllyShield')).to_have_text(shield_label)
        expect(target.locator('#lastAllyShield')).to_have_text(shield_label)
    commit, caster_result, *shield_events = actor.evaluate(observe)
    assert commit['kind'] == 'action_committed'
    assert commit['unit_target'] == {key: target_entity[key] for key in ('id', 'generation')}
    assert target.evaluate(observe) == [caster_result, *shield_events]
    assert all(peer.evaluate(observe) == [] for peer in peers)
    assert [caster_result[key] for key in ('source', 'target', 'hp_restored')] == [
        source_entity, target_entity, restored,
    ]
    assert actor.evaluate('window.__lastSnapshot.self.hp') == before['hp']
    result = {'missing': missing, 'restored': restored, 'source': source_entity['id'], 'target': target_entity['id']}
    if not conversion:
        assert not shield_events and target.evaluate('window.__lastSnapshot.self.shield_total') == 0
        return result
    grant, = shield_events
    assert [caster_result[key] for key in ('raw_heal', 'adjusted_heal', 'multiplier_bps')] == [raw, raw, 10000]
    assert [grant[key] for key in ('source', 'target', 'requested', 'granted', 'remaining')] == [
        source_entity, target_entity, granted, granted, granted,
    ]
    instance_id = grant['instance_id']
    target.wait_for_function("""([id, amount]) => {
        const own = window.__lastSnapshot.self;
        return own.shield_total === amount && own.statuses.some(row => row.instance_id === id);
    }""", arg=[instance_id, granted])
    instance = target.evaluate("id => window.__lastSnapshot.self.statuses.find(row => row.instance_id === id)", instance_id)
    assert [instance['source'], instance['target']] == [source_entity, target_entity]
    assert all(page.evaluate('window.__lastSnapshot.self.shield_total') == 0 for page in [actor, *peers])
    leaver, observer = (actor, target) if leaving_role == 'source' else (target, actor)
    reason = 'source_invalid' if leaving_role == 'source' else 'target_invalid'
    leaver.locator('#leave').click()
    observer.wait_for_function("""([id, reason]) => window.__probeEvents.some(row =>
        row.event?.kind === 'status_removed' && row.event.reason === reason &&
        row.event.instance_ids.includes(id))""", arg=[instance_id, reason])
    if leaving_role == 'source':
        target.wait_for_function("window.__lastSnapshot.self.shield_total === 0")
    assert all(not peer.evaluate("""id => window.__probeEvents.some(row =>
        row.event?.instance_ids?.includes(id))""", instance_id) for peer in peers)
    return {**result, 'raw': raw, 'granted': granted, 'reason': reason}


def verify_ally_hot(pages, expected):
    first, leaving_role = expected
    caster, target, second = pages[:3]
    sources = [caster, second] if leaving_role == 'source' else [caster]
    own = 'window.__lastSnapshot.self'
    target_entity = target.evaluate(own + '.participant.entity')
    target_id = target_entity['id']
    source_ids = [source.evaluate(own + '.participant.entity.id') for source in sources]

    notices = """([kind, id]) => window.__probeEvents.map(row => row.event).filter(event =>
        event?.kind === kind && (event.instance_id === id || event.status?.instance_id === id ||
        event.instance_ids?.includes(id)))"""

    target.locator('#selfCast').click()
    target.wait_for_function("""window.__lastSnapshot.self.hp === window.__lastSnapshot.self.max_hp - 360""")
    for source in sources:
        source.locator('#skill').select_option('ally-renew')
        expect(source.locator('#cast')).to_be_disabled()
        source.locator('#allyTarget').select_option(label=target_id)
        source.locator('#cast').click()

    status_filter = "s => s.definition_id === 'ally-renewal'"
    target.wait_for_function(f"count => {own}.statuses.filter({status_filter}).length === count", arg=len(sources))
    statuses = target.evaluate(f"{own}.statuses.filter({status_filter})")
    instances = [next(row for row in statuses if row['source']['id'] == id) for id in source_ids]
    assert len({row['instance_id'] for row in instances}) == len(sources)
    for source, instance in zip(sources, instances):
        assert instance['target'] == target_entity
        assert instance['source'] == source.evaluate(own + '.participant.entity')
        assert instance['periodic']['raw_heal'] == first
        for page in (source, target):
            page.wait_for_function("""id => window.__probeEvents.some(row =>
                row.event?.kind === 'status_applied' && row.event.status.instance_id === id)
            """, arg=instance['instance_id'])
        assert len(source.evaluate(notices, ['status_applied', instance['instance_id']])) == 1
        assert len(target.evaluate(notices, ['status_applied', instance['instance_id']])) == 1
    assert all(page.evaluate(f"{own}.statuses.filter({status_filter}).length") == 0
               for page in pages if page != target)

    first_hp = target.evaluate(own + '.max_hp') - 360 + first * len(sources)
    target.wait_for_function(f"hp => {own}.hp === hp", arg=first_hp)
    has_event = """([kind, id, count]) => window.__probeEvents.filter(row =>
        row.event?.kind === kind && (row.event.instance_id === id ||
        row.event.status?.instance_id === id)).length >= count"""
    first_amounts = []
    for source, instance in zip(sources, instances):
        label = f"友方持续治疗 · {instance['source']['id']} → {target_id} · 第 1 跳 · 实回 {first}"
        expect(source.locator('#lastAllyHeal')).to_have_text(label)
        target.wait_for_function(has_event, arg=['periodic_heal_tick', instance['instance_id'], 1])
        tick_rows = target.evaluate(notices, ['periodic_heal_tick', instance['instance_id']])
        assert len(tick_rows) == 1
        assert tick_rows == source.evaluate(notices, ['periodic_heal_tick', instance['instance_id']])
        first_amounts.append(tick_rows[0]['hp_restored'])
    assert first_amounts == [first] * len(sources)
    for page in pages:
        if page != target and page not in sources:
            expect(page.locator('#lastAllyHeal')).to_be_hidden()

    ended = instances[0]
    reason = 'source_invalid' if leaving_role == 'source' else 'target_invalid'
    leaver, observer = (caster, target) if leaving_role == 'source' else (target, caster)
    leaver.locator('#leave').click()
    observer.wait_for_function("""([id, reason]) => window.__probeEvents.some(row =>
        row.event?.kind === 'status_removed' && row.event.reason === reason &&
        row.event.instance_ids.includes(id))""", arg=[ended['instance_id'], reason])
    continued = None
    if leaving_role == 'source':
        survivor = instances[1]
        target.wait_for_function(f"""([id, hp]) => {{
            const active = {own}.statuses.filter({status_filter});
            return {own}.hp === hp && active.length === 1 && active[0].instance_id === id;
        }}""", arg=[survivor['instance_id'], first_hp + first])
        expect(second.locator('#lastAllyHeal')).to_contain_text(f'第 2 跳 · 实回 {first}')
        target.wait_for_function(has_event, arg=['periodic_heal_tick', survivor['instance_id'], 2])
        continued = target.evaluate(notices, ['periodic_heal_tick', survivor['instance_id']])[1]['hp_restored']
        assert continued == first
    next_due = ended['applied_at_ms'] + ended['periodic']['interval_ms'] * 2
    observer.wait_for_function('due => window.__lastSnapshot.logical_ms >= due', arg=next_due)
    for source, instance in zip(sources, instances):
        for page in pages:
            if page not in (source, target):
                for event in ('status_applied', 'periodic_heal_tick', 'status_removed'):
                    assert page.evaluate(notices, [event, instance['instance_id']]) == []
    for participant in (caster, target):
        assert len(participant.evaluate(notices, ['periodic_heal_tick', ended['instance_id']])) == 1
    return {'sources': source_ids, 'target': target_id, 'firstAmounts': first_amounts,
            'reason': reason, 'continued': continued, 'endedTicks': 1}


def verify_ally_shield(pages, expected):
    granted, leaving_role = expected
    caster, target = pages[:2]
    own = 'window.__lastSnapshot.self'
    source_entity = caster.evaluate(own + '.participant.entity')
    target_entity = target.evaluate(own + '.participant.entity')
    source_id, target_id = source_entity['id'], target_entity['id']
    caster.locator('#skill').select_option('ally-guard')
    expect(caster.locator('#cast')).to_be_disabled()
    caster.locator('#allyTarget').select_option(label=target_id)
    caster.locator('#cast').click()
    status_filter = "s => s.definition_id === 'ally-barrier'"
    target.wait_for_function(f"{own}.statuses.some({status_filter})")
    instance = target.evaluate(f"{own}.statuses.find({status_filter})")
    instance_id = instance['instance_id']
    assert instance['source'] == source_entity and instance['target'] == target_entity
    label = f"友方护盾 · {source_id} → {target_id} · 实授 {granted}"
    expect(caster.locator('#lastAllyShield')).to_have_text(label)
    expect(target.locator('#lastAllyShield')).to_have_text(label)
    for page in pages:
        if page != target:
            assert page.evaluate(own + '.shield_total') == 0
        if page not in (caster, target):
            expect(page.locator('#lastAllyShield')).to_be_hidden()
    probe = """([id, target]) => {
        const rows = window.__probeEvents.map(row => row.event ?? row);
        const at = rows.findIndex(event => event.kind === 'shield_absorbed' &&
            event.consumptions.some(slice => slice.instance_id === id));
        if (at < 0) return null;
        const hit = rows.slice(at + 1).find(event => event.kind === 'damage' &&
            event.source.id === 'shield-probe' && event.target.id === target);
        return hit ? [rows[at], hit] : null;
    }"""
    target.wait_for_function(probe, arg=[instance_id, target_id])
    shield_hit, damage = target.evaluate(probe, [instance_id, target_id])
    absorbed = shield_hit['amount']
    remaining = granted - absorbed
    assert shield_hit['source'] == 'shield-probe'
    assert damage['amount'] == 0 and absorbed > 0 and remaining > 0
    assert shield_hit['consumptions'] == [{'instance_id': instance_id, 'absorbed': absorbed,
                                         'remaining': remaining}]
    target.wait_for_function(f"amount => {own}.shield_total === amount", arg=remaining)
    expect(target.locator('#own')).to_contain_text(f'本人护盾 {remaining}')
    assert caster.evaluate(probe, [instance_id, target_id]) is None

    leaver, observer = (caster, target) if leaving_role == 'source' else (target, caster)
    reason = 'source_invalid' if leaving_role == 'source' else 'target_invalid'
    leaver.locator('#leave').click()
    observer.wait_for_function("""([id, reason]) => window.__probeEvents.some(row =>
        row.event?.kind === 'status_removed' && row.event.reason === reason &&
        row.event.instance_ids.includes(id))""", arg=[instance_id, reason])
    after_exit_damage = None
    if leaving_role == 'source':
        after_exit = """([id, target]) => {
            const rows = window.__probeEvents.map(row => row.event ?? row);
            const at = rows.findIndex(event => event.kind === 'status_removed' &&
                event.reason === 'source_invalid' && event.instance_ids.includes(id));
            if (at < 0 || window.__lastSnapshot.self.shield_total !== 0) return null;
            return rows.slice(at + 1).find(event => event.kind === 'damage' &&
                event.source.id === 'shield-probe' && event.target.id === target && event.amount > 0)?.amount;
        }"""
        target.wait_for_function(after_exit, arg=[instance_id, target_id])
        after_exit_damage = target.evaluate(after_exit, [instance_id, target_id])
        assert after_exit_damage == absorbed
    for peer in pages[2:]:
        assert not peer.evaluate("""id => window.__probeEvents.some(row =>
            row.event?.instance_id === id || row.event?.status?.instance_id === id ||
            row.event?.consumptions?.some(slice => slice.instance_id === id) ||
            row.event?.instance_ids?.includes(id))""", instance_id)
    return {'source': source_id, 'target': target_id, 'granted': granted, 'absorbed': absorbed,
            'hpDamage': damage['amount'], 'shieldRemaining': remaining, 'reason': reason,
            'afterExitDamage': after_exit_damage}


def verify_ally_hot_overheal_shield(pages, expected):
    missing, leaving_role, expected_ticks = expected
    caster, target = pages[:2]
    own = 'window.__lastSnapshot.self'
    source_entity = caster.evaluate(own + '.participant.entity')
    target_entity = target.evaluate(own + '.participant.entity')
    if missing:
        target.locator('#selfCast').click()
    target.wait_for_function(f"missing => {own}.hp === {own}.max_hp - missing", arg=missing)
    caster.locator('#skill').select_option('ally-renew')
    expect(caster.locator('#cast')).to_be_disabled()
    caster.locator('#allyTarget').select_option(label=target_entity['id'])
    caster.locator('#cast').click()

    observe = """() => window.__probeEvents.map(row => row.event).filter(event =>
        (event?.kind === 'periodic_heal_tick' && event.definition_id === 'ally-renewal') ||
        (event?.kind === 'shield_granted' && event.definition_id === 'overflow-shield'))"""
    total = sum(row[1] for row in expected_ticks)
    count = len(expected_ticks) + sum(row[1] > 0 for row in expected_ticks)
    target.wait_for_function(f"""([total, count]) => ({observe})().length >= count &&
        {own}.shield_total === total && {own}.hp === {own}.max_hp""", arg=[total, count])
    caster.wait_for_function(f"count => ({observe})().length >= count", arg=count)
    events = target.evaluate(observe)
    assert events == caster.evaluate(observe)
    assert all(page.evaluate(observe) == [] for page in pages[2:])
    status_filter = "s => ['ally-renewal', 'overflow-shield'].includes(s.definition_id)"
    instances = target.evaluate(f"{own}.statuses.filter({status_filter})")
    assert len(instances) == 2
    hot = next(row for row in instances if row['definition_id'] == 'ally-renewal')
    shield = next(row for row in instances if row['definition_id'] == 'overflow-shield')
    assert all(row['source'] == source_entity and row['target'] == target_entity for row in [*events, *instances])
    ticks = []
    for row in events:
        if row['kind'] == 'periodic_heal_tick':
            assert row['instance_id'] == hot['instance_id'] and row['tick_index'] == len(ticks) + 1
            assert row['outcome'] == 'applied'
            ticks.append([row['hp_restored'], 0])
        else:
            assert row['instance_id'] == shield['instance_id'] and ticks[-1][1] == 0
            ticks[-1][1] = row['granted']
    assert ticks == expected_ticks
    assert all(page.evaluate(f"{own}.statuses.filter({status_filter}).length") == 0
               and page.evaluate(own + '.shield_total') == 0 for page in [caster, *pages[2:]])
    expect(target.locator('#own')).to_contain_text(f'本人护盾 {total}')
    expect(caster.locator('#lastAllyHeal')).to_contain_text('第 3 跳')
    for peer in pages[2:]:
        expect(peer.locator('#lastAllyShield')).to_be_hidden()

    leaver, observer = (caster, target) if leaving_role == 'source' else (target, caster)
    reason = 'source_invalid' if leaving_role == 'source' else 'target_invalid'
    ids = [row['instance_id'] for row in instances]
    leaver.locator('#leave').click()
    observer.wait_for_function("""([ids, reason]) => window.__probeEvents.some(row =>
        row.event?.kind === 'status_removed' && row.event.reason === reason &&
        ids.every(id => row.event.instance_ids.includes(id)))""", arg=[ids, reason])
    if leaving_role == 'source':
        target.wait_for_function(f"{own}.shield_total === 0 && !{own}.statuses.some({status_filter})")
    assert all(not peer.evaluate("""ids => window.__probeEvents.some(row =>
        row.event?.instance_ids?.some(id => ids.includes(id)))""", ids) for peer in pages[2:])
    return {'source': source_entity['id'], 'target': target_entity['id'], 'ticks': ticks,
            'reason': reason, 'cleared': len(ids)}


def verify_npc_silence(pages, skill_id):
    actor, peer = pages[:2]
    own = 'window.__lastSnapshot.self'
    before = actor.evaluate(own)
    target = before['participant']['entity']
    actor.locator('#selfCast').click()
    actor.wait_for_function(f"skill => {own}.pending_action?.skill_id === skill", arg=skill_id)
    actor.wait_for_function(f"""{own}.pending_action === null &&
        {own}.statuses.some(row => row.attribute === 'silence') &&
        window.__probeEvents.some(row => row.event?.kind === 'action_interrupted')""")
    silence = actor.evaluate(f"{own}.statuses.find(row => row.attribute === 'silence')")
    assert silence['source']['id'] == 'silence-probe' and silence['target'] == target
    assert silence['expires_at_ms'] - silence['applied_at_ms'] == 2800
    indices = actor.evaluate("""([id, target]) => {
        const events = window.__probeEvents;
        return [
            events.findIndex(row => row.kind === 'damage' && row.source.id === 'silence-probe' && row.target.id === target),
            events.findIndex(row => row.event?.kind === 'status_applied' && row.event.status.instance_id === id),
            events.findIndex(row => row.event?.kind === 'action_interrupted'),
        ];
    }""", [silence['instance_id'], target['id']])
    assert 0 <= indices[0] < indices[1] < indices[2]
    assert actor.evaluate(own + '.spirit') == before['spirit']
    assert actor.evaluate(f"skill => {own}.player_skills.find(row => row.skill_id === skill).ready_at_ms", skill_id) == 0
    expect(actor.locator('#own')).to_contain_text('沉默中')
    expect(actor.locator('#selfCast')).to_be_disabled()
    expect(peer.locator('#selfCast')).to_be_enabled()
    position = actor.evaluate(own + '.position')
    actor.keyboard.down('d')
    try:
        actor.wait_for_function(f"position => {own}.position.some((v, i) => v !== position[i])", arg=position)
    finally:
        actor.keyboard.up('d')
    actor.locator('#attack').click()
    actor.wait_for_function('window.__basicCommits === 1')
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < silence['expires_at_ms']
    actor.wait_for_function("""([id, until]) => window.__lastSnapshot.logical_ms >= until &&
        !window.__lastSnapshot.self.statuses.some(row => row.instance_id === id) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'expired' && row.event.instance_ids.includes(id))""",
        arg=[silence['instance_id'], silence['expires_at_ms']])
    expect(actor.locator('#selfCast')).to_be_enabled()
    actor.locator('#selfCast').click()
    actor.wait_for_function("""skill => window.__probeEvents.filter(row =>
        row.event?.kind === 'action_committed' && row.event.skill_id === skill).length === 1""", arg=skill_id)
    for page in pages[1:]:
        assert not page.evaluate("""id => window.__probeEvents.some(row =>
            row.event?.status?.instance_id === id || row.event?.instance_ids?.includes(id))""", silence['instance_id'])
    return {'source': silence['source']['id'], 'target': target['id'], 'durationMs': 2800,
            'interrupted': True, 'basic': True, 'resumed': True}


def verify_control_immunity(pages, skill_id):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    target = actor.evaluate(own + '.participant.entity')
    actor.locator('#skill').select_option('protect-self')
    actor.locator('#cast').click()
    actor.wait_for_function(f"{own}.statuses.some(row => row.attribute === 'control_immunity')")
    guard = actor.evaluate(f"{own}.statuses.find(row => row.attribute === 'control_immunity')")
    assert guard['source'] == guard['target'] == target
    assert guard['expires_at_ms'] - guard['applied_at_ms'] == 2600
    expect(actor.locator('#own')).to_contain_text('控制免疫中')
    actor.locator('#selfCast').click()
    actor.wait_for_function(f"""skill => {own}.pending_action?.skill_id === skill &&
        window.__probeEvents.some(row => row.event?.kind === 'status_rejected' && row.event.reason === 'IMMUNE')""",
        arg=skill_id)
    expect(actor.locator('#lastControlRejection')).to_have_text(f"已免疫控制 · silence-probe → {target['id']}")
    first_damage, rejection = actor.evaluate("""target => {
        const rows = window.__probeEvents;
        const at = rows.findIndex(row => row.event?.kind === 'status_rejected' && row.event.reason === 'IMMUNE');
        return [rows.slice(0, at).find(row => row.kind === 'damage' && row.source.id === 'silence-probe'
            && row.target.id === target)?.amount, rows[at].event];
    }""", target['id'])
    assert first_damage > 0 and rejection['definition_ids'] == ['npc-silence']
    assert rejection['source']['id'] == 'silence-probe' and rejection['target'] == target
    assert not actor.evaluate(f"{own}.statuses.some(row => row.attribute === 'silence')")
    actor.wait_for_function(f"""skill => !{own}.statuses.some(row => row.attribute === 'control_immunity') &&
        {own}.statuses.some(row => row.attribute === 'silence') && window.__probeEvents.some(row =>
            row.event?.kind === 'action_committed' && row.event.skill_id === skill)""", arg=skill_id)
    silence = actor.evaluate(f"{own}.statuses.find(row => row.attribute === 'silence')")
    assert [silence['source'], silence['target']] == [rejection['source'], rejection['target']]
    assert silence['applied_at_ms'] >= guard['expires_at_ms']
    expect(actor.locator('#selfCast')).to_be_disabled()
    expect(actor.locator('#own')).to_contain_text('沉默中')
    order = actor.evaluate("""([guard, silence, skill]) => {
        const rows = window.__probeEvents.map(row => row.event);
        return [
            rows.findIndex(row => row?.kind === 'status_removed' && row.reason === 'expired' && row.instance_ids.includes(guard)),
            rows.findIndex(row => row?.kind === 'action_committed' && row.skill_id === skill),
            rows.findIndex(row => row?.kind === 'status_applied' && row.status.instance_id === silence),
        ];
    }""", [guard['instance_id'], silence['instance_id'], skill_id])
    assert 0 <= order[0] < order[2] and 0 <= order[1] < order[2]
    for page in pages[1:]:
        expect(page.locator('#lastControlRejection')).to_be_hidden()
        assert not page.evaluate("""ids => window.__probeEvents.some(row =>
            ids.includes(row.event?.status?.instance_id) || row.event?.reason === 'IMMUNE')""",
            [guard['instance_id'], silence['instance_id']])
    return {'source': rejection['source']['id'], 'target': target['id'], 'durationMs': 2600,
            'firstDamage': first_damage, 'protectedCommit': True, 'appliedAfterExpiry': True}


def verify_first_control_immunity(pages, skill_id):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    target = actor.evaluate(own + '.participant.entity')
    actor.locator('#skill').select_option('guard-first')
    actor.locator('#cast').click()
    actor.wait_for_function(f"{own}.statuses.some(row => row.attribute === 'first_control_immunity')")
    guard = actor.evaluate(f"{own}.statuses.find(row => row.attribute === 'first_control_immunity')")
    assert guard['source'] == guard['target'] == target
    assert guard['expires_at_ms'] - guard['applied_at_ms'] == 10000
    expect(actor.locator('#own')).to_contain_text('首控免疫可用')
    actor.locator('#selfCast').click()
    actor.wait_for_function(f"""skill => {own}.pending_action?.skill_id === skill &&
        window.__probeEvents.some(row => row.event?.kind === 'status_rejected' && row.event.reason === 'IMMUNE')""",
        arg=skill_id)
    expect(actor.locator('#lastControlRejection')).to_have_text(f"已免疫控制 · silence-probe → {target['id']}")
    first_damage, rejection = actor.evaluate("""target => {
        const rows = window.__probeEvents;
        const at = rows.findIndex(row => row.event?.kind === 'status_rejected' && row.event.reason === 'IMMUNE');
        return [rows.slice(0, at).find(row => row.kind === 'damage' && row.source.id === 'silence-probe'
            && row.target.id === target)?.amount, rows[at].event];
    }""", target['id'])
    assert first_damage > 0 and rejection['definition_ids'] == ['npc-silence']
    assert rejection['source']['id'] == 'silence-probe' and rejection['target'] == target
    actor.wait_for_function(f"""id => !{own}.statuses.some(row => row.instance_id === id) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'depleted' && row.event.instance_ids.includes(id))""", arg=guard['instance_id'])
    assert not actor.evaluate(f"{own}.statuses.some(row => row.attribute === 'silence')")
    expect(actor.locator('#own')).not_to_contain_text('首控免疫可用')
    actor.wait_for_function(f"""skill => !{own}.statuses.some(row => row.attribute === 'first_control_immunity') &&
        {own}.statuses.some(row => row.attribute === 'silence') && window.__probeEvents.some(row =>
            row.event?.kind === 'action_committed' && row.event.skill_id === skill)""", arg=skill_id)
    silence = actor.evaluate(f"{own}.statuses.find(row => row.attribute === 'silence')")
    assert [silence['source'], silence['target']] == [rejection['source'], rejection['target']]
    assert silence['applied_at_ms'] < guard['expires_at_ms']
    assert silence['expires_at_ms'] - silence['applied_at_ms'] == 2800
    expect(actor.locator('#selfCast')).to_be_disabled()
    expect(actor.locator('#own')).to_contain_text('沉默中')
    order = actor.evaluate("""([guard, silence, skill]) => {
        const rows = window.__probeEvents.map(row => row.event);
        return [
            rows.findIndex(row => row?.kind === 'status_rejected' && row.reason === 'IMMUNE'),
            rows.findIndex(row => row?.kind === 'status_removed' && row.reason === 'depleted' && row.instance_ids.includes(guard)),
            rows.findIndex(row => row?.kind === 'action_committed' && row.skill_id === skill),
            rows.findIndex(row => row?.kind === 'status_applied' && row.status.instance_id === silence),
        ];
    }""", [guard['instance_id'], silence['instance_id'], skill_id])
    assert 0 <= order[0] < order[1] < order[2] < order[3]
    for page in pages[1:]:
        expect(page.locator('#lastControlRejection')).to_be_hidden()
        assert not page.evaluate("""ids => window.__probeEvents.some(row =>
            ids.includes(row.event?.status?.instance_id) ||
            row.event?.instance_ids?.some(id => ids.includes(id)) || row.event?.reason === 'IMMUNE')""",
            [guard['instance_id'], silence['instance_id']])
    return {'source': rejection['source']['id'], 'target': target['id'], 'durationMs': 10000,
            'firstDamage': first_damage, 'protectedCommit': True, 'firstGuardConsumed': True,
            'secondDurationMs': 2800, 'secondBeforeOriginalExpiry': True}


def verify_control_duration_reduction(pages, expected):
    bps, first_duration = expected
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    target = actor.evaluate(own + '.participant.entity')
    actor.locator('#skill').select_option('shorten-self')
    actor.locator('#cast').click()
    reduction = f"{own}.statuses.find(row => row.attribute === 'control_duration_reduction')"
    silence = f"{own}.statuses.find(row => row.attribute === 'silence')"
    actor.wait_for_function(f"Boolean({reduction})")
    guard = actor.evaluate(reduction)
    assert guard['source'] == guard['target'] == target
    assert guard['modifier_bps'] == bps
    assert guard['expires_at_ms'] - guard['applied_at_ms'] == 2200
    expect(actor.locator('#own')).to_contain_text('控制时长减免中')
    actor.wait_for_function(f"Boolean({silence})")
    first = actor.evaluate(silence)
    assert first['source']['id'] == 'silence-probe' and first['target'] == target
    assert first['expires_at_ms'] - first['applied_at_ms'] == first_duration
    assert first['applied_at_ms'] < guard['expires_at_ms'] < first['expires_at_ms']
    expect(actor.locator('#selfCast')).to_be_disabled()
    expect(actor.locator('#own')).to_contain_text('沉默中')
    actor.wait_for_function(f"!({reduction})")
    assert actor.evaluate(silence) == first
    actor.wait_for_function(f"""id => !({silence}) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'expired' && row.event.instance_ids.includes(id))""", arg=first['instance_id'])
    logical_ms = actor.evaluate('window.__lastSnapshot.logical_ms')
    assert first['expires_at_ms'] <= logical_ms < first['applied_at_ms'] + 2800
    expect(actor.locator('#selfCast')).to_be_enabled()
    actor.wait_for_function(f"Boolean({silence})")
    second = actor.evaluate(silence)
    assert second['instance_id'] != first['instance_id']
    assert second['source'] == first['source'] and second['target'] == target
    assert second['expires_at_ms'] - second['applied_at_ms'] == 2800
    ids = [state['instance_id'] for state in (guard, first, second)]
    order = actor.evaluate("""([guard, first, second]) => {
        const events = window.__probeEvents.map(row => row.event);
        const applied = id => events.findIndex(event =>
            event?.kind === 'status_applied' && event.status.instance_id === id);
        const expired = id => events.findIndex(event =>
            event?.kind === 'status_removed' && event.reason === 'expired' && event.instance_ids.includes(id));
        return [applied(guard), applied(first), expired(guard), expired(first), applied(second)];
    }""", ids)
    assert 0 <= order[0] < order[1] < order[2] < order[3] < order[4]
    for peer in pages[1:]:
        assert not peer.evaluate("""ids => window.__probeEvents.some(row =>
            ids.includes(row.event?.status?.instance_id) ||
            row.event?.instance_ids?.some(id => ids.includes(id))) ||
            window.__lastSnapshot.self.statuses.some(row =>
                ['control_duration_reduction', 'silence'].includes(row.attribute))""", ids)
    return {'source': first['source']['id'], 'target': target['id'], 'reductionBps': bps,
            'firstDurationMs': first_duration, 'secondDurationMs': 2800, 'frozen': True}


def verify_item_cleanse(pages, item):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    silence = f"{own}.statuses.find(row => row.attribute === 'silence')"
    target = actor.evaluate(own + '.participant.entity')
    actor.locator('#useItemChoice').select_option(item)
    actor.wait_for_function(f"Boolean({silence})")
    first = actor.evaluate(silence)
    assert first['source']['id'] == 'silence-probe' and first['target'] == target
    expect(actor.locator('#selfCast')).to_be_disabled()
    expect(actor.locator('#useItem')).to_have_text('使用净化道具')
    expect(actor.locator('#useItem')).to_be_enabled()
    before = actor.evaluate(own)
    actor.locator('#useItem').click()
    actor.wait_for_function(f"""([id, item, revision]) => !({silence}) &&
        {own}.inventory_revision === revision + 1 &&
        {own}.inventory.some(row => row.item_id === item && row.quantity === 1) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'cleansed' && row.event.instance_ids.includes(id))""",
        arg=[first['instance_id'], item, before['inventory_revision']])
    after = actor.evaluate(own)
    assert (after['hp'], after['spirit']) == (before['hp'], before['spirit'])
    expect(actor.locator('#lastItemUse')).to_have_text('已净化状态')
    assert actor.evaluate('window.__lastItemUsed.hp_restored') == 0
    assert actor.evaluate('window.__lastItemUsed.spirit_restored === undefined')
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < first['expires_at_ms']
    expect(actor.locator('#useItem')).to_be_disabled()
    expect(actor.locator('#selfCast')).to_be_enabled()
    actor.locator('#selfCast').click()
    actor.wait_for_function("""window.__probeEvents.some(row =>
        row.event?.kind === 'action_committed' && row.event.skill_id?.startsWith('self-'))""")
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < first['expires_at_ms']
    actor.wait_for_function(f"Boolean({silence})")
    second = actor.evaluate(silence)
    assert second['instance_id'] != first['instance_id']
    assert second['source'] == first['source'] and second['target'] == target
    assert second['expires_at_ms'] - second['applied_at_ms'] == 2800
    ids = [first['instance_id'], second['instance_id']]
    for peer in pages[1:]:
        assert not peer.evaluate("""ids => window.__probeEvents.some(row =>
            ids.includes(row.event?.status?.instance_id) ||
            row.event?.instance_ids?.some(id => ids.includes(id)) ||
            row.event?.kind === 'item_used') ||
            window.__lastSnapshot.self.statuses.some(row => row.attribute === 'silence')""", ids)
    return {'source': first['source']['id'], 'target': target['id'],
            'cleansed': True, 'resumed': True, 'secondDurationMs': 2800}


def verify_npc_root(pages, skill):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    root = f"{own}.statuses.find(row => row.attribute === 'root')"
    target = actor.evaluate(own + '.participant.entity')
    actor.wait_for_function(f"Boolean({root})")
    state = actor.evaluate(root)
    assert state['source']['id'] == 'root-probe' and state['target'] == target
    assert state['expires_at_ms'] - state['applied_at_ms'] == 2800
    position = actor.evaluate(own + '.position')
    expect(actor.locator('#own')).to_contain_text('定身中')
    expect(actor.locator('#toExit')).to_be_disabled()
    expect(actor.locator('#selfCast')).to_be_enabled()
    actor.keyboard.down('d')
    try:
        actor.locator('#selfCast').click()
        actor.wait_for_function("""skill => window.__probeEvents.some(row =>
            row.event?.kind === 'action_committed' && row.event.skill_id === skill) &&
            window.__lastSnapshot.logical_ms >= window.__lastSnapshot.self.recovery_until_ms""", arg=skill)
    finally:
        actor.keyboard.up('d')
    actor.locator('#attack').click()
    actor.wait_for_function('window.__basicCommits === 1')
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < state['expires_at_ms']
    assert actor.evaluate(own + '.position') == position
    actor.wait_for_function(f"""([id, end]) => !({root}) &&
        window.__lastSnapshot.logical_ms >= end &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'expired' && row.event.instance_ids.includes(id))""",
        arg=[state['instance_id'], state['expires_at_ms']])
    assert actor.evaluate(own + '.position') == position
    expect(actor.locator('#toExit')).to_be_enabled()
    actor.keyboard.down('d')
    try:
        actor.wait_for_function(f"position => {own}.position.some((value, i) => value !== position[i])",
                                arg=position)
    finally:
        actor.keyboard.up('d')
    for peer in pages[1:]:
        assert not peer.evaluate("""id => window.__probeEvents.some(row =>
            row.event?.status?.instance_id === id || row.event?.instance_ids?.includes(id)) ||
            window.__lastSnapshot.self.statuses.some(row => row.attribute === 'root')""", state['instance_id'])
    return {'source': state['source']['id'], 'target': target['id'], 'durationMs': 2800,
            'stationary': True, 'cast': True, 'basic': True, 'movedAfterExpiry': True}


def verify_npc_stun(pages, skill):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    stun = f"{own}.statuses.find(row => row.attribute === 'stun')"
    before = actor.evaluate(own)
    target = before['participant']['entity']
    actor.locator('#selfCast').click()
    actor.wait_for_function(f"skill => {own}.pending_action?.skill_id === skill", arg=skill)
    actor.wait_for_function(f"""Boolean({stun}) && !{own}.pending_action &&
        window.__probeEvents.some(row => row.event?.kind === 'action_interrupted')""")
    state = actor.evaluate(stun)
    assert state['source']['id'] == 'stun-probe' and state['target'] == target
    assert state['expires_at_ms'] - state['applied_at_ms'] == 2800
    after = actor.evaluate(own)
    assert after['hp'] < before['hp'] and after['spirit'] == before['spirit']
    assert next(row for row in after['player_skills'] if row['skill_id'] == skill)['ready_at_ms'] == 0
    expect(actor.locator('#own')).to_contain_text('眩晕中')
    for control in ('#selfCast', '#attack', '#toExit', '#extract'):
        expect(actor.locator(control)).to_be_disabled()
    expect(actor.locator('#stop')).to_be_enabled()
    assert after['position'] == before['position']
    actor.keyboard.down('d')
    try:
        actor.wait_for_function(f"""([id, end]) => !({stun}) &&
            window.__lastSnapshot.logical_ms >= end &&
            window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
                row.event.reason === 'expired' && row.event.instance_ids.includes(id))""",
            arg=[state['instance_id'], state['expires_at_ms']])
        assert actor.evaluate(own + '.position') == before['position']
    finally:
        actor.keyboard.up('d')
    assert not actor.evaluate("""skill => window.__probeEvents.some(row =>
        row.event?.kind === 'action_committed' && row.event.skill_id === skill)""", skill)
    expect(actor.locator('#toExit')).to_be_enabled()
    expect(actor.locator('#selfCast')).to_be_enabled()
    actor.keyboard.down('d')
    try:
        actor.wait_for_function(f"position => {own}.position.some((value, i) => value !== position[i])",
                                arg=before['position'])
    finally:
        actor.keyboard.up('d')
    actor.locator('#selfCast').click()
    actor.wait_for_function("""skill => window.__probeEvents.filter(row =>
        row.event?.kind === 'action_committed' && row.event.skill_id === skill).length === 1""", arg=skill)
    for peer in pages[1:]:
        assert not peer.evaluate("""id => window.__probeEvents.some(row =>
            row.event?.status?.instance_id === id || row.event?.instance_ids?.includes(id)) ||
            window.__lastSnapshot.self.statuses.some(row => row.attribute === 'stun')""", state['instance_id'])
    return {'source': state['source']['id'], 'target': target['id'], 'durationMs': 2800,
            'stationary': True, 'interrupted': True, 'noCost': True,
            'castAfterExpiry': True, 'movedAfterExpiry': True}


def verify_npc_movement_slow(pages, expected):
    skill, bps, expected_speed = expected
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    slow = f"{own}.statuses.find(row => row.attribute === 'movement_slow')"
    target = actor.evaluate(own + '.participant.entity')

    def speed(expected_speed):
        start_ms, start = actor.evaluate(f"[window.__lastSnapshot.logical_ms, {own}.position]")
        actor.wait_for_function("at => window.__lastSnapshot.logical_ms >= at + 180", arg=start_ms)
        end_ms, end = actor.evaluate(f"[window.__lastSnapshot.logical_ms, {own}.position]")
        actual = math.dist(start, end) / ((end_ms - start_ms) / 1000)
        assert abs(actual - expected_speed) < 1e-6, (actual, expected_speed)
        return round(actual)

    actor.locator('#selfCast').click()
    actor.wait_for_function(f"skill => {own}.pending_action?.skill_id === skill", arg=skill)
    actor.wait_for_function(f"Boolean({slow})")
    state = actor.evaluate(slow)
    assert state['source']['id'] == 'slow-probe' and state['target'] == target
    assert state['modifier_bps'] == bps
    assert state['expires_at_ms'] - state['applied_at_ms'] == 2800
    assert actor.evaluate(own + '.pending_action.skill_id') == skill
    expect(actor.locator('#own')).to_contain_text('减速中')
    position = actor.evaluate(own + '.position')
    actor.keyboard.down('d')
    try:
        actor.wait_for_function(f"position => {own}.position.some((value, i) => value !== position[i])",
                                arg=position)
        slowed_speed = speed(expected_speed)
        actor.wait_for_function("""skill => window.__probeEvents.some(row =>
            row.event?.kind === 'action_committed' && row.event.skill_id === skill)""", arg=skill)
        assert actor.evaluate('window.__lastSnapshot.logical_ms') < state['expires_at_ms']
        actor.wait_for_function(f"""([id, end]) => !({slow}) &&
            window.__lastSnapshot.logical_ms >= end &&
            window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
                row.event.reason === 'expired' && row.event.instance_ids.includes(id))""",
            arg=[state['instance_id'], state['expires_at_ms']])
        restored_speed = speed(180)
    finally:
        actor.keyboard.up('d')
    expect(actor.locator('#own')).not_to_contain_text('减速中')
    assert not actor.evaluate("window.__probeEvents.some(row => row.event?.kind === 'action_interrupted')")
    for peer in pages[1:]:
        assert not peer.evaluate("""id => window.__probeEvents.some(row =>
            row.event?.status?.instance_id === id || row.event?.instance_ids?.includes(id)) ||
            window.__lastSnapshot.self.statuses.some(row => row.attribute === 'movement_slow')""", state['instance_id'])
    return {'source': state['source']['id'], 'target': target['id'], 'durationMs': 2800,
            'modifierBps': bps, 'slowedSpeed': slowed_speed, 'restoredSpeed': restored_speed,
            'castWhileSlowed': True, 'continuedMovement': True}


def verify_movement_haste(pages, expected):
    haste_bps, slow_bps, combined, slowed = expected
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    slow = f"{own}.statuses.find(row => row.attribute === 'movement_slow')"
    haste = f"{own}.statuses.find(row => row.attribute === 'movement_haste')"
    target = actor.evaluate(own + '.participant.entity')

    def speed(expected_speed):
        start_ms, start = actor.evaluate(f"[window.__lastSnapshot.logical_ms, {own}.position]")
        actor.wait_for_function("at => window.__lastSnapshot.logical_ms >= at + 180", arg=start_ms)
        end_ms, end = actor.evaluate(f"[window.__lastSnapshot.logical_ms, {own}.position]")
        actual = math.dist(start, end) / ((end_ms - start_ms) / 1000)
        assert abs(actual - expected_speed) < 1e-6, (actual, expected_speed)
        return round(actual)

    def expired(status, lookup):
        actor.wait_for_function(f"""([id, end]) => !({lookup}) &&
            window.__lastSnapshot.logical_ms >= end &&
            window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
                row.event.reason === 'expired' && row.event.instance_ids.includes(id))""",
            arg=[status['instance_id'], status['expires_at_ms']])

    actor.locator('#selfCast').click()
    actor.wait_for_function(f"Boolean({haste})")
    buff = actor.evaluate(haste)
    assert buff['source'] == buff['target'] == target
    assert buff['modifier_bps'] == haste_bps
    assert buff['expires_at_ms'] - buff['applied_at_ms'] == 3000
    expect(actor.locator('#own')).to_contain_text('加速中')
    actor.wait_for_function(f"Boolean({slow})")
    state = actor.evaluate(slow)
    assert state['source']['id'] == 'slow-probe' and state['target'] == target
    assert state['modifier_bps'] == slow_bps
    assert state['expires_at_ms'] - state['applied_at_ms'] == 2800
    assert state['applied_at_ms'] < buff['expires_at_ms'] < state['expires_at_ms']
    expect(actor.locator('#own')).to_contain_text('减速中')
    position = actor.evaluate(own + '.position')
    actor.keyboard.down('d')
    try:
        actor.wait_for_function(f"position => {own}.position.some((value, i) => value !== position[i])",
                                arg=position)
        combined_speed = speed(combined)
        expired(buff, haste)
        assert actor.evaluate(slow) == state
        expect(actor.locator('#own')).not_to_contain_text('加速中')
        slowed_speed = speed(slowed)
        expired(state, slow)
        restored_speed = speed(180)
    finally:
        actor.keyboard.up('d')
    expect(actor.locator('#own')).not_to_contain_text('减速中')
    ids = [state['instance_id'], buff['instance_id']]
    for peer in pages[1:]:
        assert not peer.evaluate("""ids => window.__probeEvents.some(row =>
            ids.includes(row.event?.status?.instance_id) ||
            row.event?.instance_ids?.some(id => ids.includes(id))) ||
            window.__lastSnapshot.self.statuses.some(row =>
                ['movement_haste', 'movement_slow'].includes(row.attribute))""", ids)
    return {'source': state['source']['id'], 'target': target['id'], 'modifierBps': slow_bps,
            'hasteBps': haste_bps, 'combinedSpeed': combined_speed, 'slowedSpeed': slowed_speed,
            'restoredSpeed': restored_speed, 'continuedMovement': True}


def verify_cooldown_reduction(pages, expected):
    bps, first_duration = expected
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    focus = f"{own}.statuses.find(row => row.attribute === 'cooldown_reduction')"
    marker = f"{own}.statuses.find(row => row.definition_id === 'demo-guard')"
    ready = f"{own}.player_skills.find(row => row.skill_id === 'cycle-probe').ready_at_ms"
    target = actor.evaluate(own + '.participant.entity')
    actor.locator('#skill').select_option('focus-cooldown')
    actor.locator('#cast').click()
    actor.wait_for_function(f"Boolean({focus})")
    buff = actor.evaluate(focus)
    assert buff['source'] == buff['target'] == target
    assert buff['modifier_bps'] == bps
    assert buff['expires_at_ms'] - buff['applied_at_ms'] == 900
    focus_ready = actor.evaluate(f"{own}.player_skills.find(row => row.skill_id === 'focus-cooldown').ready_at_ms")
    assert focus_ready - buff['applied_at_ms'] == 6000
    expect(actor.locator('#own')).to_contain_text('冷却缩减中')
    actor.locator('#skill').select_option('cycle-probe')
    actor.locator('#cast').click()
    actor.wait_for_function(f"Boolean({marker})")
    first = actor.evaluate(marker)
    deadline = actor.evaluate(ready)
    assert deadline - first['applied_at_ms'] == first_duration
    assert first['applied_at_ms'] < buff['expires_at_ms'] < deadline
    expect(actor.locator('#cast')).to_be_disabled()
    actor.wait_for_function(f"""id => !({focus}) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'expired' && row.event.instance_ids.includes(id))""", arg=buff['instance_id'])
    assert actor.evaluate(ready) == deadline
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < deadline
    expect(actor.locator('#cast')).to_be_disabled()
    expect(actor.locator('#own')).not_to_contain_text('冷却缩减中')
    expect(actor.locator('#cast')).to_be_enabled()
    actor.locator('#cast').click()
    actor.wait_for_function(f"previous => ({marker}).applied_at_ms > previous", arg=first['applied_at_ms'])
    second = actor.evaluate(marker)
    assert second['instance_id'] == first['instance_id']
    assert second['applied_at_ms'] >= deadline
    assert actor.evaluate(ready) - second['applied_at_ms'] == 2000
    assert actor.evaluate("""window.__probeEvents.filter(row => row.event?.kind === 'action_committed' &&
        row.event.skill_id === 'cycle-probe').length""") == 2
    ids = [buff['instance_id'], first['instance_id']]
    for peer in pages[1:]:
        assert not peer.evaluate("""ids => window.__probeEvents.some(row =>
            ids.includes(row.event?.status?.instance_id) ||
            row.event?.instance_ids?.some(id => ids.includes(id))) ||
            window.__lastSnapshot.self.statuses.some(row =>
                row.attribute === 'cooldown_reduction' || row.definition_id === 'demo-guard')""", ids)
    return {'source': target['id'], 'target': target['id'], 'reductionBps': bps,
            'firstDurationMs': first_duration, 'secondDurationMs': 2000, 'frozen': True}


def verify_damage_modifier(pages, bps, expected_damage, attribute, skill, label, baseline=66):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    focus = f"{own}.statuses.find(row => row.attribute === '{attribute}')"
    source = actor.evaluate(own + '.participant.entity')
    target = 'armor-target'
    damage_events = """window.__probeEvents.filter(row =>
        row.kind === 'damage' && row.source.id === 'P00' && row.target.id === 'armor-target')"""

    def attack():
        total = actor.evaluate(f"({damage_events}).length")
        actor.wait_for_function(f"window.__lastSnapshot.logical_ms >= {own}.basic_ready_at_ms")
        actor.locator('#attack').click()
        actor.wait_for_function(f"count => ({damage_events}).length === count", arg=total + 1)
        return actor.evaluate(f"({damage_events}).at(-1).amount")

    before = attack()
    actor.locator('#skill').select_option(skill)
    actor.locator('#cast').click()
    actor.wait_for_function(f"Boolean({focus})")
    buff = actor.evaluate(focus)
    assert buff['source'] == buff['target'] == source
    assert buff['modifier_bps'] == bps
    assert buff['expires_at_ms'] - buff['applied_at_ms'] == 1000
    expect(actor.locator('#own')).to_contain_text(label)
    boosted = attack()
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < buff['expires_at_ms']
    actor.wait_for_function(f"""id => !({focus}) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'expired' && row.event.instance_ids.includes(id))""", arg=buff['instance_id'])
    expect(actor.locator('#own')).not_to_contain_text(label)
    after = attack()
    damage = [before, boosted, after]
    assert damage == [baseline, expected_damage, baseline]
    assert actor.evaluate(f"({damage_events}).at(-1).hp") == 1000 - sum(damage)
    for peer in pages[1:]:
        assert not peer.evaluate("""([id, attribute]) => window.__probeEvents.some(row =>
            row.event?.status?.instance_id === id || row.event?.instance_ids?.includes(id)) ||
            window.__lastSnapshot.self.statuses.some(row => row.attribute === attribute)""",
            [buff['instance_id'], attribute])
    return {'source': source['id'], 'target': target, 'damage': damage}


def verify_critical_damage_bonus(pages, bps, damage):
    actor = pages[0]
    chance = "window.__lastSnapshot.self.statuses.find(row => row.attribute === 'critical_chance')"
    actor.locator('#skill').select_option('focus-critical')
    actor.locator('#cast').click()
    actor.wait_for_function(f"Boolean({chance})")
    critical = actor.evaluate(chance)
    assert critical['modifier_bps'] == 10000
    result = verify_damage_modifier(pages, bps, damage, 'critical_damage_bonus',
                                    'focus-critical-damage', '暴伤加成中', baseline=100)
    assert actor.evaluate(chance) == critical
    return result


def projectile_damage(actor, index, source, target):
    shots = f"""window.__probeEvents.filter(row =>
        row.kind === 'projectile_started' && row.projectile.source.id === '{source}')"""
    actor.wait_for_function(f"index => ({shots}).length > index", arg=index)
    projectile = actor.evaluate(f"index => ({shots})[index].projectile.id", index)
    actor.wait_for_function("""id => window.__probeEvents.some(row =>
        row.kind === 'projectile_ended' && row.projectile_id === id)""", arg=projectile)
    damage = actor.evaluate(f"""id => {{
        const rows = window.__probeEvents;
        const start = rows.findIndex(row => row.kind === 'projectile_started' && row.projectile.id === id);
        const end = rows.findIndex(row => row.kind === 'projectile_ended' && row.projectile_id === id);
        return rows.slice(start + 1, end).filter(row => row.kind === 'damage' && row.source.id === '{source}');
    }}""", projectile)
    assert len(damage) <= 1 and all(row['target'] == target for row in damage)
    return damage[0]['amount'] if damage else 0


def verify_action_totals(pages, cast):
    actor=pages[0]
    own='window.__lastSnapshot.self'
    source=actor.evaluate(own+'.participant.entity.id')
    targets=actor.evaluate("window.__lastSnapshot.monsters.filter(n=>n.template_id==='total-target')")
    assert len(targets)==2
    center=[sum(n['position'][i] for n in targets)/2 for i in range(2)]
    position=actor.evaluate(own+'.position')
    seal_skill=actor.locator('#skill option').evaluate_all("opts=>opts.some(x=>x.value==='next-bolt')?'next-bolt':'short-field'")
    cast(seal_skill,[position[0]-30,position[1]])
    actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='operation-seal')")
    tracked=actor.evaluate(own+".statuses.find(s=>s.definition_id==='operation-seal')")
    assert tracked['window']['recorded']==0
    before_spirit=actor.evaluate(own+'.spirit')
    totals=[]
    for index in range(2):
        start=actor.evaluate('window.__probeEvents.length')
        cast('small-pulse',center)
        actor.wait_for_function("start=>window.__probeEvents.slice(start).some(r=>r.kind==='area_started'&&r.area.skill_id==='small-pulse')",arg=start)
        area=actor.evaluate("start=>window.__probeEvents.slice(start).find(r=>r.kind==='area_started'&&r.area.skill_id==='small-pulse').area",start)
        action_id=actor.evaluate("start=>window.__probeEvents.slice(start).find(r=>r.event?.kind==='action_committed'&&r.event.skill_id==='small-pulse').event.action_id",start)
        actor.wait_for_function("id=>window.__probeEvents.some(r=>r.event?.kind==='combat_action_total'&&r.event.action_id===id)",arg=action_id)
        events=actor.evaluate('start=>window.__probeEvents.slice(start)',start)
        summary=next(r['event'] for r in events if r.get('event',{}).get('kind')=='combat_action_total' and r['event']['action_id']==action_id)
        summary_index=next(i for i,r in enumerate(events) if r.get('event')==summary)
        end_index=next(i for i,r in enumerate(events) if r.get('kind')=='area_ended' and r['area_id']==area['id'])
        tick_index=next(i for i,r in enumerate(events) if r.get('kind')=='area_tick' and r['area_id']==area['id'])
        assert tick_index<end_index<summary_index
        assert summary['fired'] and summary['contacts']==6 and summary['total_amount']==(1344 if len(pages)==4 else 1296)
        hits=[r for r in events[:summary_index] if r.get('kind')=='damage' and r['source']['id']==source and r['target']['id'] in [n['entity']['id'] for n in targets]]
        assert len(hits)==6 and sum(r['amount'] for r in hits)==summary['total_amount']
        heals=[r['event'] for r in events[:summary_index] if r.get('event',{}).get('kind')=='heal_resolved' and r['event']['heal_kind']=='triggered']
        assert len(heals)==1 and heals[0]['raw_heal']==summary['total_amount']//2
        rewards=actor.evaluate("window.__probeEvents.filter(r=>r.event?.kind==='combat_trigger_fired'&&r.event.trigger_id==='target-progress').length")
        assert rewards==(0 if index==0 else 2)
        totals.append(summary['total_amount'])
        ledger=actor.evaluate(own+".statuses.find(s=>s.definition_id==='operation-seal').window")
        assert ledger['recorded']==min(ledger['cap'],sum(totals)) and ledger['contacts']==6*(index+1)
        expect(actor.locator('#own')).to_contain_text('窗口记录')
        expect(actor.locator('#own')).to_contain_text('动作汇总')
        expect(actor.locator('#own')).to_contain_text('接触 6')
        if index==0:
            actor.reload(wait_until='domcontentloaded')
            actor.wait_for_function('window.__lastSnapshot?.self.ready')
    assert actor.evaluate(own+'.spirit')==min(actor.evaluate(own+'.max_spirit'),before_spirit+12)
    for peer in pages[1:]:
        assert not peer.evaluate("window.__probeEvents.some(r=>r.event?.kind==='combat_action_total')")
    window_probe=verify_window_recovery(pages,cast,tracked)
    return {'totals':totals,'contacts':[6,6],'rewards':2,'targetRewards':2,'reconnected':True,'windowProbe':window_probe}



def verify_window_recovery(pages, cast, tracked):
    actor,receiver=pages[:2]
    own='window.__lastSnapshot.self'
    source=actor.evaluate(own+'.participant.entity.id')
    target=receiver.evaluate(own+'.participant.entity.id')
    before_fee=actor.evaluate(own)
    primary=before_fee['player_skills'][0]['skill_id']
    fee_start=actor.evaluate('window.__probeEvents.length')
    position=before_fee['position']
    cast(primary,[position[0]-30,position[1]])
    committed="start=>window.__probeEvents.slice(start).find(r=>r.event?.kind==='action_committed')?.event"
    actor.wait_for_function(committed,arg=fee_start)
    fee=actor.evaluate(committed,fee_start)
    assert fee['skill_id']==primary
    # The window can restore HP before the next published snapshot. Derive the
    # pre-end deficit from the committed native costs, then verify fact order
    # and final resource conservation instead of waiting for a transient view.
    before={'hp':before_fee['hp']-fee['hp_cost'],
            'spirit':min(before_fee['max_spirit'],before_fee['spirit']-fee['spirit_cost']+1),
            'max_hp':before_fee['max_hp'],'max_spirit':before_fee['max_spirit']}
    event="id=>window.__probeEvents.find(r=>r.event?.kind==='window_record_ended'&&r.event.instance_id===id)?.event"
    actor.wait_for_function(event,arg=tracked['instance_id'])
    ended=actor.evaluate(event,tracked['instance_id'])
    fee_events=actor.evaluate('start=>window.__probeEvents.slice(start)',fee_start)
    commit_index=next(i for i,r in enumerate(fee_events) if r.get('event')==fee)
    end_index=next(i for i,r in enumerate(fee_events) if r.get('event')==ended)
    assert commit_index<end_index
    assert sum(r.get('event',{}).get('trigger_id')=='commit-spirit' for r in fee_events[:end_index])==1
    expected=[2000,12,200,40] if len(pages)==4 else [2592,12,180,25]
    assert [ended[k] for k in ['recorded','contacts','raw_hp','raw_spirit']]==expected
    assert ended['reason']=='expired' and 'suppressed' not in ended
    assert ended['hp_restored']==min(ended['raw_hp'],before['max_hp']-before['hp'])
    assert ended['spirit_restored']==min(ended['raw_spirit'],before['max_spirit']-before['spirit'])
    actor.wait_for_function('v=>window.__lastSnapshot.self.hp===v[0]&&window.__lastSnapshot.self.spirit===v[1]',arg=[before['hp']+ended['hp_restored'],before['spirit']+ended['spirit_restored']])
    assert actor.evaluate("id=>window.__probeEvents.filter(r=>r.event?.kind==='window_record_ended'&&r.event.instance_id===id).length",tracked['instance_id'])==1
    expect(actor.locator('#own')).to_contain_text('窗口结束')
    for peer in pages[1:]:
        assert not peer.evaluate(event,tracked['instance_id'])
    # An ordinary committed action creates a real one-point spirit gap on the ally.
    receiver.locator('#skill').select_option('focus-power')
    receiver.locator('#cast').click()
    receiver.wait_for_function('window.__lastSnapshot.self.spirit===window.__lastSnapshot.self.max_spirit-1')
    receiver.wait_for_function("window.__lastSnapshot.self.statuses.every(s=>s.definition_id!=='power-focus')")
    def apply_tempo():
        receiver.wait_for_function("()=>{const s=window.__lastSnapshot;return s.self.pending_action===null&&s.logical_ms>=s.self.recovery_until_ms&&s.logical_ms>=s.self.player_skills.find(x=>x.skill_id==='training-tempo').ready_at_ms}")
        receiver.locator('#skill').select_option('training-tempo')
        receiver.locator('#cast').click()
        receiver.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='training-tempo')")
        return receiver.evaluate("window.__lastSnapshot.self.statuses.find(s=>s.definition_id==='training-tempo')")
    apply_tempo()
    apply_tempo()
    receiver.wait_for_function("window.__lastSnapshot.self.statuses.every(s=>s.definition_id!=='training-tempo')")
    assert receiver.evaluate(own+'.spirit')==receiver.evaluate(own+'.max_spirit')-3
    source_spirit=actor.evaluate(own+'.spirit')
    actor.locator('#skill').select_option('ally-renew')
    actor.locator('#allyTarget').select_option(label=target)
    ally_start=actor.evaluate('window.__probeEvents.length')
    cast('ally-renew')
    actor.wait_for_function(committed,arg=ally_start)
    cost=actor.evaluate(committed,ally_start)['spirit_cost']
    receiver.wait_for_function(own+".statuses.some(s=>s.definition_id==='ally-renewal')")
    hot=receiver.evaluate(own+".statuses.find(s=>s.definition_id==='ally-renewal')")
    assert hot['window']['basis']=='none' and hot['window']['recorded']==0
    assert hot['source']['id']==source and hot['target']['id']==target
    assert hot['cleanse']=={'on_tick':True,'on_expired':True}
    cleanse_event="id=>window.__probeEvents.filter(r=>r.event?.kind==='status_cleanse_resolved'&&r.event.instance_id===id).map(r=>r.event)"
    early=apply_tempo()
    receiver.wait_for_function("id=>window.__probeEvents.some(r=>r.event?.kind==='status_cleanse_resolved'&&r.event.instance_id===id&&r.event.tick_index===1)",arg=hot['instance_id'])
    assert not receiver.evaluate('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',early['instance_id'])
    actor.reload(wait_until='domcontentloaded')
    actor.wait_for_function('window.__lastSnapshot?.self.ready')
    active=receiver.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',hot['instance_id'])
    assert active['applied_at_ms']==hot['applied_at_ms'] and active['expires_at_ms']==hot['expires_at_ms']
    receiver.wait_for_function("id=>window.__probeEvents.some(r=>r.event?.kind==='status_cleanse_resolved'&&r.event.instance_id===id&&r.event.tick_index===3)",arg=hot['instance_id'])
    late=apply_tempo()
    receiver.wait_for_function(event,arg=hot['instance_id'])
    result=receiver.evaluate(event,hot['instance_id'])
    assert result['raw_hp']==hot['window']['max_hp_at_apply']//10
    assert result['raw_spirit']==hot['window']['max_spirit_at_apply']//10
    assert result['spirit_restored']==1 and 'suppressed' not in result
    actor.wait_for_function(event,arg=hot['instance_id'])
    assert actor.evaluate(event,hot['instance_id'])==result
    for page in [actor,receiver]:
        assert page.evaluate("id=>window.__probeEvents.filter(r=>r.event?.kind==='window_record_ended'&&r.event.instance_id===id).length",hot['instance_id'])==1
    receiver.wait_for_function('window.__lastSnapshot.self.spirit===window.__lastSnapshot.self.max_spirit')
    actor.wait_for_function('v=>window.__lastSnapshot.self.spirit===v',arg=source_spirit-cost+1)
    expect(receiver.locator('#own')).to_contain_text('窗口结束')
    expect(receiver.locator('#own')).to_contain_text('灵力 1')
    for peer in pages[2:]:
        assert not peer.evaluate(event,hot['instance_id'])
    clean=receiver.evaluate(cleanse_event,hot['instance_id'])
    assert len(clean)==4 and [row['phase'] for row in clean]==['tick','tick','tick','expired']
    assert actor.evaluate(cleanse_event,hot['instance_id'])==clean[1:]
    assert [row['removed_count'] for row in clean]==[1,0,0,1]
    assert [row['scheduled_at_ms']-hot['applied_at_ms'] for row in clean[:3]]==[1000,2000,3000]
    assert all('suppressed' not in row and 'removed_ids' not in row and 'instance_ids' not in row for row in clean)
    assert not receiver.evaluate('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)',late['instance_id'])
    assert receiver.evaluate("id=>{const rows=window.__probeEvents;return rows.findIndex(r=>r.event?.kind==='status_cleanse_resolved'&&r.event.instance_id===id&&r.event.phase==='expired')<rows.findIndex(r=>r.event?.kind==='window_record_ended'&&r.event.instance_id===id)}",hot['instance_id'])
    for peer in pages[2:]:
        assert not peer.evaluate("id=>window.__probeEvents.some(r=>r.event?.kind==='status_cleanse_resolved'&&r.event.instance_id===id)",hot['instance_id'])
    expect(receiver.locator('#own')).to_contain_text('状态净化')
    spirit_event="id=>window.__probeEvents.filter(r=>r.event?.kind==='periodic_spirit_tick'&&r.event.instance_id===id).map(r=>r.event)"
    resources=receiver.evaluate(spirit_event,hot['instance_id'])
    assert len(resources)==3 and actor.evaluate(spirit_event,hot['instance_id'])==resources[1:]
    base,converted,spent=(1,3,6) if len(pages)==4 else (2,2,8)
    assert hot['periodic']['raw_spirit']==base and hot['periodic']['power_sampling']==('source_snapshot' if len(pages)==4 else 'source_live')
    assert [r['base_restored'] for r in resources]==[base,0,0]
    assert [r['converted_spirit'] for r in resources]==[converted,0,0]
    assert [r['spent_heal'] for r in resources]==[spent,0,0]
    assert all(r['outcome']=='applied' and r['hp_excess']==r['spent_heal']+r['remaining_excess'] for r in resources)
    assert [r['hp_excess'] for r in resources]==[66 if len(pages)==4 else 65]*3
    shields=receiver.evaluate('source=>window.__probeEvents.filter(r=>r.event?.kind==="shield_granted"&&r.event.definition_id==="overflow-shield"&&r.event.source.id===source).map(r=>r.event.granted)',source)
    assert shields==([30,33,33] if len(pages)==4 else [28,32,32])
    assert receiver.evaluate("id=>{const rows=window.__probeEvents;const h=rows.findIndex(r=>r.event?.kind==='periodic_heal_tick'&&r.event.instance_id===id);const s=rows.findIndex(r=>r.event?.kind==='periodic_spirit_tick'&&r.event.instance_id===id);const b=rows.findIndex(r=>r.event?.kind==='shield_granted'&&r.event.definition_id==='overflow-shield');const c=rows.findIndex(r=>r.event?.kind==='status_cleanse_resolved'&&r.event.instance_id===id);return h<s&&s<b&&b<c}",hot['instance_id'])
    for peer in pages[2:]:
        assert not peer.evaluate("id=>window.__probeEvents.some(r=>r.event?.kind==='periodic_spirit_tick'&&r.event.instance_id===id)",hot['instance_id'])
    expect(receiver.locator('#own')).to_contain_text('周期回灵')
    resource_probe={'base':[base,0,0],'converted':[converted,0,0],'spent':[spent,0,0],'shield':shields,'conserved':True,'sourceOwnUnchanged':True}
    cleanse_probe={'ticks':[1,2,3],'removed':[1,0,0,1],'reconnected':True,'endBeforeRecovery':True,'privateCountOnly':True}
    # Ordinary page reload preserves the HOT-created native record and frozen balance.
    overflow=receiver.evaluate("window.__lastSnapshot.self.statuses.find(s=>s.definition_id==='overflow-shield')")
    assert overflow['shield']['remaining']==(96 if len(pages)==4 else 92)
    assert overflow['shield']['record']=={'basis':'incoming' if len(pages)==4 else 'own_absorbed','recorded':0,'cap':40 if len(pages)==4 else 50}
    assert overflow['modifiers']==[{'attribute':'damage_reduction','modifier_bps':2000 if len(pages)==4 else 1000}]
    home=receiver.evaluate(own+'.position')
    before_reload_ms=receiver.evaluate('window.__lastSnapshot.logical_ms')
    receiver.reload(wait_until='domcontentloaded')
    receiver.wait_for_function('window.__lastSnapshot?.self.ready')
    reloaded_overflow=receiver.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',overflow['instance_id'])
    assert reloaded_overflow==overflow, {'expected':overflow,'actual':reloaded_overflow,'before_reload_ms':before_reload_ms,'after_reload_ms':receiver.evaluate('window.__lastSnapshot.logical_ms')}
    def move_receiver(destination):
        position=receiver.evaluate(own+'.position')
        box=receiver.locator('#map').bounding_box()
        dx=((destination[0]-destination[1])-(position[0]-position[1]))/2
        dy=((destination[0]+destination[1])-(position[0]+position[1]))/4
        receiver.locator('#map').click(position={'x':box['width']/2+dx,'y':box['height']/2+dy})
    destination=receiver.evaluate("window.__lastSnapshot.monsters.find(n=>n.entity.id==='power-target').position")
    move_receiver([destination[0]-12,destination[1]])
    end_event="id=>window.__probeEvents.filter(r=>r.event?.kind==='shield_record_ended'&&r.event.instance_id===id).map(r=>r.event)"
    receiver.wait_for_function('id=>window.__probeEvents.some(r=>r.event?.kind==="shield_record_ended"&&r.event.instance_id===id)',arg=overflow['instance_id'])
    overflow_ends=receiver.evaluate(end_event,overflow['instance_id'])
    assert len(overflow_ends)==1 and actor.evaluate(end_event,overflow['instance_id'])==overflow_ends
    overflow_end=overflow_ends[0]
    assert [overflow_end[k] for k in ['reason','recorded','raw_counter','targets_hit']]==['depleted',40 if len(pages)==4 else 50,20 if len(pages)==4 else 30,1]
    assert 'suppressed' not in overflow_end
    move_receiver(home)
    receiver.wait_for_function("Math.hypot(...window.__lastSnapshot.self.position.map((x,i)=>x-window.__lastSnapshot.monsters.find(n=>n.entity.id==='power-target').position[i]))>60")
    receiver.locator('#stop').click()
    expect(receiver.locator('#own')).to_contain_text('盾结束')
    for peer in pages[2:]:
        assert not peer.evaluate(end_event,overflow['instance_id'])
    overheal_record_probe={'balance':overflow['shield']['remaining'],'basis':overflow['shield']['record']['basis'],'recorded':overflow_end['recorded'],'rawCounter':overflow_end['raw_counter'],'targetsHit':1,'reconnected':True,'privateRecipients':2}
    return {'recorded':ended['recorded'],'contacts':12,'rawHp':ended['raw_hp'],'rawSpirit':ended['raw_spirit'],'allySpiritRestored':1,'source':source,'target':target,'cleanseProbe':cleanse_probe,'resourceProbe':resource_probe,'overhealRecordProbe':overheal_record_probe}


def verify_status_operations(pages):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    npc = "window.__lastSnapshot.monsters.find(n => n.entity.id === 'power-target')"
    source = actor.evaluate(own + '.participant.entity.id')
    def map_click(destination, select=False):
        if select:
            actor.locator('#selectGround').click()
        position = actor.evaluate(own + '.position')
        box = actor.locator('#map').bounding_box()
        dx = ((destination[0]-destination[1])-(position[0]-position[1]))/2
        dy = ((destination[0]+destination[1])-(position[0]+position[1]))/4
        actor.locator('#map').click(position={'x':box['width']/2+dx,'y':box['height']/2+dy})
    def cast(skill, point=None):
        actor.wait_for_function("id => {const s=window.__lastSnapshot;return s.self.pending_action===null && s.logical_ms>=s.self.recovery_until_ms && s.logical_ms>=s.self.player_skills.find(x=>x.skill_id===id).ready_at_ms;}", arg=skill)
        actor.locator('#skill').select_option(skill)
        if point is not None:
            map_click(point, True)
        actor.locator('#cast').click()
    destination = actor.evaluate(npc + '.position')
    map_click([destination[0]-100,destination[1]])
    actor.wait_for_function("Math.hypot(...window.__lastSnapshot.self.position.map((x,i)=>x-window.__lastSnapshot.monsters.find(n=>n.entity.id==='power-target').position[i]))>60")
    # Selecting the current position finishes navigation through ordinary UI input.
    map_click(actor.evaluate(own+'.position'))
    aggregate_probe=verify_action_totals(pages,cast)
    cast('focus-target', destination)
    actor.wait_for_function(f"{npc}.statuses.some(s=>s.definition_id==='offer')")
    offered = actor.evaluate(f"{npc}.statuses.find(s=>s.definition_id==='offer')")
    control=actor.evaluate(f"{npc}.statuses.find(s=>s.definition_id==='compound-lock')")
    assert control['flags']==(['silence','root'] if len(pages)==4 else ['stun'])
    assert control['modifiers']==[{'attribute':'def','modifier_bps':-2000 if len(pages)==4 else -3000}]
    assert actor.evaluate(f"{npc}.statuses.filter(s=>s.definition_id==='compound-lock').length")==1
    expect(actor.locator('#own')).to_contain_text('目标复合标记')
    actor.reload(wait_until='domcontentloaded')
    actor.wait_for_function('window.__lastSnapshot?.self.ready')
    assert actor.evaluate('id=>window.__lastSnapshot.monsters.find(n=>n.entity.id==="power-target").statuses.find(s=>s.instance_id===id)',control['instance_id'])==control
    cast('focus-target', destination)
    actor.wait_for_function('id=>window.__lastSnapshot.self.statuses.some(s=>s.instance_id===id)', arg=offered['instance_id'])
    taken = actor.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)', offered['instance_id'])
    moved_immunity=actor.evaluate("window.__probeEvents.find(r=>r.event?.kind==='status_transferred'&&r.event.status.definition_id==='life-window').event.status")
    initial_immunity=actor.evaluate("id=>window.__probeEvents.find(r=>r.event?.kind==='status_applied'&&r.event.status.instance_id===id).event.status",moved_immunity['instance_id'])
    assert moved_immunity=={**initial_immunity,'target':actor.evaluate(own+'.participant.entity')}
    assert moved_immunity['source']['id']=='power-target'
    assert moved_immunity.get('flags',[])==([] if len(pages)==4 else ['control_immunity','damage_immunity','first_control_immunity'])
    moved_hot=actor.evaluate("window.__probeEvents.find(r=>r.event?.kind==='status_transferred'&&r.event.status.definition_id==='renewal').event.status")
    initial_hot=actor.evaluate("id=>window.__probeEvents.find(r=>r.event?.kind==='status_applied'&&r.event.status.instance_id===id).event.status",moved_hot['instance_id'])
    assert moved_hot=={**initial_hot,'target':actor.evaluate(own+'.participant.entity')}
    assert moved_hot['source']['id']=='power-target'
    npc_hp_after_transfer=actor.evaluate(npc+'.hp')
    actor.wait_for_function('old=>window.__lastSnapshot.monsters.find(n=>n.entity.id==="power-target").statuses.some(s=>s.instance_id===old.instance_id&&s.applied_at_ms>old.applied_at_ms)',arg=control)
    refreshed=actor.evaluate('id=>window.__lastSnapshot.monsters.find(n=>n.entity.id==="power-target").statuses.find(s=>s.instance_id===id)',control['instance_id'])
    assert {**refreshed,'applied_at_ms':control['applied_at_ms'],'expires_at_ms':control['expires_at_ms']}==control
    assert refreshed['expires_at_ms']-refreshed['applied_at_ms']==(3600 if len(pages)==4 else 4500)
    for peer in pages[1:]:
        assert not peer.evaluate('id=>window.__lastSnapshot.monsters.some(n=>(n.statuses??[]).some(s=>s.instance_id===id))||window.__probeEvents.some(r=>r.event?.status?.instance_id===id)',control['instance_id'])
    consumptions=actor.evaluate("id=>window.__probeEvents.flatMap(r=>r.event?.kind==='shield_absorbed'?r.event.consumptions:[]).filter(r=>r.instance_id===id)",offered['instance_id'])
    assert len(consumptions)==1 and consumptions[0]['absorbed']==1
    expected_shield={**offered['shield'],'remaining':consumptions[0]['remaining']}
    if 'record' in expected_shield:expected_shield['record']={**expected_shield['record'],'recorded':1}
    assert taken == {**offered,'shield':expected_shield,'target':actor.evaluate(own+'.participant.entity')}
    expect(actor.locator('#own')).to_contain_text('状态转移')
    actor.reload(wait_until='domcontentloaded')
    actor.wait_for_function('window.__lastSnapshot?.self.ready')
    assert actor.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',taken['instance_id'])==taken
    assert actor.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',moved_immunity['instance_id'])==moved_immunity
    restored_gift=actor.evaluate('id=>window.__lastSnapshot.self.statuses.find(s=>s.instance_id===id)',moved_hot['instance_id'])
    assert [restored_gift[k] for k in ['instance_id','source','target','applied_at_ms','expires_at_ms']]==[moved_hot[k] for k in ['instance_id','source','target','applied_at_ms','expires_at_ms']]
    assert restored_gift['periodic']['ticks_done']>=moved_hot['periodic']['ticks_done']
    for peer in pages[1:]:
        assert not peer.evaluate("window.__probeEvents.some(r=>r.event?.kind==='status_transferred')")
    cast('training-tempo')
    actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='training-tempo')")
    removable = actor.evaluate("window.__lastSnapshot.self.statuses.find(s=>s.definition_id==='training-tempo')")
    position = actor.evaluate(own+'.position')
    cast('small-pulse', [position[0]-20,position[1]])
    actor.wait_for_function("id=>window.__probeEvents.some(r=>r.event?.kind==='status_removed'&&r.event.reason==='cleansed'&&r.event.instance_ids.includes(id))",arg=removable['instance_id'])
    seal_skill = actor.locator('#skill option').evaluate_all("opts=>opts.some(x=>x.value==='next-bolt')?'next-bolt':'short-field'")
    cast(seal_skill, [position[0]-30,position[1]])
    actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='operation-seal')")
    cast('training-tempo')
    actor.wait_for_function("window.__probeEvents.some(r=>r.event?.kind==='status_rejected'&&r.event.reason==='IMMUNE'&&r.event.definition_ids.includes('training-tempo'))")
    assert not actor.evaluate("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='training-tempo')")
    actor.wait_for_function("window.__lastSnapshot.self.statuses.every(s=>s.definition_id!=='operation-seal')")
    cast('training-tempo')
    actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s=>s.definition_id==='training-tempo')")
    destination = actor.evaluate(npc+'.position')
    actor.wait_for_function('id=>window.__lastSnapshot.monsters.every(n=>(n.statuses??[]).every(s=>s.instance_id!==id))',arg=control['instance_id'])
    actor.wait_for_function('id=>window.__probeEvents.some(r=>r.event?.kind==="status_removed"&&r.event.reason==="expired"&&r.event.instance_ids.includes(id))',arg=control['instance_id'])
    actor.wait_for_function('id=>window.__lastSnapshot.self.statuses.every(s=>s.instance_id!==id)',arg=taken['instance_id'])
    transferred_ends=actor.evaluate("id=>window.__probeEvents.filter(r=>r.event?.kind==='shield_record_ended'&&r.event.instance_id===id).map(r=>r.event)",taken['instance_id'])
    assert len(transferred_ends)==(0 if len(pages)==4 else 1)
    if transferred_ends:assert [transferred_ends[0][k] for k in ['reason','recorded']]==['expired',1] and transferred_ends[0]['target']['id']==source
    shield_transfer_probe={'granted':200,'absorbed':1,'balance':199,'recorded':None if len(pages)==4 else 1,'recordCap':None if len(pages)==4 else 180,'reconnected':True,'expired':True,'endEvents':len(transferred_ends)}
    actor.wait_for_function('id=>window.__lastSnapshot.self.statuses.every(s=>s.instance_id!==id)',arg=moved_hot['instance_id'])
    periodic_rows=actor.evaluate("id=>window.__probeEvents.filter(r=>r.event?.kind==='periodic_heal_tick'&&r.event.instance_id===id).map(r=>r.event)",moved_hot['instance_id'])
    assert periodic_rows and periodic_rows[-1]['tick_index']==moved_hot['periodic']['total_ticks']
    assert len(set(r['tick_index'] for r in periodic_rows))==len(periodic_rows)
    assert all(r['source']['id']=='power-target' and r['target']['id']==source and r['scheduled_at_ms']==moved_hot['applied_at_ms']+r['tick_index']*moved_hot['periodic']['interval_ms'] for r in periodic_rows)
    assert actor.evaluate(npc+'.hp')==npc_hp_after_transfer
    for peer in pages[1:]:assert not peer.evaluate("id=>window.__probeEvents.some(r=>r.event?.kind==='periodic_heal_tick'&&r.event.instance_id===id)",moved_hot['instance_id'])
    periodic_transfer_probe={'source':'power-target','target':source,'totalTicks':moved_hot['periodic']['total_ticks'],'lastTick':periodic_rows[-1]['tick_index'],'sampling':moved_hot['periodic']['power_sampling'],'reconnected':True,'expired':True,'privateRecipients':1}
    actor.wait_for_function('id=>window.__lastSnapshot.self.statuses.every(s=>s.instance_id!==id)',arg=moved_immunity['instance_id'])
    immunity_ends=actor.evaluate("id=>window.__probeEvents.filter(r=>r.event?.kind==='status_removed'&&r.event.reason==='expired'&&r.event.instance_ids.includes(id))",moved_immunity['instance_id'])
    assert len(immunity_ends)==1
    for peer in pages[1:]:assert not peer.evaluate("id=>window.__probeEvents.some(r=>r.event?.kind==='status_removed'&&r.event.reason==='expired'&&r.event.instance_ids.includes(id))",moved_immunity['instance_id'])
    control_transfer_probe={'kind':'scalar' if len(pages)==4 else 'compound','source':'power-target','target':source,'reconnected':True,'expired':True,'privateRecipients':1,'duration':1500}
    compound_probe={'flags':control['flags'],'modifierBps':control['modifiers'][0]['modifier_bps'],'immunityFlags':['control_immunity'],'oneIdentity':True,'reconnected':True,'refreshed':True,'expired':True}
    map_click([destination[0]-12,destination[1]])
    actor.wait_for_function("window.__probeEvents.some(r=>r.event?.kind==='death_guard_activated')")
    saved=actor.evaluate("window.__probeEvents.find(r=>r.event?.kind==='death_guard_activated').event")
    max_hp=actor.evaluate(own+'.max_hp')
    assert saved['hp']==1+max_hp*(1500 if len(pages)==4 else 2000)//10000
    expect(actor.locator('#own')).to_contain_text('致死保护')
    actor.reload(wait_until='domcontentloaded')
    actor.wait_for_function('window.__lastSnapshot?.self.ready')
    destination=actor.evaluate(npc+'.position')
    map_click([destination[0]-12,destination[1]])
    actor.wait_for_function("['settling','terminal'].includes(window.__lastSnapshot.self.phase)")
    assert not actor.evaluate("window.__probeEvents.some(r=>r.event?.kind==='death_guard_activated')")
    for peer in pages[1:]:
        assert not peer.evaluate("window.__probeEvents.some(r=>r.event?.kind==='death_guard_activated')")
    return {'transferred':3,'preservedInstance':True,'cleansed':1,'blocked':1,'deathGuards':1,'reconnected':True,'protectedHp':saved['hp'],'aggregateProbe':aggregate_probe,'compoundProbe':compound_probe,'shieldTransferProbe':shield_transfer_probe,'periodicTransferProbe':periodic_transfer_probe,'controlTransferProbe':control_transfer_probe}


def verify_skill_power(pages, power_bps, boosted_damage, expected=None, spirit_gain=0, compound=False):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    source = actor.evaluate(own + '.participant.entity.id')
    npc = "window.__lastSnapshot.monsters.find(row => row.entity.id === 'power-target')"
    target = actor.evaluate(npc + '.entity')
    view_url = actor.evaluate("new URL('/api/view' + location.search, location.href).href")
    response = actor.request.get(view_url)
    assert response.ok
    skills = response.json()['configuration']['skills']
    primary = skills[0]
    self_skill = next(row for row in skills if row['id'] == 'focus-power')
    focus = f"{own}.statuses.find(row => row.attribute === 'skill_power_bonus')"

    def attack(index, is_skill):
        ready = (f"{own}.player_skills.find(row => row.skill_id === '{primary['id']}').ready_at_ms"
                 if is_skill else own + '.basic_ready_at_ms')
        actor.wait_for_function(f"window.__lastSnapshot.logical_ms >= ({ready}) && window.__lastSnapshot.logical_ms >= {own}.recovery_until_ms && {own}.pending_action === null")
        if is_skill:
            actor.locator('#skill').select_option(primary['id'])
            actor.locator('#cast').click()
        else:
            actor.locator('#attack').click()
        amount = projectile_damage(actor, index, source, target)
        return amount

    before = attack(0, False)
    initial_spirit = actor.evaluate(own + '.spirit')
    actor.locator('#skill').select_option(self_skill['id'])
    actor.locator('#cast').click()
    actor.wait_for_function(f"Boolean({focus})")
    buff = actor.evaluate(focus)
    assert buff['source'] == buff['target'] == actor.evaluate(own + '.participant.entity')
    assert buff['modifier_bps'] == power_bps
    assert buff['expires_at_ms'] - buff['applied_at_ms'] == 1300
    expect(actor.locator('#own')).to_contain_text('技能威力加成中')
    boosted = attack(1, True)
    actor.wait_for_function(f"{npc}.hp === {1000 - before - boosted}")
    assert actor.evaluate(own + '.spirit') == initial_spirit - self_skill['cost'] - primary['cost'] + 2 * spirit_gain
    basic = attack(2, False)
    actor.wait_for_function(f"{npc}.hp === {1000 - before - boosted - basic}")
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < buff['expires_at_ms']
    assert actor.evaluate(own + '.spirit') == initial_spirit - self_skill['cost'] - primary['cost'] + 2 * spirit_gain + 2
    actor.wait_for_function(f"""id => !({focus}) && window.__probeEvents.some(row =>
        row.event?.kind === 'status_removed' && row.event.reason === 'expired' && row.event.instance_ids.includes(id))""",
        arg=buff['instance_id'])
    after = attack(3, True)
    damage = [before, boosted, basic, after]
    assert damage == (expected or [66, boosted_damage, 66, 66]), damage
    actor.wait_for_function(f"{npc}.hp === {1000 - sum(damage)}")
    assert actor.evaluate(own + '.spirit') == initial_spirit - self_skill['cost'] - 2 * primary['cost'] + 2 + 3 * spirit_gain
    for peer in pages[1:]:
        assert not peer.evaluate("""id => window.__probeEvents.some(row =>
            row.event?.status?.instance_id === id || row.event?.instance_ids?.includes(id)) ||
            window.__lastSnapshot.self.statuses.some(row => row.attribute === 'skill_power_bonus')""", buff['instance_id'])
    if compound:
        actor.wait_for_function('window.__lastSnapshot.logical_ms >= window.__lastSnapshot.self.recovery_until_ms && window.__lastSnapshot.self.pending_action === null')
        caps = actor.evaluate('({hp:window.__lastSnapshot.self.max_hp, spirit:window.__lastSnapshot.self.max_spirit})')
        actor.locator('#skill').select_option('battle-pose')
        actor.locator('#cast').click()
        actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s => s.definition_id === 'battle-pose')")
        composite = actor.evaluate("window.__lastSnapshot.self.statuses.find(s => s.definition_id === 'battle-pose')")
        assert len(composite['modifiers']) == 5
        assert composite['flags']==['control_immunity']
        expect(actor.locator('#own')).to_contain_text('控制免疫中')
        assert actor.evaluate(own + '.max_hp') == caps['hp'] * 3 // 2
        assert actor.evaluate(own + '.max_spirit') == caps['spirit'] * 3 // 2
        assert actor.evaluate(own + '.effective_atk') == 120
        for peer in pages[1:]:
            assert not peer.evaluate("window.__lastSnapshot.self.statuses.some(s => s.definition_id === 'battle-pose')")
        actor.wait_for_function("id => !window.__lastSnapshot.self.statuses.some(s => s.instance_id === id) && window.__lastSnapshot.self.effective_atk === 100", arg=composite['instance_id'])
        assert actor.evaluate(own + '.max_hp') == caps['hp']
        assert actor.evaluate(own + '.max_spirit') == caps['spirit']
        triggers = actor.evaluate("window.__probeEvents.filter(row => row.event?.kind === 'combat_trigger_fired')")
        assert len(triggers) == 4
        actor.locator('#skill').select_option('training-tempo')
        actor.locator('#cast').click()
        actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s => s.definition_id === 'training-tempo')")
        tempo = actor.evaluate("window.__lastSnapshot.self.statuses.find(s => s.definition_id === 'training-tempo')")
        assert tempo['attribute'] == 'attack_speed' and tempo['modifier_bps'] == -3000
        assert actor.evaluate(own + '.effective_atk') == 100
        attack_intervals = []
        for expected_interval in [858, 600]:
            actor.wait_for_function('window.__lastSnapshot.logical_ms >= window.__lastSnapshot.self.basic_ready_at_ms && window.__lastSnapshot.self.pending_action === null')
            actor.locator('#attack').click()
            actor.wait_for_function('window.__lastSnapshot.self.pending_action !== null')
            commit_at = actor.evaluate(own + '.pending_action.commits_at_ms')
            actor.wait_for_function('window.__lastSnapshot.self.pending_action === null')
            actual_interval = actor.evaluate(own + '.basic_ready_at_ms') - commit_at
            assert actual_interval == expected_interval
            attack_intervals.append(actual_interval)
            if expected_interval == 858:
                for peer in pages[1:]:
                    assert not peer.evaluate("window.__lastSnapshot.self.statuses.some(s => s.definition_id === 'training-tempo')")
                actor.wait_for_function("window.__lastSnapshot.self.statuses.every(s => s.definition_id !== 'training-tempo')")
        actor.wait_for_function("window.__lastSnapshot.logical_ms >= window.__lastSnapshot.self.player_skills.find(s => s.skill_id === 'battle-pose').ready_at_ms")
        actor.wait_for_function("source => window.__lastSnapshot.projectiles.every(p => p.source.id !== source)", arg=source)
        spirit_before_guard = actor.evaluate(own + '.spirit')
        actor.locator('#skill').select_option('battle-pose')
        actor.locator('#cast').click()
        actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s => s.definition_id === 'record-guard')")
        guard = actor.evaluate("window.__lastSnapshot.self.statuses.find(s => s.definition_id === 'record-guard')")
        expected_record, expected_counter = (150, 90) if power_bps == 5000 else (180, 108)
        assert guard['shield']['granted'] == caps['hp'] * 2000 // 10000 and guard['shield']['record']['cap'] == expected_record
        before_events = actor.evaluate('window.__probeEvents.length')
        own_position = actor.evaluate(own + '.position')
        destination = actor.evaluate(npc + '.position')
        destination[0] -= 12
        box = actor.locator('#map').bounding_box()
        dx = ((destination[0] - destination[1]) - (own_position[0] - own_position[1])) / 2
        dy = ((destination[0] + destination[1]) - (own_position[0] + own_position[1])) / 4
        actor.locator('#map').click(position={'x': box['width'] / 2 + dx, 'y': box['height'] / 2 + dy})
        actor.wait_for_function("window.__lastSnapshot.self.statuses.some(s => s.definition_id === 'record-guard' && s.shield.record.recorded === 100)")
        assert actor.evaluate(own + '.effective_atk') == 100
        assert '护盾记录' in actor.locator('#own').inner_text()
        pose_skill = next(s for s in skills if s['id'] == 'battle-pose')
        assert actor.evaluate(own + '.spirit') == spirit_before_guard - pose_skill['cost'] + 1 + 7
        actor.wait_for_function("id => window.__probeEvents.some(r => r.event?.kind === 'shield_record_ended' && r.event.instance_id === id)", arg=guard['instance_id'])
        ended = actor.evaluate("id => window.__probeEvents.find(r => r.event?.kind === 'shield_record_ended' && r.event.instance_id === id).event", guard['instance_id'])
        assert [ended['reason'], ended['recorded'], ended['raw_counter'], ended['targets_hit']] == ['depleted', expected_record, expected_counter, 1]
        assert 'suppressed' not in ended
        actor.wait_for_function("document.querySelector('#own').textContent.includes('盾结束') && document.querySelector('#own').textContent.includes('状态消费')")
        consumed = actor.evaluate("start => window.__probeEvents.slice(start).filter(r => r.event?.kind === 'status_consumed')", before_events)
        assert len(consumed) == 1 and consumed[0]['event']['definition_id'] == 'battle-pose'
        for peer in pages[1:]:
            assert not peer.evaluate("id => window.__lastSnapshot.self.statuses.some(s => s.instance_id === id) || window.__probeEvents.some(r => r.event?.instance_id === id || r.event?.status?.instance_id === id)", guard['instance_id'])
        guard_probe = {'recorded': ended['recorded'], 'rawCounter': ended['raw_counter'], 'targetsHit': ended['targets_hit'], 'consumed': len(consumed), 'source': source}
        status_probe = verify_status_operations(pages)
        compound_probe = status_probe.pop('compoundProbe')
        shield_transfer_probe = status_probe.pop('shieldTransferProbe')
        periodic_transfer_probe = status_probe.pop('periodicTransferProbe')
        control_transfer_probe = status_probe.pop('controlTransferProbe')
        aggregate_probe = status_probe.pop('aggregateProbe')
        window_probe = aggregate_probe.pop('windowProbe')
        cleanse_probe = window_probe.pop('cleanseProbe')
        resource_probe = window_probe.pop('resourceProbe')
        overheal_record_probe = window_probe.pop('overhealRecordProbe')
    return {'source': source, 'target': target['id'], 'powerBps': power_bps, 'damage': damage, 'compound': compound, **({'attackIntervals': attack_intervals, 'guardProbe': guard_probe, 'statusProbe': status_probe, 'aggregateProbe': aggregate_probe, 'windowProbe':window_probe,'compoundProbe':compound_probe,'cleanseProbe':cleanse_probe,'resourceProbe':resource_probe,'overhealRecordProbe':overheal_record_probe,'shieldTransferProbe':shield_transfer_probe,'periodicTransferProbe':periodic_transfer_probe,'controlTransferProbe':control_transfer_probe} if compound else {})}


def verify_member_attack_policy(pages, enabled, ignore_armor=False):
    actor = pages[0]
    source = actor.evaluate('window.__lastSnapshot.self.participant.entity.id')
    npc_id = 'armor-target' if ignore_armor else 'unavoidable-target'
    npc = f"window.__lastSnapshot.monsters.find(row => row.entity.id === '{npc_id}')"
    target = actor.evaluate(npc + '.entity')
    spirit = actor.evaluate('window.__lastSnapshot.self.spirit')
    actor.locator('#attack').click()
    basic_damage = projectile_damage(actor, 0, source, target)
    view_url = actor.evaluate("new URL('/api/view' + location.search, location.href).href")
    response = actor.request.get(view_url)
    assert response.ok
    skill = response.json()['configuration']['skills'][0]
    actor.locator('#skill').select_option(skill['id'])
    actor.locator('#cast').click()
    skill_damage = projectile_damage(actor, 1, source, target)
    damage = [basic_damage, skill_damage]
    expected = ([66, 100] if enabled else [66, 66]) if ignore_armor else ([66, 66] if enabled else [0, 66])
    assert damage == expected, damage
    actor.wait_for_function(f"{npc}.hp === {1000 - sum(damage)}")
    assert actor.evaluate('window.__lastSnapshot.self.spirit') == spirit - skill['cost']
    return {'source': source, 'target': target['id'],
            **({'ignoreArmor': enabled} if ignore_armor else {'basicUnavoidable': enabled, 'skillUnavoidable': True}),
            'damage': damage}


def verify_unavoidable_projectile(pages, unavoidable):
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    source = 'unavoidable-probe'
    target = actor.evaluate(own + '.participant.entity')

    def cast(skill, attribute, label):
        focus = f"{own}.statuses.find(row => row.attribute === '{attribute}')"
        actor.locator('#skill').select_option(skill)
        actor.locator('#cast').click()
        actor.wait_for_function(f"Boolean({focus})")
        buff = actor.evaluate(focus)
        assert buff['source'] == buff['target'] == target
        assert buff['expires_at_ms'] - buff['applied_at_ms'] == 1300
        expect(actor.locator('#own')).to_contain_text(label)
        return buff

    before = projectile_damage(actor, 0, source, target)
    dodge = cast('focus-dodge', 'dodge_chance', '闪避率加成中')
    assert dodge['modifier_bps'] == 5000
    dodging = projectile_damage(actor, 1, source, target)
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < dodge['expires_at_ms']
    guard = cast('protect-damage', 'damage_immunity', '伤害免疫中')
    immune = projectile_damage(actor, 2, source, target)
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < guard['expires_at_ms']
    actor.wait_for_function("""id => !window.__lastSnapshot.self.statuses.some(row => row.instance_id === id) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'expired' && row.event.instance_ids.includes(id))""", arg=guard['instance_id'])
    after = projectile_damage(actor, 3, source, target)
    damage = [before, dodging, immune, after]
    assert damage == ([66, 66, 0, 66] if unavoidable else [66, 0, 0, 66]), damage
    for peer in pages[1:]:
        assert not peer.evaluate("""ids => window.__probeEvents.some(row =>
            ids.includes(row.event?.status?.instance_id) || row.event?.instance_ids?.some(id => ids.includes(id))) ||
            window.__lastSnapshot.self.statuses.some(row => ['dodge_chance','damage_immunity'].includes(row.attribute))""",
            [dodge['instance_id'], guard['instance_id']])
    return {'source': source, 'target': target['id'], 'unavoidable': unavoidable, 'damage': damage}


def verify_member_defense(pages, bps, npc_hit=None, damage_immunity=False):
    source, attribute, skill, label = (
        ('immunity-probe', 'damage_immunity', 'protect-damage', '伤害免疫中') if damage_immunity
        else ('evasion-probe', 'dodge_chance', 'focus-dodge', '闪避率加成中'))
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    focus = f"{own}.statuses.find(row => row.attribute === '{attribute}')"
    target = actor.evaluate(own + '.participant.entity')
    hits = f"""rows => rows.filter(row => row.kind === 'damage' && row.source.id === '{source}')"""

    before = projectile_damage(actor, 0, source, target)
    actor.locator('#skill').select_option(skill)
    actor.locator('#cast').click()
    actor.wait_for_function(f"Boolean({focus})")
    buff = actor.evaluate(focus)
    assert buff['source'] == buff['target'] == target
    if not damage_immunity:
        assert buff['modifier_bps'] == bps
    assert buff['expires_at_ms'] - buff['applied_at_ms'] == 1300
    expect(actor.locator('#own')).to_contain_text(label)
    boosted = projectile_damage(actor, 1, source, target)
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < buff['expires_at_ms']
    actor.wait_for_function(f"""id => !({focus}) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'expired' && row.event.instance_ids.includes(id))""", arg=buff['instance_id'])
    after = projectile_damage(actor, 2, source, target)
    damage = [before, boosted, after]
    expected_damage = [66, 66, 66] if npc_hit == 100 else [66, 0, 66]
    assert damage == expected_damage, damage
    assert actor.evaluate(f"({hits})(window.__probeEvents).length") == sum(amount > 0 for amount in expected_damage)
    for peer in pages[1:]:
        assert not peer.evaluate(f"""id => window.__probeEvents.some(row =>
            row.event?.status?.instance_id === id || row.event?.instance_ids?.includes(id)) ||
            window.__lastSnapshot.self.statuses.some(row => row.attribute === '{attribute}')""",
            buff['instance_id'])
    return {'source': source, 'target': target['id'],
            **({'immunityDurationMs': 1300} if damage_immunity else {'dodgeBps': bps}),
            **({'npcHit': npc_hit} if npc_hit is not None else {}), 'damage': damage}


def verify_npc_dodge_modifier(pages, bps, attribute):
    skill, target, label, key = {
        'dodge_penalty': ('focus-dodge', 'dodge-target', '闪避压制中', 'penaltyBps'),
        'accuracy': ('focus-accuracy', 'accuracy-target', '命中加成中', 'accuracyBps'),
    }[attribute]
    actor = pages[0]
    own = 'window.__lastSnapshot.self'
    focus = f"{own}.statuses.find(row => row.attribute === '{attribute}')"
    source = actor.evaluate(own + '.participant.entity')
    npc = f"window.__lastSnapshot.monsters.find(row => row.entity.id === '{target}')"
    start = actor.evaluate(npc + '.position')
    shots = """window.__probeEvents.filter(row =>
        row.kind === 'projectile_started' && row.projectile.source.id === 'P00')"""
    hits = f"""window.__probeEvents.filter(row =>
        row.kind === 'damage' && row.source.id === 'P00' && row.target.id === '{target}')"""
    origins = []

    def attack():
        total, previous_hits = actor.evaluate(f"[({shots}).length, ({hits}).length]")
        actor.wait_for_function(f"window.__lastSnapshot.logical_ms >= {own}.basic_ready_at_ms")
        actor.locator('#attack').click()
        actor.wait_for_function(f"count => ({shots}).length === count", arg=total + 1)
        projectile = actor.evaluate(f"({shots}).at(-1).projectile")
        origins.append(projectile['position'])
        actor.wait_for_function("""id => window.__probeEvents.some(row =>
            row.kind === 'projectile_ended' && row.projectile_id === id)""", arg=projectile['id'])
        new_hits = actor.evaluate(f"count => ({hits}).slice(count)", previous_hits)
        assert len(new_hits) <= 1
        return new_hits[0]['amount'] if new_hits else 0

    before = attack()
    actor.locator('#skill').select_option(skill)
    actor.locator('#cast').click()
    actor.wait_for_function(f"Boolean({focus})")
    buff = actor.evaluate(focus)
    assert buff['source'] == buff['target'] == source
    assert buff['modifier_bps'] == bps
    assert buff['expires_at_ms'] - buff['applied_at_ms'] == 1000
    expect(actor.locator('#own')).to_contain_text(label)
    boosted = attack()
    assert actor.evaluate('window.__lastSnapshot.logical_ms') < buff['expires_at_ms']
    actor.wait_for_function(f"""id => !({focus}) &&
        window.__probeEvents.some(row => row.event?.kind === 'status_removed' &&
            row.event.reason === 'expired' && row.event.instance_ids.includes(id))""", arg=buff['instance_id'])
    after = attack()
    damage = [before, boosted, after]
    assert damage == [0, 66, 0]
    assert origins == [origins[0]] * 3
    assert actor.evaluate(f"({hits}).length") == 1
    actor.wait_for_function(f"{npc}.hp === 934")
    assert actor.evaluate(npc + '.position') == start
    for peer in pages[1:]:
        assert not peer.evaluate(f"""id => window.__probeEvents.some(row =>
            row.event?.status?.instance_id === id || row.event?.instance_ids?.includes(id)) ||
            window.__lastSnapshot.self.statuses.some(row => row.attribute === '{attribute}')""",
            buff['instance_id'])
    return {'source': source['id'], 'target': target, key: bps, 'damage': damage}


def journey(browser, request, address, output, scenario, seats, hp_deficit, spirit_deficit, restored,
            item='pill_mana', attached_status=None, quality_steps=None, mitigation=None, healing_received=None,
            item_healing_received=None, recovery_orb=False, recovery_strength=None, spirit_recovery_lock=False,
            npc_spirit_recovery_lock=False, overheal_shield=None, periodic_overheal_shield=None, instant_self_heal=None, ally_heal=None, ally_hot=None, ally_shield=None, ally_overheal_shield=None, ally_hot_overheal_shield=None, npc_silence=None, control_immunity=None, control_duration_reduction=None, first_control_immunity=None, item_cleanse=False, npc_root=None, npc_stun=None, npc_movement_slow=None, movement_haste=None, cooldown_reduction=None, armor_penetration=None, critical_chance=None, critical_damage_bonus=None, dodge_penalty=None, dodge_chance=None, accuracy=None, npc_hit=None, damage_immunity=False, unavoidable=None, member_unavoidable=None, ignore_armor=None, skill_power=None):
    response = request.post('/api/rooms', data={'scenario': scenario})
    assert response.status == 201, response.text()
    links = response.json()['links']
    assert len(links) == seats
    contexts = [browser.new_context(viewport={'width': 1100, 'height': 950}) for _ in links]
    pages = [context.new_page() for context in contexts]
    actor = pages[0]
    if mitigation or healing_received or item_healing_received or periodic_overheal_shield or instant_self_heal or ally_heal or ally_hot or ally_shield or ally_overheal_shield or ally_hot_overheal_shield or npc_silence or control_immunity or control_duration_reduction or first_control_immunity or npc_root or npc_stun or npc_movement_slow or movement_haste or cooldown_reduction or armor_penetration or critical_chance or critical_damage_bonus or dodge_penalty or dodge_chance or accuracy or npc_hit or damage_immunity or unavoidable is not None or member_unavoidable is not None or ignore_armor is not None or skill_power or item_cleanse:
        for page in pages if healing_received or item_healing_received or ally_heal or ally_hot or ally_shield or ally_overheal_shield or ally_hot_overheal_shield or npc_silence or control_immunity or control_duration_reduction or first_control_immunity or npc_root or npc_stun or npc_movement_slow or movement_haste or cooldown_reduction or armor_penetration or critical_chance or critical_damage_bonus or dodge_penalty or dodge_chance or accuracy or npc_hit or damage_immunity or unavoidable is not None or member_unavoidable is not None or ignore_armor is not None or skill_power or item_cleanse else [actor]:
            page.add_init_script("""(() => {
            const NativeEventSource = window.EventSource;
            window.__probeEvents = [];
            window.__probeHit = (row, periodic) => {
                const hit = periodic ? row.event : row;
                return hit?.kind === (periodic ? 'periodic_tick' : 'damage') &&
                    hit.source.id === 'damage-probe' && hit.target.id === 'P00';
            };
            window.EventSource = class extends NativeEventSource {
                constructor(...args) {
                    super(...args);
                    this.addEventListener('gameplay', e => window.__probeEvents.push(JSON.parse(e.data)));
                }
            };
        })();""")
    errors = []
    quality_items = ['pill_mana_low', 'pill_mana_high']
    steps = quality_steps or [(item, *restored)]
    used_ids = {step[0] for step in steps}
    items = [step[0] for step in quality_steps] if quality_steps else quality_items if item in quality_items else [item]
    picked_up = dict.fromkeys(items, 2)
    consumed = {item_id: int(item_id in used_ids) for item_id in items}
    if recovery_orb or recovery_strength or spirit_recovery_lock or npc_spirit_recovery_lock or periodic_overheal_shield or instant_self_heal or ally_heal or ally_hot or ally_shield or ally_overheal_shield or ally_hot_overheal_shield or npc_silence or control_immunity or control_duration_reduction or first_control_immunity or npc_root or npc_stun or npc_movement_slow or movement_haste or cooldown_reduction or armor_penetration or critical_chance or critical_damage_bonus or dodge_penalty or dodge_chance or accuracy or npc_hit or damage_immunity or unavoidable is not None or member_unavoidable is not None or ignore_armor is not None or skill_power:
        items, picked_up, consumed = [], {}, {}
    elif item_healing_received:
        picked_up = {item: 3, 'care_tonic': 1}
        consumed = {item: 2, 'care_tonic': 1}
        items = list(picked_up)

    def held(item_id):
        return f"(window.__lastSnapshot.self.inventory.find(i => i.item_id === '{item_id}')?.quantity ?? 0)"

    try:
        for page, link in zip(pages, links):
            page.on('pageerror', lambda error: errors.append(str(error)))
            page.on('console', lambda message: errors.append(message.text) if message.type == 'error' else None)
            page.goto(address + link['url'], wait_until='domcontentloaded')
        for page in pages:
            page.wait_for_function("window.__lastSnapshot?.phase === 'active' && window.__lastSnapshot.self.ready")
        if not (recovery_orb or recovery_strength or spirit_recovery_lock or npc_spirit_recovery_lock
                or periodic_overheal_shield or instant_self_heal or ally_heal or ally_hot or ally_shield or ally_overheal_shield or ally_hot_overheal_shield or npc_silence or control_immunity or control_duration_reduction or first_control_immunity or npc_root or npc_stun or npc_movement_slow or movement_haste or cooldown_reduction or armor_penetration or critical_chance or critical_damage_bonus or dodge_penalty or dodge_chance or accuracy or npc_hit or damage_immunity or unavoidable is not None or member_unavoidable is not None or ignore_armor is not None or skill_power):
            actor.get_by_role('button', name='普攻', exact=True).click()
            actor.wait_for_function('ids => ids.every(id => window.__lastSnapshot.resources.some(r => r.item_id === id))', arg=items)
            for _ in items:
                revision = actor.evaluate('window.__lastSnapshot.self.inventory_revision')
                actor.locator('#pickup').click()
                actor.wait_for_function('rev => window.__lastSnapshot.self.inventory_revision > rev', arg=revision)
            assert all(actor.evaluate(held(item_id)) == quantity for item_id, quantity in picked_up.items())
        if skill_power:
            restore_result = verify_skill_power(pages, *skill_power)
            status = {}
        elif ignore_armor is not None:
            restore_result = verify_member_attack_policy(pages, ignore_armor, True)
            status = {}
        elif member_unavoidable is not None:
            restore_result = verify_member_attack_policy(pages, member_unavoidable)
            status = {}
        elif unavoidable is not None:
            restore_result = verify_unavoidable_projectile(pages, unavoidable)
            status = {}
        elif dodge_chance or npc_hit or damage_immunity:
            restore_result = verify_member_defense(pages, 5000 if npc_hit else dodge_chance, npc_hit, damage_immunity)
            status = {}
        elif dodge_penalty or accuracy:
            restore_result = (verify_npc_dodge_modifier(pages, accuracy, 'accuracy') if accuracy
                              else verify_npc_dodge_modifier(pages, dodge_penalty, 'dodge_penalty'))
            status = {}
        elif critical_damage_bonus:
            bps, damage = critical_damage_bonus
            restore_result = verify_critical_damage_bonus(pages, bps, damage)
            status = {'bonusBps': bps}
        elif critical_chance:
            bps, damage = critical_chance
            restore_result = verify_damage_modifier(pages, bps, damage, 'critical_chance',
                                                    'focus-critical', '暴击率加成中')
            status = {'criticalBps': bps}
        elif armor_penetration:
            bps, damage = armor_penetration
            restore_result = verify_damage_modifier(pages, bps, damage, 'armor_penetration',
                                                    'focus-armor', '破甲中')
            status = {'penetrationBps': bps}
        elif cooldown_reduction:
            restore_result = verify_cooldown_reduction(pages, cooldown_reduction)
            status = {}
        elif movement_haste:
            restore_result = verify_movement_haste(pages, movement_haste)
            status = {}
        elif npc_movement_slow:
            restore_result = verify_npc_movement_slow(pages, npc_movement_slow)
            status = {}
        elif npc_stun:
            restore_result = verify_npc_stun(pages, npc_stun)
            status = {}
        elif npc_root:
            restore_result = verify_npc_root(pages, npc_root)
            status = {}
        elif item_cleanse:
            restore_result = verify_item_cleanse(pages, item)
            status = {}
        elif first_control_immunity:
            restore_result = verify_first_control_immunity(pages, first_control_immunity)
            status = {}
        elif control_duration_reduction:
            restore_result = verify_control_duration_reduction(pages, control_duration_reduction)
            status = {}
        elif control_immunity:
            restore_result = verify_control_immunity(pages, control_immunity)
            status = {}
        elif npc_silence:
            restore_result = verify_npc_silence(pages, npc_silence)
            status = {}
        elif ally_hot_overheal_shield:
            restore_result = verify_ally_hot_overheal_shield(pages, ally_hot_overheal_shield)
            status = {}
        elif ally_overheal_shield:
            amount = ally_overheal_shield[2]
            restore_result = verify_ally_heal(actor, pages[1], pages[2:], (amount, amount), ally_overheal_shield)
            status = {}
        elif ally_shield:
            restore_result = verify_ally_shield(pages, ally_shield)
            status = {}
        elif ally_hot:
            restore_result = verify_ally_hot(pages, ally_hot)
            status = {}
        elif ally_heal:
            restore_result = verify_ally_heal(actor, pages[1], pages[2:], ally_heal)
            status = {}
        elif instant_self_heal:
            restore_result = verify_instant_self_heal(actor, pages[1:], instant_self_heal)
            status = {}
        elif periodic_overheal_shield:
            restore_result = verify_periodic_overheal_shield(actor, pages[1:], periodic_overheal_shield)
            status = {}
        elif overheal_shield:
            restore_result = verify_overheal_shield(actor, pages[1:], item, *overheal_shield)
            status = {}
        elif recovery_strength:
            restore_result = verify_recovery_strength(actor, pages[1:], hp_deficit, *recovery_strength, restored)
            status = {}
        elif recovery_orb or spirit_recovery_lock or npc_spirit_recovery_lock:
            restore_result = verify_recovery_orb(actor, pages[1:], hp_deficit, restored,
                                                 spirit_recovery_lock or npc_spirit_recovery_lock,
                                                 npc_spirit_recovery_lock)
            status = {}
        elif item_healing_received:
            status = verify_item_healing_received(actor, pages, item, *item_healing_received)
            restore_result = {}
        elif healing_received:
            status = verify_periodic_healing_received(actor, pages, item, *healing_received)
            restore_result = {}
        elif mitigation:
            status = verify_mitigation(actor, pages, *mitigation)
            restore_result = {}
        else:
            actor.locator('#selfCast').click()
            actor.wait_for_function('''([hp, spirit]) => {
                const own = window.__lastSnapshot.self;
                return own.hp === own.max_hp - hp && own.spirit === own.max_spirit - spirit;
            }''', arg=[hp_deficit, spirit_deficit])
            before = actor.evaluate('window.__lastSnapshot.self')
            after = before
            observed = []
            for item_id, hp, spirit, *quality in steps:
                prior = after
                actor.locator('#useItemChoice').select_option(item_id)
                expect(actor.locator('#useItem')).to_be_enabled()
                actor.locator('#useItem').click()
                actor.wait_for_function('''id => {
                    const own = window.__lastSnapshot.self;
                    return window.__lastItemUsed?.item_id === id &&
                        own.inventory.find(row => row.item_id === id)?.quantity === 1 &&
                        own.item_cooldowns?.some(row => row.item_id === id);
                }''', arg=item_id)
                after = actor.evaluate('window.__lastSnapshot.self')
                assert after['inventory_revision'] == prior['inventory_revision'] + 1
                assert [after['hp'] - prior['hp'], after['spirit'] - prior['spirit']] == [hp, spirit]
                fact = actor.evaluate('window.__lastItemUsed')
                assert [fact['hp_restored'], fact.get('spirit_restored', 0)] == [hp, spirit]
                if quality or (attached_status and hp > 0):
                    assert 'spirit_restored' not in fact
                    label = f'恢复生命 {hp}'
                elif attached_status:
                    label = f'恢复灵力 {spirit}'
                else:
                    label = f'恢复生命 {hp}，恢复灵力 {spirit}'
                expect(actor.locator('#lastItemUse')).to_have_text(label)
                expect(actor.locator('#useItem')).to_be_disabled()
                if quality:
                    modifier, duration, effective = quality
                    instances = [row for row in after['statuses'] if row['definition_id'] == 'pill-fury']
                    assert len(instances) == 1
                    instance = instances[0]
                    assert instance['modifier_bps'] == modifier
                    assert instance['expires_at_ms'] - instance['applied_at_ms'] == duration
                    assert after['effective_atk'] == effective
                    expect(actor.locator('#own')).to_contain_text(f'攻击 {effective}')
                    observed.append({'itemId': item_id, 'hp': hp, 'modifierBps': modifier, 'durationMs': duration,
                                     'effectiveAtk': effective, 'instanceId': instance['instance_id'],
                                     'deadline': instance['expires_at_ms']})
            for sibling in items:
                if sibling not in used_ids:
                    assert actor.evaluate(held(sibling)) == 2
                    actor.locator('#useItemChoice').select_option(sibling)
                    assert actor.locator('#useItem').is_disabled()
            assert all(page.evaluate('window.__lastItemUsed ?? null') is None for page in pages[1:])
            status = {}
            if attached_status:
                definition_id, attribute, active_value = attached_status
                instance = next(row for row in after['statuses'] if row['definition_id'] == definition_id)
                assert after[attribute] == active_value
                label = '攻击' if attribute == 'effective_atk' else '防御'
                expect(actor.locator('#own')).to_contain_text(f'{label} {active_value}')
                assert all(page.evaluate('id => !window.__lastSnapshot.self.statuses.some(s => s.definition_id === id)',
                                         definition_id) for page in pages[1:])
            actor.screenshot(path=str(output / f'{scenario}-dual-recovery.png'), full_page=True)
            if attached_status:
                actor.wait_for_function('''([id, end, attribute, base]) => {
                    const snapshot = window.__lastSnapshot;
                    return snapshot.logical_ms >= end && snapshot.self[attribute] === base &&
                        !snapshot.self.statuses.some(s => s.instance_id === id);
                }''', arg=[instance['instance_id'], instance['expires_at_ms'], attribute, before[attribute]])
                status = {'status': {'definitionId': definition_id, 'attribute': attribute,
                                     'activeValue': active_value, 'expired': True}}
            if quality_steps:
                first, latest = observed
                assert latest['instanceId'] == first['instanceId']
                shortened = latest['deadline'] < first['deadline']
                assert shortened == (scenario == 'eight')
                assert all(page.evaluate("!window.__lastSnapshot.self.statuses.some(s => s.definition_id === 'pill-fury')")
                           for page in pages[1:])
                actor.wait_for_function('''([deadline, base]) => {
                    const snapshot = window.__lastSnapshot;
                    return snapshot.logical_ms >= deadline && snapshot.self.effective_atk === base &&
                        !snapshot.self.statuses.some(row => row.definition_id === 'pill-fury');
                }''', arg=[latest['deadline'], before['effective_atk']])
                status = {'status': {'steps': observed, 'deadlineShortened': shortened, 'expired': True}}

            restore_result = {'restored': [row['hp'] for row in observed] if quality_steps else restored}

        early_exit = ally_hot or ally_shield or ally_overheal_shield or ally_hot_overheal_shield
        exited = pages[0 if early_exit[1] == 'source' else 1] if early_exit else None
        for page in pages:
            if page != exited and page.evaluate("!['settling','terminal'].includes(window.__lastSnapshot.self.phase)"):
                page.locator('#leave').click()
        end = time.monotonic() + 15
        while True:
            ledgers = [request.get('/api/ledger?' + urlsplit(link['url']).query,
                                   max_retries=2, timeout=3000).json() for link in links]
            if all(row['phase'] == 'closed' and not row['host_live'] for row in ledgers):
                break
            assert time.monotonic() < end
            time.sleep(0.04)
        assert all(row['ledger']['admissions'] == row['ledger']['settlements'] == 1 for row in ledgers)
        balances = {item_id: sum(row['ledger']['balances'].get(item_id, 0) for row in ledgers) for item_id in items}
        assert all(balances[item_id] == picked_up[item_id] - consumed[item_id] for item_id in items)
        assert not errors and all(not row['errors'] for row in ledgers)
        return {'scenario': scenario, 'pages': seats, **restore_result,
                'consumed': sum(consumed.values()),
                'remaining': balances[item] if len(items) == 1 or item_healing_received else balances,
                'settled': seats, 'errors': errors,
                **({'itemId': item} if not quality_steps and (len(items) > 1 or attached_status) else {}), **status}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('server', 'binary', 'configs', 'out', 'cwd'):
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--port', type=int, default=4271)
    profile = parser.add_mutually_exclusive_group()
    profile.add_argument('--ratio', action='store_true')
    profile.add_argument('--quality', action='store_true')
    profile.add_argument('--status', action='store_true')
    profile.add_argument('--status-quality', action='store_true')
    profile.add_argument('--mitigation', action='store_true')
    profile.add_argument('--periodic-mitigation', action='store_true')
    profile.add_argument('--periodic-healing-received', action='store_true')
    profile.add_argument('--item-healing-received', action='store_true')
    profile.add_argument('--recovery-orb', action='store_true')
    profile.add_argument('--recovery-strength', action='store_true')
    profile.add_argument('--spirit-recovery-lock', action='store_true')
    profile.add_argument('--npc-spirit-recovery-lock', action='store_true')
    profile.add_argument('--overheal-shield', action='store_true')
    profile.add_argument('--periodic-overheal-shield', action='store_true')
    profile.add_argument('--instant-self-heal', action='store_true')
    profile.add_argument('--ally-heal', action='store_true')
    profile.add_argument('--ally-hot', action='store_true')
    profile.add_argument('--ally-shield', action='store_true')
    profile.add_argument('--ally-overheal-shield', action='store_true')
    profile.add_argument('--ally-hot-overheal-shield', action='store_true')
    profile.add_argument('--npc-silence', action='store_true')
    profile.add_argument('--control-immunity', action='store_true')
    profile.add_argument('--control-duration-reduction', action='store_true')
    profile.add_argument('--first-control-immunity', action='store_true')
    profile.add_argument('--item-cleanse', action='store_true')
    profile.add_argument('--npc-root', action='store_true')
    profile.add_argument('--npc-stun', action='store_true')
    profile.add_argument('--npc-movement-slow', action='store_true')
    profile.add_argument('--movement-haste', action='store_true')
    profile.add_argument('--cooldown-reduction', action='store_true')
    profile.add_argument('--armor-penetration', action='store_true')
    profile.add_argument('--critical-chance', action='store_true')
    profile.add_argument('--critical-damage-bonus', action='store_true')
    profile.add_argument('--skill-power', action='store_true')
    profile.add_argument('--ignore-armor', action='store_true')
    profile.add_argument('--member-unavoidable', action='store_true')
    profile.add_argument('--unavoidable', action='store_true')
    profile.add_argument('--damage-immunity', action='store_true')
    profile.add_argument('--npc-hit', action='store_true')
    profile.add_argument('--accuracy', action='store_true')
    profile.add_argument('--dodge-chance', action='store_true')
    profile.add_argument('--dodge-penalty', action='store_true')
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    address = f'http://127.0.0.1:{args.port}'
    command = ['node', str(args.server), '--binary', str(args.binary),
               '--configs', str(args.configs / 'catalog.json'), '--host-config', str(args.configs / 'host.json'),
               '--database', str(args.out / 'rooms.sqlite'), '--port', str(args.port)]
    with (args.out / 'server.log').open('w') as log:
        server = subprocess.Popen(command, cwd=args.cwd, stdout=log, stderr=subprocess.STDOUT)
        try:
            with sync_playwright() as playwright:
                request = playwright.request.new_context(base_url=address)
                end = time.monotonic() + 15
                while True:
                    try:
                        response = request.get('/api/config', timeout=1000)
                    except Error:
                        response = None
                    if response and response.ok:
                        break
                    assert server.poll() is None and time.monotonic() < end
                    time.sleep(0.1)
                eight = (0, 6, [0, 6])
                heal_options = {}
                if args.skill_power:
                    capability, mechanism = 'memberSkillPower', 104
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'skill_power': (5000, 100)}, 'eight': {'skill_power': (10000, 133)}}
                elif args.ignore_armor:
                    capability, mechanism = 'directIgnoreArmor', 103
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'ignore_armor': True}, 'eight': {'ignore_armor': False}}
                elif args.member_unavoidable:
                    capability, mechanism = 'memberUnavoidable', 102
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'member_unavoidable': True}, 'eight': {'member_unavoidable': False}}
                elif args.unavoidable:
                    capability, mechanism = 'memberNpcUnavoidable', 101
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'unavoidable': True}, 'eight': {'unavoidable': False}}
                elif args.damage_immunity:
                    capability, mechanism = 'memberDamageImmunity', 100
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'damage_immunity': True}, 'eight': {'damage_immunity': True}}
                elif args.npc_hit:
                    capability, mechanism = 'memberNpcHit', 99
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'npc_hit': 100}, 'eight': {'npc_hit': 50}}
                elif args.accuracy:
                    capability, mechanism = 'memberAccuracy', 98
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'accuracy': 10000}, 'eight': {'accuracy': 7500}}
                elif args.dodge_chance:
                    capability, mechanism = 'memberDodgeChance', 97
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'dodge_chance': 2500}, 'eight': {'dodge_chance': 5000}}
                elif args.dodge_penalty:
                    capability, mechanism = 'memberDodgePenalty', 96
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'dodge_penalty': 6000}, 'eight': {'dodge_penalty': 8000}}
                elif args.critical_damage_bonus:
                    capability, mechanism = 'memberCriticalDamageBonus', 95
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'critical_damage_bonus': (7500, 150)},
                                    'eight': {'critical_damage_bonus': (15000, 200)}}
                elif args.critical_chance:
                    capability, mechanism = 'memberCriticalChance', 94
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'critical_chance': (10000, 100)},
                                    'eight': {'critical_chance': (10000, 133)}}
                elif args.armor_penetration:
                    capability, mechanism = 'memberArmorPenetration', 93
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'armor_penetration': (5000, 80)},
                                    'eight': {'armor_penetration': (2500, 72)}}
                elif args.cooldown_reduction:
                    capability, mechanism = 'memberCooldownReduction', 92
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'cooldown_reduction': (2500, 1500)},
                                    'eight': {'cooldown_reduction': (4000, 1200)}}
                elif args.movement_haste:
                    capability, mechanism = 'memberMovementHaste', 91
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'movement_haste': (3000, -2500, 189, 135)},
                                    'eight': {'movement_haste': (5000, -4000, 198, 108)}}
                elif args.npc_movement_slow:
                    capability, mechanism = 'memberNpcMovementSlow', 90
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'npc_movement_slow': ('self-resolve', -2500, 135)},
                                    'eight': {'npc_movement_slow': ('self-guard', -4000, 108)}}
                elif args.npc_stun:
                    capability, mechanism = 'memberNpcStun', 89
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'npc_stun': 'self-resolve'}, 'eight': {'npc_stun': 'self-guard'}}
                elif args.npc_root:
                    capability, mechanism = 'memberNpcRoot', 88
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'npc_root': 'self-resolve'}, 'eight': {'npc_root': 'self-guard'}}
                elif args.item_cleanse:
                    capability, mechanism = 'memberItemCleanse', 87
                    four = eight = (0, 0, [0, 0], 'cleanse-tonic')
                    heal_options = {scenario: {'item_cleanse': True} for scenario in ('four', 'eight')}
                elif args.first_control_immunity:
                    capability, mechanism = 'memberFirstControlImmunity', 86
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'first_control_immunity': 'self-resolve'},
                                    'eight': {'first_control_immunity': 'self-guard'}}
                elif args.control_duration_reduction:
                    capability, mechanism = 'memberControlDurationReduction', 85
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'control_duration_reduction': (2500, 2100)},
                                    'eight': {'control_duration_reduction': (4000, 1680)}}
                elif args.control_immunity:
                    capability, mechanism = 'memberControlImmunity', 84
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'control_immunity': 'self-resolve'},
                                    'eight': {'control_immunity': 'self-guard'}}
                elif args.npc_silence:
                    capability, mechanism = 'memberNpcSilence', 83
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'npc_silence': 'self-resolve'}, 'eight': {'npc_silence': 'self-guard'}}
                elif args.ally_hot_overheal_shield:
                    capability, mechanism = 'memberAllyHotOverhealShield', 82
                    four = eight = (0, 0, [0, 0])
                    heal_options = {
                        'four': {'ally_hot_overheal_shield': (0, 'source', [[0, 55], [0, 55], [0, 0]])},
                        'eight': {'ally_hot_overheal_shield': (160, 'target', [[100, 0], [60, 20], [0, 50]])},
                    }
                elif args.ally_overheal_shield:
                    capability, mechanism = 'memberAllyOverhealShield', 81
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'ally_overheal_shield': (100, 'source', 0, 50)},
                                    'eight': {'ally_overheal_shield': (300, 'target', 160, 70)}}
                elif args.ally_shield:
                    capability, mechanism = 'memberAllyShield', 80
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'ally_shield': (220, 'source')}, 'eight': {'ally_shield': (200, 'target')}}
                elif args.ally_hot:
                    capability, mechanism = 'memberAllyHot', 79
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'ally_hot': (110, 'source')}, 'eight': {'ally_hot': (100, 'target')}}
                elif args.ally_heal:
                    capability, mechanism = 'memberAllyHeal', 78
                    four = eight = (0, 0, [0, 0])
                    heal_options = {'four': {'ally_heal': (160, 120)}, 'eight': {'ally_heal': (360, 300)}}
                elif args.instant_self_heal:
                    capability, mechanism = 'memberInstantSelfHeal', 77
                    four = (0, 0, [0, 0])
                    eight = (160, 0, [0, 0])
                    heal_options = {'four': {'instant_self_heal': (0, 100, 0, 50)},
                                    'eight': {'instant_self_heal': (160, 300, 160, 70)}}
                elif args.periodic_overheal_shield:
                    capability, mechanism = 'memberPeriodicOverhealShield', 76
                    four = (0, 0, [0, 0], None, None, None, None, None, None,
                            False, None, False, False, None, [[0, 50], [0, 50], [0, 0]])
                    eight = (160, 0, [0, 0], None, None, None, None, None, None,
                             False, None, False, False, None, [[100, 0], [60, 20], [0, 50]])
                elif args.overheal_shield:
                    capability, mechanism = 'memberOverhealShield', 75
                    four = (0, 0, [0, 0], 'overflow-tonic', None, None, None, None, None,
                            False, None, False, False, (0, 100))
                    eight = (360, 0, [0, 0], 'overflow-tonic', None, None, None, None, None,
                             False, None, False, False, (360, 120))
                elif args.npc_spirit_recovery_lock:
                    capability, mechanism = 'memberNpcSpiritRecoveryLock', 74
                    four = (330, 150, [30, 30], None, None, None, None, None, None, False, None, False, True)
                    eight = (360, 150, [150, 120], None, None, None, None, None, None, False, None, False, True)
                elif args.spirit_recovery_lock:
                    capability, mechanism = 'memberSpiritRecoveryLock', 73
                    four = (330, 150, [30, 30], None, None, None, None, None, None, False, None, True)
                    eight = (360, 150, [150, 120], None, None, None, None, None, None, False, None, True)
                elif args.recovery_strength:
                    capability, mechanism = 'memberRecoveryStrength', 72
                    four = (330, 150, [5, 1], None, None, None, None, None, None, False, (10000, [10, 2]))
                    eight = (360, 150, [15, 3], None, None, None, None, None, None, False, (-5000, [7, 1]))
                elif args.recovery_orb:
                    capability, mechanism = 'memberRecoveryOrb', 71
                    four = (330, 150, [30, 30], None, None, None, None, None, None, True)
                    eight = (360, 150, [150, 120], None, None, None, None, None, None, True)
                elif args.item_healing_received:
                    capability, mechanism = 'memberItemHealingReceived', 70
                    four = (0, 0, [0, 0], 'pill_renewal', None, None, None, None, (5000, 79))
                    eight = (0, 0, [0, 0], 'pill_renewal', None, None, None, None, (-5000, 26))
                elif args.periodic_healing_received:
                    capability, mechanism = 'memberPeriodicHealingReceived', 69
                    four = (0, 0, [0, 0], 'pill_renewal', None, None, None, (5000, 76))
                    eight = (0, 0, [0, 0], 'pill_renewal', None, None, None, (-5000, 25))
                elif args.periodic_mitigation:
                    capability, mechanism = 'memberPeriodicDamageReduction', 68
                    four = (0, 0, [0, 0], 'pill_guard', None, None, (2500, 75, 75))
                    eight = (0, 0, [0, 0], 'pill_guard', None, None, (3200, 68, 40))
                elif args.mitigation:
                    capability, mechanism = 'memberItemDamageReduction', 67
                    four = (0, 0, [0, 0], 'pill_guard', None, None, (2500, 75))
                    eight = (0, 0, [0, 0], 'pill_guard', None, None, (3200, 68))
                elif args.status_quality:
                    capability, mechanism = 'memberItemStatusQuality', 66
                    low = ('pill_berserk_low', 140, 0, 1500, 4500, 115)
                    high = ('pill_berserk_high', 300, 0, 3900, 7500, 139)
                    four = (330, 80, None, 'pill_berserk_low', None, [low, (high[0], 190, *high[2:])])
                    eight = (360, 40, None, 'pill_berserk_high', None, [high, (low[0], 60, *low[2:])])
                elif args.status:
                    capability, mechanism = 'memberItemSelfStatus', 65
                    four = (300, 50, [200, 0], 'pill_berserk', ('item-fury', 'effective_atk', 130))
                    eight = (0, 6, [0, 6], 'ward_tonic', ('item-guard', 'effective_def', 120))
                elif args.quality:
                    capability, mechanism = 'memberItemQualityRecovery', 64
                    four = (330, 80, [300, 60], 'pill_mana_high')
                    eight = (0, 40, [0, 28], 'pill_mana_low')
                elif args.ratio:
                    capability, mechanism = 'memberItemRatioRecovery', 63
                    four = (300, 50, [200, 40])
                else:
                    capability, mechanism = 'memberItemDualRecovery', 62
                    four = (60, 8, [40, 8])
                assert response.json()['sdk'][capability]['minimumMechanismVersion'] == mechanism
                browser = playwright.chromium.launch()
                cases = [journey(browser, request, address, args.out, *case,
                                 **heal_options.get(case[0], {}))
                         for case in
                         [('four', 4, *four), ('eight', 8, *eight)]]
                browser.close()
                request.dispose()
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
    report = {'passed': True, 'binarySha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'cases': cases}
    (args.out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
