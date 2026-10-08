//! Run perks: upgrades that last the rest of the run.
//!
//! One table ([`PERKS`]) feeds two places:
//!   * the purple UPGRADE NODES, which sell an extra heart, a heart refill and
//!     three perks rolled from the table when the node is touched;
//!   * the BOSS REWARD: after a victory the player picks one of three, free,
//!     before tethering out of the victory orbit.
//!
//! Several perks change how the swing plays rather than adding a number:
//! SLINGSHOT throws you further off every release, FEATHER FALL lets you hang
//! in the air, BUFF BATTERY keeps a boss-damage buff alive longer.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perk {
    ExtraHeart,
    FullHeal,
    Magnetism,
    Breathing,
    Momentum,
    LongArms,
    Slingshot,
    FeatherFall,
    BuffBattery,
    GoldenPaws,
    SecondWind,
    Sunscreen,
}

pub const PERKS: [Perk; 12] = [
    Perk::ExtraHeart, Perk::FullHeal, Perk::Magnetism, Perk::Breathing,
    Perk::Momentum, Perk::LongArms, Perk::Slingshot, Perk::FeatherFall,
    Perk::BuffBattery, Perk::GoldenPaws, Perk::SecondWind, Perk::Sunscreen,
];

impl Perk {
    pub fn name(self) -> &'static str {
        match self {
            Perk::ExtraHeart => "Extra Heart",
            Perk::FullHeal => "Full Heal",
            Perk::Magnetism => "Magnetism",
            Perk::Breathing => "Controlled Breathing",
            Perk::Momentum => "Momentum",
            Perk::LongArms => "Long Arms",
            Perk::Slingshot => "Slingshot",
            Perk::FeatherFall => "Feather Fall",
            Perk::BuffBattery => "Buff Battery",
            Perk::GoldenPaws => "Golden Paws",
            Perk::SecondWind => "Second Wind",
            Perk::Sunscreen => "Sunscreen",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Perk::ExtraHeart => "+1 heart, and +1 to your maximum.",
            Perk::FullHeal => "Refill every heart.",
            Perk::Magnetism => "Coins fly to you from further away.",
            Perk::Breathing => "Oxygen drains more slowly in space.",
            Perk::Momentum => "A higher top speed.",
            Perk::LongArms => "Grab nodes from 10% further away.",
            Perk::Slingshot => "Every release throws you 12% harder.",
            Perk::FeatherFall => "You fall 12% more slowly.",
            Perk::BuffBattery => "Boss-damage buffs last 50% longer.",
            Perk::GoldenPaws => "Every coin is worth one more.",
            Perk::SecondWind => "Saved once when your last heart would go.",
            Perk::Sunscreen => "Shrug off two flare burns.",
        }
    }
    /// The perk's colour (card edge, title).
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Perk::ExtraHeart | Perk::FullHeal | Perk::SecondWind => (255, 120, 140),
            Perk::Magnetism | Perk::GoldenPaws => (250, 214, 92),
            Perk::Breathing | Perk::Sunscreen => (120, 220, 255),
            Perk::Momentum | Perk::Slingshot => (255, 170, 90),
            Perk::LongArms | Perk::FeatherFall => (150, 240, 170),
            Perk::BuffBattery => (110, 235, 255),
        }
    }
    /// Can it do anything right now?
    fn useful(self, s: &State) -> bool {
        match self {
            Perk::FullHeal => s.hearts < s.max_hearts,
            Perk::FeatherFall => s.perk_gravity_mult > 0.72,
            Perk::LongArms => s.perm_reach_mult < 1.6,
            Perk::Slingshot => s.perk_release_mult < 1.5,
            _ => true,
        }
    }
    /// Take it.
    pub fn apply(self, s: &mut State) {
        match self {
            Perk::ExtraHeart => { s.max_hearts += 1; s.hearts += 1; s.run_heart_buys += 1; }
            Perk::FullHeal => s.hearts = s.max_hearts,
            Perk::Magnetism => s.run_magnet_buys += 1,
            Perk::Breathing => {
                s.oxygen_drain_scale = UPGRADE_BREATH_DRAIN_SCALE.min(s.oxygen_drain_scale) * if s.run_breath_buys > 0 { 0.85 } else { 1.0 };
                s.run_breath_buys += 1;
            }
            Perk::Momentum => s.run_momentum_buys += 1,
            Perk::LongArms => s.perm_reach_mult *= 1.10,
            Perk::Slingshot => s.perk_release_mult += 0.12,
            Perk::FeatherFall => s.perk_gravity_mult = (s.perk_gravity_mult * 0.88).max(0.7),
            Perk::BuffBattery => s.perk_buff_mult += 0.5,
            Perk::GoldenPaws => s.perk_coin_bonus += 1,
            Perk::SecondWind => s.free_respawns_left += 1,
            Perk::Sunscreen => s.flare_wards_left += 2,
        }
        s.perks_taken += 1;
    }
}

