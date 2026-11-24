
use crate::dict::{self, Dictionary};
use crate::layout::{self, Lang};
use crate::morph;
use crate::ngram::{self, Context};
use crate::typo::{adjacent_keys_raw, en_affix_only, en_end_letter_only, en_known_form, first_letter_kept, match_case, sound_alike, tail_ok, Level, MAX_RANK, TAIL_RANK};

pub const EDIT_WEIGHT: f64 = 3.0;
pub const RANK_OFFSET: f64 = 100.0;
pub const CONTEXT_WEIGHT: f64 = 1.2;
pub const TAIL_BONUS: f64 = 0.8;

pub fn budget(n: usize, level: Level) -> u32 {
    match (level, n) {
        (_, 0..=2) => 0,
        (Level::Cautious, _) => 0,
        (Level::Normal, 3) => 10,
        (Level::Normal, 4) => 12,
        (Level::Normal, 5..=6) => 20,
        (Level::Normal, 7..=8) => 22,
        (Level::Normal, _) => 32,
        (Level::Aggressive, 3) => 10,
        (Level::Aggressive, 4) => 16,
        (Level::Aggressive, 5..=6) => 22,
        (Level::Aggressive, 7) => 26,
        (Level::Aggressive, _) => 34,
    }
}

pub fn margin(level: Level, capitalised: bool) -> f64 {
    let m = match level {
        Level::Aggressive => 1.1,
        _ => 1.8,
    };
    if capitalised { m + 0.8 } else { m }
}

fn short_rank_limit(n: usize) -> u32 {
    match n {
        0..=3 => 2_000,
        4 => 8_000,
        _ => MAX_RANK,
    }
}

const TRANSPOSE: u32 = 8;
pub const DROPPED_RANK: u32 = 10_000;

fn extra_or_cut_end(typed: &[char], cand: &[char]) -> bool {
    let (long, short) = if typed.len() > cand.len() { (typed, cand) } else { (cand, typed) };
    if long.len() == short.len() || !long.starts_with(short) {
        return false;
    }
    !long[short.len()..].iter().all(|&c| Some(&c) == short.last())
}

fn is_subsequence(short: &[char], long: &[char]) -> bool {
    let mut it = long.iter();
    short.iter().all(|c| it.any(|d| d == c))
}

fn unit_dist(a: &[char], b: &[char], max: usize) -> usize {
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > max {
        return max + 1;
    }
    let mut prev2 = vec![0; m + 1];
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0; m + 1];
    for i in 1..=n {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=m {
            let sub = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + sub);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] && a[i - 1] != a[i - 2] {
                v = v.min(prev2[j - 2] + 1);
            }
            cur[j] = v;
            row_min = row_min.min(v);
        }
        if row_min > max {
            return max + 1;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

pub const ONE_SLIP: u32 = 12;

struct Pairs {
    adjacent: Vec<bool>,
    sound: Vec<bool>,
}

const SLOTS: usize = 33;

fn slot(c: char, lang: Lang) -> Option<usize> {
    let base = match lang {
        Lang::Ru => 'а',
        Lang::En => 'a',
    } as u32;
    (c as u32).checked_sub(base).map(|i| i as usize).filter(|&i| i < SLOTS)
}

fn pairs(lang: Lang) -> &'static Pairs {
    static RU: std::sync::OnceLock<Pairs> = std::sync::OnceLock::new();
    static EN: std::sync::OnceLock<Pairs> = std::sync::OnceLock::new();
    let build = || {
        let base = match lang {
            Lang::Ru => 'а',
            Lang::En => 'a',
        } as u32;
        let ch = |i: usize| char::from_u32(base + i as u32).unwrap_or(' ');
        let mut p = Pairs { adjacent: vec![false; SLOTS * SLOTS], sound: vec![false; SLOTS * SLOTS] };
        for i in 0..SLOTS {
            for j in 0..SLOTS {
                p.adjacent[i * SLOTS + j] = adjacent_keys_raw(ch(i), ch(j), lang);
                p.sound[i * SLOTS + j] = sound_alike(ch(i), ch(j), lang);
            }
        }
        p
    };
    match lang {
        Lang::Ru => RU.get_or_init(build),
        Lang::En => EN.get_or_init(build),
    }
}

