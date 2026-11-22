//! Typist testers: a seeded, engine-level keystroke simulation of two typist
//! personas — torpyga, gramotey, zlodey — (no GUI, no hooks, no LLM). Run with:
//!
//! ```text
//! EVAL_SEED=76 EVAL_N=5000 EVAL_OUT=/tmp/typist.md \
//!   cargo test --release -p layout_keeper_core --test typist_eval -- --ignored --nocapture
//! ```
//!
//! Env: `EVAL_SEED` (default 76), `EVAL_N` (phrases at Normal, default 5000),
//! `EVAL_N_OTHER` (Cautious/Aggressive, default 1000), `EVAL_LEVEL`
//! (`normal` / `cautious` / `aggressive` / `all`, default `all`),
//! `EVAL_PERSONA` (`torpyga` / `gramotey` / `zlodey` / `all`), `EVAL_OUT` (markdown
//! and/or JSON path). Options follow [`Config::engine_options`] (typos and
//! missing-space fixes forced on; `undo_on_backspace` as in a new config).
use layout_keeper_core::{
    layout::{self, Lang},
    typo, Config, Engine, EngineOptions, Key, Replacement,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

const PHYS_ROWS: [&str; 4] = [
    "`1234567890-=",
    "qwertyuiop[]",
    "asdfghjkl;'",
    "zxcvbnm,./",
];
const SPACE_NEIGHBOURS: &[char] = &['c', 'v', 'b', 'n', 'm', ',', '.', 'x'];

const US_SHIFT: &[(char, char)] = &[
    ('`', '~'),
    ('1', '!'),
    ('2', '@'),
    ('3', '#'),
    ('4', '$'),
    ('5', '%'),
    ('6', '^'),
    ('7', '&'),
    ('8', '*'),
    ('9', '('),
    ('0', ')'),
    ('-', '_'),
    ('=', '+'),
    ('[', '{'),
    (']', '}'),
    (';', ':'),
    ('\'', '"'),
    (',', '<'),
    ('.', '>'),
    ('/', '?'),
    ('\\', '|'),
];

// ---------------------------------------------------------------------------
// RNG (same LCG family as context_eval / correction_eval)
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self.0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
    fn chance(&mut self, p: f64) -> bool {
        if p <= 0.0 {
            return false;
        }
        if p >= 1.0 {
            return true;
        }
        (self.next(1_000_000) as f64) / 1_000_000.0 < p
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.next(v.len())]
    }
}

fn seed_of(base: u64, persona: &str, level: typo::Level) -> u64 {
    let mut s = base ^ 0x9E37_79B9_7F4A_7C15;
    for b in persona.bytes() {
        s = s.wrapping_mul(131).wrapping_add(b as u64);
    }
    s ^ match level {
        typo::Level::Cautious => 1,
        typo::Level::Normal => 2,
        typo::Level::Aggressive => 3,
    }
}

// ---------------------------------------------------------------------------
// Physical keyboard
// ---------------------------------------------------------------------------

fn unshift(us: char) -> (char, bool) {
    if us.is_ascii_uppercase() {
        return (us.to_ascii_lowercase(), true);
    }
    for &(lo, hi) in US_SHIFT {
        if us == hi {
            return (lo, true);
        }
    }
    (us, false)
}

fn with_shift(phys: char, shift: bool) -> char {
    if !shift {
        return phys;
    }
    if phys.is_ascii_lowercase() {
        return phys.to_ascii_uppercase();
    }
    for &(lo, hi) in US_SHIFT {
        if phys == lo {
            return hi;
        }
    }
    phys
}

fn produce(layout: Lang, phys: char, shift: bool) -> char {
    let us = with_shift(phys, shift);
    match layout {
        Lang::En => us,
        Lang::Ru => layout::en_to_ru(us).unwrap_or(us),
    }
}

fn locate(layout: Lang, c: char) -> Option<(char, bool)> {
    let us = layout::us_key_for(c, layout)?;
    Some(unshift(us))
}

fn neighbours(phys: char) -> Vec<char> {
    if phys == ' ' {
        return SPACE_NEIGHBOURS.to_vec();
    }
    let rows: Vec<Vec<char>> = PHYS_ROWS.iter().map(|r| r.chars().collect()).collect();
    let Some((r, p)) = rows.iter().enumerate().find_map(|(r, row)| {
        row.iter().position(|&k| k == phys).map(|p| (r as i32, p as i32))
    }) else {
        return vec![];
    };
    let mut out = vec![];
    for (dr, dc) in [(0, -1), (0, 1), (1, -1), (1, 0), (1, 1), (-1, 0), (-1, 1), (-1, -1)] {
        let (rr, cc) = (r + dr, p + dc);
        if rr >= 0 && (rr as usize) < rows.len() && cc >= 0 {
            if let Some(&k) = rows[rr as usize].get(cc as usize) {
                out.push(k);
            }
        }
    }
    out
}

fn is_url_char(c: char) -> bool {
    matches!(c, '/' | ':' | '@' | '.' | '-' | '_' | '=' | '&' | '%' | '#' | '+' | '~' | '\\')
}

