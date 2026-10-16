//! SKY: the shop's sky cosmetics. A colour for the nebula (bought once,
//! then picked like any look) and LIVING NEBULA, a one-off purchase that is
//! then switched on and off.
//!
//! A theme recolours the one nebula image rather than shipping a new one:
//! each pixel's brightness is mapped through the theme's three-stop gradient
//! (deep, body, glow), its transparency kept, so every theme has the same
//! clouds in its own colours. ORIGINAL leaves the image as painted.

use std::sync::Arc;

use crate::cosmetics::Price;

pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub price: Price,
    /// Deep, body, glow: dark parts of the cloud to its brightest.
    pub stops: Option<[[f32; 3]; 3]>,
    /// The card's colour.
    pub swatch: (u8, u8, u8),
    pub blurb: &'static str,
}

pub const THEMES: [Theme; 5] = [
    Theme { id: "original", name: "ORIGINAL", price: Price::Free, stops: None,
        swatch: (170, 120, 200), blurb: "THE NEBULA AS IT WAS PAINTED" },
    Theme { id: "crimson", name: "CRIMSON", price: Price::Meta(400),
        stops: Some([[38.0, 2.0, 12.0], [205.0, 34.0, 58.0], [255.0, 196.0, 170.0]]),
        swatch: (205, 40, 60), blurb: "A SKY ON FIRE" },
    Theme { id: "emerald", name: "EMERALD", price: Price::Meta(400),
        stops: Some([[0.0, 28.0, 22.0], [30.0, 186.0, 124.0], [205.0, 255.0, 225.0]]),
        swatch: (40, 190, 130), blurb: "DEEP GREEN GAS AND STARLIGHT" },
    Theme { id: "gold", name: "SOLAR GOLD", price: Price::Meta(500),
        stops: Some([[42.0, 20.0, 0.0], [232.0, 146.0, 22.0], [255.0, 242.0, 185.0]]),
        swatch: (235, 160, 40), blurb: "WARM AS A DYING SUN" },
    Theme { id: "void", name: "VOID", price: Price::Meta(650),
        stops: Some([[4.0, 0.0, 12.0], [64.0, 22.0, 132.0], [214.0, 160.0, 255.0]]),
        swatch: (110, 50, 210), blurb: "NEAR-BLACK, LIT AT THE EDGES" },
];

pub const LIVING_PRICE: Price = Price::Meta(800);
pub const LIVING_KEY: &str = "sky:living";
pub const LIVING_BLURB: &str = "THE NEBULA DRIFTS, CHURNS AND BREATHES  \u{2022}  SELECT TO TURN ON / OFF";

/// The planet below. EARTH is the original horizon photo; the rest were made
/// from it (`examples/make_planets.rs`), so they share its framing and
/// fidelity and drop into the sky in its place.
pub struct Planet {
    pub id: &'static str,
    pub name: &'static str,
    pub price: Price,
    /// The picture (None: the original photo).
    pub image: Option<&'static [u8]>,
    pub swatch: (u8, u8, u8),
    pub blurb: &'static str,
    /// Lightning and a storm's eye in its clouds (WEATHER is these worlds').
    pub weather: bool,
    /// The aurora's displays come over this world alone.
    pub aurora: bool,
}

pub const PLANETS: [Planet; 7] = [
    Planet { id: "earth", name: "EARTH", price: Price::Free, image: None, swatch: (150, 150, 170),
        blurb: "HOME", weather: false, aurora: false },
    Planet { id: "gas", name: "GAS GIANT", price: Price::Meta(500), image: Some(include_bytes!("../assets/planets/gas_giant.jpg")),
        swatch: (214, 160, 104), blurb: "ENDLESS BELTS OF CLOUD  \u{2022}  LIGHTNING AND A GREAT STORM", weather: true, aurora: false },
    Planet { id: "ice", name: "ICE WORLD", price: Price::Meta(450), image: Some(include_bytes!("../assets/planets/ice.jpg")),
        swatch: (190, 230, 250), blurb: "FROZEN SOLID  \u{2022}  AURORAS DANCE OVER IT", weather: false, aurora: true },
    Planet { id: "ocean", name: "OCEAN WORLD", price: Price::Meta(450), image: Some(include_bytes!("../assets/planets/ocean.jpg")),
        swatch: (40, 120, 210), blurb: "ONE SEA, ALL THE WAY ROUND, ALWAYS MOVING", weather: false, aurora: false },
    Planet { id: "toxic", name: "TOXIC WORLD", price: Price::Meta(550), image: Some(include_bytes!("../assets/planets/toxic.jpg")),
        swatch: (150, 210, 50), blurb: "ACID HAZE  \u{2022}  STORMS FLICKER IN IT", weather: true, aurora: false },
    Planet { id: "desert", name: "DESERT WORLD", price: Price::Meta(400), image: Some(include_bytes!("../assets/planets/desert.jpg")),
        swatch: (210, 120, 60), blurb: "RUST AND DRIFTING DUST", weather: false, aurora: false },
    Planet { id: "violet", name: "VIOLET STORM", price: Price::Meta(650), image: Some(include_bytes!("../assets/planets/violet_storm.jpg")),
        swatch: (170, 100, 240), blurb: "A WORLD UNDER A PURPLE STORM", weather: false, aurora: false },
];