pub(crate) fn adjacent_fast(x: char, y: char, lang: Lang) -> bool {
    match (slot(x, lang), slot(y, lang)) {
        (Some(i), Some(j)) => pairs(lang).adjacent[i * SLOTS + j],
        _ => adjacent_keys_raw(x, y, lang),
    }
}

pub const DOUBLE_MISSING: u32 = 5;
pub const DOUBLE_EXTRA: u32 = 6;
pub const VOWEL_COST: u32 = 7;

pub(crate) fn vowel_confusion(x: char, y: char, lang: Lang) -> bool {
    const GROUPS: [(char, char); 6] = [('а', 'о'), ('е', 'и'), ('е', 'а'), ('е', 'я'), ('и', 'я'), ('а', 'я')];
    lang == Lang::Ru && GROUPS.iter().any(|&(a, b)| (a, b) == (x, y) || (b, a) == (x, y))
}

fn subst_cost(x: char, y: char, n: usize, inner: bool, lang: Lang) -> u32 {
    if inner && n > crate::typo::SHORT_SUBST_LEN && vowel_confusion(x, y, lang) {
        return VOWEL_COST;
    }
    let (adj, sound) = match (slot(x, lang), slot(y, lang)) {
        (Some(i), Some(j)) => {
            let p = pairs(lang);
            (p.adjacent[i * SLOTS + j], p.sound[i * SLOTS + j])
        }
        _ => (adjacent_keys_raw(x, y, lang), sound_alike(x, y, lang)),
    };
    if adj {
        10
    } else if sound {
        if n > crate::typo::SHORT_SUBST_LEN { 11 } else { 15 }
    } else {
        24
    }
}

fn del_cost(a: &[char], i: usize, lang: Lang) -> u32 {
    let c = a[i];
    let prev = i.checked_sub(1).map(|k| a[k]);
    let next = a.get(i + 1).copied();
    if prev == Some(c) || next == Some(c) {
        DOUBLE_EXTRA
    } else if prev.is_some_and(|p| adjacent_fast(c, p, lang)) || next.is_some_and(|p| adjacent_fast(c, p, lang)) {
        9
    } else {
        13
    }
}

fn ins_cost(b: &[char], j: usize) -> u32 {
    let c = b[j];
    if (j > 0 && b[j - 1] == c) || b.get(j + 1) == Some(&c) {
        DOUBLE_MISSING
    } else {
        10
    }
}

pub fn wdist(a: &[char], b: &[char], max: u32, lang: Lang) -> u32 {
    let dels: Vec<u32> = (0..a.len()).map(|i| del_cost(a, i, lang)).collect();
    wdist_with(a, &dels, b, max, lang, &mut Scratch::default())
}

#[derive(Default)]
pub struct Scratch {
    prev2: Vec<u32>,
    prev: Vec<u32>,
    cur: Vec<u32>,
}

