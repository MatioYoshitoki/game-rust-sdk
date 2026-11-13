#!/usr/bin/env python3
"""Verify SDK-5-17 documentation; never run runtime/old acceptance harnesses."""
import hashlib
import json
import re
import subprocess
from collections import Counter
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'artifacts/sdk-5-17'
AUDIT = ROOT / 'docs/audits/sdk-5-17'


def load(p):
    return json.loads((ROOT / p).read_text())


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def unchanged(entries):
    bad = []
    for name, expected in entries.items():
        p = ROOT / name
        if not p.is_file() or sha(p) != expected:
            bad.append(name)
    assert not bad, bad
    return len(entries)


def main():
    # Separate regeneration check from this independent structural/integrity review.
    run = subprocess.run(['python3', 'scripts/map_sdk_5_17.py', '--check'], cwd=ROOT,
                         text=True, capture_output=True, check=True)
    generation = json.loads(run.stdout)
    baseline = load('artifacts/sdk-5-17/protected-baseline.json')
    runtime_count = unchanged(baseline['runtimeInputs'])
    source_count = unchanged(baseline['sourceFiles'])
    assert sha(ROOT / 'SDK.md') == baseline['sdkReadmeHash']
    previous = baseline['previousEvidence']
    cleaned = [p for p in previous if not (ROOT / p).exists()]
    # User explicitly answered "清理了" during this turn. No other mutation is excused.
    present = {p: h for p, h in previous.items() if (ROOT / p).exists()}
    unchanged(present)
    assert len(previous) == 119 and len(cleaned) == 119
    assert all(p.startswith('artifacts/') for p in cleaned)

    items = load('docs/audits/sdk-5-17/content-mapping.json')['items']
    scenarios = load('docs/audits/sdk-5-17/scenario-mapping.json')['scenarios']
    vectors = load('docs/audits/sdk-5-17/numeric-mapping.json')['vectors']
    domains = load('docs/audits/sdk-5-17/functional-domain-mapping.json')['domains']
    capabilities = load('docs/audits/sdk-5-17/capability-catalog.json')
    rules = load('docs/audits/sdk-5-17/rule-gates.json')['rules']
    roadmap = load('docs/plans/sdk-delivery-roadmap.json')
    original = load('rust-match-kernel-v1.1/content-audit.json')['items']
    assert {(x['kind'], x['id']) for x in items} == {(x['kind'], x['id']) for x in original}
    assert Counter(x['kind'] for x in items) == roadmap['sourceInventory']['kinds']
    assert all(x['wholeItemStatus'] == 'not_accepted' and x['exactAcceptance'] == 'not_run' for x in items)
    assert len(scenarios) == 48 and sum(len(s['assertions']) for s in scenarios) == 122
    assert len(vectors) == 50 and len(domains) == 14
    assert all(s['exactExecutionStatus'] == 'not_run' for s in scenarios + vectors)
    assert all(a['exactExecutionStatus'] == 'not_run' for s in scenarios for a in s['assertions'])
    assert len(capabilities['existing']) == 20 and len(capabilities['missing']) == 18
    assert len(rules) == 22
    assert all(r['status'] == 'open_product_decision' for r in rules if r['id'] in ['D-03', 'D-05'])

    wp = {w['id']: w for w in roadmap['workPackages']}
    visited = set()
    visiting = set()

    def visit(key):
        assert key in wp
        assert key not in visiting, 'dependency cycle: ' + key
        if key in visited:
            return
        visiting.add(key)
        for dep in wp[key]['dependsOn']:
            visit(dep)
        visiting.remove(key)
        visited.add(key)

    for key in wp:
        visit(key)
    assert len(wp) == 25 and len(roadmap['milestones']) == 6
    assignments = {g['id']: g for g in roadmap['gapAssignments']}
    assert len(assignments) == 32 and all(g['status'] == 'open' for g in assignments.values())
    for row in items + scenarios + vectors + domains:
        assert row['workPackages'] and set(row['workPackages']) <= set(wp)
        assert set(row['gapIds']) <= set(assignments)
    assert roadmap['sourceInventory']['routingCompleted'] is True
    assert roadmap['sourceInventory']['behaviorCoverageVerified'] is False
    assert sum(w['status'].startswith('completed_mapping') for w in wp.values()) == 3
    assert roadmap['counts']['completedRoadmapMilestones'] == 1
    assert roadmap['nextBatch']['id'] == 'SDK-5-18'
    assert roadmap['nextBatch']['status'] == 'specified_not_implemented'
    spec = (ROOT / roadmap['nextBatch']['spec']).read_text()
    ids = re.findall(r'^\| (TM\d\d) \|', spec, re.M)
    assert ids == [f'TM{i:02}' for i in range(1, 14)]
    assert set(roadmap['nextBatch']['workPackages']) == {'R2.1', 'R2.2', 'R2.3'}

    # Representative semantic assertions guard against misleading whole-item reuse.
    by_id = {(x['kind'], x['id']): x for x in items}
    assert 'OBS-06' in by_id['skill', 'liuyun-huifeng']['ruleGates']
    assert 'OBS-02' in by_id['skill', 'huoqiu-shu']['ruleGates']
    assert 'M04' in by_id['skill', 'huoqiu-shu']['gapIds']
    assert 'OBS-04' in by_id['status', 'effect_berserk']['ruleGates']
    assert any('DEF' in q['meaning'] for q in by_id['status', 'effect_shield']['requirements'])
    assert not any(g.startswith('M') for g in by_id['status', 'effect_shield']['gapIds'])
    assert 'M03' in by_id['status', 'effect_berserk']['gapIds']
    assert 'M04' in by_id['factor', 'factor_deal_damage']['gapIds']
    assert 'D-03' in by_id['status', 'effect_slow']['ruleGates']
    assert all('M08' in x['gapIds'] for x in items if x['kind'] == 'map')
    assert all('N01' in s['missingCapabilities'] or 'N02' in s['missingCapabilities'] for s in scenarios)
    assert next(v for v in vectors if v['id'] == 'heal-over')['missingCapabilities'] == ['N08']
    assert 'N07' in next(v for v in vectors if v['id'] == 'defense-pierce')['missingCapabilities']
    assert all(c['historicalReportAvailability'] == 'cleaned_by_user_2026-11-05' for c in capabilities['existing'])

    documents = [ROOT / p for p in [
        'PRD-367-independent-rust-match-kernel.md', 'README.md',
        'docs/plans/sdk-delivery-roadmap.md', 'docs/checklists/sdk-requirements-gap.md',
        'docs/checklists/independent-match-kernel.CHECKLIST.md',
        'docs/batches/SDK-5-17-requirements-mapping.md',
        'docs/batches/SDK-5-18-two-member-loop.md',
        'docs/evidence/SDK-5-17-requirements-mapping-2026-11-05.md']]
    documents += sorted(AUDIT.glob('*.md'))
    unavailable_links = []
    checked_links = 0
    for doc in documents:
        for link in re.findall(r'\[[^\]]*\]\(([^)]+)\)', doc.read_text()):
            if '://' in link or link.startswith('#'):
                continue
            link = unquote(link.split('#', 1)[0].split(' "', 1)[0])
            target = (doc.parent / link).resolve()
            if target == (OUT / 'verification.json').resolve():
                continue  # This script is about to produce it.
            if not target.exists():
                rel = str(target.relative_to(ROOT))
                if rel.startswith('artifacts/') and not rel.startswith('artifacts/sdk-5-17/'):
                    unavailable_links.append({'file': str(doc.relative_to(ROOT)), 'target': rel,
                                              'status': 'historical_artifacts_cleaned_by_user'})
                    continue
                raise AssertionError(('broken new/local link', str(doc), link))
            checked_links += 1

    protected_report = {
        'runtimeInputsUnchanged': runtime_count, 'sourceFilesUnchanged': source_count,
        'sdkMdUnchanged': True, 'historicalFilesOriginallyFingerprinted': len(previous),
        'historicalFilesCurrentlyUnavailable': len(cleaned),
        'cleanupAuthorization': {'userReply': '清理了', 'date': '2026-11-05',
                                 'effect': '记录历史文件不可复核例外，不恢复/补造旧报告，不增加行为通过数'},
        'historicalFiles': [{'path': p, 'originalSha256': previous[p], 'status': 'cleaned_by_user'} for p in cleaned],
    }
    criteria = {f'MP{i:02}': {'status': 'verified_mapping_only'} for i in range(1, 9)}
    criteria['MP02'] = {'status': 'verified_with_evidence_exception',
                        'limitation': '源码/契约及历史验收身份可路由；用户清理后历史passed日志不可独立复核'}
    criteria['MP07'] = {'status': 'verified_with_evidence_exception',
                        'limitation': '新产物/来源/工作包/依赖/生成一致；历史artifact链接和passed日志不可访问'}
    criteria['MP08'] = {'status': 'authorized_cleanup_exception',
                        'limitation': '246运行时+SDK.md+17来源保持；119历史文件被用户清理，原保留条件不能标passed'}
    report = dict(schemaVersion=1, date='2026-11-05', batch='SDK-5-17',
                  status='mapping_verified_with_explicit_historical_evidence_exception',
                  mappingComplete=True, originalCriteriaAllUnconditionallyPassed=False,
                  historicalEvidenceReverified=False, runtimeChanged=False,
                  baseline={'crate':'0.19.0','mechanismVersion':34,'rustTestsHistoricallyReported':1146,
                            'nodeTestsHistoricallyReported':22,'currentArtifactAvailability':'cleaned_by_user'},
                  generation=generation, functionalDomains=14, originalAssertions=122,
                  milestones=6, workPackages=25, remainingImplementationWorkPackages=22,
                  openGapGroups={'M':14,'I':9,'P':9}, dependencyGraphAcyclic=True,
                  newRuntimeTestsRun=0, originalScenariosExecuted=0, originalVectorsExecuted=0,
                  criteria=criteria, protected=protected_report,
                  documentLinksChecked=checked_links, unavailableHistoricalLinks=unavailable_links,
                  nextBatch=roadmap['nextBatch'])
    inputs = documents + sorted(AUDIT.glob('*.json')) + [
        ROOT/'scripts/map_sdk_5_17.py', ROOT/'scripts/verify_sdk_5_17.py',
        ROOT/'docs/plans/sdk-delivery-roadmap.json', OUT/'protected-baseline.json']
    report['inputHashes'] = {str(p.relative_to(ROOT)):sha(p) for p in sorted(set(inputs))}
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT/'verification.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({k:report[k] for k in ['status','mappingComplete','historicalEvidenceReverified',
                         'originalAssertions','documentLinksChecked','nextBatch']}, ensure_ascii=False))


if __name__ == '__main__':
    main()