fn is_boundary(c: char) -> bool {
    c.is_whitespace() || !(layout::is_word_char(c) || is_url_char(c))
}

// ---------------------------------------------------------------------------
// Profiles
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Rates {
    sub: f64,
    ins: f64,
    omit: f64,
    double: f64,
    transp: f64,
    wrong_layout: f64,
    mid_switch: f64,
    hand_shift: f64,
    miss_space: f64,
    space_nb: f64,
    stuck_shift: f64,
    caps: f64,
    spelling: f64,
    self_fix: f64,
    undo_retype: f64,
}

#[derive(Clone, Copy)]
struct Mix {
    ru: f64,
    en: f64,
    chat: f64,
}

#[derive(Clone, Copy)]
struct Profile {
    name: &'static str,
    rates: Rates,
    mix: Mix,
}

/// Fast typist: many layout misses and neighbour slips, little jargon.
const TORPYGA: Profile = Profile {
    name: "torpyga",
    rates: Rates {
        sub: 0.040,
        ins: 0.015,
        omit: 0.015,
        double: 0.012,
        transp: 0.012,
        wrong_layout: 0.10,
        mid_switch: 0.04,
        hand_shift: 0.015,
        miss_space: 0.03,
        space_nb: 0.012,
        stuck_shift: 0.008,
        caps: 0.006,
        spelling: 0.18,
        self_fix: 0.18,
        undo_retype: 0.08,
    },
    mix: Mix { ru: 0.70, en: 0.25, chat: 0.05 },
};

/// Careful typist: few slips, lots of code / terms / nicks / mixed language (FP hunt).
const GRAMOTEY: Profile = Profile {
    name: "gramotey",
    rates: Rates {
        sub: 0.006,
        ins: 0.002,
        omit: 0.002,
        double: 0.002,
        transp: 0.002,
        wrong_layout: 0.012,
        mid_switch: 0.005,
        hand_shift: 0.0,
        miss_space: 0.004,
        space_nb: 0.001,
        stuck_shift: 0.004,
        caps: 0.002,
        spelling: 0.03,
        self_fix: 0.55,
        undo_retype: 0.12,
    },
    mix: Mix { ru: 0.18, en: 0.17, chat: 0.65 },
};

/// Angry fast typist: caps, mat, dropped spaces, doubled letters, sticky shift.
const ZLODEY: Profile = Profile {
    name: "zlodey",
    rates: Rates {
        sub: 0.050,
        ins: 0.020,
        omit: 0.020,
        double: 0.040,
        transp: 0.018,
        wrong_layout: 0.08,
        mid_switch: 0.03,
        hand_shift: 0.010,
        miss_space: 0.08,
        space_nb: 0.030,
        stuck_shift: 0.040,
        caps: 0.080,
        spelling: 0.10,
        self_fix: 0.08,
        undo_retype: 0.04,
    },
    mix: Mix { ru: 0.35, en: 0.10, chat: 0.55 },
};

fn profiles() -> Vec<Profile> {
    vec![TORPYGA, GRAMOTEY, ZLODEY]
}

// ---------------------------------------------------------------------------
// Corpora
// ---------------------------------------------------------------------------

fn load_lines(path: &str) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return vec![];
    };
    text.lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(|s| {
            // Keep typist phrases short enough that thousands finish in well under a minute.
            if s.chars().count() <= 200 {
                s.to_string()
            } else {
                let mut n = 0;
                let mut cut = 0;
                for (i, c) in s.char_indices() {
                    n += 1;
                    if n >= 180 && (c.is_whitespace() || is_boundary(c)) {
                        cut = i;
                        break;
                    }
                }
                if cut == 0 { s.chars().take(180).collect() } else { s[..cut].trim().to_string() }
            }
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn load_spelling(path: &str) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return vec![];
    };
    text.lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .filter_map(|s| {
            let (a, b) = s.split_once('\t')?;
            let (a, b) = (a.trim(), b.trim());
            (!a.is_empty() && !b.is_empty() && a != b).then(|| (a.to_string(), b.to_string()))
        })
        .collect()
}

struct Corpus {
    ru: Vec<String>,
    en: Vec<String>,
    chat: Vec<String>,
    /// lower(correct) -> wrong form
    spelling_right_to_wrong: BTreeMap<String, String>,
}

impl Corpus {
    fn load() -> Self {
        let dir = std::env::var("EVAL_HELDOUT").unwrap_or_else(|_| DATA.into());
        let mut spelling_right_to_wrong = BTreeMap::new();
        for (w, r) in load_spelling(&format!("{dir}/typist_spelling.txt")) {
            spelling_right_to_wrong.entry(r.to_lowercase()).or_insert(w);
        }
        Self {
            ru: load_lines(&format!("{dir}/heldout_ru.txt")),
            en: load_lines(&format!("{dir}/heldout_en.txt")),
            chat: load_lines(&format!("{dir}/typist_chat.txt")),
            spelling_right_to_wrong,
        }
    }

