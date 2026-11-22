//! Offline typo fixing: one edit (extra, missing, wrong or two swapped neighbour letters)
//! away from a frequent dictionary word of the same language.
//!
//! Conservative on purpose: words of 4+ letters only, natural casing only, and only when
//! one candidate clearly wins (the only one, or far more frequent than the runner-up).
//! Fix targets come from the whole 40 000-word list; the rarer half wins only when the
//! slip keeps the last two letters.
//! Two edits are tried only for long words (8+ letters) that have no one-edit candidate.
//!
//! Since 0.3.72 these strict rules are the [`Level::Cautious`] behaviour. At the default
//! [`Level::Normal`] (and [`Level::Aggressive`]) a word they leave alone goes on to the
//! wider, scored search in [`crate::typo_wide`]: 3-letter words, two slips from 5
//! letters, three from 9, ranked by keyboard-weighted edit cost and word frequency,
//! with an ambiguity guard.
//!
//! Since 0.3.82 a 3-letter token that is not on the frequency list (and is not a word
//! in the other layout) is also corrected when exactly one transposition or rotation
//! of its letters is a very frequent word (rank < [`SHORT_PERM_RANK`]): `ядл` → `для`,
//! `ент` → `нет`, `teh` → `the`. Ambiguous anagrams (`про`/`пор`, `was`/`saw`) stay.

use crate::dict::{self, Dictionary};
use crate::layout::Lang;
use crate::morph;
use crate::ngram::{self, Context};
use std::collections::HashSet;
use std::sync::OnceLock;

/// How hard the typo fixer tries (settings: «Осторожно / Обычно / Смело»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// The strict 0.3.71 rules only.
    Cautious,
    /// Strict rules, then the scored wider search.
    #[default]
    Normal,
    /// Wider budgets and a smaller ambiguity margin.
    Aggressive,
}

/// Minimum letters in the typed word (strict rules).
pub const MIN_LEN: usize = 4;
/// Minimum letters for a two-edit fix.
pub const MIN_LEN_2: usize = 8;
/// Candidates rarer than this are ignored. Since 0.3.67 this is the whole 40 000-word
/// list: up to 0.3.66 it was 20 000 (the old list size), so every word ranked
/// 20 000–40 000 (`ярлык`, `minute`, `serve`) could never be a fix and `ярдык` stayed.
pub const MAX_RANK: u32 = 40_000;
/// Candidates ranked from here on (the rarer half of the list, where names live) count
/// only when the slip keeps the typed word's last two letters ([`tail_ok`]).
pub const TAIL_RANK: u32 = 20_000;
/// The winner must be this many times more frequent than the runner-up.
pub const DOMINANCE: u32 = 20;
/// 3-letter transposition / rotation targets must be this frequent (`для` is 37, `нет` is 16).
pub const SHORT_PERM_RANK: u32 = 300;
/// Length of [`short_perm_fix`]: only three-letter tokens.
pub const SHORT_PERM_LEN: usize = 3;

const EN_ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";
const RU_ALPHABET: &str = "абвгдежзийклмнопрстуфхцчшщъыьэюя";

static RU_TECH_STEMS: &str = include_str!("../data/ru_tech_stems.txt");

/// `Titlecase`: first letter upper, the rest lower, at least two letters.
pub(crate) fn is_title_case(word: &str) -> bool {
    let mut it = word.chars().filter(|c| c.is_alphabetic());
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => a.is_uppercase() && b.is_lowercase() && it.all(|c| !c.is_uppercase()),
        _ => false,
    }
}

fn is_mid_sentence(ctx: &Context) -> bool {
    ctx.prev.as_deref().is_some_and(|p| p != ngram::START)
}

/// Identifier shape: snake_case, digits in the token, or internal caps (`winRes`).
/// Plain `Titlecase` or `ALLCAPS` is not this.
pub(crate) fn looks_like_identifier(word: &str) -> bool {
    if word.contains('_') {
        return true;
    }
    let has_letter = word.chars().any(|c| c.is_alphabetic());
    let has_digit = word.chars().any(|c| c.is_ascii_digit());
    if has_letter && has_digit {
        return true;
    }
    let mut chars = word.chars().filter(|c| c.is_alphabetic());
    let Some(first) = chars.next() else { return false };
    let rest: Vec<char> = chars.collect();
    if rest.is_empty() {
        return false;
    }
    let any_upper = rest.iter().any(|c| c.is_uppercase());
    let any_lower = first.is_lowercase() || rest.iter().any(|c| c.is_lowercase());
    any_upper && any_lower && !is_title_case(word)
}

/// A Russian token that is a known IT-slang stem plus a regular verb/noun ending
/// (`парсит`, `логируй`, `пушь`, `токенов`, `разрулю`).
pub(crate) fn is_ruslish_form(word: &str) -> bool {
    let lower = Dictionary::normalize(word);
    if lower.chars().count() < 3 || !lower.chars().all(|c| ('а'..='я').contains(&c)) {
        return false;
    }
    stems().iter().any(|stem| {
        lower.strip_prefix(stem.as_str()).is_some_and(is_ru_ending)
    })
}

fn stems() -> &'static [String] {
    static STEMS: OnceLock<Vec<String>> = OnceLock::new();
    STEMS.get_or_init(|| {
        let mut v: Vec<String> = RU_TECH_STEMS
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(Dictionary::normalize)
            .filter(|s| s.chars().count() >= 3)
            .collect();
        v.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
        v.dedup();
        v
    })
}

fn is_ru_ending(rest: &str) -> bool {
    static ENDINGS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    ENDINGS
        .get_or_init(|| {
            [
                "", "ь", "й",
                "а", "я", "ы", "и", "у", "ю", "е", "о",
                "ой", "ою", "ею", "ей", "ом", "ем", "ём",
                "ам", "ям", "ами", "ями", "ах", "ях",
                "ов", "ев", "ёв",
                "ка", "ки", "ке", "ку", "кой", "кою", "ок", "кам", "ками", "ках",
                "ит", "ет", "ют", "ят", "ут", "ат",
                "ишь", "ешь", "им", "ем", "ите", "ете",
                "ил", "ила", "или", "ило",
                "ал", "ала", "али", "ало",
                "ул", "ула", "ули", "уло",
                "нул", "нула", "нули", "нуло", "ни", "ну",
                "уй", "юй", "ай", "яй", "ий",
                "ить", "ать", "ять", "еть", "нуть",
                "ан", "ана", "ано", "аны",
                "ен", "ена", "ено", "ены",
                "ирует", "ируют", "ируй", "ировать", "ирую", "ируешь", "ируем", "ируете", "ируешь",
                "ировал", "ировала", "ировали", "ирует",
            ]
            .into_iter()
            .collect()
        })
        .contains(rest)
}


/// All strings one edit away from `w` (lowercase).
fn edits1(w: &[char], alphabet: &[char]) -> Vec<String> {
    let n = w.len();
    let mut out = Vec::with_capacity(n * (2 * alphabet.len() + 2) + alphabet.len());
    let s = |v: Vec<char>| v.into_iter().collect::<String>();
    for i in 0..n {
        // deletion
        let mut v = w.to_vec();
        v.remove(i);
        out.push(s(v));
        // transposition
        if i + 1 < n && w[i] != w[i + 1] {
            let mut v = w.to_vec();
            v.swap(i, i + 1);
            out.push(s(v));
        }
        // substitution
        for &c in alphabet {
            if c != w[i] {
                let mut v = w.to_vec();
                v[i] = c;
                out.push(s(v));
            }
        }
    }
    // insertion
    for i in 0..=n {
        for &c in alphabet {
            let mut v = w.to_vec();
            v.insert(i, c);
            out.push(s(v));
        }
    }
    out
}

/// Re-apply the typed word's casing (lower / UPPER / Capitalised) to the fix.
pub(crate) fn match_case(typed: &str, fix: &str) -> String {
    let letters: Vec<char> = typed.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        return fix.to_uppercase();
    }
    if letters.first().is_some_and(|c| c.is_uppercase()) {
        let mut cs = fix.chars();
        return match cs.next() {
            Some(f) => f.to_uppercase().chain(cs).collect(),
            None => String::new(),
        };
    }
    fix.to_string()
}

/// Best single fix for `word` (letters only) in `lang`, or `None` when unsure.
pub fn suggest(word: &str, lang: Lang) -> Option<String> {
    suggest_level(word, lang, Level::default())
}

/// [`suggest`] at a given [`Level`].
pub fn suggest_level(word: &str, lang: Lang, level: Level) -> Option<String> {
    suggest_impl(dict::get(lang), word, lang, false, level)
}

/// [`suggest_level`] with the words typed before (see [`crate::ngram`]): context picks
/// between close candidates of the scored search and vetoes a fix that does not fit.
pub fn suggest_ctx(word: &str, lang: Lang, level: Level, ctx: &Context) -> Option<String> {
    suggest_impl_ctx(dict::get(lang), word, lang, false, level, ctx)
}

pub fn suggest_in(d: &Dictionary, word: &str, lang: Lang) -> Option<String> {
    suggest_impl(d, word, lang, false, Level::default())
}

/// Audit helper: like [`suggest_in`], but treats `word` as unknown even when it is a real
/// word (as if it were a name or slang missing from every dictionary). Never returns the
/// word itself. Used to measure how often unknown words would be changed.
#[doc(hidden)]
pub fn suggest_as_unknown(d: &Dictionary, word: &str, lang: Lang) -> Option<String> {
    suggest_as_unknown_level(d, word, lang, Level::default())
}

#[doc(hidden)]
pub fn suggest_as_unknown_level(d: &Dictionary, word: &str, lang: Lang, level: Level) -> Option<String> {
    suggest_as_unknown_ctx(d, word, lang, level, &Context::none())
}

#[doc(hidden)]
pub fn suggest_as_unknown_ctx(d: &Dictionary, word: &str, lang: Lang, level: Level, ctx: &Context) -> Option<String> {
    suggest_impl_ctx(d, word, lang, true, level, ctx).filter(|f| Dictionary::normalize(f) != Dictionary::normalize(word))
}

fn suggest_impl(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool, level: Level) -> Option<String> {
    suggest_impl_ctx(d, word, lang, assume_unknown, level, &Context::none())
}

fn suggest_impl_ctx(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool, level: Level, ctx: &Context) -> Option<String> {
    let fix = suggest_core(d, word, lang, assume_unknown, level, ctx)?;
    // A new word of regular formation (`хранилка`, `перезалить`) is not a typo of another
    // lemma: a fix must keep its prefix or suffix.
    if lang == Lang::Ru && !morph::keeps_derivation(d, &Dictionary::normalize(word), &Dictionary::normalize(&fix)) {
        return None;
    }
    Some(fix)
}

