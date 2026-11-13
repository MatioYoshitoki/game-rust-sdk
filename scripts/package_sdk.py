"""Build a local Cargo + vendored-source + ordinary JSONL SDK delivery."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / 'artifacts/sdk-delivery'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(name, command, *, cwd, env=None, logs):
    p = subprocess.run(command, cwd=cwd, env={**os.environ, 'CARGO_TERM_COLOR':'never', **(env or {})},
                       capture_output=True, text=True, timeout=1200)
    (logs / f'{name}.log').write_text(p.stdout+p.stderr)
    if p.returncode:
        raise RuntimeError(f'{name} failed:\n{p.stdout[-2000:]}\n{p.stderr[-4000:]}')
    print(f'{name}: passed', flush=True)
    return p.stdout


def extract_crate(archive, destination, prefix):
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            parts = PurePosixPath(member.name).parts
            if not parts or parts[0] != prefix or '..' in parts:
                raise ValueError(f'invalid crate path: {member.name}')
            path = destination.joinpath(*parts[1:])
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                path.parent.mkdir(parents=True, exist_ok=True)
                with tar.extractfile(member) as source:
                    path.write_bytes(source.read())
                path.chmod(member.mode & 0o777)
            else:
                raise ValueError(f'unsupported crate entry: {member.name}')


def build(output):
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    package = tomllib.loads((ROOT/'Cargo.toml').read_text())['package']
    version = package['version']
    rustc = run('rustc', ['rustc','-vV'], cwd=ROOT, logs=output)
    target = next(line.split(': ',1)[1] for line in rustc.splitlines() if line.startswith('host: '))
    name = f'match-kernel-{version}-{target}'
    with tempfile.TemporaryDirectory(prefix='match-sdk-build-') as directory:
        work = Path(directory)
        root = work/name
        root.mkdir()
        run('cargo-package', ['cargo','package','--offline','--locked','--allow-dirty','--no-verify',
                              '--target-dir',str(work/'package-target')], cwd=ROOT, logs=output)
        crate = work/'package-target/package'/f'match-kernel-{version}.crate'
        (root/'sources').mkdir()
        shutil.copy2(crate,root/'sources'/crate.name)
        extract_crate(crate,root/'sdk',f'match-kernel-{version}')
        run('cargo-vendor',['cargo','vendor','--offline','--locked','--versioned-dirs',str(root/'vendor')],
            cwd=ROOT,logs=output)
        (root/'.cargo').mkdir()
        (root/'.cargo/config.toml').write_text('[source.crates-io]\nreplace-with = "vendored-sources"\n'
                                              '[source.vendored-sources]\ndirectory = "vendor"\n')
        clean_env = {'CARGO_HOME':str(work/'empty-cargo-home'), 'CARGO_NET_OFFLINE':'true',
                     'CARGO_TARGET_DIR':str(work/'release-target')}
        Path(clean_env['CARGO_HOME']).mkdir()
        run('release-host',['cargo','build','--offline','--locked','--release','--target',target,
                            '--manifest-path','sdk/Cargo.toml','--bin','kernel-host'],cwd=root,env=clean_env,logs=output)
        (root/'bin').mkdir()
        shutil.copy2(work/'release-target'/target/'release/kernel-host',root/'bin/kernel-host')
        info = json.loads(run('sdk-info',[str(root/'bin/kernel-host'),'--sdk-info'],cwd=work,env=clean_env,logs=output))
        assert info['crateVersion']==version and not info['clocks']['replay']
        (root/'support.json').write_text(json.dumps(info,indent=2)+'\n')
        (root/'hosts').mkdir()
        shutil.copy2(ROOT/'adapters/python/asset_host.py',root/'hosts/asset_host.py')
        (root/'hosts/node').mkdir()
        for source in ['asset-host.mjs','asset-store.mjs','controlled-store.mjs']:
            shutil.copy2(ROOT/'adapters/node'/source,root/'hosts/node'/source)
        for source in ['two-member-host.mjs','member-host.mjs','delta-host.mjs','multiplayer-periodic-host.mjs','multiplayer-shield-host.mjs','multiplayer-combat-host.mjs','member-area-host.mjs','member-line-host.mjs','member-piercing-host.mjs','member-skills-host.mjs','skill-hp-cost-host.mjs','skill-slot-probe.mjs']:
            shutil.copy2(ROOT/'adapters/node'/source,root/'hosts/node'/source)
        (root/'hosts/h5').mkdir()
        for source in ['persistence.mjs','two-member-server.mjs','member-server.mjs','delta-server.mjs','multiplayer-periodic-server.mjs','multiplayer-shield-server.mjs','multiplayer-combat-server.mjs','member-area-server.mjs','member-line-server.mjs','member-piercing-server.mjs','member-skills-server.mjs','skill-hp-cost-server.mjs']:
            shutil.copy2(ROOT/'adapters/h5'/source,root/'hosts/h5'/source)
        (root/'web').mkdir()
        for source in ['two-member.html','two-member.js','member.html','member.js','delta.html','delta.js','replication.mjs','periodic.mjs','multiplayer-periodic.html','multiplayer-periodic.js','shields.mjs','multiplayer-shield.html','multiplayer-shield.js','combat.mjs','multiplayer-combat.html','multiplayer-combat.js','areas.mjs','member-area.html','member-area.js','member-line.html','member-line.js','flights.mjs','member-piercing.html','member-piercing.js','skill-slots.mjs','member-skills.html','member-skills.js','hp-costs.mjs','skill-hp-cost.html','skill-hp-cost.js']:
            shutil.copy2(ROOT/'web'/source,root/'web'/source)
        (root/'docs/interfaces').mkdir(parents=True)
        for source in ['cast-interception-v1.md','cast-interception-v1.schema.json','paid-resource-effects-v1.md','paid-resource-effects-v1.schema.json','conditional-activation-v1.md','conditional-activation-v1.schema.json','status-counters-v1.md','status-counters-v1.schema.json','source-gates-v1.md','source-gates-v1.schema.json','status-ends-v1.md','status-ends-v1.schema.json','character-build-v1.md','character-build-v1.schema.json','resource-steal-v1.md','resource-steal-v1.schema.json','resource-commit-v1.md','resource-commit-v1.schema.json','unit-effects-v1.md','unit-effects-v1.schema.json','skill-loadouts-v1.md','skill-loadouts-v1.schema.json','cast-lifecycle-v1.md','cast-lifecycle-v1.schema.json','two-members-v4.md','two-members-v4.schema.json','members-v5.md','members-v5.schema.json','publications-v6.md','publications-v6.schema.json','multiplayer-periodic-v1.md','multiplayer-periodic-v1.schema.json','multiplayer-shields-v1.md','multiplayer-shields-v1.schema.json','unified-member-combat-v1.md','unified-member-combat-v1.schema.json','member-circle-plans-v1.md','member-circle-plans-v1.schema.json','member-line-plans-v1.md','member-line-plans-v1.schema.json','member-piercing-v1.md','member-piercing-v1.schema.json','member-skill-slots-v1.md','member-skill-slots-v1.schema.json','skill-hp-cost-v1.md','skill-hp-cost-v1.schema.json','cast-movement-v1.md','direction-charge-v1.md','charge-reaim-v1.md','fixed-channel-v1.md','projectile-recast-v1.md','recovery-followup-v1.md','cancelable-recovery-v1.md','basic-followup-v1.md','locked-basic-target-v1.md','basic-pursuit-v1.md','auto-basic-pursuit-v1.md','member-loot-drop-v1.md','member-item-use-v1.md','member-item-cooldown-v1.md','member-item-spirit-v1.md','member-item-cooldown-group-v1.md','member-item-dual-recovery-v1.md','member-item-ratio-recovery-v1.md','member-item-quality-recovery-v1.md','member-item-self-status-v1.md','member-item-status-quality-v1.md','member-item-damage-reduction-v1.md','member-periodic-damage-reduction-v1.md','member-periodic-healing-received-v1.md','member-item-healing-received-v1.md','member-recovery-orb-v1.md','member-recovery-strength-v1.md','member-spirit-recovery-lock-v1.md','member-npc-spirit-recovery-lock-v1.md','member-overheal-shield-v1.md','member-periodic-overheal-shield-v1.md','member-instant-self-heal-v1.md','member-ally-heal-v1.md','member-ally-hot-v1.md','member-ally-shield-v1.md','member-ally-overheal-shield-v1.md','member-ally-hot-overheal-shield-v1.md','member-npc-silence-v1.md','member-control-immunity-v1.md','member-control-duration-reduction-v1.md','member-first-control-immunity-v1.md','member-item-cleanse-v1.md','member-npc-root-v1.md','member-npc-stun-v1.md','member-npc-movement-slow-v1.md','member-movement-haste-v1.md','member-cooldown-reduction-v1.md','member-armor-penetration-v1.md','member-critical-chance-v1.md','member-critical-damage-bonus-v1.md','member-dodge-penalty-v1.md','member-dodge-chance-v1.md','member-accuracy-v1.md','member-npc-hit-v1.md','member-damage-immunity-v1.md','member-npc-unavoidable-v1.md','member-unavoidable-v1.md','direct-ignore-armor-v1.md','member-skill-power-v1.md','combat-rules-v1.md','combat-rules-v1.schema.json']:
            shutil.copy2(ROOT/'docs/interfaces'/source,root/'docs/interfaces'/source)
            if source.endswith('.md'):
                file=root/'docs/interfaces'/source
                file.write_text(file.read_text().replace('[批次规格](../batches/SDK-5-18-two-member-loop.md)','批次规格（仓库SDK-5-18）；当前能力以support.json为准').replace('[DP01～DP12](../batches/SDK-5-20-member-delta.md)','仓库SDK-5-20规格DP01～DP12').replace('[规格MC01～MC10](../batches/SDK-5-19-member-capacity.md)','仓库SDK-5-19规格MC01～MC10'))
        (root/'docs/batches').mkdir()
        for source in ['world-geometry-v1.md', 'npc-combat-ai-v1.md', 'participant-ai-v1.md', 'world-loot-v1.md', 'settlement-rewards-v1.md', 'settlement-facts-v1.md', 'settlement-facts-v1.schema.json', 'world-boons-v1.md']:
            shutil.copy2(ROOT/'docs/interfaces'/source,root/'docs/interfaces'/source)
        shutil.copy2(ROOT/'docs/batches/SDK-5-21-multiplayer-periodic.md',root/'docs/batches/SDK-5-21-multiplayer-periodic.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-22-multiplayer-shields.md',root/'docs/batches/SDK-5-22-multiplayer-shields.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-23-unified-member-combat.md',root/'docs/batches/SDK-5-23-unified-member-combat.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-24-member-circle-plans.md',root/'docs/batches/SDK-5-24-member-circle-plans.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-25-member-line-plans.md',root/'docs/batches/SDK-5-25-member-line-plans.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-26-member-piercing.md',root/'docs/batches/SDK-5-26-member-piercing.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-27-member-skill-slots.md',root/'docs/batches/SDK-5-27-member-skill-slots.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-28-skill-hp-cost.md',root/'docs/batches/SDK-5-28-skill-hp-cost.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-29-cast-movement.md',root/'docs/batches/SDK-5-29-cast-movement.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-30-direction-charge.md',root/'docs/batches/SDK-5-30-direction-charge.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-31-charge-reaim.md',root/'docs/batches/SDK-5-31-charge-reaim.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-32-fixed-channel.md',root/'docs/batches/SDK-5-32-fixed-channel.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-33-projectile-recast.md',root/'docs/batches/SDK-5-33-projectile-recast.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-34-recovery-followup.md',root/'docs/batches/SDK-5-34-recovery-followup.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-35-cancelable-recovery.md',root/'docs/batches/SDK-5-35-cancelable-recovery.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-36-basic-followup.md',root/'docs/batches/SDK-5-36-basic-followup.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-37-locked-basic-target.md',root/'docs/batches/SDK-5-37-locked-basic-target.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-38-basic-pursuit.md',root/'docs/batches/SDK-5-38-basic-pursuit.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-39-auto-basic-pursuit.md',root/'docs/batches/SDK-5-39-auto-basic-pursuit.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-40-member-loot-drop.md',root/'docs/batches/SDK-5-40-member-loot-drop.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-41-member-item-use.md',root/'docs/batches/SDK-5-41-member-item-use.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-42-member-item-cooldown.md',root/'docs/batches/SDK-5-42-member-item-cooldown.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-43-member-item-spirit.md',root/'docs/batches/SDK-5-43-member-item-spirit.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-44-member-item-cooldown-group.md',root/'docs/batches/SDK-5-44-member-item-cooldown-group.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-45-member-item-dual-recovery.md',root/'docs/batches/SDK-5-45-member-item-dual-recovery.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-46-member-item-ratio-recovery.md',root/'docs/batches/SDK-5-46-member-item-ratio-recovery.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-47-member-item-quality-recovery.md',root/'docs/batches/SDK-5-47-member-item-quality-recovery.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-48-member-item-self-status.md',root/'docs/batches/SDK-5-48-member-item-self-status.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-49-member-item-status-quality.md',root/'docs/batches/SDK-5-49-member-item-status-quality.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-50-member-item-damage-reduction.md',root/'docs/batches/SDK-5-50-member-item-damage-reduction.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-51-member-periodic-damage-reduction.md',root/'docs/batches/SDK-5-51-member-periodic-damage-reduction.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-52-member-periodic-healing-received.md',root/'docs/batches/SDK-5-52-member-periodic-healing-received.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-53-member-item-healing-received.md',root/'docs/batches/SDK-5-53-member-item-healing-received.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-54-member-recovery-orb.md',root/'docs/batches/SDK-5-54-member-recovery-orb.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-55-member-recovery-strength.md',root/'docs/batches/SDK-5-55-member-recovery-strength.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-56-member-spirit-recovery-lock.md',root/'docs/batches/SDK-5-56-member-spirit-recovery-lock.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-57-member-npc-spirit-recovery-lock.md',root/'docs/batches/SDK-5-57-member-npc-spirit-recovery-lock.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-58-member-overheal-shield.md',root/'docs/batches/SDK-5-58-member-overheal-shield.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-59-member-periodic-overheal-shield.md',root/'docs/batches/SDK-5-59-member-periodic-overheal-shield.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-60-member-instant-self-heal.md',root/'docs/batches/SDK-5-60-member-instant-self-heal.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-61-member-ally-heal.md',root/'docs/batches/SDK-5-61-member-ally-heal.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-62-member-ally-hot.md',root/'docs/batches/SDK-5-62-member-ally-hot.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-63-member-ally-shield.md',root/'docs/batches/SDK-5-63-member-ally-shield.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-64-member-ally-overheal-shield.md',root/'docs/batches/SDK-5-64-member-ally-overheal-shield.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-65-member-ally-hot-overheal-shield.md',root/'docs/batches/SDK-5-65-member-ally-hot-overheal-shield.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-66-member-npc-silence.md',root/'docs/batches/SDK-5-66-member-npc-silence.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-67-member-control-immunity.md',root/'docs/batches/SDK-5-67-member-control-immunity.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-68-member-control-duration-reduction.md',root/'docs/batches/SDK-5-68-member-control-duration-reduction.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-69-member-first-control-immunity.md',root/'docs/batches/SDK-5-69-member-first-control-immunity.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-70-member-item-cleanse.md',root/'docs/batches/SDK-5-70-member-item-cleanse.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-71-member-npc-root.md',root/'docs/batches/SDK-5-71-member-npc-root.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-72-member-npc-stun.md',root/'docs/batches/SDK-5-72-member-npc-stun.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-73-member-npc-movement-slow.md',root/'docs/batches/SDK-5-73-member-npc-movement-slow.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-74-member-movement-haste.md',root/'docs/batches/SDK-5-74-member-movement-haste.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-75-member-cooldown-reduction.md',root/'docs/batches/SDK-5-75-member-cooldown-reduction.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-76-member-armor-penetration.md',root/'docs/batches/SDK-5-76-member-armor-penetration.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-77-member-critical-chance.md',root/'docs/batches/SDK-5-77-member-critical-chance.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-78-member-critical-damage-bonus.md',root/'docs/batches/SDK-5-78-member-critical-damage-bonus.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-79-member-dodge-penalty.md',root/'docs/batches/SDK-5-79-member-dodge-penalty.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-80-member-dodge-chance.md',root/'docs/batches/SDK-5-80-member-dodge-chance.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-81-member-accuracy.md',root/'docs/batches/SDK-5-81-member-accuracy.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-82-member-npc-hit.md',root/'docs/batches/SDK-5-82-member-npc-hit.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-83-member-damage-immunity.md',root/'docs/batches/SDK-5-83-member-damage-immunity.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-84-member-npc-unavoidable.md',root/'docs/batches/SDK-5-84-member-npc-unavoidable.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-85-member-unavoidable.md',root/'docs/batches/SDK-5-85-member-unavoidable.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-86-direct-ignore-armor.md',root/'docs/batches/SDK-5-86-direct-ignore-armor.md')
        shutil.copy2(ROOT/'docs/batches/SDK-5-87-member-skill-power.md',root/'docs/batches/SDK-5-87-member-skill-power.md')
        for source in ['SDK-5-88-combat-and-casting.md','SDK-5-88-coverage.json','SDK-5-88-cast-interception.md','SDK-5-88-source-execution.md','SDK-5-88-source-cast-scenarios.md','SDK-5-88-action-terminal-counters.md','SDK-5-88-basic-wave.md','SDK-5-88-basic-profiles.md','SDK-5-88-basic-impact.md','SDK-5-88-basic-hitscan-patterns.md','SDK-5-88-basic-beam.md','SDK-5-88-basic-wave-controls.md','SDK-5-88-basic-projectile-followups.md','SDK-5-88-basic-projectile-paths.md','SDK-5-88-basic-world-fields.md','SDK-5-88-basic-burning-fields.md','SDK-5-88-basic-charge.md','SDK-5-88-basic-flight-and-stack-finisher.md','SDK-5-88-weapon-equipment.md','SDK-5-88-moving-enter-area.md','SDK-5-88-detached-enter-area.md','SDK-5-88-npc-ground-field.md','SDK-5-88-npc-ground-strike.md','SDK-5-88-checkpoint-zones.md','SDK-5-88-member-ground-traps.md','SDK-5-88-member-cast-anchors.md','SDK-5-88-member-anchor-recast.md','SDK-5-88-anchor-following-fields.md','SDK-5-88-anchor-return-and-end.md','SDK-5-88-source-time-current.md','SDK-5-88-source-primordial-seal.md','SDK-5-88-source-retaliation-window.md','SDK-5-88-source-guard-families.md','SDK-5-88-source-stillness-guard.md','SDK-5-88-source-ally-recovery.md','SDK-5-88-source-ally-self-and-marks.md']:
            shutil.copy2(ROOT/'docs/batches'/source,root/'docs/batches'/source)
        (root/'docs/decisions').mkdir(exist_ok=True)
        for source in (ROOT/'docs/decisions').glob('SDK-5-88-*.md'):
            shutil.copy2(source,root/'docs/decisions'/source.name)
        for decision in ['SDK-5-88-movement-status-mapping.md','SDK-5-88-attack-speed-mapping.md']:
            shutil.copy2(ROOT/'docs/decisions'/decision,root/'docs/decisions'/decision)
        document=root/'docs/interfaces/unified-member-combat-v1.md'
        document.write_text(document.read_text().replace('(unit-targets-v1.md)','(../../sdk/SDK.md)'))
        document=root/'docs/interfaces/multiplayer-shields-v1.md'
        document.write_text(document.read_text().replace('(absorption-shields-v1.md)','(../../sdk/SDK.md)'))
        document=root/'docs/interfaces/multiplayer-periodic-v1.md'
        document.write_text(document.read_text().replace('(periodic-damage-v1.md)','(../../sdk/SDK.md)').replace('(periodic-healing-v1.md)','(../../sdk/SDK.md)'))
        (root/'docs/guides').mkdir(exist_ok=True)
        for source in ['node-quickstart.md','sdk-integration.md','h5-lab.md']:
            guide=root/'docs/guides'/source
            shutil.copy2(ROOT/'docs/guides'/source,guide)
            # 交付包目录布局与仓库不同：hosts/consumer 替代 adapters/examples，仓库文档不入包。
            text=(guide.read_text()
                  .replace('(../../adapters/node/two-member-host.test.mjs)','(../hosts/node/two-member-host.mjs)')
                  .replace('adapters/node/two-member-host.test.mjs（测试即样例）','hosts/node/two-member-host.mjs（库本体；边界用例测试在仓库）')
                  .replace('${SDK}/adapters/node/','${SDK}/hosts/node/')
                  .replace('${SDK}/target/release/kernel-host','${SDK}/bin/kernel-host')
                  .replace('../../adapters/node/','../hosts/node/')
                  .replace('../../adapters/h5/','../hosts/h5/')
                  .replace('../../examples/minimal-host/','../consumer/')
                  .replace('(../../web/app.js)','(../web/member.js)')
                  .replace('(../../SDK.md)','(../sdk/SDK.md)')
                  .replace('../../src/','../sdk/src/')
                  .replace('[AGENTS.md](../../AGENTS.md)','仓库 AGENTS.md')
                  .replace('[PRD §3.7](../../PRD-367-independent-rust-match-kernel.md)','PRD §3.7（仓库 PRD-367）'))
            guide.write_text(text)
        shutil.copytree(ROOT/'examples/minimal-host/src',root/'consumer/src')
        consumer=(ROOT/'examples/minimal-host/Cargo.toml').read_text().replace('path = "../.."','path = "../sdk"')
        (root/'consumer/Cargo.toml').write_text(consumer)
        shutil.copy2(ROOT/'examples/minimal-host/Cargo.lock',root/'consumer/Cargo.lock')
        shutil.copy2(ROOT/'delivery/README.md',root/'README.md')
        shutil.copy2(ROOT/'delivery/verify_bundle.py',root/'verify_bundle.py')
        dependencies=[]
        for file in sorted((root/'vendor').glob('*/Cargo.toml')):
            data=tomllib.loads(file.read_text())['package']
            dependencies.append({'name':data['name'],'version':data['version'],'license':data.get('license')})
        manifest={'schemaVersion':1,'package':package['name'],'version':version,
                  'distribution':'local-development','publish':False,'target':target,
                  'profile':'release','rustc':rustc.strip(),'python':sys.version,
                  'support':info,'dependencies':dependencies,
                  'sourceCrateSha256':digest(crate),
                  'contents':{'sdk':'sdk','rustConsumer':'consumer','jsonlHost':'bin/kernel-host',
                              'persistentHost':'hosts/asset_host.py'},
                  'generatedPaths':['target','consumer/target','sdk/target']}
        (root/'distribution.json').write_text(json.dumps(manifest,indent=2)+'\n')
        for file in root.rglob('*.json'):
            relative = file.relative_to(root)
            if relative.parts[0] == 'vendor':
                continue
            if relative.as_posix() in {
                'support.json', 'distribution.json', 'sdk/.cargo_vcs_info.json',
                'docs/batches/SDK-5-88-coverage.json',
            } or (relative.parts[:2] == ('docs', 'interfaces') and file.name.endswith('.schema.json')):
                continue
            raise ValueError(f'unexpected JSON in SDK delivery: {relative}')
        files=sorted(p for p in root.rglob('*') if p.is_file())
        (root/'SHA256SUMS').write_text(''.join(f'{digest(p)}  {p.relative_to(root).as_posix()}\n' for p in files))
        run('bundle-check',[sys.executable,'verify_bundle.py'],cwd=root,logs=output)
        archive=output/(name+'.tar.gz')
        pending=output/(name+'.tar.gz.tmp')
        with tarfile.open(pending,'w:gz') as tar:
            for path in sorted(root.rglob('*')):
                tar.add(path,arcname=str(Path(name)/path.relative_to(root)),recursive=False)
        pending.replace(archive)
        result={'archive':str(archive),'sha256':digest(archive),'rootName':name,'version':version,
                'target':target,'sourceCrateSha256':digest(crate),'binarySha256':digest(root/'bin/kernel-host'),
                'packageFiles':len(files),'dependencies':dependencies}
        (output/'package.json').write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps({k:v for k,v in result.items() if k!='dependencies'}),flush=True)
        return result


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out',type=Path,default=DEFAULT_OUTPUT)
    build(parser.parse_args().out)
