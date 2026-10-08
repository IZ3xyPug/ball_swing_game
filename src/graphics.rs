//! Graphics quality: LOW / MEDIUM / HIGH, chosen on the settings page and
//! kept on the profile.
//!
//! It trims what costs the most on a phone, never anything the player needs
//! to read:
//!   * the aurora (a full-screen shader) is off on LOW;
//!   * boss-attack particles: about 40% on LOW, 70% on MEDIUM, all on HIGH;
//!   * screen shake is lighter on lower settings;
//!   * fewer pieces of background decor at once on LOW.
//! The default is MEDIUM on a phone and HIGH on a desktop.

use quartz::*;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Low, Quality::Medium, Quality::High];
    pub fn index(self) -> u8 { match self { Quality::Low => 0, Quality::Medium => 1, Quality::High => 2 } }
    pub fn from_index(i: i64) -> Self { match i { 0 => Quality::Low, 1 => Quality::Medium, _ => Quality::High } }
    pub fn label(self) -> &'static str { match self { Quality::Low => "LOW", Quality::Medium => "MEDIUM", Quality::High => "HIGH" } }
    /// What a fresh profile gets.
    pub fn default_for_platform() -> Self {
        if cfg!(target_os = "android") || cfg!(target_os = "ios") { Quality::Medium } else { Quality::High }
    }
}

/// Effect switches (settings): set bits are OFF.
pub const FX_SHAKE: u32 = 1;
pub const FX_HITSTOP: u32 = 2;
pub const FX_AURORA: u32 = 4;
pub const FX_ALL: [(u32, &str); 3] = [(FX_SHAKE, "SHAKE"), (FX_HITSTOP, "HITSTOP"), (FX_AURORA, "AURORA")];
static FX_OFF: AtomicU32 = AtomicU32::new(0);

/// Whether effect `bit` is on.
pub fn fx_on(bit: u32) -> bool { FX_OFF.load(Ordering::Relaxed) & bit == 0 }

/// Flip effect `bit` and keep it on the profile.
pub fn toggle_fx(bit: u32) {
    let v = FX_OFF.load(Ordering::Relaxed) ^ bit;
    FX_OFF.store(v, Ordering::Relaxed);
    apply_shake_setting();
    { let g = crate::profile::profile(); g.lock().unwrap().fx_off = v; }
    crate::profile::save_profile();
}

/// The current quality, readable from anywhere (particle pools have no canvas).
static QUALITY: AtomicU8 = AtomicU8::new(2);

pub fn quality() -> Quality { Quality::from_index(QUALITY.load(Ordering::Relaxed) as i64) }

/// Load the profile's choice (a profile without one gets the platform default).
pub fn load_from_profile() {
    let g = crate::profile::profile();
    let saved = g.lock().unwrap().graphics_quality;
    let q = if saved <= 0 { Quality::default_for_platform() } else { Quality::from_index(saved as i64 - 1) };
    QUALITY.store(q.index(), Ordering::Relaxed);
    FX_OFF.store(g.lock().unwrap().fx_off, Ordering::Relaxed);
    apply_shake_setting();
}

/// Choose a quality and keep it on the profile.
pub fn set_quality(q: Quality) {
    QUALITY.store(q.index(), Ordering::Relaxed);
    {
        let g = crate::profile::profile();
        g.lock().unwrap().graphics_quality = q.index() as i32 + 1;
    }
    crate::profile::save_profile();
}

/// Multiplier on camera shake.
/// The quality's share of shake (the engine applies the global switch).
pub fn shake_scale(_c: &Canvas) -> f32 {
    match quality() { Quality::Low => 0.5, Quality::Medium => 0.8, Quality::High => 1.0 }
}

/// Tell the engine how much the camera may shake (every shake, everywhere).
fn apply_shake_setting() {
    quartz::camera::set_shake_scale(if fx_on(FX_SHAKE) { 1.0 } else { 0.0 });
}

/// Whether the aurora is drawn at all.
pub fn aurora_enabled() -> bool { quality() != Quality::Low && fx_on(FX_AURORA) }

/// Most pieces of background decor on screen at once.
pub fn decor_max_live(full: usize) -> usize {
    match quality() { Quality::Low => (full / 2).max(1), _ => full }
}

/// Whether to keep one more effect particle: drops a steady share of them on
/// lower settings (every particle asked for is counted, so the share holds
/// whatever the effect).
pub fn keep_particle() -> bool {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    match quality() {
        Quality::High => true,
        Quality::Medium => n % 10 < 7,
        Quality::Low => n % 10 < 4,
    }
}