fn suggest_core(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool, level: Level, ctx: &Context) -> Option<String> {
    if looks_like_identifier(word) {
        return None;
    }
    if lang == Lang::Ru && is_ruslish_form(word) {
        return None;
    }
    if is_title_case(word) && is_mid_sentence(ctx) {
        return None;
    }
    // 3-letter transposition / rotation of a very frequent word (`ядл` → `для`,
    // `ент` → `нет`). Not at Cautious. Tried before the scored search, which only
    // looks at words that keep the first letter (or swap the first two), and
    // which skips tokens that happen to sit in the unranked full-form set (`ент`).
    if level != Level::Cautious {
        if let Some(fix) = short_perm_fix(d, word, lang) {
            if !ngram::veto(ctx, word, &fix, lang) {
                return Some(fix);
            }
        }
    }
    if let Some(fix) = dedupe_fix(d, word, lang, assume_unknown) {
        if !ngram::veto(ctx, word, &fix, lang) {
            return Some(fix);
        }
    }
    let strict = suggest_one(d, word, lang, assume_unknown)
        .or_else(|| hush_soft_sign(d, word, lang, assume_unknown))
        .or_else(|| verb_soft_sign(d, word, lang, assume_unknown));
    // A strict fix the context rejects falls through to the scored search, which weighs
    // the context itself (and may pick another candidate or nothing).
    let vetoed = strict.as_deref().is_some_and(|f| ngram::veto(ctx, word, f, lang));
    if (strict.is_some() && !vetoed) || level == Level::Cautious {
        return strict.filter(|_| !vetoed);
    }
    // Whole-token home-row misplacement (`ротань` → `привет`): tried before the
    // scored search, which cannot reach a 5–6 letter column shift.
    if let Some((fix, dest)) = hand_shift_fix(word, lang, level, None) {
        if dest == lang && !ngram::veto(ctx, word, &fix, lang) {
            return Some(fix);
        }
    }
    let wide = crate::typo_wide::suggest_wide(d, word, lang, assume_unknown, level, ctx);
    if wide.is_some() {
        return wide;
    }
    // After the first-letter-keeping search: an extra neighbour key at the start of a
    // short unknown word (`лдва` → `два`). Cautious stays on the strict rules only.
    leading_neighbour_insert(d, word, lang, assume_unknown)
        .filter(|f| !ngram::veto(ctx, word, f, lang))
        .or_else(|| channel_fix(d, word, lang, assume_unknown).filter(|f| !ngram::veto(ctx, word, f, lang)))
}

pub fn dedupe_fix(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 4 || !chars.iter().all(|c| c.is_alphabetic()) {
        return None;
    }
    let lower = Dictionary::normalize(word);
    let w: Vec<char> = lower.chars().collect();
    if w.len() != chars.len() {
        return None;
    }
    let known = |x: &str| d.is_word(x) || (lang == Lang::Ru && morph::is_known_form(d, x));
    if !assume_unknown && (known(&lower) || d.is_name_form(&lower)) {
        return None;
    }
    let mut found: Option<String> = None;
    for i in 2..w.len() {
        if w[i] != w[i - 1] || w[i - 2] == w[i] {
            continue;
        }
        let mut v = w.clone();
        v.remove(i);
        let c: String = v.iter().collect();
        if c.chars().count() >= 3 && d.rank_exact(&c).is_some_and(|r| r < DEDUPE_MAX_RANK) {
            if found.as_ref().is_some_and(|f| *f != c) {
                return None;
            }
            found = Some(c);
        }
    }
    let c = found?;
    let g = ngram::get(lang);
    let seen = g.count(&[&lower]);
    if seen > 0.0 && g.count(&[&c]) < seen * DEDUPE_SEEN_RATIO {
        return None;
    }
    Some(match_case(word, &c))
}

const DEDUPE_SEEN_RATIO: f64 = 100.0;
const DEDUPE_MAX_RANK: u32 = 20_000;

pub fn is_dedupe_of(typed: &str, fix: &str) -> bool {
    let a: Vec<char> = Dictionary::normalize(typed).chars().collect();
    let b: Vec<char> = Dictionary::normalize(fix).chars().collect();
    a.len() == b.len() + 1 && (1..a.len()).any(|i| a[i] == a[i - 1] && { let mut v = a.clone(); v.remove(i); v == b })
}

pub const CHANNEL_MIN_LEN: usize = 4;
pub const CHANNEL_MARGIN: f64 = 1.5;
pub const CHANNEL_CROWD: f64 = 10.0;
pub const NC_RIVAL_UNSEEN: f64 = 0.5;
pub const CHANNEL_MIN_COUNT: f64 = 2.0;
pub const CHANNEL_SHORT_RANK: u32 = 10_000;

const CHANNEL_W_NEAR_EXTRA: f64 = 0.2;
const CHANNEL_END_RIVAL: f64 = 1.5;
const CHANNEL_SEEN_RATIO: f64 = 100.0;
const CHANNEL_SHORT_LEN: usize = 5;
const CHANNEL_SHORT_MARGIN: f64 = 4.0;
const CHANNEL_W_SUBST: f64 = 0.25;
const CHANNEL_W_OMIT: f64 = 1.0;

fn channel_fix(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    if n < CHANNEL_MIN_LEN || !chars.iter().all(|c| c.is_alphabetic()) {
        return None;
    }
    if chars[0].is_uppercase() {
        return None;
    }
    let lower = Dictionary::normalize(word);
    if !assume_unknown && (d.is_word(&lower) || (lang == Lang::Ru && morph::is_known_form(d, &lower)) || d.is_name_form(&lower)) {
        return None;
    }
    let w: Vec<char> = lower.chars().collect();
    if w.len() != n {
        return None;
    }
    let alphabet: Vec<char> = match lang {
        Lang::En => EN_ALPHABET.chars().collect(),
        Lang::Ru => RU_ALPHABET.chars().collect(),
    };
    let g = ngram::get(lang);
    let total = g.total().max(1.0);
    let mut best: std::collections::HashMap<String, (f64, bool)> = std::collections::HashMap::new();
    let mut offer = |cand: Vec<char>, p: f64| {
        let c: String = cand.iter().collect();
        let m = cand.len();
        let eligible = match d.rank_exact(&c) {
            Some(r) => !(m < 3 || (m <= 4 && r >= CHANNEL_SHORT_RANK)),
            None => {
                if m < 4 || !d.in_full_forms(&c) {
                    return;
                }
                false
            }
        };
        let cnt = g.count(&[&c]);
        let eligible = eligible && cnt >= CHANNEL_MIN_COUNT;
        let score = cnt.max(NC_RIVAL_UNSEEN) / total * p;
        let e = best.entry(c).or_insert((0.0, false));
        if score > e.0 {
            e.0 = score;
        }
        e.1 |= eligible;
    };
    for i in 1..n {
        let mut v = w.clone();
        let extra = v.remove(i);
        let doubled = w[i - 1] == extra || (i + 1 < n && w[i + 1] == extra);
        let near = adjacent_keys(extra, w[i - 1], lang) || (i + 1 < n && adjacent_keys(extra, w[i + 1], lang));
        if doubled {
            offer(v, 1.0);
        } else if near {
            offer(v, CHANNEL_W_NEAR_EXTRA);
        }
        if i + 1 < n && w[i] != w[i + 1] {
            let mut v = w.clone();
            v.swap(i, i + 1);
            offer(v, 1.0);
        }
        for &c in &alphabet {
            if c != w[i] && adjacent_keys(c, w[i], lang) {
                let mut v = w.clone();
                v[i] = c;
                offer(v, CHANNEL_W_SUBST);
            }
        }
    }
    for i in 1..=n {
        for &c in &alphabet {
            let mut v = w.clone();
            v.insert(i, c);
            let doubled = w[i - 1] == c || (i < n && w[i] == c);
            offer(v, if doubled { 1.0 } else { CHANNEL_W_OMIT });
        }
    }
    let mut ranked: Vec<(String, f64, bool)> = best.into_iter().map(|(k, (v, e))| (k, v, e)).collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.cmp(&b.0)));
    if lang == Lang::En {
        let end_edit = |c: &str| {
            let cc: Vec<char> = c.chars().collect();
            let last_only = cc.len() == w.len() && cc[..cc.len() - 1] == w[..w.len() - 1];
            en_end_letter_only(&lower, c) || last_only
        };
        let end_best = ranked.iter().filter(|x| end_edit(&x.0)).map(|x| x.1).fold(0.0, f64::max);
        ranked.retain(|(c, _, _)| !end_edit(c));
        if ranked.first().is_some_and(|x| end_best * CHANNEL_END_RIVAL >= x.1) {
            return None;
        }
    }
    let (top, s1, ok) = ranked.first()?.clone();
    if !ok {
        return None;
    }
    let s2 = ranked.get(1).map_or(0.0, |x| x.1);
    let seen = g.count(&[&lower]);
    if seen > 0.0 && s1 * total < seen * CHANNEL_SEEN_RATIO {
        return None;
    }
    if ranked.iter().filter(|x| x.1 * CHANNEL_CROWD >= s1).count() > 2 {
        return None;
    }
    if lang == Lang::Ru {
        let tc: Vec<char> = top.chars().collect();
        let common = tc.iter().zip(&w).take_while(|(a, b)| a == b).count();
        let soft = |c: &char| "аеёиоуыэюяйь".contains(*c);
        if common + 1 >= tc.len().min(w.len()) && tc[common..].iter().all(soft) && w[common..].iter().all(soft) {
            return None;
        }
        let rival_form = ranked.iter().skip(1).any(|(c, _, _)| {
            let cc: Vec<char> = c.chars().collect();
            let common = tc.iter().zip(&cc).take_while(|(a, b)| a == b).count();
            common + 2 >= tc.len().max(cc.len())
        });
        if rival_form {
            return None;
        }
    }
    let margin: f64 = std::env::var("LK_CM").ok().and_then(|v| v.parse().ok()).unwrap_or(CHANNEL_MARGIN);
    let margin = if n <= CHANNEL_SHORT_LEN { margin * CHANNEL_SHORT_MARGIN } else { margin };
    if s1 < s2 * margin {
        return None;
    }
    Some(match_case(word, &top))
}