/// The carousel: the themes, the LIVING card, the planets, then SKY EVENTS
/// (bought once, then switched on and off).
pub const ITEMS: usize = THEMES.len() + 1 + PLANETS.len() + 1;
pub const LIVING_IDX: usize = THEMES.len();
pub const PLANET0: usize = LIVING_IDX + 1;
pub const EVENTS_IDX: usize = PLANET0 + PLANETS.len();
pub const SWATCHES: [(u8, u8, u8); ITEMS] = [
    THEMES[0].swatch, THEMES[1].swatch, THEMES[2].swatch, THEMES[3].swatch, THEMES[4].swatch, (120, 200, 255),
    PLANETS[0].swatch, PLANETS[1].swatch, PLANETS[2].swatch, PLANETS[3].swatch,
    PLANETS[4].swatch, PLANETS[5].swatch, PLANETS[6].swatch,
    (255, 200, 140),
];
pub const NAMES: [&str; ITEMS] = [
    THEMES[0].name, THEMES[1].name, THEMES[2].name, THEMES[3].name, THEMES[4].name, "LIVING NEBULA",
    PLANETS[0].name, PLANETS[1].name, PLANETS[2].name, PLANETS[3].name,
    PLANETS[4].name, PLANETS[5].name, PLANETS[6].name,
    "SKY EVENTS",
];
pub const PLANET_VAR: &str = "sky_planet";
pub const EVENTS_VAR: &str = "sky_events";
pub const EVENTS_PRICE: Price = Price::Meta(650);

/// Cards switched on and off rather than worn.
pub fn is_toggle(idx: usize) -> bool { idx == LIVING_IDX || idx == EVENTS_IDX }

/// A planet by its id (saves keep the id, so the list can change).
pub fn planet_by_id(id: &str) -> Option<usize> { PLANETS.iter().position(|p| p.id == id) }

/// How far the sky pulls a planet towards the background's colour: the
/// original photo is grey and takes it strongly (as it always has); the new
/// worlds have colours of their own to keep.
pub fn planet_tint(i: usize) -> f32 { if i == 0 { 0.70 } else { 0.15 } }

/// The planet's picture (the original photo for EARTH).
pub fn planet_photo(i: usize) -> image::RgbaImage {
    let bytes = PLANETS[i.min(PLANETS.len() - 1)].image.unwrap_or(crate::constants::ASSET_AURORA_EARTH_GIF);
    image::load_from_memory(bytes).map(|i| i.to_rgba8())
        .unwrap_or_else(|_| image::RgbaImage::from_pixel(4, 4, image::Rgba([90, 100, 140, 255])))
}

/// The picked planet, from a canvas.
pub fn planet_of(c: &quartz::Canvas) -> usize { (c.get_i32(PLANET_VAR).max(0) as usize).min(PLANETS.len() - 1) }

/// Build the picked planet's sky and night ahead of the run.
pub fn prewarm_planet(c: &quartz::Canvas) {
    let planet = planet_of(c);
    let bg_sel = (c.get_i32("player_bg_selected").max(0) as usize).min(crate::shop::SHOP_BG_COLORS.len() - 1);
    crate::scenes::game::build_scene::prewarm_sky(planet, bg_sel);
    if planet == 0 { crate::scenes::game::planet::prewarm(bg_sel); }
    if Some(planet) == planet_by_id("ocean") { crate::scenes::game::water::prewarm(crate::shop::SHOP_BG_COLORS[bg_sel]); }
}