    fn pick(&self, mix: Mix, rng: &mut Rng) -> &str {
        let sum = mix.ru + mix.en + mix.chat;
        let t = (rng.next(10_000) as f64) / 10_000.0 * sum;
        let (bucket, fallback) = if t < mix.ru {
            (&self.ru, &self.chat)
        } else if t < mix.ru + mix.en {
            (&self.en, &self.chat)
        } else {
            (&self.chat, &self.ru)
        };
        if bucket.is_empty() {
            fallback[rng.next(fallback.len())].as_str()
        } else {
            bucket[rng.next(bucket.len())].as_str()
        }
    }
}

fn match_case(sample: &str, word: &str) -> String {
    if sample.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()) && sample.chars().any(|c| c.is_alphabetic()) {
        return word.chars().flat_map(|c| c.to_uppercase()).collect();
    }
    let mut sc = sample.chars();
    let Some(f) = sc.next() else { return word.to_string() };
    if f.is_uppercase() && sc.all(|c| !c.is_alphabetic() || c.is_lowercase()) {
        let mut cs = word.chars();
        return cs
            .next()
            .map(|c| c.to_uppercase().chain(cs).collect())
            .unwrap_or_default();
    }
    word.to_string()
}

// ---------------------------------------------------------------------------
// Keystroke plan
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Action {
    Switch(Lang),
    /// Physical unshifted key and whether Shift is held.
    Key { phys: char, shift: bool },
    /// A character with no physical mapping (em dash, guillemets, …).
    Exact(char),
    Backspace,
}

fn split_phrase(s: &str) -> Vec<(String, String)> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < cs.len() {
        if is_boundary(cs[i]) {
            let j = (i..cs.len()).find(|&j| !is_boundary(cs[j])).unwrap_or(cs.len());
            if out.is_empty() {
                out.push((String::new(), cs[i..j].iter().collect::<String>()));
            } else {
                out.last_mut().unwrap().1.extend(cs[i..j].iter());
            }
            i = j;
        } else {
            let j = (i..cs.len()).find(|&j| is_boundary(cs[j])).unwrap_or(cs.len());
            out.push((cs[i..j].iter().collect::<String>(), String::new()));
            i = j;
        }
    }
    out
}

fn plan_keys(
    form: &str,
    intended_layout: Lang,
    typist_layout: Lang,
    mid_at: Option<usize>,
    rng: &mut Rng,
    rates: Rates,
) -> Vec<Action> {
    let mut actions = Vec::new();
    let mut typist = typist_layout;
    let chars: Vec<char> = form.chars().collect();
    let caps = rng.chance(rates.caps);
    let mut stuck = if rng.chance(rates.stuck_shift) {
        2 + rng.next(3) as u32
    } else {
        0
    };
    for (i, &ch) in chars.iter().enumerate() {
        if mid_at == Some(i) && typist != intended_layout {
            typist = intended_layout;
            actions.push(Action::Switch(typist));
        }
        match locate(intended_layout, ch) {
            Some((phys, mut shift)) => {
                if caps && ch.is_alphabetic() {
                    shift = !shift;
                }
                if stuck > 0 {
                    shift = true;
                    stuck -= 1;
                }
                actions.push(Action::Key { phys, shift });
            }
            None => actions.push(Action::Exact(ch)),
        }
    }
    actions
}


fn shift_actions(actions: &[Action], dir: i32) -> Option<Vec<Action>> {
    let mut out = Vec::with_capacity(actions.len());
    let mut keys = 0;
    for a in actions {
        match *a {
            Action::Key { phys, shift } => {
                keys += 1;
                out.push(Action::Key { phys: layout::shift_phys(phys, dir)?, shift });
            }
            ref other => out.push(other.clone()),
        }
    }
    (keys >= 4).then_some(out)
}

fn intended_layout_for(word: &str, current: Lang) -> Lang {
    layout::detect_script(word).unwrap_or(current)
}

fn perturb(mut actions: Vec<Action>, rates: Rates, rng: &mut Rng) -> Vec<Action> {
    let mut out = Vec::with_capacity(actions.len() + 4);
    let mut i = 0;
    while i < actions.len() {
        match actions[i] {
            Action::Key { phys, shift } => {
                if rng.chance(rates.omit) {
                    i += 1;
                    continue;
                }
                if rng.chance(rates.transp) {
                    if let Some(j) = (i + 1..actions.len()).find(|&j| matches!(actions[j], Action::Key { .. })) {
                        actions.swap(i, j);
                    }
                }
                let (mut p, sh) = match actions[i] {
                    Action::Key { phys, shift } => (phys, shift),
                    _ => (phys, shift),
                };
                if rng.chance(rates.sub) {
                    let nb = neighbours(p);
                    if !nb.is_empty() {
                        p = *rng.pick(&nb);
                    }
                }
                if rng.chance(rates.ins) {
                    let nb = neighbours(p);
                    if !nb.is_empty() {
                        out.push(Action::Key { phys: *rng.pick(&nb), shift: sh });
                    }
                }
                out.push(Action::Key { phys: p, shift: sh });
                if rng.chance(rates.double) {
                    out.push(Action::Key { phys: p, shift: sh });
                }
                i += 1;
            }
            ref other => {
                out.push(other.clone());
                i += 1;
            }
        }
    }
    out
}


