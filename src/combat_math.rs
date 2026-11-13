//! Pure arithmetic shared by direct and periodic authoritative combat.
use crate::config::DamageKind;

pub(crate) struct DamageFactors {
    pub raw: u32,
    pub kind: DamageKind,
    pub tier_multiplier: f64,
    pub element_bps: u32,
    pub weapon_bps: u32,
    pub armor: u32,
    pub baseline_attack: u32,
    pub penetration_bps: u32,
    pub ignore_armor: bool,
    pub defense_scale: f64,
    pub defense_cap: f64,
    pub reduction_bps: u32,
    pub damage_bonus_bps: u32,
    pub damage_taken_bonus_bps: u32,
    pub critical_bps: u32,
    pub minimum: u32,
}
pub(crate) struct DefenseContext {
    pub baseline: u32,
    pub pierce: u32,
    pub ignore: bool,
    pub scale: f64,
    pub cap: f64,
    pub critical: f64,
    pub minimum: u32,
}
pub(crate) fn defense_damage(raw: f64, armor: f64, f: DefenseContext) -> u32 {
    let armor = armor.max(0.0) * (1.0 - f64::from(f.pierce) / 10000.0).max(0.0);
    let reduction = if f.ignore {
        0.0
    } else {
        (armor / (armor + (f64::from(f.baseline) * f.scale).max(1.0))).min(f.cap)
    };
    (raw * (1.0 - reduction) * f.critical)
        .floor()
        .max(f64::from(f.minimum))
        .min(f64::from(u32::MAX)) as u32
}

pub(crate) fn damage(f: DamageFactors) -> u32 {
    if f.kind == DamageKind::TrueDamage {
        return f.raw;
    }
    let raw = f64::from(f.raw) * f.tier_multiplier * f64::from(f.element_bps) / 10000.0
        * f64::from(f.weapon_bps)
        / 10000.0
        * f64::from(10000 - f.reduction_bps)
        / 10000.0;
    let raw = if f.damage_taken_bonus_bps == 0 {
        raw
    } else {
        raw * f64::from(10000 + f.damage_taken_bonus_bps) / 10000.0
    };
    let raw = if f.damage_bonus_bps == 0 {
        raw
    } else {
        raw * f64::from(10000 + f.damage_bonus_bps) / 10000.0
    };
    defense_damage(
        raw,
        f64::from(f.armor),
        DefenseContext {
            baseline: f.baseline_attack,
            pierce: f.penetration_bps,
            ignore: f.ignore_armor,
            scale: f.defense_scale,
            cap: f.defense_cap,
            critical: f64::from(f.critical_bps) / 10000.0,
            minimum: f.minimum,
        },
    )
}