/// A shop picture of a planet: the lower part of its photo, the horizon.
pub fn planet_thumbnail(i: usize, w: u32) -> image::RgbaImage {
    // EARTH as the game draws it (the photo's clouds over its ocean), not the
    // bare photo, which is all cloud.
    let p = if i == 0 { (*crate::scenes::game::build_scene::earth_day_sky(0)).clone() } else { planet_photo(i) };
    let (pw, ph) = p.dimensions();
    let y0 = ph * 45 / 100;
    let crop = image::imageops::crop_imm(&p, 0, y0, pw, ph - y0).to_image();
    let h = (w as f32 * crop.height() as f32 / pw as f32).round() as u32;
    image::imageops::resize(&crop, w, h.max(1), image::imageops::FilterType::Triangle)
}

/// A planet for its shop CARD: the middle half of its horizon, taller than
/// the preview's wide strip so it fills the card's picture space.
pub fn planet_card_art(i: usize, w: u32) -> image::RgbaImage {
    let p = if i == 0 { (*crate::scenes::game::build_scene::earth_day_sky(0)).clone() } else { planet_photo(i) };
    let (pw, ph) = p.dimensions();
    let (x0, y0) = (pw / 4, ph * 52 / 100);
    let crop = image::imageops::crop_imm(&p, x0, y0, pw / 2, ph - y0).to_image();
    let h = (w as f32 * crop.height() as f32 / crop.width() as f32).round() as u32;
    image::imageops::resize(&crop, w, h.max(1), image::imageops::FilterType::Triangle)
}

/// Canvas vars the game reads (set from the profile, and by the shop).
pub const THEME_VAR: &str = "sky_theme";
pub const LIVING_VAR: &str = "sky_living";

pub fn theme_key(i: usize) -> String { format!("sky:{}", THEMES[i.min(THEMES.len() - 1)].id) }

/// What a card costs and its ownership key.
pub fn item(idx: usize) -> (String, Price) {
    if idx == EVENTS_IDX { ("sky:events".to_string(), EVENTS_PRICE) }
    else if idx >= PLANET0 {
        let p = &PLANETS[(idx - PLANET0).min(PLANETS.len() - 1)];
        (format!("planet:{}", p.id), p.price)
    } else if idx == LIVING_IDX { (LIVING_KEY.to_string(), LIVING_PRICE) } else { (theme_key(idx), THEMES[idx].price) }
}

/// The card's line under the carousel.
pub fn blurb(idx: usize) -> &'static str {
    if idx == EVENTS_IDX { "METEOR SHOWERS NOW AND THEN  \u{2022}  ON / OFF" }
    else if idx >= PLANET0 { PLANETS[(idx - PLANET0).min(PLANETS.len() - 1)].blurb }
    else if idx == LIVING_IDX { LIVING_BLURB } else { THEMES[idx].blurb }
}

/// The nebula image in `theme`'s colours (ORIGINAL returns it untouched).
pub fn recolour(src: &Arc<image::RgbaImage>, theme: usize) -> Arc<image::RgbaImage> {
    let Some(stops) = THEMES[theme.min(THEMES.len() - 1)].stops else { return src.clone() };
    let lum = |p: &image::Rgba<u8>| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
    // Stretch over the brightness the clouds actually span, so every theme
    // reaches both its deep and its glow.
    let (mut lo, mut hi) = (255.0f32, 0.0f32);
    for p in src.pixels() {
        if p[3] < 12 { continue; }
        let l = lum(p);
        lo = lo.min(l);
        hi = hi.max(l);
    }
    let span = (hi - lo).max(1.0);
    let mut out = (**src).clone();
    for p in out.pixels_mut() {
        if p[3] == 0 { continue; }
        let t = ((lum(p) - lo) / span).clamp(0.0, 1.0);
        let (a, b, k) = if t < 0.5 { (stops[0], stops[1], t * 2.0) } else { (stops[1], stops[2], (t - 0.5) * 2.0) };
        let k = k * k * (3.0 - 2.0 * k);
        for i in 0..3 { p[i] = (a[i] + (b[i] - a[i]) * k).round().clamp(0.0, 255.0) as u8; }
    }
    Arc::new(out)
}