/// Treat hyphenated words as typo targets: errors land in either letter segment,
/// not on the hyphen itself.
fn perturb_hyphen_aware(form: &str, actions: Vec<Action>, rates: Rates, rng: &mut Rng) -> Vec<Action> {
    if !form.contains('-') {
        return perturb(actions, rates, rng);
    }
    let mut segs: Vec<Vec<Action>> = vec![Vec::new()];
    for a in actions {
        let hyphen = matches!(a, Action::Key { phys: '-', .. } | Action::Exact('-'));
        if hyphen {
            segs.push(vec![a]);
            segs.push(Vec::new());
        } else {
            segs.last_mut().unwrap().push(a);
        }
    }
    let letter_idx: Vec<usize> = segs
        .iter()
        .enumerate()
        .filter(|(_, s)| s.iter().any(|a| matches!(a, Action::Key { .. })))
        .map(|(i, _)| i)
        .collect();
    let typo_at = if !letter_idx.is_empty() && rng.chance(0.65) {
        Some(*rng.pick(&letter_idx))
    } else {
        None
    };
    let mut out = Vec::new();
    for (i, seg) in segs.into_iter().enumerate() {
        if typo_at == Some(i) {
            out.extend(perturb(seg, rates, rng));
        } else {
            out.extend(seg);
        }
    }
    out
}

fn replay_chars(actions: &[Action], mut layout: Lang) -> String {
    let mut s = String::new();
    for a in actions {
        match *a {
            Action::Switch(l) => layout = l,
            Action::Key { phys, shift } => s.push(produce(layout, phys, shift)),
            Action::Exact(c) => s.push(c),
            Action::Backspace => {
                s.pop();
            }
        }
    }
    s
}

fn self_fix(actions: Vec<Action>, start: Lang, target: &str, p: f64, rng: &mut Rng) -> Vec<Action> {
    if target.is_empty() || !rng.chance(p) {
        return actions;
    }
    if replay_chars(&actions, start) == target {
        return actions;
    }
    let mut layout = start;
    let mut produced = String::new();
    let mut prefix = Vec::new();
    let want: Vec<char> = target.chars().collect();
    let mut cut = None;
    for a in &actions {
        match *a {
            Action::Switch(l) => {
                layout = l;
                prefix.push(a.clone());
            }
            Action::Key { phys, shift } => {
                let c = produce(layout, phys, shift);
                let i = produced.chars().count();
                produced.push(c);
                prefix.push(a.clone());
                if want.get(i) != Some(&c) {
                    cut = Some(i);
                    break;
                }
            }
            Action::Exact(c) => {
                let i = produced.chars().count();
                produced.push(c);
                prefix.push(a.clone());
                if want.get(i) != Some(&c) {
                    cut = Some(i);
                    break;
                }
            }
            Action::Backspace => {
                produced.pop();
                prefix.push(a.clone());
            }
        }
    }
    let Some(i) = cut else { return actions };
    prefix.push(Action::Backspace);
    // After the typo, type the rest of `target` in the intended script of `target`.
    let mut layout = layout::detect_script(target).unwrap_or(start);
    for ch in want.into_iter().skip(i) {
        if let Some((phys, shift)) = locate(layout, ch) {
            prefix.push(Action::Key { phys, shift });
        } else if let Some((phys, shift)) = locate(layout.other(), ch) {
            layout = layout.other();
            prefix.push(Action::Switch(layout));
            prefix.push(Action::Key { phys, shift });
        } else {
            prefix.push(Action::Exact(ch));
        }
    }
    prefix
}

fn apply_rep(out: &mut Vec<char>, r: Replacement) {
    let keep = out.len().saturating_sub(r.delete);
    out.truncate(keep);
    out.extend(r.insert.chars());
}

fn apply_key(e: &mut Engine, out: &mut Vec<char>, key: Key) -> Option<Replacement> {
    if let Key::Char(c) = key {
        out.push(c);
    }
    match e.feed(key, false) {
        Some(r) => {
            apply_rep(out, r.clone());
            Some(r)
        }
        None => {
            match key {
                Key::Boundary(c) => out.push(c),
                Key::Backspace => {
                    out.pop();
                }
                _ => {}
            }
            None
        }
    }
}

fn exec_actions(e: &mut Engine, out: &mut Vec<char>, actions: &[Action], mut layout: Lang) -> Lang {
    e.options.preferred_lang = Some(layout);
    for a in actions {
        match *a {
            Action::Switch(l) => {
                layout = l;
                e.options.preferred_lang = Some(layout);
            }
            Action::Key { phys, shift } => {
                let c = produce(layout, phys, shift);
                apply_key(e, out, Key::Char(c));
            }
            Action::Exact(c) => {
                apply_key(e, out, Key::Char(c));
            }
            Action::Backspace => {
                apply_key(e, out, Key::Backspace);
            }
        }
    }
    layout
}