/// `-ить` / `-еть` written for `-ит` / `-ет` (`услышить` → `услышит`): the typed form is in
/// no list (the inflection guesser alone accepts it as an infinitive), the form without ь
/// is a verb form of 6+ letters whose infinitive is not the typed word.
fn verb_soft_sign(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool) -> Option<String> {
    let lower = Dictionary::normalize(word);
    let base = lower.strip_suffix('ь').filter(|b| lang == Lang::Ru && (b.ends_with("ит") || b.ends_with("ет")))?;
    if base.chars().count() < MIN_LEN_VERB_SOFT_SIGN || !word.chars().all(char::is_alphabetic) || word.ends_with('Ь') {
        return None;
    }
    if (!assume_unknown && d.is_word(&lower)) || !d.is_word(base) {
        return None;
    }
    // The form without ь must be a verb (its 3rd person plural exists: `услышат`, not
    // the noun `динамит`), and for `-ить` the verb's infinitive is another one
    // (`услышать`, `увидеть`; `зеркалить` is the infinitive of `зеркалит`).
    let stem = &base[..base.len() - "ит".len()];
    let known = |end: &str| d.is_word(&format!("{stem}{end}"));
    let verb = if lower.ends_with("ить") {
        (known("ят") || known("ат")) && (known("еть") || known("ать") || known("ять"))
    } else {
        known("ут") || known("ют")
    };
    if !verb {
        return None;
    }
    Some(word.chars().take(word.chars().count() - 1).collect())
}

/// Minimum letters (without ь) for [`verb_soft_sign`]: short ones are often names.
pub const MIN_LEN_VERB_SOFT_SIGN: usize = 6;

/// A soft sign after ж/ш/ч/щ at the end of an unknown word (`клавишь`, `хорош`-style
/// spelling slip) when the word without it is a known form (`клавиш`) or has a fix of its
/// own (`клапвишь` → `клапвиш` → `клавиш`). Real words with that ending (`мышь`, `ночь`,
/// `делаешь`, `режь`) are known and never get here.
fn hush_soft_sign(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool) -> Option<String> {
    let lower = Dictionary::normalize(word);
    let base = lower.strip_suffix('ь').filter(|b| lang == Lang::Ru && b.ends_with(['ж', 'ш', 'ч', 'щ']))?;
    if base.chars().count() < MIN_LEN || !word.chars().all(char::is_alphabetic) || word.ends_with('Ь') {
        return None;
    }
    if !assume_unknown && (d.is_word(&lower) || morph::is_known_form(d, &lower)) {
        return None;
    }
    let typed_base: String = word.chars().take(word.chars().count() - 1).collect();
    if d.is_word(base) || morph::is_known_form(d, base) {
        return Some(typed_base); // as typed: `гадёнышь` → `гадёныш`
    }
    // Otherwise only an extra key pressed together with its neighbour (`клапвиш`: п next
    // to а) is fixed in the rest; changed or added letters stay (`лефковичь` is a
    // surname, not `левкович`).
    let fix = suggest_one(d, &typed_base, lang, false)?;
    let (bv, fv): (Vec<char>, Vec<char>) = (base.chars().collect(), Dictionary::normalize(&fix).chars().collect());
    if fv.len() + 1 != bv.len() || !fv.last().is_some_and(|c| "жшчщ".contains(*c)) {
        return None;
    }
    let at = bv.iter().zip(&fv).take_while(|(a, b)| a == b).count();
    let mut rest = bv.clone();
    rest.remove(at);
    let extra = bv[at];
    // Any of the equal letters could be the extra one (`клаапвиш`); check its neighbours.
    let near = |i: usize| (i > 0 && adjacent_keys(extra, bv[i - 1], lang)) || (i + 1 < bv.len() && adjacent_keys(extra, bv[i + 1], lang));
    let mut i = at;
    let mut ok = near(i);
    while !ok && i + 1 < bv.len() && bv[i + 1] == extra {
        i += 1;
        ok = near(i);
    }
    (rest == fv && ok).then_some(fix)
}

fn suggest_one(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < MIN_LEN || !chars.iter().all(|c| c.is_alphabetic()) || (!assume_unknown && d.is_word(word)) {
        return None;
    }
    let lower_s = Dictionary::normalize(word);
    // A Russian form missing from the list but inflected from a known word
    // (`коррекцию` from `коррекция`) is not a typo.
    if !assume_unknown && lang == Lang::Ru && morph::is_known_form(d, &lower_s) {
        return None;
    }
    // English forms missing from the list (`typos`, `installer`, `uninstall`).
    if !assume_unknown && lang == Lang::En && en_known_form(d, &lower_s) {
        return None;
    }
    let lower: Vec<char> = lower_s.chars().collect();
    let alphabet: Vec<char> = match lang {
        Lang::En => EN_ALPHABET,
        Lang::Ru => RU_ALPHABET,
    }
    .chars()
    .collect();
    // Russian forms missing from the list (`коррекцию`) count only when they keep the
    // typed ending: they exist to preserve the case, not to widen the search.
    let suffix: Vec<char> = lower[lower.len() - SUFFIX_LEN.min(lower.len())..].to_vec();
    let rank = |cand: &str| -> Option<u32> {
        match lang {
            Lang::Ru if d.contains(cand) => morph::candidate_rank(d, cand),
            Lang::Ru if cand.chars().collect::<Vec<_>>().ends_with(&suffix) || is_swap(&lower, cand) => {
                morph::candidate_rank(d, cand)
            }
            Lang::Ru => None,
            Lang::En => d.rank(cand),
        }
        .filter(|&r| r < MAX_RANK)
    };
    // A rare candidate always competes (it can veto a wrong fix: `minut` is not `mint`
    // when `minute` exists), but wins only when [`tail_ok`].
    let can_win = |r: u32, cand: &str| r < TAIL_RANK || tail_ok(&lower, cand);
    let mut pick = Pick::new(&lower);
    let mut direct_pick = Pick::new(&lower);
    let mut saw_len_change = false;
    for cand in edits1(&lower, &alphabet) {
        if cand.chars().count() < MIN_LEN - 1 {
            continue;
        }
        // A wrong letter counts only when its key is next to the right one, or (6+
        // letters) for vowel / voiced-voiceless slips: `оцени` must not become `олени`
        // (ц and л are far apart), `recieve` not `relieve`.
        let first_slip = first_key_slip(&lower, &cand, lang);
        // The first letter left out (`траница` → `страница`): 6+ typed letters, and the
        // same frequency limit as a wrong first key.
        let first_missing = lower.len() >= MIN_LEN_FIRST_MISSING && {
            let cv: Vec<char> = cand.chars().collect();
            cv.len() == lower.len() + 1 && cv[1..] == lower[..]
        };
        if !substitution_ok(&lower, &cand, lang) || !(first_letter_kept(&lower, &cand) || first_slip || first_missing) {
            continue;
        }
        // Never change just the ending of a form: `удаляю` / `удалю`, `файлами` / `файлам`.
        // A same-row neighbour-key slip in the ending is still a typo (`услушу` →
        // `услугу`: ш↔г), not two case forms of one word (`файлами` / `файлам`).
        if lang == Lang::Ru && morph::only_ending_differs(&lower_s, &cand) && !is_direct_neighbour_subst(&lower, &cand, lang) {
            continue;
        }
        if lang == Lang::En && (en_affix_only(&lower_s, &cand) || en_end_letter_only(&lower_s, &cand)) {
            continue;
        }
        let limit = if first_missing { first_slip_rank(lower.len() + 1, lang) } else { first_slip_rank(lower.len(), lang) };
        if let Some(r) = rank(&cand).filter(|&r| !(first_slip || first_missing) || r < limit) {
            // Two neighbour letters swapped is the most common slip: such a fix counts
            // as SWAP_BONUS times more frequent (`компьютре` → `компьютер`, not `компьютере`).
            let ok = can_win(r, &cand);
            let r = if is_swap(&lower, &cand) { r / SWAP_BONUS } else { r };
            // Neighbour-key substitutions and neighbour transpositions beat
            // sound-alike or far substitutions when both exist (`услушу` → `услугу`,
            // not `услышу`).
            if cand.chars().count() != lower.len() {
                saw_len_change = true;
            }
            if is_direct_neighbour_subst(&lower, &cand, lang) {
                direct_pick.offer(r, cand.clone(), ok);
            }
            pick.offer(r, cand, ok);
        }
    }
    // A letter typed two places off (`чтокой` → `точкой`, `tihs` style slips across one
    // letter): only for 6+ letters and only when no ordinary one-edit fix exists.
    if pick.is_empty() && lower.len() >= MIN_LEN_MOVE {
        for cand in moves2(&lower) {
            if lang == Lang::Ru && morph::only_ending_differs(&lower_s, &cand) {
                continue;
            }
            if let Some(r) = rank(&cand) {
                let ok = can_win(r, &cand);
                pick.offer(r, cand, ok);
            }
        }
    }
    // Two edits: 8+ letters, or 7 letters when the edits are two swaps of neighbour
    // letters (`влкдаки` → `вкладки`: same letters, just shuffled). A rare candidate with
    // exactly the typed letters (8+) wins without [`tail_ok`]: the ending only had its
    // letters reordered, nothing was added or dropped (`удосбвта` → `удобства`).
    // One-slip candidates (above) block the full-form search below; two-slip ones only
    // when one of them can win (`рзделено`: the rare `разделены` two slips away loses,
    // the form `разделено` one slip away wins).
    let near_offered = !pick.is_empty();
    if pick.is_empty() && lower.len() >= MIN_LEN_2 - 1 {
        let swaps_only = lower.len() < MIN_LEN_2;
        let mut sorted_typed = lower.clone();
        sorted_typed.sort_unstable();
        // 8+ letters: a reordered ending wins without [`tail_ok`]; at 7 letters such
        // shuffles turn names into words (`баркину` → `банкиру`, `fannell` → `flannel`).
        let shuffled = |f: &[char]| lower.len() >= MIN_LEN_2 && {
            let mut v = f.to_vec();
            v.sort_unstable();
            v == sorted_typed
        };
        let same_letters = |f: &[char]| {
            let mut v = f.to_vec();
            v.sort_unstable();
            v == sorted_typed
        };
        // Only words that can keep the first letter (or swap the first two): a slice of
        // the sorted list instead of all 40 000 words.
        let first: String = lower[..1].iter().collect();
        let swapped: String = [lower[1], lower[0]].iter().collect();
        let pool = d.starting_with(&first).chain(d.starting_with(&swapped).filter(|_| lower[0] != lower[1]));
        for (cand, r) in pool {
            if r >= MAX_RANK || !first_letter_kept(&lower, cand) {
                continue;
            }
            let n = cand.chars().count();
            if n + 3 < lower.len() || n > lower.len() + 3 || n < MIN_LEN_2 - 2 {
                continue;
            }
            // Keep the typed ending: `кореркцию` is two edits from `коррекция` but one
            // from its form `коррекцию`.
            if lang == Lang::Ru {
                let mut offered = false;
                for (f, safe) in morph::reinflect(cand, &lower_s) {
                    let fv: Vec<char> = f.chars().collect();
                    if f != lower_s
                        && stem_differs(&lower, &fv)
                        && distance_kb(&lower, &fv, 2, lang) <= 2
                        && (!swaps_only || same_letters(&fv))
                        && morph::reinflect_ok(d, &f, safe)
                    {
                        let ok = can_win(r, &f) || shuffled(&fv);
                        pick.offer(r, f, ok);
                        offered = true;
                    }
                }
                if offered {
                    continue;
                }
            }
            if n + 2 < lower.len() || n > lower.len() + 2 || n < MIN_LEN_2 - 1 {
                continue;
            }
            let cv: Vec<char> = cand.chars().collect();
            if stem_differs(&lower, &cv)
                && !(lang == Lang::Ru && morph::only_ending_differs(&lower_s, cand))
                && !(lang == Lang::En && en_affix_only(&lower_s, cand))
                && distance(&lower, &cv, 2) == 2
                && distance_kb(&lower, &cv, 2, lang) <= 2
                && (!swaps_only || same_letters(&cv))
            {
                pick.offer(r, cand.to_string(), can_win(r, cand) || shuffled(&cv));
            }
        }
    }
    // Nothing in the frequency list (one or two slips away): any real word form one slip
    // away; frequency-list fixes keep priority (`definatly` → `definitely`, not `defiantly`);
    // `привественный` → `приветственный`, a form outside the 40 000-word list.
    // Not for Capitalised / UPPER words: unknown ones are mostly names.
    let lowercase = chars.iter().all(|c| c.is_lowercase());
    let undecided = pick.is_empty() || (!near_offered && pick.clone().winner().is_none());
    if undecided && lower.len() >= FULL_MIN_LEN && lowercase && !frequent_within_two(d, &lower) {
        let mut found = full_candidates(d, &lower, &lower_s, lang);
        // Past a losing two-slip candidate only a left-out letter is put back
        // (`рзделено` → `разделено`); a dropped one is not (`контингенте` stays).
        if !pick.is_empty() {
            found.retain(|c| c.chars().count() == lower.len() + 1);
        }
        if !found.is_empty() {
            return full_winner(&lower, found).map(|w| match_case(word, &w));
        }
    }
    let same_suffix_seen = pick.same.best.is_some();
    let w = pick.winner();
    // A unique same-row neighbour substitution beats a suffix-keeping diagonal
    // or sound-alike substitution (`услушу` → `услугу`, not `услышу`), but does
    // not override an insertion, deletion or transposition.
    if let Some(direct) = direct_pick.winner() {
        let subst = |cand: &str| {
            let cv: Vec<char> = cand.chars().collect();
            cv.len() == lower.len() && cv.iter().zip(&lower).filter(|(a, b)| *a != *b).count() == 1
        };
        if let Some(cur) = &w {
            if subst(cur) && !is_direct_neighbour_subst(&lower, cur, lang) {
                return Some(match_case(word, &direct));
            }
        } else if !same_suffix_seen && !saw_len_change {
            // No suffix-keeping candidate: the unique same-row neighbour slip sat
            // in the other-ending bucket and was blocked by a runner-up there.
            // Do not steal a missing-letter fix (`програма` → `программа`).
            return Some(match_case(word, &direct));
        }
    }
    Some(match_case(word, &w?))
}