/// The nebula image itself (the game's one set).
pub fn base_nebula() -> Option<Arc<image::RgbaImage>> {
    static IMG: std::sync::OnceLock<Option<Arc<image::RgbaImage>>> = std::sync::OnceLock::new();
    IMG.get_or_init(|| {
        let bytes = crate::scenes::game::nebula::NEBULA_SETS[0].1?;
        Some(Arc::new(image::load_from_memory(bytes).ok()?.to_rgba8()))
    }).clone()
}

/// A small picture of `theme`'s nebula on the night sky, for its card and the
/// preview pane, `w` px wide. `wide` takes the whole cloud band; otherwise
/// the densest stretch of it, near square, for a card. The clouds are laid
/// on twice: in the sky they are soft over a big area, and a thumbnail of
/// them at their own strength reads as mud.
pub fn thumbnail(theme: usize, w: u32, wide: bool) -> Option<image::RgbaImage> {
    let src = recolour(&base_nebula()?, theme);
    let (sw, sh) = src.dimensions();
    let (y0, y1) = ((sh as f32 * 0.04) as u32, (sh as f32 * 0.44) as u32);
    let (x0, x1) = if wide { (0, sw) } else { ((sw as f32 * 0.07) as u32, (sw as f32 * 0.52) as u32) };
    let band = image::imageops::crop_imm(&*src, x0, y0, x1 - x0, y1 - y0).to_image();
    let h = ((w as f32 * (y1 - y0) as f32 / (x1 - x0) as f32).round() as u32).max(1);
    let small = image::imageops::resize(&band, w, h, image::imageops::FilterType::Triangle);
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([6, 5, 18, 255]));
    // A few stars under the clouds.
    let mut seed = 0x2545_F491u32;
    for _ in 0..(w * h / 700).max(8) {
        seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
        let (x, y) = (seed % w, (seed / w) % h);
        let v = 150 + (seed % 100) as u8;
        img.put_pixel(x, y, image::Rgba([v, v, v.saturating_add(20), 255]));
    }
    image::imageops::overlay(&mut img, &small, 0, 0);
    image::imageops::overlay(&mut img, &small, 0, 0);
    Some(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_card_has_a_name_and_a_price() {
        assert_eq!(NAMES.len(), ITEMS);
        assert_eq!(item(0).1, Price::Free);
        assert_eq!(item(LIVING_IDX).0, LIVING_KEY);
    }

    #[test]
    fn every_card_is_bought_under_its_own_key() {
        let keys: Vec<String> = (0..ITEMS).map(|i| item(i).0).collect();
        for (i, k) in keys.iter().enumerate() {
            assert!(!keys[..i].contains(k), "card {i} shares the key {k}");
        }
    }

    #[test]
    fn only_living_nebula_and_sky_events_are_switches() {
        for i in 0..ITEMS {
            assert_eq!(is_toggle(i), i == LIVING_IDX || i == EVENTS_IDX, "card {i}");
        }
    }

    #[test]
    fn every_planet_is_found_by_its_id() {
        for (i, p) in PLANETS.iter().enumerate() { assert_eq!(planet_by_id(p.id), Some(i)); }
        assert_eq!(planet_by_id("lava"), None, "the removed lava world is gone");
        assert_eq!(PLANETS[0].id, "earth", "EARTH is the free default");
        assert!(PLANETS[0].image.is_none(), "EARTH is the original photo");
    }

    #[test]
    fn weather_and_auroras_belong_to_their_worlds() {
        let has = |f: fn(&Planet) -> bool| PLANETS.iter().filter(|p| f(p)).map(|p| p.id).collect::<Vec<_>>();
        assert_eq!(has(|p| p.weather), ["gas", "toxic"]);
        assert_eq!(has(|p| p.aurora), ["ice"]);
    }

    #[test]
    fn a_theme_keeps_the_clouds_shape() {
        let mut img = image::RgbaImage::new(4, 1);
        img.put_pixel(0, 0, image::Rgba([10, 10, 10, 200]));
        img.put_pixel(1, 0, image::Rgba([240, 240, 240, 90]));
        let src = Arc::new(img);
        let out = recolour(&src, 1);
        // Transparency untouched; the darkest pixel takes the deep stop.
        for (a, b) in src.pixels().zip(out.pixels()) { assert_eq!(a[3], b[3]); }
        assert_eq!(out.get_pixel(0, 0)[0], 38);
        assert!(out.get_pixel(1, 0)[0] > 240);
    }
}
