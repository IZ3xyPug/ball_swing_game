//! The leaderboard: the best runs on this computer, across every profile, per
//! mode. Kept in its own file beside the profile slots, so deleting a profile
//! does not rewrite history (and its runs keep the name they were set under).
//!
//! Each mode is ranked by what that mode is about (`GameMode::record_kind`):
//! CASUAL by distance, NORMAL by score, BOSS RUSH by bosses beaten and then
//! by the fastest time to beat them.
//!
//! The first time it is read with no file yet, each profile's existing bests
//! seed it, so a player who has been playing a while does not open it empty.

use std::cmp::Ordering;
use std::sync::{Mutex, OnceLock};

/// Rows kept per mode.
pub const KEEP: usize = 10;
/// Mode indices as `GameMode::index()`: casual 0, normal 1, boss rush 2.
pub const MODES: usize = 3;
const CASUAL: usize = 0;
const NORMAL: usize = 1;
const RUSH: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub distance: u32,
    pub score: u32,
    /// Bosses beaten in the run, and how long it took (seconds; 0 unknown).
    pub bosses: u32,
    pub secs: u32,
    pub day: i64,
}

/// One finished run, as the board needs it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Run { pub distance: u32, pub score: u32, pub bosses: u32, pub secs: u32 }

type Board = [Vec<Entry>; MODES];

fn board() -> &'static Mutex<Board> {
    static B: OnceLock<Mutex<Board>> = OnceLock::new();
    B.get_or_init(|| Mutex::new(load()))
}

fn path() -> std::path::PathBuf { crate::profile::saves_dir().join("leaderboard.txt") }

/// One line of the file: `mode|distance|day|score|bosses|secs|name`, or the
/// first version's `mode|distance|day|name`.
fn parse(line: &str) -> Option<(usize, Entry)> {
    let f: Vec<&str> = line.splitn(7, '|').collect();
    let num = |s: &str| s.parse::<u32>().ok();
    let (m, distance, day) = (f.first()?.parse::<usize>().ok()?, num(f.get(1)?)?, f.get(2)?.parse::<i64>().ok()?);
    let e = if f.len() >= 7 {
        Entry { name: f[6].to_string(), distance, score: num(f[3])?, bosses: num(f[4])?, secs: num(f[5])?, day }
    } else {
        let name = line.splitn(4, '|').nth(3)?.to_string();
        Entry { name, distance, score: 0, bosses: 0, secs: 0, day }
    };
    (m < MODES).then_some((m, e))
}

fn load() -> Board {
    let mut b: Board = Default::default();
    match std::fs::read_to_string(path()) {
        Ok(text) => {
            for line in text.lines() {
                if line.starts_with('#') { continue; }
                if let Some((m, e)) = parse(line) { b[m].push(e); }
            }
        }
        Err(_) => {
            // No file yet: seed CASUAL from what the profiles already hold
            // (the others are ranked by things profiles never kept).
            for (slot, p) in crate::profile::all_profiles().iter().enumerate() {
                if !crate::profile::slot_exists(slot) { continue; }
                let d = p.best_distance_casual;
                if d > 0 { b[CASUAL].push(Entry { name: display_name(&p.name, slot), distance: d, score: 0, bosses: 0, secs: 0, day: 0 }); }
            }
        }
    }
    // Rows that cannot be ranked by their mode's measure (written before the
    // board kept scores and bosses) drop out.
    b[NORMAL].retain(|e| e.score > 0);
    b[RUSH].retain(|e| e.bosses > 0);
    for (m, rows) in b.iter_mut().enumerate() { tidy(m, rows); }
    b
}

/// Best first, by the mode's own measure.
fn better(mode: usize, a: &Entry, b: &Entry) -> Ordering {
    match mode {
        NORMAL => b.score.cmp(&a.score).then(b.distance.cmp(&a.distance)),
        RUSH => {
            // More bosses first; then the faster run (an unknown time last).
            let t = |e: &Entry| if e.secs == 0 { u32::MAX } else { e.secs };
            b.bosses.cmp(&a.bosses).then(t(a).cmp(&t(b))).then(b.distance.cmp(&a.distance))
        }
        _ => b.distance.cmp(&a.distance),
    }
}

fn tidy(mode: usize, rows: &mut Vec<Entry>) {
    rows.sort_by(|a, b| better(mode, a, b));
    rows.truncate(KEEP);
}