fn wdist_with(a: &[char], dels: &[u32], b: &[char], max: u32, lang: Lang, s: &mut Scratch) -> u32 {
    let (n, m) = (a.len(), b.len());
    for v in [&mut s.prev2, &mut s.prev, &mut s.cur] {
        v.clear();
        v.resize(m + 1, 0);
    }
    let (mut prev2, mut prev, mut cur) = (std::mem::take(&mut s.prev2), std::mem::take(&mut s.prev), std::mem::take(&mut s.cur));
    for j in 1..=m {
        prev[j] = prev[j - 1] + ins_cost(b, j - 1);
    }
    let mut result = None;
    for i in 1..=n {
        let d = dels[i - 1];
        cur[0] = prev[0] + d;
        let mut row_min = cur[0];
        for j in 1..=m {
            let sub = if a[i - 1] == b[j - 1] { 0 } else { subst_cost(a[i - 1], b[j - 1], n, i < n && j < m, lang) };
            let mut v = (prev[j] + d).min(cur[j - 1] + ins_cost(b, j - 1)).min(prev[j - 1] + sub);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] && a[i - 1] != a[i - 2] {
                v = v.min(prev2[j - 2] + TRANSPOSE);
            }
            cur[j] = v;
            row_min = row_min.min(v);
        }
        if row_min > max {
            result = Some(max + 1);
            break;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    let r = result.unwrap_or(prev[m]);
    (s.prev2, s.prev, s.cur) = (prev2, prev, cur);
    r
}

#[derive(Debug, Clone)]
struct Scored {
    score: f64,
    word: String,
    ok: bool,
    cost: u32,
    spell: bool,
}

fn offer(best: &mut Option<Scored>, second: &mut Option<f64>, s: Scored) {
    match best {
        Some(b) if b.word == s.word => {}
        Some(b) if s.score < b.score => {
            *second = Some(second.map_or(b.score, |x| x.min(b.score)));
            *best = Some(s);
        }
        Some(_) => *second = Some(second.map_or(s.score, |x| x.min(s.score))),
        None => *best = Some(s),
    }
}

pub const CLOSER_GAP: u32 = 10;

pub const SPELL_MIN_LEN: usize = 5;
pub const SPELL_OPS: usize = 2;
pub const SPELL_RANK: u32 = MAX_RANK;

fn ending_variant(a: &str, b: &str) -> bool {
    morph::only_ending_differs(a, b)
        || ["ся", "сь"].iter().any(|r| match (a.strip_suffix(r), b.strip_suffix(r)) {
            (Some(x), Some(y)) => morph::only_ending_differs(x, y),
            _ => false,
        })
}

fn spell_tail_ok(typed: &[char], cand: &[char], lang: Lang) -> bool {
    let (n, m) = (typed.len(), cand.len());
    if n < 2 || m < 2 {
        return false;
    }
    typed[n - 2..] == cand[m - 2..]
        || (n >= 6 && typed[n - 1] == cand[m - 1] && vowel_confusion(typed[n - 2], cand[m - 2], lang))
}

fn spelling_variants(d: &Dictionary, w: &[char], lang: Lang) -> Vec<(String, bool)> {
    let vowels: &[char] = match lang {
        Lang::Ru => &['а', 'о', 'е', 'и', 'я'],
        Lang::En => &[],
    };
    let step = |v: &[char], t: &[bool]| -> Vec<(Vec<char>, Vec<bool>, bool)> {
        let mut out = Vec::new();
        for i in 1..v.len() {
            let c = v[i];
            if t[i] {
                continue;
            }
            let last = i + 1 == v.len();
            if v[i - 1] == c {
                if i >= 2 && !t[i - 1] {
                    let (mut u, mut ut) = (v.to_vec(), t.to_vec());
                    u.remove(i);
                    ut.remove(i);
                    ut[i - 1] = true;
                    out.push((u, ut, false));
                }
            } else if v.get(i + 1) != Some(&c) && !vowels.contains(&c) {
                let (mut u, mut ut) = (v.to_vec(), t.to_vec());
                u.insert(i, c);
                ut[i] = true;
                ut.insert(i, true);
                out.push((u, ut, false));
            }
            for &x in vowels {
                if !last && x != c && vowel_confusion(c, x, lang) {
                    let (mut u, mut ut) = (v.to_vec(), t.to_vec());
                    u[i] = x;
                    ut[i] = true;
                    out.push((u, ut, true));
                }
            }
        }
        out
    };
    let mut seen: std::collections::HashSet<Vec<char>> = std::collections::HashSet::new();
    let mut frontier = vec![(w.to_vec(), vec![false; w.len()])];
    seen.insert(w.to_vec());
    let mut found = Vec::new();
    for depth in 0..SPELL_OPS {
        let mut next = Vec::new();
        for (v, t) in &frontier {
            for (u, ut, vowel) in step(v, t) {
                if seen.insert(u.clone()) {
                    let s: String = u.iter().collect();
                    if d.in_full_forms(&s) && !d.is_name_form(&s) && !d.is_brand(&s) {
                        found.push((s, depth == 0 && vowel));
                    }
                    next.push((u, ut));
                }
            }
        }
        frontier = next;
    }
    found
}

pub fn suggest_wide(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool, level: Level, ctx: &Context) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    if n < 3 || !chars.iter().all(|c| c.is_alphabetic()) || layout::detect_script(word) != Some(lang) {
        return None;
    }
    let lowercase = chars.iter().all(|c| c.is_lowercase());
    let up = chars.iter().filter(|c| c.is_uppercase()).count();
    let natural = up == 0 || up == n || (up == 1 && chars[0].is_uppercase());
    if !natural || (!lowercase && n < 5) {
        return None;
    }
    let lower_s = Dictionary::normalize(word);
    if !assume_unknown {
        if d.is_word(word) {
            return None;
        }
        if lang == Lang::Ru && morph::is_known_form(d, &lower_s) {
            return None;
        }
        if lang == Lang::En && en_known_form(d, &lower_s) {
            return None;
        }
    }
    let max = budget(n, level);
    if max == 0 {
        return None;
    }
    let lower: Vec<char> = lower_s.chars().collect();
    let rank_limit = if lowercase { short_rank_limit(n) } else { TAIL_RANK };
    let first: String = lower[..1].iter().collect();
    let swapped: String = [lower[1], lower[0]].iter().collect();
    let spread = (max / 6) as usize;
    let (lo, hi) = (n.saturating_sub(spread).max(2), n + spread);
    let pool = d.starting_with_len(&first, lo, hi).chain(d.starting_with_len(&swapped, lo, hi).filter(|_| lower[0] != lower[1]));
    let mut all: Vec<Scored> = Vec::new();
    let tail2 = &lower[n - 2..];
    let odd = d.unseen_bigrams(&lower_s) >= 1;
    let dels: Vec<u32> = (0..n).map(|i| del_cost(&lower, i, lang)).collect();
    let mut scratch = Scratch::default();
    let mut cv: Vec<char> = Vec::with_capacity(n + 4);
    let has_ctx = !ctx.is_empty();
    for (cand, r) in pool {
        if r >= MAX_RANK {
            continue;
        }
        cv.clear();
        cv.extend(cand.chars());
        let m = cv.len();
        if m < 2 || (m.abs_diff(n) as u32) * 6 > max || cand == lower_s {
            continue;
        }
        if !first_letter_kept(&lower, cand) {
            continue;
        }
        let extra = crate::typo::bag_excess(cv.iter().copied(), &lower);
        let missing = (extra + n).saturating_sub(m);
        if (extra.max(missing) as u32) * DOUBLE_MISSING > max {
            continue;
        }
        let cost = wdist_with(&lower, &dels, &cv, max, lang, &mut scratch);
        if cost > max || cost == 0 {
            continue;
        }
        let ok = m >= 3
            && r < rank_limit
            && (r < TAIL_RANK || tail_ok(&lower, cand))
            && !(lang == Lang::Ru && morph::only_ending_differs(&lower_s, cand))
            && !(lang == Lang::En && (en_affix_only(&lower_s, cand) || en_end_letter_only(&lower_s, cand)))
            && (odd || (cost <= ONE_SLIP && lowercase) || (lowercase && m > n && r < DROPPED_RANK && (is_subsequence(&lower, &cv) || unit_dist(&lower, &cv, 2) <= 2)) || (lowercase && level == Level::Aggressive))
            && !extra_or_cut_end(&lower, &cv)
            && (cost <= budget(n, Level::Normal) || m.abs_diff(n) <= 1);
        if m < 3 && r >= crate::engine::SHORT_WORD_MAX_RANK {
            continue;
        }
        let mut score = cost as f64 / 10.0 * EDIT_WEIGHT + (r as f64 + RANK_OFFSET).ln();
        if cv.len() >= 2 && cv[cv.len() - 2..] == *tail2 {
            score -= TAIL_BONUS;
        }
        if has_ctx {
            score -= CONTEXT_WEIGHT * ngram::fit(ctx, cand, lang).unwrap_or(0.0);
        }
        all.push(Scored { score, word: cand.to_string(), ok, cost, spell: false });
    }
    if lowercase && n >= SPELL_MIN_LEN && lang == Lang::Ru {
        for (cand, lone_vowel) in spelling_variants(d, &lower, lang) {
            if d.rank_exact(&cand).is_some() {
                continue;
            }
            cv.clear();
            cv.extend(cand.chars());
            let cost = wdist_with(&lower, &dels, &cv, max, lang, &mut scratch);
            if cost > max || cost == 0 {
                continue;
            }
            let ok = (!lone_vowel || level == Level::Aggressive)
                && spell_tail_ok(&lower, &cv, lang)
                && !(lang == Lang::Ru && ending_variant(&lower_s, &cand))
                && !(lang == Lang::En && (en_affix_only(&lower_s, &cand) || en_end_letter_only(&lower_s, &cand)));
            let mut score = cost as f64 / 10.0 * EDIT_WEIGHT + (SPELL_RANK as f64 + RANK_OFFSET).ln();
            if cv.len() >= 2 && cv[cv.len() - 2..] == *tail2 {
                score -= TAIL_BONUS;
            }
            if has_ctx {
                score -= CONTEXT_WEIGHT * ngram::fit(ctx, &cand, lang).unwrap_or(0.0);
            }
            all.push(Scored { score, word: cand, ok, cost, spell: true });
        }
    }
    if let Some(c) = all.iter().filter(|s| s.spell && s.ok).map(|s| s.cost).min() {
        all.retain(|s| s.cost < c + CLOSER_GAP);
    }
    let mut best: Option<Scored> = None;
    let mut second: Option<f64> = None;
    for s in all {
        offer(&mut best, &mut second, s);
    }
    let best = best?;
    if !best.ok {
        return None;
    }
    if second.is_some_and(|s| s - best.score < margin(level, !lowercase)) {
        return None;
    }
    if ngram::veto(ctx, word, &best.word, lang) {
        return None;
    }
    Some(match_case(word, &best.word))
}