/// A frequent word (rank under [`TAIL_RANK`]) two slips away makes a rare one-slip fix
/// a guess: `пирвте` is `привет` with two swaps, not the rare form `пирате`.
fn frequent_within_two(d: &Dictionary, lower: &[char]) -> bool {
    let first: String = lower[..1].iter().collect();
    let swapped: String = [lower[1], lower[0]].iter().collect();
    let pool = d.starting_with(&first).chain(d.starting_with(&swapped).filter(|_| lower[0] != lower[1]));
    let found = pool.filter(|&(_, r)| r < TAIL_RANK).any(|(w, _)| {
        let n = w.chars().count();
        n + 2 >= lower.len() && n <= lower.len() + 2 && {
            let cv: Vec<char> = w.chars().collect();
            distance(lower, &cv, 2) <= 2
        }
    });
    found
}

/// Minimum letters for a fix taken from the full-form set (outside the frequency list).
pub const FULL_MIN_LEN: usize = 5;

/// Search at most this many full-form neighbours (more means the word is too ambiguous).
const FULL_LIMIT: usize = 16;

/// Real word forms (full-form set) one slip away that pass the same safety rules as
/// list fixes: first letter kept, neighbour-key or sound-alike letters, not just another
/// ending of the same word, last two letters kept. Two slips are not searched here: on
/// the holdout they fixed nothing more and changed far more unknown words (+12 points).
fn full_candidates(d: &Dictionary, lower: &[char], lower_s: &str, lang: Lang) -> Vec<String> {
    let max = 1;
    let mut first = vec![lower[0]];
    if lower.len() > 1 && lower[1] != lower[0] {
        first.push(lower[1]);
    }
    let mut out = Vec::new();
    for cand in d.full_forms_near(lower_s, max, &first, FULL_LIMIT) {
        let cv: Vec<char> = cand.chars().collect();
        if cv.len() < FULL_MIN_LEN - 1 || !first_letter_kept(lower, &cand) {
            continue;
        }
        if !substitution_ok(lower, &cand, lang) {
            continue;
        }
        if distance_kb(lower, &cv, max, lang) > max {
            continue;
        }
        if lang == Lang::Ru && morph::only_ending_differs(lower_s, &cand) {
            continue;
        }
        if lang == Lang::En && (en_affix_only(lower_s, &cand) || en_end_letter_only(lower_s, &cand)) {
            continue;
        }
        if !tail_ok(lower, &cand) {
            continue;
        }
        out.push(cand);
    }
    // A letter typed two places off in a long word (`опциаонльно` → `опционально`):
    // moved, not swapped across one (that turns real rare words into others).
    if out.is_empty() && lower.len() >= MIN_LEN_FULL_MOVE {
        for cand in moves2(lower) {
            let cv: Vec<char> = cand.chars().collect();
            let moved = (0..lower.len().saturating_sub(2)).any(|i| {
                let mut v = lower.to_vec();
                let c = v.remove(i);
                v.insert(i + 2, c);
                let mut u = lower.to_vec();
                let c = u.remove(i + 2);
                u.insert(i, c);
                v == cv || u == cv
            });
            if moved
                && first_letter_kept(lower, &cand)
                && d.in_full_forms(&cand)
                && tail_ok(lower, &cand)
                && !(lang == Lang::Ru && morph::only_ending_differs(lower_s, &cand))
                && !out.contains(&cand)
            {
                out.push(cand);
            }
        }
    }
    out
}


/// Full-form fixes carry no frequency, so only an unambiguous one wins: the only
/// candidate, or the only one that swaps two neighbour letters.
fn full_winner(lower: &[char], found: Vec<String>) -> Option<String> {
    if found.len() == 1 {
        return found.into_iter().next();
    }
    let swaps: Vec<&String> = found.iter().filter(|c| is_swap(lower, c)).collect();
    if swaps.len() == 1 {
        return Some(swaps[0].clone());
    }
    None
}

/// A rarer candidate (rank [`TAIL_RANK`]+) must keep the typed last two letters: the
/// slip is inside the word (`ярдык` → `ярлык`), while a changed or dropped ending turns
/// unknown names and forms into other rare words (`айса` → `айс`, `minut` → `mint`).
pub(crate) fn tail_ok(typed: &[char], cand: &str) -> bool {
    let cv: Vec<char> = cand.chars().collect();
    typed.len() >= 2 && cv.len() >= 2 && typed[typed.len() - 2..] == cv[cv.len() - 2..]
}

/// Two-edit fixes must touch the stem: the words differ before their last two letters
/// (`трудностей` / `трудности`, `рекламный` / `рекламы` are different forms or words).
fn stem_differs(a: &[char], b: &[char]) -> bool {
    let common = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    common + 2 < a.len().min(b.len())
}

/// English `cand` only adds or drops a suffix (`vibrate` / `vibrates`, `midge` /
/// `midget`): another word, not a typo.
pub(crate) fn en_affix_only(typed: &str, cand: &str) -> bool {
    const SUFFIXES: [&str; 14] = ["s", "es", "d", "ed", "er", "ers", "al", "ly", "an", "ian", "ing", "t", "y", "ness"];
    let (long, short) = if typed.len() > cand.len() { (typed, cand) } else { (cand, typed) };
    long.strip_prefix(short).is_some_and(|tail| SUFFIXES.contains(&tail))
}

/// English `cand` only adds or drops one letter at the very end (`realme` / `realm`,
/// `apple` / `appl`): usually another word (a brand, a name, a form), not a slip. The
/// Russian counterpart is [`morph::only_ending_differs`]. A doubled last letter
/// (`helloo` → `hello`) is still a slip and may be fixed.
pub(crate) fn en_end_letter_only(typed: &str, cand: &str) -> bool {
    let (t, c): (Vec<char>, Vec<char>) = (typed.chars().collect(), cand.chars().collect());
    if t.len() == c.len() + 1 && t.starts_with(&c) {
        return t[t.len() - 1] != t[t.len() - 2];
    }
    c.len() == t.len() + 1 && c.starts_with(&t)
}