/// A perk's place in `PERKS` (how State stores it).
pub fn index_of(p: Perk) -> u8 {
    PERKS.iter().position(|&q| q == p).unwrap_or(0) as u8
}

/// Three different perks from `pool`, skipping ones that would do nothing.
pub fn roll_three(s: &mut State, pool: &[Perk]) -> [Perk; 3] {
    let mut picks: Vec<Perk> = Vec::new();
    let candidates: Vec<Perk> = pool.iter().copied().filter(|p| p.useful(s)).collect();
    let mut tries = 0;
    while picks.len() < 3 && tries < 64 {
        tries += 1;
        let i = (lcg(&mut s.seed) * candidates.len() as f32) as usize % candidates.len().max(1);
        if let Some(&p) = candidates.get(i) {
            if !picks.contains(&p) { picks.push(p); }
        }
    }
    while picks.len() < 3 { picks.push(Perk::ExtraHeart); }
    [picks[0], picks[1], picks[2]]
}

/// What the upgrade node rolls its three perks from (the heart and the refill
/// are always on its menu already).
pub const NODE_POOL: [Perk; 10] = [
    Perk::Magnetism, Perk::Breathing, Perk::Momentum, Perk::LongArms, Perk::Slingshot,
    Perk::FeatherFall, Perk::BuffBattery, Perk::GoldenPaws, Perk::SecondWind, Perk::Sunscreen,
];

// ── Boss reward ──────────────────────────────────────────────────────────────

pub const REWARD_PENDING_VAR: &str = "boss_reward_pending";
const CARD_W: f32 = 860.0;
const CARD_H: f32 = 560.0;
const CARD_GAP: f32 = 80.0;
const CARD_Y: f32 = 860.0;

fn card_x(i: usize) -> f32 {
    (VW - (3.0 * CARD_W + 2.0 * CARD_GAP)) * 0.5 + i as f32 * (CARD_W + CARD_GAP)
}

/// The overlay's objects, for the scene builder (hidden).
pub fn reward_objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let mut v: Vec<(String, GameObject)> = Vec::new();
    let mut add = |ctx: &mut Context, id: String, size: (f32, f32), pos: (f32, f32), layer: i32| {
        let mut o = GameObject::build(id.clone()).size(size.0, size.1).position(pos.0, pos.1).tag("hud").build(ctx);
        o.ignore_zoom = true;
        // Over a boss's darkness too: HUD, never lit by the scene.
        o.unlit = true;
        o.visible = false;
        o.layer = layer;
        v.push((id, o));
    };
    add(ctx, "reward_veil".into(), (VW * 2.0, VH * 2.0), (-VW * 0.5, -VH * 0.5), 160);
    add(ctx, "reward_title".into(), (VW, 120.0), (0.0, 540.0), 162);
    add(ctx, "reward_sub".into(), (VW, 70.0), (0.0, 690.0), 162);
    for i in 0..3 {
        let x = card_x(i);
        add(ctx, format!("reward_card_{i}"), (CARD_W, CARD_H), (x, CARD_Y), 161);
        add(ctx, format!("reward_key_{i}"), (CARD_W, 70.0), (x, CARD_Y + 50.0), 162);
        add(ctx, format!("reward_name_{i}"), (CARD_W, 90.0), (x, CARD_Y + 170.0), 162);
        add(ctx, format!("reward_desc_{i}"), (CARD_W, 140.0), (x, CARD_Y + 300.0), 162);
    }
    v
}