pub fn layout_typo_min_len(level: Level) -> usize {
    match level {
        Level::Aggressive => 3,
        _ => 4,
    }
}

pub fn layout_typo(conv: &str, dst: Lang, level: Level) -> Option<String> {
    layout_typo_ctx(conv, dst, level, &Context::none())
}

pub fn layout_typo_ctx(conv: &str, dst: Lang, level: Level, ctx: &Context) -> Option<String> {
    if level == Level::Cautious || conv.chars().count() < layout_typo_min_len(level) {
        return None;
    }
    if !conv.chars().all(char::is_alphabetic) || layout::detect_script(conv) != Some(dst) {
        return None;
    }
    let d = dict::get(dst);
    if d.is_word(conv) {
        return None;
    }
    crate::typo::as_converted(|| crate::typo::suggest_ctx(conv, dst, level, ctx))
}

pub fn looks_foreign(typed_letters: &str, src: Lang) -> bool {
    let odd = dict::get(src).unseen_bigrams(typed_letters);
    let conv = layout::convert(typed_letters, src);
    let conv_letters: String = conv.chars().filter(|c| c.is_alphabetic()).collect();
    odd >= 1 && dict::get(src.other()).unseen_bigrams(&conv_letters) < odd
}

#[doc(hidden)]
pub fn layout_typo_as_unknown(t: &str, lang: Lang) -> Option<String> {
    layout_typo_as_unknown_level(t, lang, Level::default())
}