fn plan_correct(word: &str, layout: Lang) -> Vec<Action> {
    let mut actions = Vec::new();
    for ch in word.chars() {
        if let Some((phys, shift)) = locate(layout, ch) {
            actions.push(Action::Key { phys, shift });
        } else {
            actions.push(Action::Exact(ch));
        }
    }
    actions
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Kind {
    Ok,
    Fp,
    Fixed,
    Wrong,
    Missed,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Ok => "clean & untouched",
            Kind::Fp => "clean & changed (FALSE POSITIVE)",
            Kind::Fixed => "error & fixed correctly",
            Kind::Wrong => "error & fixed wrongly",
            Kind::Missed => "error & missed",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Kind::Ok => "ok",
            Kind::Fp => "fp",
            Kind::Fixed => "fixed",
            Kind::Wrong => "wrong",
            Kind::Missed => "missed",
        }
    }
}

fn classify(intended: &str, typed: &str, output: &str) -> Kind {
    if typed == intended {
        if output == intended { Kind::Ok } else { Kind::Fp }
    } else if output == intended {
        Kind::Fixed
    } else if output == typed {
        Kind::Missed
    } else {
        Kind::Wrong
    }
}

fn strip_trail_ws(s: &str) -> &str {
    s.trim_end_matches(|c: char| c == '\n' || c == '\r' || c == '\t')
}

struct TokenHit {
    kind: Kind,
    typed: String,
    output: String,
    intended: String,
}

#[derive(Default)]
struct Score {
    counts: BTreeMap<Kind, u32>,
    words: u32,
    phrases: u32,
    /// group key -> (count, example)
    groups: BTreeMap<Kind, BTreeMap<String, (u32, String)>>,
}

impl Score {
    fn add(&mut self, t: TokenHit) {
        self.words += 1;
        *self.counts.entry(t.kind).or_insert(0) += 1;
        if matches!(t.kind, Kind::Fp | Kind::Wrong | Kind::Missed) {
            let line = format!("feedback: {} → {} (wanted {})", t.typed, t.output, t.intended);
            let key = format!("{}|{}|{}", t.typed, t.output, t.intended);
            let e = self.groups.entry(t.kind).or_default().entry(key).or_insert((0, line));
            e.0 += 1;
        }
    }
    fn get(&self, k: Kind) -> u32 {
        self.counts.get(&k).copied().unwrap_or(0)
    }
}


fn maybe_spell(word: &str, map: &BTreeMap<String, String>, p: f64, rng: &mut Rng) -> String {
    if let Some(wrong) = map.get(&word.to_lowercase()) {
        if rng.chance(p) {
            return match_case(word, wrong);
        }
    }
    word.to_string()
}

fn play_form(
    e: &mut Engine,
    out: &mut Vec<char>,
    typist: Lang,
    intended_layout: Lang,
    form: &str,
    mid_at: Option<usize>,
    rates: Rates,
    rng: &mut Rng,
    col_shift: Option<i32>,
) -> Lang {
    let plan = plan_keys(form, intended_layout, typist, mid_at, rng, rates);
    let (plan, shifted) = if let Some(dir) = col_shift {
        match shift_actions(&plan, dir) {
            Some(p) => (p, true),
            None => (plan, false),
        }
    } else {
        (plan, false)
    };
    // Keep a clean whole-hand shift so the engine can recover it; other slips
    // are applied on unshifted words as before.
    let plan = if shifted { plan } else { perturb_hyphen_aware(&form, plan, rates, rng) };
    let plan = self_fix(plan, typist, form, rates.self_fix, rng);
    exec_actions(e, out, &plan, typist)
}

fn finish_token(
    e: &mut Engine,
    out: &mut Vec<char>,
    last_sep: &str,
    is_last: bool,
    layout: Lang,
) -> bool {
    e.options.preferred_lang = Some(layout);
    if last_sep.is_empty() {
        return apply_key(e, out, if is_last { Key::Boundary('\n') } else { Key::Char(' ') }).is_some();
    }
    let mut did = false;
    for (k, ch) in last_sep.chars().enumerate() {
        let r = apply_key(e, out, Key::Char(ch));
        if k == 0 {
            did = r.is_some();
        }
    }
    did
}

fn output_span(tail: &str, last_sep: &str, _is_last: bool) -> String {
    if last_sep.is_empty() {
        return strip_trail_ws(tail).to_string();
    }
    if let Some(stripped) = tail.strip_suffix(last_sep) {
        return stripped.to_string();
    }
    strip_trail_ws(tail).to_string()
}