/// The fix starts with the typed first letter, or swaps the first two: slips at the
/// very start are rare, while `упускать` / `пускать`, `входной` / `выходной`,
/// `ренди` / `бренди` are different words.
/// Neighbour-key typo at the very start of an unknown word, after the first-letter-
/// keeping search. Two cases:
///
/// * Insertion of a neighbour before a short frequent word (`лдва` → `два`): only
///   4-letter tokens, rest rank < [`FIRST_SLIP_RANK_SHORT`]. A 5-letter typo like
///   `ждети` (for `ждете`) must not become `дети`.
/// * For longer tokens (6+), insertion of a leading neighbour, or (Russian only)
///   substitution of the first letter by a neighbouring key (`тллюминатора` →
///   `иллюминатора`), when the result is a known word or form (not only a frequent
///   one — long words are rarely ambiguous) and exactly one such candidate exists.
///   English first-letter substitution is not applied here: unknown names and tools
///   (`rustup`, `clippy`) too often have a unique dictionary neighbour.
///
/// First-letter-keeping fixes still win (`лдень` → `лень`, not `день`). Cautious
/// never reaches this function.
fn leading_neighbour_insert(d: &Dictionary, word: &str, lang: Lang, assume_unknown: bool) -> Option<String> {
    let lower = Dictionary::normalize(word);
    let chars: Vec<char> = lower.chars().collect();
    let n = chars.len();
    if !word.chars().all(char::is_alphabetic) || n < 4 {
        return None;
    }
    if !assume_unknown {
        if d.is_word(&lower) {
            return None;
        }
        if lang == Lang::Ru && morph::is_known_form(d, &lower) {
            return None;
        }
        if lang == Lang::En && en_known_form(d, &lower) {
            return None;
        }
    }
    if n == 4 {
        if !adjacent_keys(chars[0], chars[1], lang) {
            return None;
        }
        let rest: String = chars[1..].iter().collect();
        let rank = match lang {
            Lang::Ru => morph::candidate_rank(d, &rest).or_else(|| d.rank(&rest)),
            Lang::En => d.rank(&rest),
        }?;
        return (rank < FIRST_SLIP_RANK_SHORT).then(|| match_case(word, &rest));
    }
    // 5-letter tokens: no leading insertion (`ждети` must not become `дети`).
    // First-letter substitution of short words stays on the ranked `suggest_one` path.
    if n < 6 {
        return None;
    }
    let mut hits: Vec<String> = Vec::new();
    if adjacent_keys(chars[0], chars[1], lang) {
        let rest: String = chars[1..].iter().collect();
        if is_known_leading_target(d, &rest, lang) {
            hits.push(rest);
        }
    }
    // First-letter substitution is Russian-only: English unknown tokens
    // (names, tools: rustup, clippy) too often have a unique neighbour
    // that is a dictionary word (dustup, flippy).
    if lang == Lang::Ru {
        for c in RU_ALPHABET.chars() {
            if c == chars[0] || !adjacent_keys(chars[0], c, lang) {
                continue;
            }
            let mut cand = chars.clone();
            cand[0] = c;
            let s: String = cand.iter().collect();
            if is_known_leading_target(d, &s, lang) && !hits.contains(&s) {
                hits.push(s);
            }
        }
    }
    (hits.len() == 1).then(|| match_case(word, &hits[0]))
}

fn is_known_leading_target(d: &Dictionary, w: &str, lang: Lang) -> bool {
    if d.is_word(w) {
        return true;
    }
    match lang {
        Lang::Ru => morph::is_known_form(d, w),
        Lang::En => d.rank(w).is_some(),
    }
}

pub(crate) fn first_letter_kept(typed: &[char], cand: &str) -> bool {
    let cv: Vec<char> = cand.chars().collect();
    match (cv.first(), typed.first()) {
        (Some(a), Some(b)) if a == b => true,
        _ => cv.len() == typed.len() && cv.len() >= 2 && cv[0] == typed[1] && cv[1] == typed[0],
    }
}

/// A wrong first letter is fixed only into a word this frequent (`шлавный` → `главный`).
pub const FIRST_SLIP_RANK: u32 = 10_000;
/// The same for 4-letter words (`ъотя` → `хотя`): only the most frequent ones, since
/// short unknown tokens are often names and abbreviations.
pub const FIRST_SLIP_RANK_SHORT: u32 = 1_000;

/// The same for Russian 8+ letter words (`жлементы` → `элементы`, rank 12 409): a long
/// word rarely has a real neighbour that differs in the first key alone. Not English: its
/// list misses many real words that would turn into others (`cemented` → `demented`,
/// `desisted` → `resisted`).
pub const FIRST_SLIP_RANK_LONG: u32 = 20_000;

/// Rank limit for a first-letter fix of a `len`-letter word.
pub fn first_slip_rank(len: usize, lang: Lang) -> u32 {
    match len {
        0..=4 => FIRST_SLIP_RANK_SHORT,
        8.. if lang == Lang::Ru => FIRST_SLIP_RANK_LONG,
        _ => FIRST_SLIP_RANK,
    }
}

/// `cand` differs from `typed` only in the first letter, and the two keys are neighbours
/// (ш and г): such a slip is fixed, but only into a frequent word ([`FIRST_SLIP_RANK`];
/// 4-letter words [`FIRST_SLIP_RANK_SHORT`]); `упускать` / `пускать`-style different words are still never touched.
fn first_key_slip(typed: &[char], cand: &str, lang: Lang) -> bool {
    let cv: Vec<char> = cand.chars().collect();
    typed.len() >= 4 && cv.len() == typed.len() && cv[1..] == typed[1..] && cv[0] != typed[0] && adjacent_keys(typed[0], cv[0], lang)
}

/// Rank divisor for a fix that only swaps two neighbour letters.
pub const SWAP_BONUS: u32 = 10;

fn is_swap(typed: &[char], cand: &str) -> bool {
    let cv: Vec<char> = cand.chars().collect();
    if cv.len() != typed.len() {
        return false;
    }
    let diff: Vec<usize> = (0..cv.len()).filter(|&i| cv[i] != typed[i]).collect();
    matches!(diff.as_slice(), [i, j] if *j == i + 1 && typed[*i] == cv[*j] && typed[*j] == cv[*i])
}