fn save(b: &Board) {
    let mut out = String::from("# Ball Swing leaderboard: mode|distance|day|score|bosses|secs|name\n");
    for (m, rows) in b.iter().enumerate() {
        for e in rows {
            out.push_str(&format!("{m}|{}|{}|{}|{}|{}|{}\n", e.distance, e.day, e.score, e.bosses, e.secs, e.name));
        }
    }
    let p = path();
    if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
    let _ = std::fs::write(p, out);
}

/// A profile's name as the board shows it.
pub fn display_name(name: &str, slot: usize) -> String {
    let clean: String = name.chars().filter(|c| *c != '|' && !c.is_control()).collect();
    let clean = clean.trim();
    if clean.is_empty() { format!("PLAYER {}", slot + 1) } else { clean.to_uppercase() }
}

/// A finished run: onto the board if it makes the top `KEEP`. Returns its
/// place (1-based) if it did.
pub fn record(mode_idx: i32, name: &str, slot: usize, run: Run) -> Option<usize> {
    let m = (mode_idx.max(0) as usize).min(MODES - 1);
    // Nothing to rank by the mode's own measure.
    let nothing = match m { RUSH => run.bosses == 0, NORMAL => run.score == 0, _ => run.distance == 0 };
    if nothing { return None; }
    let mut b = board().lock().unwrap();
    let e = Entry { name: display_name(name, slot), distance: run.distance, score: run.score,
        bosses: run.bosses, secs: run.secs, day: crate::daily::today() };
    b[m].push(e.clone());
    tidy(m, &mut b[m]);
    let place = b[m].iter().position(|x| *x == e).map(|i| i + 1);
    if place.is_some() { save(&b); }
    place
}

/// The board for one mode, best first.
pub fn top(mode_idx: usize) -> Vec<Entry> {
    board().lock().unwrap()[mode_idx.min(MODES - 1)].clone()
}

/// What a mode is ranked by, for the board's column.
pub fn ranked_by(mode_idx: usize) -> &'static str {
    match mode_idx { NORMAL => "SCORE", RUSH => "BOSSES", _ => "DISTANCE" }
}

/// An entry's figure, as the board shows it for its mode.
pub fn shown(mode_idx: usize, e: &Entry, group: impl Fn(u64) -> String) -> String {
    match mode_idx {
        NORMAL if e.score > 0 => group(e.score as u64),
        NORMAL => format!("{} px", group(e.distance as u64)),
        RUSH if e.bosses > 0 && e.secs > 0 => format!("{}  \u{2022}  {}:{:02}", e.bosses, e.secs / 60, e.secs % 60),
        RUSH => format!("{} px", group(e.distance as u64)),
        _ => group(e.distance as u64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(distance: u32, score: u32, bosses: u32, secs: u32) -> Entry {
        Entry { name: "P".into(), distance, score, bosses, secs, day: 0 }
    }

    #[test]
    fn keeps_the_best_ten_best_first() {
        let mut rows: Vec<Entry> = (1..=14).map(|d| e(d * 100, 0, 0, 0)).collect();
        tidy(CASUAL, &mut rows);
        assert_eq!(rows.len(), KEEP);
        assert_eq!(rows[0].distance, 1400);
        assert_eq!(rows[KEEP - 1].distance, 500);
    }

    #[test]
    fn each_mode_ranks_by_its_own_measure() {
        // Normal: score, not distance.
        let mut n = vec![e(9000, 100, 0, 0), e(1000, 900, 0, 0)];
        tidy(NORMAL, &mut n);
        assert_eq!(n[0].score, 900);
        // Boss rush: more bosses first, then the faster time.
        let mut r = vec![e(1, 0, 3, 300), e(1, 0, 4, 900), e(1, 0, 4, 600)];
        tidy(RUSH, &mut r);
        assert_eq!((r[0].bosses, r[0].secs), (4, 600));
        assert_eq!((r[1].bosses, r[1].secs), (4, 900));
        assert_eq!(r[2].bosses, 3);
    }

    #[test]
    fn the_old_file_format_still_reads() {
        let (m, old) = parse("1|25323|20740|PLAYER 1").unwrap();
        assert_eq!((m, old.distance, old.score, old.name.as_str()), (1, 25323, 0, "PLAYER 1"));
        let (m, new) = parse("2|8000|20741|0|5|412|ALEX|B").unwrap();
        assert_eq!((m, new.bosses, new.secs, new.name.as_str()), (2, 5, 412, "ALEX|B"));
    }

    #[test]
    fn names_are_cleaned_and_never_blank() {
        assert_eq!(display_name("  ", 2), "PLAYER 3");
        assert_eq!(display_name("a|b\n", 0), "AB");
    }
}
