"""Verify the exact packaged source and ordinary runtime from outside the checkout."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from package_sdk import ROOT, digest, extract_crate

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--browser-port", type=int)
    profiles = parser.add_mutually_exclusive_group()
    profiles.add_argument("--ratio", action="store_true")
    profiles.add_argument("--quality", action="store_true")
    profiles.add_argument("--status", action="store_true")
    profiles.add_argument("--status-quality", action="store_true")
    profiles.add_argument("--mitigation", action="store_true")
    profiles.add_argument("--periodic-mitigation", action="store_true")
    profiles.add_argument("--periodic-healing-received", action="store_true")
    profiles.add_argument("--item-healing-received", action="store_true")
    profiles.add_argument("--recovery-orb", action="store_true")
    profiles.add_argument("--recovery-strength", action="store_true")
    profiles.add_argument("--spirit-recovery-lock", action="store_true")
    profiles.add_argument("--npc-spirit-recovery-lock", action="store_true")
    profiles.add_argument("--overheal-shield", action="store_true")
    profiles.add_argument("--periodic-overheal-shield", action="store_true")
    profiles.add_argument("--instant-self-heal", action="store_true")
    profiles.add_argument("--ally-heal", action="store_true")
    profiles.add_argument("--ally-hot", action="store_true")
    profiles.add_argument("--ally-shield", action="store_true")
    profiles.add_argument("--ally-overheal-shield", action="store_true")
    profiles.add_argument("--ally-hot-overheal-shield", action="store_true")
    profiles.add_argument("--npc-silence", action="store_true")
    profiles.add_argument("--control-immunity", action="store_true")
    profiles.add_argument("--control-duration-reduction", action="store_true")
    profiles.add_argument("--first-control-immunity", action="store_true")
    profiles.add_argument("--item-cleanse", action="store_true")
    profiles.add_argument("--npc-root", action="store_true")
    profiles.add_argument("--npc-stun", action="store_true")
    profiles.add_argument("--npc-movement-slow", action="store_true")
    profiles.add_argument("--movement-haste", action="store_true")
    profiles.add_argument("--cooldown-reduction", action="store_true")
    profiles.add_argument("--armor-penetration", action="store_true")
    profiles.add_argument("--critical-chance", action="store_true")
    profiles.add_argument("--critical-damage-bonus", action="store_true")
    profiles.add_argument("--skill-power", action="store_true")
    profiles.add_argument("--ignore-armor", action="store_true")
    profiles.add_argument("--member-unavoidable", action="store_true")
    profiles.add_argument("--unavoidable", action="store_true")
    profiles.add_argument("--damage-immunity", action="store_true")
    profiles.add_argument("--npc-hit", action="store_true")
    profiles.add_argument("--accuracy", action="store_true")
    profiles.add_argument("--dodge-chance", action="store_true")
    profiles.add_argument("--dodge-penalty", action="store_true")
    args = parser.parse_args()
    flags, legacy_flags = [], []
    public_script = "verify_sdk_5_45_public.mjs"
    if args.skill_power:
        batch, version, mechanism, port = 87, "0.89.0", 104, 4359
        capability = "memberSkillPower"
        feature = "member-skill-power"
        legacy_feature = "direct-ignore-armor"
        public_script, legacy_script = "verify_sdk_5_87_public.mjs", "verify_sdk_5_85_public.mjs"
        flags, legacy_flags = ["--skill-power"], ["--ignore-armor"]
    elif args.ignore_armor:
        batch, version, mechanism, port = 86, "0.88.0", 103, 4357
        capability = "directIgnoreArmor"
        feature = "direct-ignore-armor"
        legacy_feature = "member-unavoidable"
        public_script = legacy_script = "verify_sdk_5_85_public.mjs"
        flags = ["--ignore-armor"]
    elif args.member_unavoidable:
        batch, version, mechanism, port = 85, "0.87.0", 102, 4355
        capability = "memberUnavoidable"
        feature = "member-unavoidable"
        legacy_feature = "member-npc-unavoidable"
        public_script, legacy_script = "verify_sdk_5_85_public.mjs", "verify_sdk_5_84_public.mjs"
        flags = ["--member-unavoidable"]
    elif args.unavoidable:
        batch, version, mechanism, port = 84, "0.86.0", 101, 4353
        capability = "memberNpcUnavoidable"
        feature = "member-npc-unavoidable"
        legacy_feature = "member-damage-immunity"
        public_script, legacy_script = "verify_sdk_5_84_public.mjs", "verify_sdk_5_80_public.mjs"
        flags, legacy_flags = ["--unavoidable"], ["--damage-immunity"]
    elif args.damage_immunity:
        batch, version, mechanism, port = 83, "0.85.0", 100, 4351
        capability = "memberDamageImmunity"
        feature = "member-damage-immunity"
        legacy_feature = "member-npc-hit"
        public_script = legacy_script = "verify_sdk_5_80_public.mjs"
        flags, legacy_flags = ["--damage-immunity"], ["--npc-hit"]
    elif args.npc_hit:
        batch, version, mechanism, port = 82, "0.84.0", 99, 4349
        capability = "memberNpcHit"
        feature = "member-npc-hit"
        legacy_feature = "member-accuracy"
        public_script, legacy_script = "verify_sdk_5_80_public.mjs", "verify_sdk_5_79_public.mjs"
        flags, legacy_flags = ["--npc-hit"], ["--accuracy"]
    elif args.accuracy:
        batch, version, mechanism, port = 81, "0.83.0", 98, 4347
        capability = "memberAccuracy"
        feature = "member-accuracy"
        legacy_feature = "member-dodge-chance"
        public_script, legacy_script = "verify_sdk_5_79_public.mjs", "verify_sdk_5_80_public.mjs"
        flags = ["--accuracy"]
    elif args.dodge_chance:
        batch, version, mechanism, port = 80, "0.82.0", 97, 4345
        capability = "memberDodgeChance"
        feature = "member-dodge-chance"
        legacy_feature = "member-dodge-penalty"
        public_script, legacy_script = "verify_sdk_5_80_public.mjs", "verify_sdk_5_79_public.mjs"
        flags = ["--dodge-chance"]
    elif args.dodge_penalty:
        batch, version, mechanism, port = 79, "0.81.0", 96, 4343
        capability = "memberDodgePenalty"
        feature = "member-dodge-penalty"
        legacy_feature = "member-critical-damage-bonus"
        public_script, legacy_script = "verify_sdk_5_79_public.mjs", "verify_sdk_5_78_public.mjs"
        flags = ["--dodge-penalty"]
    elif args.critical_damage_bonus:
        batch, version, mechanism, port = 78, "0.80.0", 95, 4341
        capability = "memberCriticalDamageBonus"
        feature = "member-critical-damage-bonus"
        legacy_feature = "member-critical-chance"
        public_script, legacy_script = "verify_sdk_5_78_public.mjs", "verify_sdk_5_77_public.mjs"
        flags = ["--critical-damage-bonus"]
    elif args.critical_chance:
        batch, version, mechanism, port = 77, "0.79.0", 94, 4339
        capability = "memberCriticalChance"
        feature = "member-critical-chance"
        legacy_feature = "member-armor-penetration"
        public_script, legacy_script = "verify_sdk_5_77_public.mjs", "verify_sdk_5_76_public.mjs"
        flags = ["--critical-chance"]
    elif args.armor_penetration:
        batch, version, mechanism, port = 76, "0.78.0", 93, 4337
        capability = "memberArmorPenetration"
        feature = "member-armor-penetration"
        legacy_feature = "member-cooldown-reduction"
        public_script, legacy_script = "verify_sdk_5_76_public.mjs", "verify_sdk_5_75_public.mjs"
        flags = ["--armor-penetration"]
    elif args.cooldown_reduction:
        batch, version, mechanism, port = 75, "0.77.0", 92, 4335
        capability = "memberCooldownReduction"
        feature = "member-cooldown-reduction"
        legacy_feature = "member-movement-haste"
        public_script, legacy_script = "verify_sdk_5_75_public.mjs", "verify_sdk_5_74_public.mjs"
        flags = ["--cooldown-reduction"]
    elif args.movement_haste:
        batch, version, mechanism, port = 74, "0.76.0", 91, 4333
        capability = "memberMovementHaste"
        feature = "member-movement-haste"
        legacy_feature = "member-npc-movement-slow"
        public_script, legacy_script = "verify_sdk_5_74_public.mjs", "verify_sdk_5_73_public.mjs"
        flags = ["--movement-haste"]
    elif args.npc_movement_slow:
        batch, version, mechanism, port = 73, "0.75.0", 90, 4331
        capability = "memberNpcMovementSlow"
        feature = "member-npc-movement-slow"
        legacy_feature = "member-npc-stun"
        public_script, legacy_script = "verify_sdk_5_73_public.mjs", "verify_sdk_5_72_public.mjs"
        flags = ["--npc-movement-slow"]
    elif args.npc_stun:
        batch, version, mechanism, port = 72, "0.74.0", 89, 4329
        capability = "memberNpcStun"
        feature = "member-npc-stun"
        legacy_feature = "member-npc-root"
        public_script, legacy_script = "verify_sdk_5_72_public.mjs", "verify_sdk_5_71_public.mjs"
        flags = ["--npc-stun"]
    elif args.npc_root:
        batch, version, mechanism, port = 71, "0.73.0", 88, 4327
        capability = "memberNpcRoot"
        feature = "member-npc-root"
        legacy_feature = "member-item-cleanse"
        public_script, legacy_script = "verify_sdk_5_71_public.mjs", "verify_sdk_5_70_public.mjs"
        flags = ["--npc-root"]
    elif args.item_cleanse:
        batch, version, mechanism, port = 70, "0.72.0", 87, 4325
        capability = "memberItemCleanse"
        feature = "member-item-cleanse"
        legacy_feature = "member-first-control-immunity"
        public_script, legacy_script = "verify_sdk_5_70_public.mjs", "verify_sdk_5_69_public.mjs"
        flags = ["--item-cleanse"]
    elif args.first_control_immunity:
        batch, version, mechanism, port = 69, "0.71.0", 86, 4323
        capability = "memberFirstControlImmunity"
        feature = "member-first-control-immunity"
        legacy_feature = "member-control-duration-reduction"
        public_script, legacy_script = "verify_sdk_5_69_public.mjs", "verify_sdk_5_68_public.mjs"
        flags = ["--first-control-immunity"]
    elif args.control_duration_reduction:
        batch, version, mechanism, port = 68, "0.70.0", 85, 4321
        capability = "memberControlDurationReduction"
        feature = "member-control-duration-reduction"
        legacy_feature = "member-control-immunity"
        public_script, legacy_script = "verify_sdk_5_68_public.mjs", "verify_sdk_5_67_public.mjs"
        flags = ["--control-duration-reduction"]
    elif args.control_immunity:
        batch, version, mechanism, port = 67, "0.69.0", 84, 4319
        capability = "memberControlImmunity"
        feature = "member-control-immunity"
        legacy_feature = "member-npc-silence"
        public_script, legacy_script = "verify_sdk_5_67_public.mjs", "verify_sdk_5_66_public.mjs"
        flags = ["--control-immunity"]
    elif args.npc_silence:
        batch, version, mechanism, port = 66, "0.68.0", 83, 4317
        capability = "memberNpcSilence"
        feature = "member-npc-silence"
        legacy_feature = "member-ally-hot-overheal-shield"
        public_script, legacy_script = "verify_sdk_5_66_public.mjs", "verify_sdk_5_65_public.mjs"
        flags = ["--npc-silence"]
    elif args.ally_hot_overheal_shield:
        batch, version, mechanism, port = 65, "0.67.0", 82, 4315
        capability = "memberAllyHotOverhealShield"
        feature = "member-ally-hot-overheal-shield"
        legacy_feature = "member-ally-overheal-shield"
        public_script, legacy_script = "verify_sdk_5_65_public.mjs", "verify_sdk_5_64_public.mjs"
        flags = ["--ally-hot-overheal-shield"]
    elif args.ally_overheal_shield:
        batch, version, mechanism, port = 64, "0.66.0", 81, 4313
        capability = "memberAllyOverhealShield"
        feature = "member-ally-overheal-shield"
        legacy_feature = "member-ally-shield"
        public_script, legacy_script = "verify_sdk_5_64_public.mjs", "verify_sdk_5_63_public.mjs"
        flags = ["--ally-overheal-shield"]
    elif args.ally_shield:
        batch, version, mechanism, port = 63, "0.65.0", 80, 4309
        capability = "memberAllyShield"
        feature = "member-ally-shield"
        legacy_feature = "member-ally-hot"
        public_script, legacy_script = "verify_sdk_5_63_public.mjs", "verify_sdk_5_62_public.mjs"
        flags = ["--ally-shield"]
    elif args.ally_hot:
        batch, version, mechanism, port = 62, "0.64.0", 79, 4307
        capability = "memberAllyHot"
        feature = "member-ally-hot"
        legacy_feature = "member-ally-heal"
        public_script, legacy_script = "verify_sdk_5_62_public.mjs", "verify_sdk_5_61_public.mjs"
        flags = ["--ally-hot"]
    elif args.ally_heal:
        batch, version, mechanism, port = 61, "0.63.0", 78, 4305
        capability = "memberAllyHeal"
        feature = "member-ally-heal"
        legacy_feature = "member-instant-self-heal"
        public_script, legacy_script = "verify_sdk_5_61_public.mjs", "verify_sdk_5_60_public.mjs"
        flags = ["--ally-heal"]
    elif args.instant_self_heal:
        batch, version, mechanism, port = 60, "0.62.0", 77, 4303
        capability = "memberInstantSelfHeal"
        feature = "member-instant-self-heal"
        legacy_feature = "member-periodic-overheal-shield"
        public_script, legacy_script = "verify_sdk_5_60_public.mjs", "verify_sdk_5_59_public.mjs"
        flags = ["--instant-self-heal"]
    elif args.periodic_overheal_shield:
        batch, version, mechanism, port = 59, "0.61.0", 76, 4299
        capability = "memberPeriodicOverhealShield"
        feature = "member-periodic-overheal-shield"
        legacy_feature = "member-overheal-shield"
        public_script, legacy_script = "verify_sdk_5_59_public.mjs", "verify_sdk_5_58_public.mjs"
        flags = ["--periodic-overheal-shield"]
    elif args.overheal_shield:
        batch, version, mechanism, port = 58, "0.60.0", 75, 4297
        capability = "memberOverhealShield"
        feature = "member-overheal-shield"
        legacy_feature = "member-npc-spirit-recovery-lock"
        public_script, legacy_script = "verify_sdk_5_58_public.mjs", "verify_sdk_5_56_public.mjs"
        flags, legacy_flags = ["--overheal-shield"], ["--npc-source"]
    elif args.npc_spirit_recovery_lock:
        batch, version, mechanism, port = 57, "0.59.0", 74, 4295
        capability = "memberNpcSpiritRecoveryLock"
        feature = "member-npc-spirit-recovery-lock"
        legacy_feature = "member-spirit-recovery-lock"
        public_script = legacy_script = "verify_sdk_5_56_public.mjs"
        flags = ["--npc-spirit-recovery-lock"]
    elif args.spirit_recovery_lock:
        batch, version, mechanism, port = 56, "0.58.0", 73, 4293
        capability = "memberSpiritRecoveryLock"
        feature = "member-spirit-recovery-lock"
        legacy_feature = "member-recovery-strength"
        public_script, legacy_script = "verify_sdk_5_56_public.mjs", "verify_sdk_5_55_public.mjs"
        flags = ["--spirit-recovery-lock"]
    elif args.recovery_strength:
        batch, version, mechanism, port = 55, "0.57.0", 72, 4291
        capability = "memberRecoveryStrength"
        feature = "member-recovery-strength"
        legacy_feature = "member-recovery-orb"
        public_script, legacy_script = "verify_sdk_5_55_public.mjs", "verify_sdk_5_54_public.mjs"
        flags = ["--recovery-strength"]
    elif args.recovery_orb:
        batch, version, mechanism, port = 54, "0.56.0", 71, 4289
        capability = "memberRecoveryOrb"
        feature = "member-recovery-orb"
        legacy_feature = "member-item-healing-received"
        public_script, legacy_script = "verify_sdk_5_54_public.mjs", "verify_sdk_5_53_public.mjs"
        flags = ["--recovery-orb"]
    elif args.item_healing_received:
        batch, version, mechanism, port = 53, "0.55.0", 70, 4287
        capability = "memberItemHealingReceived"
        feature = "member-item-healing-received"
        legacy_feature = "member-periodic-healing-received"
        public_script, legacy_script = "verify_sdk_5_53_public.mjs", "verify_sdk_5_52_public.mjs"
        flags = ["--item-healing-received"]
    elif args.periodic_healing_received:
        batch, version, mechanism, port = 52, "0.54.0", 69, 4285
        capability = "memberPeriodicHealingReceived"
        feature = "member-periodic-healing-received"
        legacy_feature = "member-periodic-damage-reduction"
        public_script = "verify_sdk_5_52_public.mjs"
        legacy_script = "verify_sdk_5_50_public.mjs"
        flags, legacy_flags = ["--periodic-healing-received"], ["--periodic"]
    elif args.periodic_mitigation:
        batch, version, mechanism, port = 51, "0.53.0", 68, 4283
        capability = "memberPeriodicDamageReduction"
        feature = "member-periodic-damage-reduction"
        legacy_feature = "member-item-damage-reduction"
        public_script = legacy_script = "verify_sdk_5_50_public.mjs"
        flags = ["--periodic-mitigation"]
    elif args.mitigation:
        batch, version, mechanism, port = 50, "0.52.0", 67, 4281
        capability = "memberItemDamageReduction"
        feature = "member-item-damage-reduction"
        legacy_feature, legacy_script = "member-item-status-quality", "verify_sdk_5_45_public.mjs"
        public_script = "verify_sdk_5_50_public.mjs"
        flags, legacy_flags = ["--mitigation"], ["--status-quality"]
    elif args.status_quality:
        batch, version, mechanism, port = 49, "0.51.0", 66, 4279
        capability = "memberItemStatusQuality"
        feature = "member-item-status-quality"
        legacy_feature, legacy_script = "member-item-self-status", "verify_sdk_5_45_public.mjs"
        flags, legacy_flags = ["--status-quality"], ["--status"]
    elif args.status:
        batch, version, mechanism, port = 48, "0.50.0", 65, 4277
        capability = "memberItemSelfStatus"
        feature = "member-item-self-status"
        legacy_feature, legacy_script = "member-item-quality-recovery", "verify_sdk_5_45_public.mjs"
        flags, legacy_flags = ["--status"], ["--quality"]
    elif args.quality:
        batch, version, mechanism, port = 47, "0.49.0", 64, 4275
        capability = "memberItemQualityRecovery"
        feature = "member-item-quality-recovery"
        legacy_feature, legacy_script = "member-item-ratio-recovery", "verify_sdk_5_45_public.mjs"
        flags, legacy_flags = ["--quality"], ["--ratio"]
    elif args.ratio:
        batch, version, mechanism, port = 46, "0.48.0", 63, 4273
        capability = "memberItemRatioRecovery"
        feature = "member-item-ratio-recovery"
        legacy_feature, legacy_script = "member-item-dual-recovery", "verify_sdk_5_45_public.mjs"
        flags = ["--ratio"]
    else:
        batch, version, mechanism, port = 45, "0.47.0", 62, 4271
        capability = "memberItemDualRecovery"
        feature = "member-item-dual-recovery"
        legacy_feature, legacy_script = "member-item-cooldown-group", "verify_sdk_5_44_public.mjs"
    out = Path(os.environ.get(f"SDK_5_{batch}_EVIDENCE_DIR", ROOT / f"artifacts/sdk-5-{batch}")).resolve()
    delivery = out / "delivery"
    test_filter = feature.replace("-", "_")
    source_test = f"offline-{feature}-tests"
    public_test = f"public-{feature}"
    legacy_test = f"legacy-mechanism-{mechanism - 1}"
    legacy_dir = f"legacy{mechanism - 1}"

    def run(name, command, cwd, env=None):
        result = subprocess.run(command, cwd=cwd, env={**os.environ, **(env or {})},
                                capture_output=True, text=True, timeout=1200)
        (delivery / f"{name}.log").write_text(result.stdout + result.stderr)
        assert result.returncode == 0, (name, result.stderr[-3000:], result.stdout[-1500:])
        print(f"{name}: passed", flush=True)
        return result.stdout

    package = json.loads((delivery / "package.json").read_text())
    assert digest(package["archive"]) == package["sha256"]
    with tempfile.TemporaryDirectory(prefix=f"sdk-5-{batch}-consume-") as directory:
        work = Path(directory)
        bundle = work / "bundle"
        extract_crate(package["archive"], bundle, package["rootName"])
        run("integrity", ["python3", "verify_bundle.py"], bundle)
        binary = bundle / "bin/kernel-host"
        assert digest(binary) == package["binarySha256"]
        sources = [*ROOT.glob("src/**/*.rs"), *ROOT.glob("src/**/*.json")]
        for source in sources:
            assert digest(source) == digest(bundle / "sdk" / source.relative_to(ROOT)), source

        cargo_home = work / "empty-cargo-home"
        cargo_home.mkdir()
        offline = {"CARGO_HOME": str(cargo_home), "CARGO_NET_OFFLINE": "true",
                   "CARGO_TARGET_DIR": str(work / "target"),
                   "CARGO_PROFILE_TEST_DEBUG": "0", "CARGO_PROFILE_DEV_DEBUG": "0"}
        source_log = run(source_test, [
            "cargo", "test", "--offline", "--locked", "--manifest-path", "sdk/Cargo.toml", test_filter,
        ], bundle, offline)
        names = sorted(name for name in re.findall(r"^test (\S+) \.\.\. ok$", source_log, re.M)
                       if f"::{test_filter}_" in name)
        assert len(names) == (4 if args.periodic_healing_received or args.instant_self_heal or args.ally_heal or args.ally_hot or args.ally_shield or args.ally_overheal_shield or args.ally_hot_overheal_shield or args.npc_silence or args.control_immunity or args.control_duration_reduction or args.first_control_immunity or args.item_cleanse or args.npc_root or args.npc_stun or args.npc_movement_slow or args.movement_haste or args.cooldown_reduction or args.armor_penetration or args.critical_chance or args.critical_damage_bonus or args.dodge_penalty or args.dodge_chance or args.accuracy or args.npc_hit or args.damage_immunity or args.unavoidable or args.member_unavoidable or args.ignore_armor or args.skill_power else 3)
        run("offline-rust-consumer", ["cargo", "build", "--offline", "--locked",
                                    "--manifest-path", "consumer/Cargo.toml"], bundle, offline)

        external = work / "external"
        external.mkdir()
        for profile in [feature, legacy_feature]:
            shutil.copytree(bundle / "samples" / profile, external / profile)
        hidden = []
        for path in [bundle / "sdk/config", bundle / "sdk/examples", bundle / "samples"]:
            destination = work / ("hidden-" + path.name)
            path.rename(destination)
            hidden.append((path, destination))
        runtime_env = {"MATCH_CONFIG_MANIFEST": str(external / "absent-default.json")}
        support = json.loads(run("ordinary-info", [str(binary), "--sdk-info"], external, runtime_env))
        assert support["crateVersion"] == version and support["mechanismVersions"]["max"] == mechanism
        assert support[capability]["minimumMechanismVersion"] == mechanism
        assert not support["clocks"]["replay"]
        run(public_test, [
            "node", str(ROOT / "scripts" / public_script), str(binary),
            str(bundle / "hosts/node/two-member-host.mjs"), str(external / feature), str(out / "public"),
            *(flags if args.accuracy or args.npc_hit or args.damage_immunity else
              ["--npc-source"] if args.npc_spirit_recovery_lock else
              ["--periodic"] if args.periodic_mitigation else
              [] if args.mitigation or args.periodic_healing_received or args.item_healing_received or args.recovery_orb
              or args.recovery_strength or args.spirit_recovery_lock or args.overheal_shield or args.periodic_overheal_shield or args.instant_self_heal or args.ally_heal or args.ally_hot or args.ally_shield or args.ally_overheal_shield or args.ally_hot_overheal_shield or args.npc_silence or args.control_immunity or args.control_duration_reduction or args.first_control_immunity or args.item_cleanse or args.npc_root or args.npc_stun or args.npc_movement_slow or args.movement_haste or args.cooldown_reduction or args.armor_penetration or args.critical_chance or args.critical_damage_bonus or args.dodge_penalty or args.dodge_chance
              else flags),
        ], external, runtime_env)
        run(legacy_test, [
            "node", str(ROOT / "scripts" / legacy_script), str(binary),
            str(bundle / "hosts/node/two-member-host.mjs"), str(external / legacy_feature), str(out / legacy_dir), *legacy_flags,
        ], external, runtime_env)
        legacy_report = out / legacy_dir / "report.json"
        legacy = json.loads(legacy_report.read_text())
        assert legacy["passed"] and legacy["binarySha256"] == package["binarySha256"]
        assert [row["members"] for row in legacy["cases"]] == [4, 8]
        assert [row["settled"] for row in legacy["cases"]] == [4, 8]
        run("browser", [
            "python3", str(ROOT / "scripts/verify_sdk_5_45_browser.py"),
            "--server", str(bundle / "hosts/h5/skill-hp-cost-server.mjs"), "--binary", str(binary),
            "--configs", str(external / feature), "--out", str(out / "browser"),
            "--cwd", str(external), "--port", str(args.browser_port or port), *flags,
        ], external, runtime_env)
        extra_logs = []
        if args.npc_root or args.npc_stun or args.npc_movement_slow or args.movement_haste or args.cooldown_reduction or args.armor_penetration or args.critical_chance or args.critical_damage_bonus or args.dodge_penalty or args.dodge_chance or args.accuracy or args.npc_hit or args.damage_immunity or args.unavoidable or args.member_unavoidable or args.ignore_armor or args.skill_power:
            run("legacy-browser", [
                "python3", str(ROOT / "scripts/verify_sdk_5_45_browser.py"),
                "--server", str(bundle / "hosts/h5/skill-hp-cost-server.mjs"), "--binary", str(binary),
                "--configs", str(external / legacy_feature), "--out", str(out / f"{legacy_dir}-browser"),
                "--cwd", str(external), "--port", str(args.browser_port or port),
                {"direct-ignore-armor": "--ignore-armor", "member-unavoidable": "--member-unavoidable", "member-npc-unavoidable": "--unavoidable", "member-damage-immunity": "--damage-immunity", "member-npc-hit": "--npc-hit", "member-accuracy": "--accuracy", "member-dodge-chance": "--dodge-chance", "member-dodge-penalty": "--dodge-penalty", "member-critical-damage-bonus": "--critical-damage-bonus", "member-critical-chance": "--critical-chance", "member-armor-penetration": "--armor-penetration", "member-cooldown-reduction": "--cooldown-reduction", "member-movement-haste": "--movement-haste", "member-npc-movement-slow": "--npc-movement-slow",
                 "member-npc-stun": "--npc-stun", "member-npc-root": "--npc-root",
                 "member-item-cleanse": "--item-cleanse"}[legacy_feature],
            ], external, runtime_env)
            extra_logs.append("legacy-browser")
        for path, destination in hidden:
            destination.rename(path)
        run("final-integrity", ["python3", "verify_bundle.py"], bundle)

    extract_crate(package["archive"], out / "candidate", package["rootName"])
    report = {
        "passed": True, "archiveSha256": package["sha256"], "binarySha256": package["binarySha256"],
        "sourceCrateSha256": package["sourceCrateSha256"], "outsideCheckout": True, "sourceFixturesHidden": True,
        "emptyCargoHome": True, "offline": True, "offlineTests": names, "rustConsumerBuilt": True,
        legacy_dir: {"cases": legacy["cases"], "reportSha256": digest(legacy_report)},
        "logs": {name: digest(delivery / f"{name}.log") for name in [
            "integrity", source_test, "offline-rust-consumer", "ordinary-info",
            public_test, legacy_test, "browser", *extra_logs, "final-integrity",
        ]},
    }
    (delivery / "independent-verification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Packaged source, mechanism {mechanism} and mechanism {mechanism - 1} consumers passed", flush=True)


if __name__ == "__main__":
    main()