fn text(c: &mut Canvas, id: &str, t: &str, size: f32, col: Color, width: f32) {
    let Some(font) = super::helpers::ui_font() else { return };
    let s = c.virtual_scale();
    if let Some(o) = c.get_game_object_mut(id) {
        o.set_drawable(Box::new(crate::objects::ui_text_spec(t, &font, size * s, col, width * s)));
        o.visible = true;
    }
}

/// A boss just died: offer three perks, free. Grabbing is held off until one
/// is chosen (see `do_grab`), so the victory orbit waits for the choice.
pub fn offer_boss_reward(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let offer = {
        let mut s = st.lock().unwrap();
        let all = PERKS;
        let o = roll_three(&mut s, &all);
        s.reward_offer = o.map(index_of);
        o
    };
    c.set_var(REWARD_PENDING_VAR, true);
    c.set_var("reward_t", 0i32);
    if let Some(o) = c.get_game_object_mut("reward_veil") {
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW * 2.0, VH * 2.0), 0.0),
            image: crate::images::solid(8, 4, 20, 150).into(), color: Some(Color(255, 255, 255, 0)) });
        o.visible = true;
    }
    text(c, "reward_title", "CHOOSE YOUR REWARD", 92.0, Color(255, 230, 150, 255), VW);
    text(c, "reward_sub", "Press 1, 2 or 3, or tap a card", 36.0, Color(215, 205, 255, 230), VW);
    for (i, p) in offer.iter().enumerate() {
        let (r, g, b) = p.rgb();
        if let Some(o) = c.get_game_object_mut(&format!("reward_card_{i}")) {
            o.set_image(crate::menu_look::glass(CARD_W, CARD_H, [r, g, b], true));
            o.visible = true;
        }
        text(c, &format!("reward_key_{i}"), &format!("{}", i + 1), 54.0, Color(r, g, b, 255), CARD_W);
        text(c, &format!("reward_name_{i}"), p.name(), 64.0, Color(r, g, b, 255), CARD_W - 60.0);
        text(c, &format!("reward_desc_{i}"), p.blurb(), 36.0, Color(225, 228, 245, 240), CARD_W - 120.0);
    }
    // A boss's own prompt would read over the cards.
    if let Some(o) = c.get_game_object_mut("start_prompt_text") { o.visible = false; }
}

pub fn reward_pending(c: &Canvas) -> bool {
    matches!(c.get_var(REWARD_PENDING_VAR), Some(Value::Bool(true)))
}