fn type_phrase(
    opts: &EngineOptions,
    phrase: &str,
    profile: Profile,
    spelling: &BTreeMap<String, String>,
    rng: &mut Rng,
    score: &mut Score,
) {
    let parts = split_phrase(phrase);
    if parts.iter().all(|(w, _)| w.is_empty()) {
        return;
    }
    let mut e = Engine::new(opts.clone());
    let mut out: Vec<char> = Vec::new();
    let first_word = parts
        .iter()
        .find(|(w, _)| !w.is_empty())
        .map(|(w, _)| w.as_str())
        .unwrap_or("");
    let mut layout = intended_layout_for(first_word, Lang::En);
    e.options.preferred_lang = Some(layout);
    // Once a word is hand-shifted, later words in the phrase often stay that way.
    let mut pending_shift: Option<i32> = None;

    let mut i = 0;
    while i < parts.len() {
        let (word, sep) = &parts[i];
        if word.is_empty() {
            e.options.preferred_lang = Some(layout);
            for ch in sep.chars() {
                apply_key(&mut e, &mut out, Key::Char(ch));
            }
            i += 1;
            continue;
        }

        let mut pieces: Vec<String> = vec![word.clone()];
        let mut forms: Vec<String> = vec![maybe_spell(word, spelling, profile.rates.spelling, rng)];
        let mut glue_phys: Vec<Option<char>> = Vec::new();
        let mut j = i;
        loop {
            let s = if j == i { sep.as_str() } else { parts[j].1.as_str() };
            let next = parts.get(j + 1).map(|(w, _)| w.as_str()).unwrap_or("");
            let only_space = !s.is_empty() && s.chars().all(char::is_whitespace) && s.contains(' ') && !next.is_empty();
            if !only_space {
                break;
            }
            if rng.chance(profile.rates.miss_space) {
                pieces.push(next.to_string());
                forms.push(maybe_spell(next, spelling, profile.rates.spelling, rng));
                glue_phys.push(None);
                j += 1;
                continue;
            }
            if rng.chance(profile.rates.space_nb) {
                pieces.push(next.to_string());
                forms.push(maybe_spell(next, spelling, profile.rates.spelling, rng));
                glue_phys.push(Some(*rng.pick(SPACE_NEIGHBOURS)));
                j += 1;
                continue;
            }
            break;
        }
        let last_sep = if j == i { sep.clone() } else { parts[j].1.clone() };
        let intended = pieces.join(" ");
        let is_last = j + 1 >= parts.len();

        let word_start = out.len();
        for (k, form) in forms.iter().enumerate() {
            if k > 0 {
                if let Some(phys) = glue_phys[k - 1] {
                    let c = produce(layout, phys, false);
                    apply_key(&mut e, &mut out, Key::Char(c));
                }
            }
            let piece = &pieces[k];
            let want = intended_layout_for(piece, layout);
            let mid = form.chars().count() >= 4 && rng.chance(profile.rates.mid_switch);
            let whole_wrong = !mid && rng.chance(profile.rates.wrong_layout);
            let typist = if mid || whole_wrong { want.other() } else { want };
            if typist != layout {
                layout = typist;
                e.options.preferred_lang = Some(layout);
            }
            let mid_at = if mid {
                Some(1 + rng.next(form.chars().count().saturating_sub(2).max(1)))
            } else {
                None
            };
            // Mid-word layout switches already mangle the token; don't also column-shift.
            let col_shift = if mid {
                pending_shift = None;
                None
            } else if let Some(dir) = pending_shift {
                Some(dir)
            } else if form.chars().count() >= 4 && rng.chance(profile.rates.hand_shift) {
                let dir = if rng.chance(0.5) { -1 } else { 1 };
                if rng.chance(0.5) {
                    pending_shift = Some(dir);
                }
                Some(dir)
            } else {
                None
            };
            layout = play_form(&mut e, &mut out, typist, want, form, mid_at, profile.rates, rng, col_shift);
        }

        let typed: String = out[word_start..].iter().collect();
        let did_rep = finish_token(&mut e, &mut out, &last_sep, is_last, layout);

        let mut typist_retyped = false;
        if did_rep && typed != intended && rng.chance(profile.rates.undo_retype) {
            apply_key(&mut e, &mut out, Key::Backspace);
            while out.len() > word_start {
                apply_key(&mut e, &mut out, Key::Backspace);
            }
            let want = intended_layout_for(word, layout);
            if want != layout {
                layout = want;
                e.options.preferred_lang = Some(layout);
            }
            for (k, piece) in intended.split(' ').enumerate() {
                if k > 0 {
                    apply_key(&mut e, &mut out, Key::Char(' '));
                }
                let plan = plan_correct(piece, layout);
                layout = exec_actions(&mut e, &mut out, &plan, layout);
            }
            finish_token(&mut e, &mut out, &last_sep, is_last, layout);
            typist_retyped = true;
        }

        let tail: String = out[word_start..].iter().collect();
        let output = output_span(&tail, &last_sep, is_last);
        let typed_for_score = if typist_retyped { intended.clone() } else { typed };
        score.add(TokenHit {
            kind: classify(&intended, &typed_for_score, &output),
            typed: typed_for_score,
            output,
            intended,
        });
        i = j + 1;
    }
}

fn options_for(level: typo::Level) -> EngineOptions {
    let mut opt = Config::default().engine_options();
    opt.typo_level = level;
    opt
}

fn level_name(l: typo::Level) -> &'static str {
    match l {
        typo::Level::Cautious => "cautious",
        typo::Level::Normal => "normal",
        typo::Level::Aggressive => "aggressive",
    }
}

fn parse_level(s: &str) -> Option<typo::Level> {
    match s {
        "cautious" => Some(typo::Level::Cautious),
        "normal" | "" => Some(typo::Level::Normal),
        "aggressive" => Some(typo::Level::Aggressive),
        _ => None,
    }
}

fn top_n(groups: &BTreeMap<String, (u32, String)>, n: usize) -> Vec<(u32, String)> {
    let mut v: Vec<(u32, String)> = groups.values().map(|(c, l)| (*c, l.clone())).collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    v.truncate(n);
    v
}