#[doc(hidden)]
pub fn layout_typo_as_unknown_level(t: &str, lang: Lang, level: Level) -> Option<String> {
    if !looks_foreign(t, lang) {
        return None;
    }
    let conv = layout::convert(t, lang);
    layout_typo(&conv, lang.other(), level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typo::suggest;

    #[test]
    fn weighted_distance() {
        let c = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(wdist(&c("helllo"), &c("hello"), 30, Lang::En), 6);
        assert_eq!(wdist(&c("teh"), &c("the"), 30, Lang::En), 8);
        assert_eq!(wdist(&c("hrllo"), &c("hello"), 30, Lang::En), 10);
        assert!(wdist(&c("hzllo"), &c("hello"), 30, Lang::En) > 20);
    }

    #[test]
    fn short_words_when_unambiguous() {
        assert_eq!(suggest("teh", Lang::En).as_deref(), Some("the"));
        assert_eq!(suggest("adn", Lang::En).as_deref(), Some("and"));
        assert_eq!(suggest("чот", Lang::Ru).as_deref(), Some("что"));
        assert_eq!(suggest("тта", Lang::Ru), None);
        assert_eq!(suggest("Teh", Lang::En), None);
    }

    #[test]
    fn more_slips_in_longer_words() {
        assert_eq!(suggest("превт", Lang::Ru).as_deref(), Some("привет"));
        assert_eq!(suggest("првте", Lang::Ru).as_deref(), Some("привет"));
        assert_eq!(suggest("сгоднч", Lang::Ru).as_deref(), Some("сегодня"));
        assert_eq!(suggest("wierd", Lang::En).as_deref(), Some("weird"));
        assert_eq!(suggest("ввесия", Lang::Ru).as_deref(), Some("версия"));
        for (w, l) in [("превт", Lang::Ru), ("сгоднч", Lang::Ru), ("wierd", Lang::En), ("ввесия", Lang::Ru)] {
            assert_eq!(crate::typo::suggest_level(w, l, Level::Cautious), None, "{w}");
        }
    }

    #[test]
    fn spelling_errors() {
        let cases = [
            ("стелаж", "стеллаж"),
            ("стелажу", "стеллажу"),
            ("сталлежу", "стеллажу"),
            ("сталлеж", "стеллаж"),
            ("програма", "программа"),
            ("калектив", "коллектив"),
            ("адресс", "адрес"),
            ("апарат", "аппарат"),
            ("милиардер", "миллиардер"),
            ("паралельно", "параллельно"),
            ("кристалический", "кристаллический"),
        ];
        for level in [Level::Normal, Level::Aggressive] {
            for (typed, fix) in cases {
                assert_eq!(crate::typo::suggest_level(typed, Lang::Ru, level).as_deref(), Some(fix), "{typed} {level:?}");
            }
            assert_eq!(crate::typo::suggest_level("adress", Lang::En, level).as_deref(), Some("address"));
        }
        let d = dict::get(Lang::Ru);
        for (w, not) in [("азриэлю", "азриелю"), ("ввязали", "вязали"), ("бахнулись", "бахнулась"), ("бритвой", "братвой")] {
            assert_ne!(crate::typo::suggest_as_unknown_level(d, w, Lang::Ru, Level::Normal).as_deref(), Some(not), "{w}");
        }
        assert!(spelling_variants(d, &"стелаж".chars().collect::<Vec<_>>(), Lang::Ru).iter().any(|(w, lone)| w == "стеллаж" && !lone));
    }

    #[test]
    fn spelling_in_a_sentence() {
        use crate::{Engine, EngineOptions, Key};
        for level in [Level::Normal, Level::Aggressive] {
            let mut e = Engine::new(EngineOptions { fix_typos: true, typo_level: level, ..Default::default() });
            let mut out: Vec<char> = Vec::new();
            for k in "по сталлежу".chars().map(Key::Char).chain([Key::Boundary(' ')]) {
                if let Key::Char(c) = k {
                    out.push(c);
                }
                match e.feed(k, false) {
                    Some(r) => {
                        let keep = out.len().saturating_sub(r.delete);
                        out.truncate(keep);
                        out.extend(r.insert.chars());
                    }
                    None => {
                        if let Key::Boundary(c) = k {
                            out.push(c);
                        }
                    }
                }
            }
            assert_eq!(out.into_iter().collect::<String>().trim_end(), "по стеллажу", "{level:?}");
        }
    }

    #[test]
    fn names_brands_and_code_stay() {
        for w in ["Ксюха", "Серёга", "Михалыч", "Шнуров", "Xiaomi", "Huawei", "GitHub", "Kubernetes", "serde", "tokio", "eprintln", "кринж", "рофл"] {
            let l = layout::detect_script(w).unwrap();
            assert_eq!(suggest(w, l), None, "{w}");
        }
        assert_eq!(suggest("Yandx", Lang::En), None);
    }

    #[test]
    fn clipped_colloquial_words_stay() {
        for w in ["загран", "универ", "препод", "инфа", "проц", "видос", "клава", "инет", "конфа", "собес", "курсач", "училка"] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
            assert_eq!(crate::typo::suggest_level(w, Lang::Ru, Level::Aggressive), None, "{w} aggressive");
        }
        assert_eq!(suggest("програма", Lang::Ru).as_deref(), Some("программа"));
    }

    #[test]
    fn layout_and_typo() {
        assert_eq!(layout_typo("чтоо", Lang::Ru, Level::Normal).as_deref(), Some("что"));
        assert_eq!(layout_typo("чтоо", Lang::Ru, Level::Cautious), None);
        assert_eq!(layout_typo("привет", Lang::Ru, Level::Normal), None);
        assert!(looks_foreign("xnjj", Lang::En));
        assert!(!looks_foreign("bizon", Lang::En));
    }
}