/// Every frame: the veil darkens and the cards rise and fade in, one after
/// another.
pub fn tick_reward(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    if !reward_pending(c) { return; }
    let t = c.get_i32("reward_t") + 1;
    c.set_var("reward_t", t);
    if t > 40 { return; }
    let offer = st.lock().unwrap().reward_offer.map(|i| PERKS[i as usize]);
    let k = (t as f32 / 24.0).min(1.0);
    let ease = 1.0 - (1.0 - k).powi(3);
    if let Some(o) = c.get_game_object_mut("reward_veil") {
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW * 2.0, VH * 2.0), 0.0),
            image: crate::images::solid(8, 4, 20, 150).into(), color: Some(Color(255, 255, 255, (ease * 255.0) as u8)) });
    }
    for (i, p) in offer.iter().enumerate() {
        // Each card a few frames after the last.
        let ki = ((t as f32 - i as f32 * 6.0) / 22.0).clamp(0.0, 1.0);
        let ei = 1.0 - (1.0 - ki).powi(3);
        let y = CARD_Y + (1.0 - ei) * 120.0;
        let (r, g, b) = p.rgb();
        if let Some(o) = c.get_game_object_mut(&format!("reward_card_{i}")) {
            let mut img = crate::menu_look::glass(CARD_W, CARD_H, [r, g, b], true);
            img.color = Some(Color(255, 255, 255, (ei * 255.0) as u8));
            o.set_image(img);
        }
        for (id, dy) in [(format!("reward_card_{i}"), 0.0), (format!("reward_key_{i}"), 50.0),
                         (format!("reward_name_{i}"), 170.0), (format!("reward_desc_{i}"), 300.0)] {
            if let Some(o) = c.get_game_object_mut(&id) {
                o.position.1 = y + dy;
                o.visible = ei > 0.05;
            }
        }
    }
}

/// Take offer `i` (0..3).
pub fn choose(c: &mut Canvas, st: &Arc<Mutex<State>>, i: usize) -> bool {
    if !reward_pending(c) || i > 2 { return false; }
    // Not before the cards are up: a held key must not pick blind.
    if c.get_i32("reward_t") < 20 { return false; }
    let p = {
        let mut s = st.lock().unwrap();
        let p = PERKS[s.reward_offer[i] as usize];
        p.apply(&mut s);
        p
    };
    log::info!("boss reward: {}", p.name());
    crate::achievements::on_upgrade_taken();
    super::hearts::hearts_hud_update(c, st);
    c.set_var(REWARD_PENDING_VAR, false);
    hide_reward(c);
    // The victory prompt again: the way out is to tether.
    if let Some(font) = super::helpers::ui_font() {
        let s = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                &format!("{} TAKEN  \u{00b7}  TETHER TO A NODE TO CONTINUE", p.name().to_uppercase()),
                &font, 46.0 * s, Color(255, 230, 140, 255), 1700.0 * s)));
            obj.visible = true;
        }
    }
    if let Some(cam) = c.camera_mut() {
        let (r, g, b) = p.rgb();
        cam.flash_with(Color(r, g, b, 90), 0.3, FlashMode::Pulse, FlashEase::Sharp, 0.6, 0.0);
    }
    true
}

pub fn hide_reward(c: &mut Canvas) {
    let mut ids = vec!["reward_veil".to_string(), "reward_title".into(), "reward_sub".into()];
    for i in 0..3 {
        for k in ["card", "key", "name", "desc"] { ids.push(format!("reward_{k}_{i}")); }
    }
    for id in ids {
        if let Some(o) = c.get_game_object_mut(&id) { o.visible = false; }
    }
}

/// Which card a click/tap at `pos` (virtual px) landed on.
pub fn card_at(pos: (f32, f32)) -> Option<usize> {
    (0..3).find(|&i| {
        let x = card_x(i);
        pos.0 >= x && pos.0 <= x + CARD_W && pos.1 >= CARD_Y && pos.1 <= CARD_Y + CARD_H
    })
}

/// A key press while the reward is up; true if used.
pub fn reward_key(c: &mut Canvas, st: &Arc<Mutex<State>>, key: &Key) -> bool {
    if !reward_pending(c) { return false; }
    let i = match key {
        Key::Character(ch) if ch == "1" => 0,
        Key::Character(ch) if ch == "2" => 1,
        Key::Character(ch) if ch == "3" => 2,
        _ => return true, // swallow other keys while choosing
    };
    choose(c, st, i);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_perk_has_words_and_a_colour() {
        for p in PERKS {
            assert!(!p.name().is_empty() && !p.blurb().is_empty());
            let _ = p.rgb();
        }
    }
}