fn write_table(out: &mut String, s: &Score) {
    let total = s.words.max(1);
    let _ = writeln!(out, "| class | n | % |");
    let _ = writeln!(out, "|---|---:|---:|");
    for k in [Kind::Ok, Kind::Fp, Kind::Fixed, Kind::Wrong, Kind::Missed] {
        let n = s.get(k);
        let _ = writeln!(out, "| {} | {} | {:.2}% |", k.name(), n, 100.0 * n as f64 / total as f64);
    }
    let _ = writeln!(out, "| words | {} |  |", s.words);
    let _ = writeln!(out, "| phrases | {} |  |", s.phrases);
}

fn write_tops(out: &mut String, s: &Score, n: usize) {
    for (k, title) in [
        (Kind::Fp, "Top false positives"),
        (Kind::Wrong, "Top wrong fixes"),
        (Kind::Missed, "Top misses"),
    ] {
        let _ = writeln!(out, "\n### {title}\n");
        let empty = BTreeMap::new();
        let g = s.groups.get(&k).unwrap_or(&empty);
        if g.is_empty() {
            let _ = writeln!(out, "_(none)_");
            continue;
        }
        for (c, line) in top_n(g, n) {
            let _ = writeln!(out, "- {c}× `{line}`");
        }
    }
}

fn print_summary(label: &str, s: &Score) {
    println!("\n== {label}  ({} phrases, {} words) ==", s.phrases, s.words);
    let total = s.words.max(1) as f64;
    for k in [Kind::Ok, Kind::Fp, Kind::Fixed, Kind::Wrong, Kind::Missed] {
        let n = s.get(k);
        println!("  {:>32}  {:>6}  {:>6.2}%", k.name(), n, 100.0 * n as f64 / total);
    }
    for (k, title) in [
        (Kind::Fp, "false positives"),
        (Kind::Wrong, "wrong fixes"),
        (Kind::Missed, "misses"),
    ] {
        let empty = BTreeMap::new();
        let g = s.groups.get(&k).unwrap_or(&empty);
        println!("  top {title}:");
        for (c, line) in top_n(g, 15) {
            println!("    {c}× {line}");
        }
        if g.is_empty() {
            println!("    (none)");
        }
    }
}

fn score_to_json(persona: &str, level: &str, s: &Score) -> serde_json::Value {
    let total = s.words.max(1) as f64;
    let mut counts = serde_json::Map::new();
    for k in [Kind::Ok, Kind::Fp, Kind::Fixed, Kind::Wrong, Kind::Missed] {
        let n = s.get(k);
        counts.insert(
            k.key().into(),
            serde_json::json!({ "n": n, "pct": 100.0 * n as f64 / total }),
        );
    }
    let tops = |k: Kind| -> Vec<serde_json::Value> {
        let empty = BTreeMap::new();
        top_n(s.groups.get(&k).unwrap_or(&empty), if std::env::var("EVAL_ALL").is_ok() { usize::MAX } else { 30 })
            .into_iter()
            .map(|(c, line)| serde_json::json!({ "n": c, "feedback": line }))
            .collect()
    };
    serde_json::json!({
        "persona": persona,
        "level": level,
        "phrases": s.phrases,
        "words": s.words,
        "counts": counts,
        "false_positives": tops(Kind::Fp),
        "wrong_fixes": tops(Kind::Wrong),
        "misses": tops(Kind::Missed),
    })
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}
fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn run_suite(n_normal: usize, n_other: usize, seed: u64, levels: &[typo::Level], personas: &[Profile], corpus: &Corpus) -> (String, serde_json::Value) {
    let t0 = Instant::now();
    let mut md = String::new();
    let mut runs = Vec::new();
    let _ = writeln!(
        &mut md,
        "# Typist testers\n\nseed = {seed}, n(normal) = {n_normal}, n(other) = {n_other}\n"
    );
    for &profile in personas {
        for &level in levels {
            let n = if level == typo::Level::Normal { n_normal } else { n_other };
            if n == 0 {
                continue;
            }
            let mut rng = Rng(seed_of(seed, profile.name, level));
            let opts = options_for(level);
            let mut score = Score::default();
            let start = Instant::now();
            for _ in 0..n {
                let phrase = corpus.pick(profile.mix, &mut rng);
                type_phrase(&opts, phrase, profile, &corpus.spelling_right_to_wrong, &mut rng, &mut score);
                score.phrases += 1;
            }
            let dt = start.elapsed();
            let label = format!("{}/{}", profile.name, level_name(level));
            let _ = writeln!(
                &mut md,
                "## {label}\n\n{} phrases in {:.2}s ({:.0} phrases/s)\n",
                score.phrases,
                dt.as_secs_f64(),
                score.phrases as f64 / dt.as_secs_f64().max(0.001)
            );
            write_table(&mut md, &score);
            write_tops(&mut md, &score, 15);
            let _ = writeln!(&mut md);
            print_summary(&label, &score);
            println!("  time: {:.2}s ({:.0} phrases/s)", dt.as_secs_f64(), score.phrases as f64 / dt.as_secs_f64().max(0.001));
            runs.push(score_to_json(profile.name, level_name(level), &score));
        }
    }
    let _ = writeln!(&mut md, "\nTotal wall time: {:.2}s\n", t0.elapsed().as_secs_f64());
    println!("\ntotal wall time: {:.2}s", t0.elapsed().as_secs_f64());
    (md, serde_json::json!({ "seed": seed, "runs": runs }))
}