/// Transpositions (any two letters swapped) and rotations of `typed`, not including itself.
fn short_perm_forms(typed: &[char]) -> Vec<String> {
    let n = typed.len();
    let mut out = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            let mut c = typed.to_vec();
            c.swap(i, j);
            if c != typed {
                out.push(c.into_iter().collect());
            }
        }
    }
    for k in 1..n {
        let rot: String = typed[k..].iter().chain(&typed[..k]).copied().collect();
        if rot.chars().ne(typed.iter().copied()) {
            out.push(rot);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// A 3-letter lowercase token that is not on the frequency list, and whose letters are a
/// transposition or rotation of exactly one very frequent word (rank < [`SHORT_PERM_RANK`]).
/// Unranked full-form crumbs (`ент` is in the form set but has no list rank) still count
/// as unknown here; frequency-list words (`кот`, `нет`) are left alone. A reading in the
/// other layout that is itself a list word is not corrected.
fn short_perm_fix(d: &Dictionary, word: &str, lang: Lang) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() != SHORT_PERM_LEN || !chars.iter().all(|c| c.is_alphabetic() && c.is_lowercase()) {
        return None;
    }
    if crate::layout::detect_script(word) != Some(lang) {
        return None;
    }
    let lower = Dictionary::normalize(word);
    if d.rank(&lower).is_some() {
        return None;
    }
    let other = match lang {
        Lang::Ru => Lang::En,
        Lang::En => Lang::Ru,
    };
    let conv = crate::layout::convert(&lower, lang);
    if dict::get(other).rank(&Dictionary::normalize(&conv)).is_some() {
        return None;
    }
    let lower_c: Vec<char> = lower.chars().collect();
    let mut hits: Vec<String> = Vec::new();
    for cand in short_perm_forms(&lower_c) {
        if d.rank(&cand).is_some_and(|r| r < SHORT_PERM_RANK) && !hits.iter().any(|w| w == &cand) {
            hits.push(cand);
        }
    }
    (hits.len() == 1).then(|| match_case(word, &hits[0]))
}

/// Words up to this length accept a substituted letter only from a neighbouring key;
/// longer ones also a vowel for a vowel or a voiced / voiceless pair.
pub const SHORT_SUBST_LEN: usize = 5;

const RU_VOWELS: &str = "аеиоуыэюя";
const EN_VOWELS: &str = "aeiouy";
const RU_PAIRS: [(char, char); 8] = [('б', 'п'), ('в', 'ф'), ('г', 'к'), ('д', 'т'), ('ж', 'ш'), ('з', 'с'), ('ш', 'щ'), ('ь', 'ъ')];

pub(crate) fn sound_alike(a: char, b: char, lang: Lang) -> bool {
    let vowels = match lang {
        Lang::Ru => RU_VOWELS,
        Lang::En => EN_VOWELS,
    };
    (vowels.contains(a) && vowels.contains(b))
        || (lang == Lang::Ru && RU_PAIRS.iter().any(|&(x, y)| (x, y) == (a, b) || (y, x) == (a, b)))
}

const RU_ROWS: [&str; 3] = ["йцукенгшщзхъ", "фывапролджэ", "ячсмитьбю"];
const EN_ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

fn key_pos(c: char, lang: Lang) -> Option<(i32, i32)> {
    let rows = match lang {
        Lang::Ru => RU_ROWS,
        Lang::En => EN_ROWS,
    };
    rows.iter().enumerate().find_map(|(r, row)| row.chars().position(|k| k == c).map(|p| (r as i32, p as i32)))
}

/// Keys touching on a staggered keyboard (same row ±1, row below at column −1/0, above 0/+1).
/// A table lookup (see [`crate::typo_wide`]); [`adjacent_keys_raw`] computes it.
pub(crate) fn adjacent_keys(a: char, b: char, lang: Lang) -> bool {
    crate::typo_wide::adjacent_fast(a, b, lang)
}

pub(crate) fn adjacent_keys_raw(a: char, b: char, lang: Lang) -> bool {
    match (key_pos(a, lang), key_pos(b, lang)) {
        (Some((ra, ca)), Some((rb, cb))) => match rb - ra {
            0 => (ca - cb).abs() == 1,
            1 => cb == ca || cb == ca - 1,
            -1 => cb == ca || cb == ca + 1,
            _ => false,
        },
        _ => false,
    }
}

/// 1 = orthogonal (same row or column, sharing a side); 2 = diagonal (sharing a corner).
pub(crate) fn key_adjacency(a: char, b: char, lang: Lang) -> Option<u8> {
    if !adjacent_keys_raw(a, b, lang) {
        return None;
    }
    let ((ra, ca), (rb, cb)) = (key_pos(a, lang)?, key_pos(b, lang)?);
    let (dr, dc) = ((rb - ra).abs(), (cb - ca).abs());
    match (dr, dc) {
        (0, 1) | (1, 0) => Some(1),
        (1, 1) => Some(2),
        _ => None,
    }
}

pub(crate) fn is_direct_neighbour_subst(typed: &[char], cand: &str, lang: Lang) -> bool {
    let cv: Vec<char> = cand.chars().collect();
    if cv.len() != typed.len() {
        return false;
    }
    let diff: Vec<usize> = (0..cv.len()).filter(|&i| cv[i] != typed[i]).collect();
    matches!(diff.as_slice(), [i] if key_adjacency(typed[*i], cv[*i], lang) == Some(1))
}

/// Same-length typo that changes exactly one letter to a neighbouring key.
#[allow(dead_code)]
pub(crate) fn is_neighbour_subst(typed: &[char], cand: &str, lang: Lang) -> bool {
    let cv: Vec<char> = cand.chars().collect();
    if cv.len() != typed.len() {
        return false;
    }
    let diff: Vec<usize> = (0..cv.len()).filter(|&i| cv[i] != typed[i]).collect();
    matches!(diff.as_slice(), [i] if adjacent_keys(typed[*i], cv[*i], lang))
}

/// `cand` is not a same-length one-letter substitution of `typed`, or the two letters
/// sit on neighbouring keys.
fn substitution_ok(typed: &[char], cand: &str, lang: Lang) -> bool {
    let cv: Vec<char> = cand.chars().collect();
    if cv.len() != typed.len() {
        return true;
    }
    let diff: Vec<usize> = (0..cv.len()).filter(|&i| cv[i] != typed[i]).collect();
    match diff.as_slice() {
        [i] => {
            adjacent_keys(typed[*i], cv[*i], lang)
                || (typed.len() > SHORT_SUBST_LEN && sound_alike(typed[*i], cv[*i], lang))
        }
        _ => true,
    }
}

/// English word built from a listed one with a common suffix or prefix
/// (`typos`, `installer`, `uninstall`, `hiding`): not a typo.
pub fn en_known_form(d: &Dictionary, w: &str) -> bool {
    let known = |b: &str| b.chars().count() >= 3 && d.rank(b).is_some_and(|r| r < TAIL_RANK * 2);
    for suf in ["s", "es", "ed", "d", "ing", "er", "ers", "est", "ly", "ness", "able", "ment", "ments"] {
        if let Some(base) = w.strip_suffix(suf) {
            if known(base) || known(&format!("{base}e")) {
                return true;
            }
            // doubled consonant: stopped → stop, running → run
            let b: Vec<char> = base.chars().collect();
            if b.len() >= 4 && b[b.len() - 1] == b[b.len() - 2] && known(&b[..b.len() - 1].iter().collect::<String>()) {
                return true;
            }
        }
    }
    if let Some(base) = w.strip_suffix("ies").or_else(|| w.strip_suffix("ied")) {
        if known(&format!("{base}y")) {
            return true;
        }
    }
    for pre in ["un", "re", "pre", "dis", "mis", "non", "over", "sub"] {
        if let Some(rest) = w.strip_prefix(pre) {
            if rest.chars().count() >= 4 && (known(rest) || en_known_form_suffix_only(d, rest)) {
                return true;
            }
        }
    }
    false
}

fn en_known_form_suffix_only(d: &Dictionary, w: &str) -> bool {
    ["s", "ed", "ing", "er"].iter().any(|suf| w.strip_suffix(suf).is_some_and(|b| b.chars().count() >= 3 && d.contains(b)))
}

/// Letters that must match at the end for a candidate to count as "same ending".
pub const SUFFIX_LEN: usize = 3;
/// Minimum letters for two-places-off moves.
pub const MIN_LEN_MOVE: usize = 6;

/// Minimum letters for a full-form fix of a letter typed two places off (`опциаонльно`).
pub const MIN_LEN_FULL_MOVE: usize = 10;

/// Minimum typed letters for putting back a left-out first letter (`траница`).
pub const MIN_LEN_FIRST_MISSING: usize = 6;

/// One letter moved two places (`чтокой` → `точкой`) or two letters one apart swapped.
fn moves2(w: &[char]) -> Vec<String> {
    let n = w.len();
    let mut out = Vec::new();
    for i in 0..n.saturating_sub(2) {
        // move w[i] right by two / w[i+2] left by two
        let mut v = w.to_vec();
        let c = v.remove(i);
        v.insert(i + 2, c);
        out.push(v.iter().collect());
        let mut v = w.to_vec();
        let c = v.remove(i + 2);
        v.insert(i, c);
        out.push(v.iter().collect());
        if w[i] != w[i + 2] {
            let mut v = w.to_vec();
            v.swap(i, i + 2);
            out.push(v.iter().collect());
        }
    }
    out
}

/// Candidates keeping the typed word's last letters (its case ending) beat the rest:
/// among them the most frequent must clearly win; only without any do the others count.
#[derive(Clone)]
struct Pick {
    suffix: Vec<char>,
    same: Ranked,
    other: Ranked,
}

#[derive(Default, Clone)]
struct Ranked {
    best: Option<(u32, String, bool)>,
    second: Option<u32>,
}

impl Pick {
    fn new(typed: &[char]) -> Self {
        let k = SUFFIX_LEN.min(typed.len());
        Self { suffix: typed[typed.len() - k..].to_vec(), same: Ranked::default(), other: Ranked::default() }
    }

    fn is_empty(&self) -> bool {
        self.same.best.is_none() && self.other.best.is_none()
    }

    /// `ok`: the candidate may win; otherwise it only competes (see `can_win`).
    fn offer(&mut self, r: u32, cand: String, ok: bool) {
        let cv: Vec<char> = cand.chars().collect();
        if cv.ends_with(&self.suffix) {
            self.same.offer(r, cand, ok);
        } else {
            self.other.offer(r, cand, ok);
        }
    }

    fn winner(self) -> Option<String> {
        if self.same.best.is_some() {
            self.same.winner()
        } else {
            self.other.winner()
        }
    }
}

impl Ranked {
    fn offer(&mut self, r: u32, cand: String, ok: bool) {
        match &self.best {
            Some((_, bw, _)) if *bw == cand => {}
            Some((br, _, _)) if r < *br => {
                self.second = Some(self.second.map_or(*br, |s| s.min(*br)));
                self.best = Some((r, cand, ok));
            }
            Some(_) => self.second = Some(self.second.map_or(r, |s| s.min(r))),
            None => self.best = Some((r, cand, ok)),
        }
    }

    fn winner(self) -> Option<String> {
        let (r, w, ok) = self.best?;
        if !ok {
            return None;
        }
        if let Some(s) = self.second {
            if r.saturating_add(1).saturating_mul(DOMINANCE) > s.saturating_add(1) {
                return None;
            }
        }
        Some(w)
    }
}

/// Damerau-Levenshtein (optimal string alignment) distance, or `max + 1` once it exceeds `max`.
fn distance(a: &[char], b: &[char], max: usize) -> usize {
    distance_by(a, b, max, |x, y| usize::from(x != y))
}

/// Like `distance`, but a wrong letter counts only for a plausible slip (neighbour key,
/// vowel for vowel, voiced/voiceless pair): `скидывать` is not `скрывать` (д and р are far).
fn distance_kb(typed: &[char], cand: &[char], max: usize, lang: Lang) -> usize {
    distance_by(typed, cand, max, |x, y| {
        if x == y {
            0
        } else if adjacent_keys(x, y, lang) || sound_alike(x, y, lang) {
            1
        } else {
            max + 1
        }
    })
}

fn distance_by(a: &[char], b: &[char], max: usize, sub: impl Fn(char, char) -> usize) -> usize {
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > max {
        return max + 1;
    }
    let mut prev2 = vec![0usize; m + 1];
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=m {
            let cost = sub(a[i - 1], b[j - 1]);
            let mut v = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
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


/// Shortest token a whole-hand column shift may fix (Normal / Aggressive only).
pub const HAND_SHIFT_MIN_LEN: usize = 4;
/// 4-letter hand-shift targets must be at least this frequent.
const HAND_SHIFT_SHORT_RANK: u32 = 5_000;
/// Longer ranked targets may be this rare.
/// An unranked full-form / curated word is accepted only from this length.
const HAND_SHIFT_WORD_LEN: usize = 6;
const HAND_SHIFT_TIE_RATIO: f64 = 4.0;

/// A whole-token home-row misplacement: every key is one column left or right
/// of what was meant, optionally also read in the other layout.
///
/// Accepts a candidate only when exactly one of the four (direction × layout)
/// readings is a known word, the typed token is not a word of either language,
/// and `level` is not Cautious. `prefer` is the shift that fixed the previous
/// word in the same phrase: that reading wins when it is a known word, even if
/// another direction also is, or if it is a longer unranked form.
pub(crate) fn hand_shift_fix(
    word: &str,
    src: Lang,
    level: Level,
    prefer: Option<(i32, Lang)>,
) -> Option<(String, Lang)> {
    if level == Level::Cautious {
        return None;
    }
    let n = word.chars().count();
    if n < HAND_SHIFT_MIN_LEN {
        return None;
    }
    if dict::get(Lang::En).is_word(word) || dict::get(Lang::Ru).is_word(word) {
        return None;
    }
    if looks_like_identifier(word) || (src == Lang::Ru && is_ruslish_form(word)) {
        return None;
    }

    struct Hit {
        text: String,
        dest: Lang,
        dir: i32,
    }
    let mut hits: Vec<Hit> = Vec::new();
    for dest in [src, src.other()] {
        for dir in [-1i32, 1] {
            let Some(cand) = crate::layout::shift_token(word, src, dest, dir) else { continue };
            if Dictionary::normalize(&cand) == Dictionary::normalize(word) {
                continue;
            }
            if !hand_shift_target_ok(&cand, dest, n) {
                continue;
            }
            if !hits.iter().any(|h| h.text == cand && h.dest == dest) {
                hits.push(Hit { text: cand, dest, dir });
            }
        }
    }

    if let Some((pdir, pdest)) = prefer {
        if let Some(h) = hits.iter().find(|h| h.dir == pdir && h.dest == pdest) {
            return Some((match_case(word, &h.text), h.dest));
        }
        // The consistent direction may be a real form that missed the rank cut.
        if let Some(cand) = crate::layout::shift_token(word, src, pdest, pdir) {
            let letters: String = cand.chars().filter(|c| c.is_alphabetic()).collect();
            if letters.chars().count() >= HAND_SHIFT_MIN_LEN
                && crate::layout::detect_script(&letters) == Some(pdest)
                && dict::get(pdest).is_word(&letters)
            {
                return Some((match_case(word, &cand), pdest));
            }
        }
    }

    if hits.len() == 1 {
        let h = &hits[0];
        return Some((match_case(word, &h.text), h.dest));
    }
    if hits.len() > 1 {
        let score = |h: &Hit| crate::ngram::get(h.dest).count(&[&Dictionary::normalize(&h.text)]) / crate::ngram::get(h.dest).total().max(1.0);
        let mut scored: Vec<(f64, usize)> = hits.iter().enumerate().map(|(i, h)| (score(h), i)).collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        if scored[0].0 > 0.0 && scored[0].0 >= scored[1].0 * HAND_SHIFT_TIE_RATIO {
            let h = &hits[scored[0].1];
            return Some((match_case(word, &h.text), h.dest));
        }
    }
    None
}

fn hand_shift_target_ok(cand: &str, dest: Lang, _typed_n: usize) -> bool {
    if !cand.chars().all(char::is_alphabetic) {
        return false;
    }
    if crate::layout::detect_script(cand) != Some(dest) {
        return false;
    }
    let n = cand.chars().count();
    if n < HAND_SHIFT_MIN_LEN {
        return false;
    }
    let d = dict::get(dest);
    if let Some(r) = d.rank(cand) {
        return if n <= 4 { r < HAND_SHIFT_SHORT_RANK } else { true };
    }
    (n >= HAND_SHIFT_WORD_LEN && d.is_word(cand)) || (dest == Lang::Ru && n >= 5 && crate::morph::is_mat_form(cand))
}

/// Which (direction, destination layout) takes `typed` to `fixed`, if unique.
pub(crate) fn hand_shift_which(typed: &str, fixed: &str) -> Option<(i32, Lang)> {
    let src = crate::layout::detect_script(typed)?;
    let want = Dictionary::normalize(fixed);
    let mut found = None;
    for dest in [src, src.other()] {
        for dir in [-1i32, 1] {
            if let Some(c) = crate::layout::shift_token(typed, src, dest, dir) {
                if Dictionary::normalize(&c) == want {
                    if found.is_some() {
                        return None;
                    }
                    found = Some((dir, dest));
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_common_typos() {
        assert_eq!(suggest("поечаток", Lang::Ru).as_deref(), Some("опечаток"));
        assert_eq!(suggest("пирвет", Lang::Ru).as_deref(), Some("привет"));
        assert_eq!(suggest("Спаисбо", Lang::Ru).as_deref(), Some("Спасибо"));
        assert_eq!(suggest("becuase", Lang::En).as_deref(), Some("because"));
        assert_eq!(suggest("wrold", Lang::En).as_deref(), Some("world"));
        assert_eq!(suggest("THNAKS", Lang::En).as_deref(), Some("THANKS"));
    }

    #[test]
    fn two_edits_only_for_long_words() {
        assert_eq!(suggest("прграмирование", Lang::Ru).as_deref(), Some("программирование"));
        assert_eq!(suggest("пожалста", Lang::Ru).as_deref(), Some("пожалуйста"));
        assert_eq!(suggest("definatly", Lang::En).as_deref(), Some("definitely"));
        assert_eq!(suggest("neccesary", Lang::En).as_deref(), Some("necessary"));
        // Short words get one edit only: пирвте is two edits from привет, so it stays.
        assert_eq!(suggest("пирвте", Lang::Ru), None);
        assert_eq!(suggest("abcdxyzq", Lang::En), None);
    }

    #[test]
    fn distance_works() {
        let c = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(distance(&c("abcd"), &c("abdc"), 2), 1);
        assert_eq!(distance(&c("kitten"), &c("sitting"), 2), 3);
        assert_eq!(distance(&c("abc"), &c("abc"), 2), 0);
    }

    #[test]
    fn leaves_known_short_and_unclear_words() {
        assert_eq!(suggest("привет", Lang::Ru), None);
        // 3 letters since 0.3.72 (Level::Normal), when one fix clearly wins.
        assert_eq!(suggest("teh", Lang::En).as_deref(), Some("the"));
        assert_eq!(suggest_level("teh", Lang::En, Level::Cautious), None); // too short for the strict rules
        // Neighbour-key insertion at the start of a short unknown word.
        assert_eq!(suggest("лдва", Lang::Ru).as_deref(), Some("два"));
        assert_eq!(suggest("Лдва", Lang::Ru).as_deref(), Some("Два"));
        assert_eq!(suggest_level("лдва", Lang::Ru, Level::Cautious), None);
        // Siblings: other neighbour insertions at the start, into a frequent word.
        assert_eq!(suggest("лдля", Lang::Ru).as_deref(), Some("для"));
        // A first-letter-keeping typo still wins (`лдень` → `лень`, not `день`).
        assert_eq!(suggest("лдень", Lang::Ru).as_deref(), Some("лень"));
        // 5 letters: do not drop a leading neighbour (`ждети` is a typo of `ждете`, not `дети`).
        assert_ne!(suggest("ждети", Lang::Ru).as_deref(), Some("дети"));
        // First-letter neighbour substitution of a long unknown form (`т` sits next to `и`).
        assert_eq!(suggest("тллюминатора", Lang::Ru).as_deref(), Some("иллюминатора"));
        assert_eq!(suggest("Тллюминатора", Lang::Ru).as_deref(), Some("Иллюминатора"));
        assert_eq!(suggest_level("тллюминатора", Lang::Ru, Level::Cautious), None);
        // Sibling: the other neighbour of `и` on the bottom row (`м`).
        assert_eq!(suggest("мллюминатора", Lang::Ru).as_deref(), Some("иллюминатора"));
        // Sibling: leading neighbour insertion on a long known form.
        assert_eq!(suggest("тиллюминатора", Lang::Ru).as_deref(), Some("иллюминатора"));
        // Real words and non-adjacent extras stay.
        for w in ["два", "слова", "лето", "льва", "дана", "лдван", "иллюминатора"] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
        assert_eq!(suggest("qzxvbn", Lang::En), None);
        // English unknown tools/names must not flip the first letter into a dictionary word.
        for w in ["rustup", "clippy", "Rustup", "Clippy"] {
            assert_eq!(suggest(w, Lang::En), None, "{w}");
        }
        // к→й is not a neighbour-key slip, so only the doubled letter is dropped.
        assert_eq!(suggest("опечаткок", Lang::Ru).as_deref(), Some("опечаток"));
    }

    #[test]
    fn three_letter_permutation_of_a_frequent_word() {
        // Rotation: ядл → для (для is rank 37).
        assert_eq!(suggest("ядл", Lang::Ru).as_deref(), Some("для"));
        assert_eq!(suggest_level("ядл", Lang::Ru, Level::Aggressive).as_deref(), Some("для"));
        assert_eq!(suggest_level("ядл", Lang::Ru, Level::Cautious), None);
        // Adjacent transposition: ент → нет (нет is rank 16). ент is an unranked
        // full-form crumb, so the scored search would skip it as a "known" word.
        assert_eq!(suggest("ент", Lang::Ru).as_deref(), Some("нет"));
        assert_eq!(suggest_level("ент", Lang::Ru, Level::Cautious), None);
        // Other transpositions / rotations of the same two words.
        assert_eq!(suggest("дял", Lang::Ru).as_deref(), Some("для"));
        assert_eq!(suggest("лдя", Lang::Ru).as_deref(), Some("для"));
        // The frequent words themselves, and other top-300 function words, stay.
        for w in ["для", "нет", "что", "как", "все", "его"] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
        // English: teh → the already from the scored search; hte is a rotation.
        assert_eq!(suggest("teh", Lang::En).as_deref(), Some("the"));
        assert_eq!(suggest("hte", Lang::En).as_deref(), Some("the"));
        assert_eq!(suggest("the", Lang::En), None);
        // A ranked mid-list anagram of a top-300 word is a real word, not a typo.
        assert_eq!(suggest("кот", Lang::Ru), None);
        // Two or more top-300 anagrams: do not guess.
        assert_eq!(suggest("роп", Lang::Ru), None); // про / пор
        assert_eq!(suggest("asw", Lang::En), None); // was / saw
        assert_eq!(suggest("woh", Lang::En), None); // how / who
        assert_eq!(suggest("onw", Lang::En), None); // now / won / own
        // Same length but not a transposition or rotation (a substituted letter).
        assert_eq!(suggest("тта", Lang::Ru), None);
        assert_eq!(suggest("нен", Lang::Ru), None);
    }

    #[test]
    fn keeps_case_ending() {
        assert_eq!(suggest("кореркцию", Lang::Ru).as_deref(), Some("коррекцию"));
        assert_eq!(suggest("Кореркцию", Lang::Ru).as_deref(), Some("Коррекцию"));
        assert_eq!(suggest("кореркцией", Lang::Ru).as_deref(), Some("коррекцией"));
        assert_eq!(suggest("прогармму", Lang::Ru).as_deref(), Some("программу"));
        assert_eq!(suggest("солвами", Lang::Ru).as_deref(), Some("словами"));
        // Correct forms missing from the list are not "fixed" into another case.
        for ok in ["коррекцию", "коррекцией", "программу", "словами", "точкой"] {
            assert_eq!(suggest(ok, Lang::Ru), None, "{ok}");
        }
    }

    #[test]
    fn user_cases_0_3_62() {
        // Two neighbour swaps in a 7-letter word.
        assert_eq!(suggest("влкдаки", Lang::Ru).as_deref(), Some("вкладки"));
        // Two swaps that reorder the ending: a rare word with the same letters wins
        // without keeping the typed last two letters (8+ letters only).
        assert_eq!(suggest("удосбвта", Lang::Ru).as_deref(), Some("удобства"));
        assert_eq!(suggest("Удосбвта", Lang::Ru).as_deref(), Some("Удобства"));
        assert_eq!(suggest("удосбтва", Lang::Ru).as_deref(), Some("удобства"));
        // 7-letter shuffles still keep the ending rule: names stay.
        assert_eq!(suggest("баркину", Lang::Ru), None);
        assert_eq!(suggest("fannell", Lang::En), None);
        // A rare full form one insertion away, lowercase only.
        assert_eq!(suggest("подстветка", Lang::Ru).as_deref(), Some("подсветка"));
    }

    #[test]
    fn slips_from_a_long_message() {
        // A left-out letter in a rare form, past a losing two-slip candidate (`разделены`).
        assert_eq!(suggest("рзделено", Lang::Ru).as_deref(), Some("разделено"));
        // A letter typed two places off in a long rare form.
        assert_eq!(suggest("опциаонльно", Lang::Ru).as_deref(), Some("опционально"));
        // A left-out first letter, frequent target.
        assert_eq!(suggest("траница", Lang::Ru).as_deref(), Some("страница"));
        assert_eq!(suggest("Траница", Lang::Ru).as_deref(), Some("Страница"));
        // `-ить` for `-ит`: only when the verb's infinitive is another word.
        assert_eq!(suggest("услышить", Lang::Ru).as_deref(), Some("услышит"));
        assert_eq!(suggest("увидить", Lang::Ru).as_deref(), Some("увидит"));
        assert_eq!(suggest("ккайф", Lang::Ru).as_deref(), Some("кайф"));
        // Left alone: real words, names / slang verbs, too short or too far.
        for w in ["буде", "коды", "зи", "динамить", "зеркалить", "архим", "ухора", "контингенте"] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
        assert_eq!(suggest("lowfps", Lang::En), None);
        // `верз`: left alone by the strict rules; since 0.3.72 the scored search fixes
        // it (з is next to х, `верх` is frequent and the only close word).
        assert_eq!(suggest_level("верз", Lang::Ru, Level::Cautious), None);
        assert_eq!(suggest("верз", Lang::Ru).as_deref(), Some("верх"));
    }

    #[test]
    fn soft_sign_after_a_hushing_letter() {
        assert_eq!(suggest("клавишь", Lang::Ru).as_deref(), Some("клавиш"));
        assert_eq!(suggest("Клавишь", Lang::Ru).as_deref(), Some("Клавиш"));
        assert_eq!(suggest("гадёнышь", Lang::Ru).as_deref(), Some("гадёныш"));
        assert_eq!(suggest("карандашь", Lang::Ru).as_deref(), Some("карандаш"));
        // ь dropped and an extra neighbour key removed: п was pressed with а.
        assert_eq!(suggest("клапвишь", Lang::Ru).as_deref(), Some("клавиш"));
        // Real words keep their ь; other changes to the rest are not made.
        for w in ["мышь", "ночь", "делаешь", "плачь", "режь", "вещь", "лефковичь"] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
        // Never the non-word «системым» (noun stem + adjective ending).
        assert_eq!(suggest("ситстемрым", Lang::Ru).as_deref(), Some("системным"));
        // Correct verb forms missing from the list stay, every time.
        for _ in 0..3 {
            for ok in ["оцени", "оцените", "оценю", "проверь", "исправьте", "разбери", "удаляю"] {
                assert_eq!(suggest(ok, Lang::Ru), None, "{ok}");
            }
        }
        // English forms built from listed words.
        for ok in ["typos", "installer", "uninstall", "layouts"] {
            assert_eq!(suggest(ok, Lang::En), None, "{ok}");
        }
        assert_eq!(suggest("recieve", Lang::En).as_deref(), Some("receive"));
    }

    #[test]
    fn real_words_missing_from_list_0_3_63() {
        assert_eq!(suggest("скидывать", Lang::Ru), None);
        assert_eq!(suggest("скидываешь", Lang::Ru), None);
    }

    #[test]
    fn en_last_letter_only_is_not_a_typo_0_3_65() {
        // Unknown word that is a listed word plus / minus its last letter: left alone.
        let d = dict::ranked_only(Lang::En);
        assert!(!d.is_word("realme"));
        assert_eq!(suggest_in(&d, "realme", Lang::En), None);
        assert_eq!(suggest("realme", Lang::En), None);
        assert!(en_end_letter_only("realme", "realm"));
        assert!(en_end_letter_only("appl", "apple"));
        assert!(!en_end_letter_only("helloo", "hello"), "doubled last letter is a slip");
        assert!(!en_end_letter_only("wrold", "world"));
        assert_eq!(suggest_in(&d, "helloo", Lang::En).as_deref(), Some("hello"));
        // Ordinary slips inside the word are still fixed.
        assert_eq!(suggest("wrold", Lang::En).as_deref(), Some("world"));
        assert_eq!(suggest("becuase", Lang::En).as_deref(), Some("because"));
    }

    #[test]
    fn rare_half_of_the_list_0_3_67() {
        // Up to 0.3.66 words ranked 20 000+ (`ярлык` is ~30 700) were never fixes.
        assert_eq!(suggest("ярдык", Lang::Ru).as_deref(), Some("ярлык"));
        assert_eq!(suggest("Ярдык", Lang::Ru).as_deref(), Some("Ярлык"));
        assert_eq!(suggest("ярдыки", Lang::Ru).as_deref(), Some("ярлыки"));
        assert!(tail_ok(&"ярдык".chars().collect::<Vec<_>>(), "ярлык"));
        // A rare fix that changes the ending does not win.
        assert!(!tail_ok(&"айса".chars().collect::<Vec<_>>(), "айс"));
        // Real words still stay.
        for ok in ["ярлык", "ярлыки", "скидывать"] {
            assert_eq!(suggest(ok, Lang::Ru), None, "{ok}");
        }
    }

    #[test]
    fn full_forms_and_first_letter_0_3_69() {
        // Forms outside the 40 000-word list are fixes too.
        assert_eq!(suggest("привественный", Lang::Ru).as_deref(), Some("приветственный"));
        assert_eq!(suggest("ярдык", Lang::Ru).as_deref(), Some("ярлык"));
        // A neighbour key for the first letter, into a frequent word.
        assert_eq!(suggest("шлавный", Lang::Ru).as_deref(), Some("главный"));
        // 8+ Russian letters: the target may be rarer (элементы is rank 12 409).
        assert_eq!(suggest("жлементы", Lang::Ru).as_deref(), Some("элементы"));
        assert_eq!(suggest("Жлементы", Lang::Ru).as_deref(), Some("Элементы"));
        // English keeps the 10 000 limit: `cemented` is a word missing from the list.
        assert_eq!(suggest("cemented", Lang::En), None);
        assert_eq!(suggest("Шлавный", Lang::Ru).as_deref(), Some("Главный"));
        // 4-letter words too, into the 1 000 most frequent ones.
        assert_eq!(suggest("ъотя", Lang::Ru).as_deref(), Some("хотя"));
        assert_eq!(suggest("Ъотя", Lang::Ru).as_deref(), Some("Хотя"));
        // Three forms one letter away (исправляет / исправлять / исправят) and no
        // frequency to choose by: left alone rather than guessed.
        assert_eq!(suggest("исправлят", Lang::Ru), None);
        assert_eq!(suggest("тсправлят", Lang::Ru), None);
        // Still left alone.
        assert_eq!(suggest("скидывать", Lang::Ru), None);
        assert_eq!(suggest("скидываешь", Lang::Ru), None);
        assert_eq!(suggest("realme", Lang::En), None);
        assert_eq!(suggest("helloo", Lang::En).as_deref(), Some("hello"));
        for ok in ["упускать", "входной", "приветственный"] {
            assert_eq!(suggest(ok, Lang::Ru), None, "{ok}");
        }
    }

    #[test]
    fn letter_two_places_off() {
        assert_eq!(suggest("чтокой", Lang::Ru).as_deref(), Some("точкой"));
        assert_eq!(suggest("чтокой", Lang::Ru).as_deref(), Some("точкой"));
        // Real words starting with «что» stay.
        for ok in ["чтобы", "что-то", "чтоб"] {
            assert_eq!(suggest(ok, Lang::Ru), None, "{ok}");
        }
    }

    #[test]
    fn tech_identifiers_and_ruslish_are_kept() {
        for w in ["struct", "rustc", "actix", "stdout", "winres", "ngrams", "Inno", "serde"] {
            assert_eq!(suggest(w, Lang::En), None, "{w}");
        }
        for w in ["парсит", "логируй", "пушь", "токенов", "разрулю", "мерж"] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
        for w in ["загран", "универ", "препод", "инфа", "проц", "видос"] {
            assert!(dict::get(Lang::Ru).is_word(w), "{w}");
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
        assert!(is_ruslish_form("парсит"));
        assert!(is_ruslish_form("логируй"));
        assert!(looks_like_identifier("winRes") || looks_like_identifier("some_var"));
        assert!(looks_like_identifier("user_id"));
        assert!(!looks_like_identifier("struct"));
        assert!(!looks_like_identifier("Bizon"));
    }


    #[test]
    fn titlecase_names_are_not_fixed_mid_sentence() {
        let mid = Context::of(&["сказал"]);
        assert_eq!(suggest_ctx("Bizon", Lang::En, Level::Normal, &mid), None);
        assert_eq!(suggest_ctx("Fadil", Lang::En, Level::Normal, &mid), None);
        assert_eq!(suggest_ctx("Стардаста", Lang::Ru, Level::Normal, &mid), None);
        // Isolated / sentence-start typo of a frequent word is unchanged (not tested here).
        // Mid-sentence is the conservative path for names.
    }

    #[test]
    fn hand_shift_recovers_home_row_misplacement() {
        assert_eq!(suggest("ротань", Lang::Ru).as_deref(), Some("привет"));
        let left = crate::layout::shift_token("привет", Lang::Ru, Lang::Ru, -1).unwrap();
        assert_eq!(suggest(&left, Lang::Ru).as_deref(), Some("привет"));
        assert_eq!(suggest("jr;;p", Lang::En).as_deref(), Some("hello"));
        let en_left = crate::layout::shift_token("hello", Lang::En, Lang::En, -1).unwrap();
        assert_eq!(suggest(&en_left, Lang::En).as_deref(), Some("hello"));
        // Combined with the other layout: typo::suggest stays in the given language;
        // the engine converts. Same-layout wrong direction is not a word.
        assert_eq!(suggest("окжжз", Lang::Ru), None);
        assert_eq!(suggest("hjnfym", Lang::En), None);
        assert_eq!(hand_shift_fix("окжжз", Lang::Ru, Level::Normal, None), Some(("hello".into(), Lang::En)));
        assert_eq!(hand_shift_fix("hjnfym", Lang::En, Level::Normal, None), Some(("привет".into(), Lang::Ru)));
        assert_eq!(suggest_level("ротань", Lang::Ru, Level::Cautious), None);
        assert_eq!(suggest_level("jr;;p", Lang::En, Level::Cautious), None);
        // Known words and earlier typo / clipping cases stay.
        assert_eq!(suggest("привет", Lang::Ru), None);
        assert_eq!(suggest("hello", Lang::En), None);
        assert_eq!(suggest("загран", Lang::Ru), None);
        assert_eq!(suggest("struct", Lang::En), None);
        assert_eq!(suggest("програма", Lang::Ru).as_deref(), Some("программа"));
        assert_eq!(suggest("превт", Lang::Ru).as_deref(), Some("привет"));
        // Short or one-off slips are not a whole-hand shift.
        assert_eq!(hand_shift_fix("рот", Lang::Ru, Level::Normal, None), None);
        assert_eq!(hand_shift_which("ротань", "привет"), Some((-1, Lang::Ru)));
        assert_eq!(hand_shift_which("jr;;p", "hello"), Some((-1, Lang::En)));
    }


    #[test]
    fn neighbour_subst_beats_non_neighbour() {
        // ш↔г are neighbours on ЙЦУКЕН; у↔ы is only a vowel/sound-alike slip.
        assert_eq!(suggest("услушу", Lang::Ru).as_deref(), Some("услугу"));
        assert_eq!(suggest_level("услушу", Lang::Ru, Level::Aggressive).as_deref(), Some("услугу"));
        assert_eq!(suggest_level("услушу", Lang::Ru, Level::Normal).as_deref(), Some("услугу"));
        // A unique same-row neighbour substitution is a strict one-edit fix, so
        // Cautious applies it too.
        assert_eq!(suggest_level("услушу", Lang::Ru, Level::Cautious).as_deref(), Some("услугу"));
        assert_eq!(suggest("Услушу", Lang::Ru).as_deref(), Some("Услугу"));
    }

}