fn write_reports(path: &str, md: &str, json: &serde_json::Value) {
    let p = std::path::Path::new(path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
    let md_path;
    let json_path;
    if ext.eq_ignore_ascii_case("json") {
        json_path = p.to_path_buf();
        md_path = p.with_extension("md");
    } else if ext.eq_ignore_ascii_case("md") {
        md_path = p.to_path_buf();
        json_path = p.with_extension("json");
    } else {
        md_path = p.with_extension("md");
        json_path = p.with_extension("json");
    }
    std::fs::write(&md_path, md).unwrap();
    std::fs::write(&json_path, serde_json::to_string_pretty(json).unwrap()).unwrap();
    println!("wrote {} and {}", md_path.display(), json_path.display());
}

fn typist_main(n_normal: usize, n_other: usize) {
    let seed = env_u64("EVAL_SEED", 76);
    let n_normal = env_usize("EVAL_N", n_normal);
    let n_other = env_usize("EVAL_N_OTHER", n_other);
    let levels: Vec<typo::Level> = match std::env::var("EVAL_LEVEL").unwrap_or_else(|_| "all".into()).to_ascii_lowercase().as_str() {
        "all" => vec![typo::Level::Cautious, typo::Level::Normal, typo::Level::Aggressive],
        s => vec![parse_level(s).unwrap_or(typo::Level::Normal)],
    };
    let want = std::env::var("EVAL_PERSONA").unwrap_or_else(|_| "all".into());
    let personas: Vec<Profile> = profiles()
        .into_iter()
        .filter(|p| want == "all" || p.name == want)
        .collect();
    assert!(!personas.is_empty(), "unknown EVAL_PERSONA={want}");
    let corpus = Corpus::load();
    assert!(!corpus.ru.is_empty() && !corpus.en.is_empty(), "heldout corpora missing");
    assert!(!corpus.chat.is_empty(), "typist_chat.txt missing");
    let (md, json) = run_suite(n_normal, n_other, seed, &levels, &personas, &corpus);
    if let Ok(p) = std::env::var("EVAL_OUT") {
        write_reports(&p, &md, &json);
    }
}

#[test]
fn typist_eval_smoke() {
    // Tiny seeded run so `cargo test` compiles and exercises the harness.
    // Does not fail on false-positive counts.
    let corpus = Corpus::load();
    assert!(!corpus.chat.is_empty() && !corpus.ru.is_empty() && !corpus.en.is_empty());
    let (md, json) = run_suite(20, 0, 1, &[typo::Level::Normal], &profiles(), &corpus);
    assert!(md.contains("torpyga") && md.contains("gramotey") && md.contains("zlodey"), "{md}");
    assert!(json["runs"].as_array().map(|a| a.len()).unwrap_or(0) == 3);
}

#[test]
#[ignore]
fn typist_eval() {
    typist_main(5000, 1000);
}

#[test]
fn physical_neighbours_include_first_letter_keys() {
    let g = neighbours('g');
    assert!(g.contains(&'f') && g.contains(&'h') && g.contains(&'t') && g.contains(&'b'), "{g:?}");
    assert_eq!(produce(Lang::Ru, 'g', false), 'п');
    assert_eq!(produce(Lang::En, 'g', false), 'g');
    // и and т are neighbours on ЙЦУКЕН (physical b / n).
    assert!(neighbours('b').contains(&'n'));
    assert_eq!(produce(Lang::Ru, 'b', false), 'и');
    assert_eq!(produce(Lang::Ru, 'n', false), 'т');
}

#[test]
fn typist_hand_shift_matches_engine_examples() {
    assert_eq!(layout::shift_token("привет", Lang::Ru, Lang::Ru, 1).as_deref(), Some("ротань"));
    assert_eq!(layout::shift_token("hello", Lang::En, Lang::En, 1).as_deref(), Some("jr;;p"));
    let plan = plan_correct("привет", Lang::Ru);
    let shifted = shift_actions(&plan, 1).expect("привет can shift right");
    assert_eq!(replay_chars(&shifted, Lang::Ru), "ротань");
    let plan = plan_correct("hello", Lang::En);
    let shifted = shift_actions(&plan, 1).expect("hello can shift right");
    assert_eq!(replay_chars(&shifted, Lang::En), "jr;;p");
}

#[test]
fn classify_cases() {
    assert_eq!(classify("привет", "привет", "привет"), Kind::Ok);
    assert_eq!(classify("привет", "привет", "привета"), Kind::Fp);
    assert_eq!(classify("привет", "ghbdtn", "привет"), Kind::Fixed);
    assert_eq!(classify("привет", "ghbdtn", "приветы"), Kind::Wrong);
    assert_eq!(classify("привет", "ghbdtn", "ghbdtn"), Kind::Missed);
}
