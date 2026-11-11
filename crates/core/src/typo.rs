//! Offline typo fixing: one edit (extra, missing, wrong or two swapped neighbour letters)
//! away from a frequent dictionary word of the same language.
//!
//! Conservative on purpose: words of 4+ letters only, natural casing only, and only when
//! one candidate clearly wins (the only one, or far more frequent than the runner-up).
//! Fix targets come from the whole 40 000-word list; the rarer half wins only when the
//! slip keeps the last two letters.
//! Two edits are tried only for long words (8+ letters) that have no one-edit candidate.

use crate::dict::{self, Dictionary};
use crate::layout::Lang;
use crate::morph;

/// Minimum letters in the typed word.
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

const EN_ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";
const RU_ALPHABET: &str = "абвгдежзийклмнопрстуфхцчшщъыьэюя";

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
fn match_case(typed: &str, fix: &str) -> String {
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
    let d = dict::get(lang);
    suggest_in(d, word, lang)
}

pub fn suggest_in(d: &Dictionary, word: &str, lang: Lang) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < MIN_LEN || !chars.iter().all(|c| c.is_alphabetic()) || d.is_word(word) {
        return None;
    }
    let lower_s = Dictionary::normalize(word);
    // A Russian form missing from the list but inflected from a known word
    // (`коррекцию` from `коррекция`) is not a typo.
    if lang == Lang::Ru && morph::is_known_form(d, &lower_s) {
        return None;
    }
    // English forms missing from the list (`typos`, `installer`, `uninstall`).
    if lang == Lang::En && en_known_form(d, &lower_s) {
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
    for cand in edits1(&lower, &alphabet) {
        if cand.chars().count() < MIN_LEN - 1 {
            continue;
        }
        // A wrong letter counts only when its key is next to the right one, or (6+
        // letters) for vowel / voiced-voiceless slips: `оцени` must not become `олени`
        // (ц and л are far apart), `recieve` not `relieve`.
        if !substitution_ok(&lower, &cand, lang) || !first_letter_kept(&lower, &cand) {
            continue;
        }
        // Never change just the ending of a form: `удаляю` / `удалю`, `файлами` / `файлам`.
        if lang == Lang::Ru && morph::only_ending_differs(&lower_s, &cand) {
            continue;
        }
        if lang == Lang::En && (en_affix_only(&lower_s, &cand) || en_end_letter_only(&lower_s, &cand)) {
            continue;
        }
        if let Some(r) = rank(&cand) {
            // Two neighbour letters swapped is the most common slip: such a fix counts
            // as SWAP_BONUS times more frequent (`компьютре` → `компьютер`, not `компьютере`).
            let ok = can_win(r, &cand);
            let r = if is_swap(&lower, &cand) { r / SWAP_BONUS } else { r };
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
    // letters (`влкдаки` → `вкладки`: same letters, just shuffled).
    if pick.is_empty() && lower.len() >= MIN_LEN_2 - 1 {
        let swaps_only = lower.len() < MIN_LEN_2;
        let mut sorted_typed = lower.clone();
        sorted_typed.sort_unstable();
        let same_letters = |f: &[char]| {
            let mut v = f.to_vec();
            v.sort_unstable();
            v == sorted_typed
        };
        for (cand, r) in d.iter() {
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
                        let ok = can_win(r, &f);
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
                pick.offer(r, cand.to_string(), can_win(r, cand));
            }
        }
    }
    let w = pick.winner()?;
    Some(match_case(word, &w))
}

/// A rarer candidate (rank [`TAIL_RANK`]+) must keep the typed last two letters: the
/// slip is inside the word (`ярдык` → `ярлык`), while a changed or dropped ending turns
/// unknown names and forms into other rare words (`айса` → `айс`, `minut` → `mint`).
fn tail_ok(typed: &[char], cand: &str) -> bool {
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
fn en_affix_only(typed: &str, cand: &str) -> bool {
    const SUFFIXES: [&str; 14] = ["s", "es", "d", "ed", "er", "ers", "al", "ly", "an", "ian", "ing", "t", "y", "ness"];
    let (long, short) = if typed.len() > cand.len() { (typed, cand) } else { (cand, typed) };
    long.strip_prefix(short).is_some_and(|tail| SUFFIXES.contains(&tail))
}

/// English `cand` only adds or drops one letter at the very end (`realme` / `realm`,
/// `apple` / `appl`): usually another word (a brand, a name, a form), not a slip. The
/// Russian counterpart is [`morph::only_ending_differs`]. A doubled last letter
/// (`helloo` → `hello`) is still a slip and may be fixed.
fn en_end_letter_only(typed: &str, cand: &str) -> bool {
    let (t, c): (Vec<char>, Vec<char>) = (typed.chars().collect(), cand.chars().collect());
    if t.len() == c.len() + 1 && t.starts_with(&c) {
        return t[t.len() - 1] != t[t.len() - 2];
    }
    c.len() == t.len() + 1 && c.starts_with(&t)
}

/// The fix starts with the typed first letter, or swaps the first two: slips at the
/// very start are rare, while `упускать` / `пускать`, `входной` / `выходной`,
/// `ренди` / `бренди` are different words.
fn first_letter_kept(typed: &[char], cand: &str) -> bool {
    let cv: Vec<char> = cand.chars().collect();
    match (cv.first(), typed.first()) {
        (Some(a), Some(b)) if a == b => true,
        _ => cv.len() == typed.len() && cv.len() >= 2 && cv[0] == typed[1] && cv[1] == typed[0],
    }
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

/// Words up to this length accept a substituted letter only from a neighbouring key;
/// longer ones also a vowel for a vowel or a voiced / voiceless pair.
pub const SHORT_SUBST_LEN: usize = 5;

const RU_VOWELS: &str = "аеиоуыэюя";
const EN_VOWELS: &str = "aeiouy";
const RU_PAIRS: [(char, char); 8] = [('б', 'п'), ('в', 'ф'), ('г', 'к'), ('д', 'т'), ('ж', 'ш'), ('з', 'с'), ('ш', 'щ'), ('ь', 'ъ')];

fn sound_alike(a: char, b: char, lang: Lang) -> bool {
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
fn adjacent_keys(a: char, b: char, lang: Lang) -> bool {
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
struct Pick {
    suffix: Vec<char>,
    same: Ranked,
    other: Ranked,
}

#[derive(Default)]
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
        assert_eq!(suggest("teh", Lang::En), None); // too short
        assert_eq!(suggest("qzxvbn", Lang::En), None);
        // к→й is not a neighbour-key slip, so only the doubled letter is dropped.
        assert_eq!(suggest("опечаткок", Lang::Ru).as_deref(), Some("опечаток"));
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
    fn letter_two_places_off() {
        assert_eq!(suggest("чтокой", Lang::Ru).as_deref(), Some("точкой"));
        assert_eq!(suggest("чтокой", Lang::Ru).as_deref(), Some("точкой"));
        // Real words starting with «что» stay.
        for ok in ["чтобы", "что-то", "чтоб"] {
            assert_eq!(suggest(ok, Lang::Ru), None, "{ok}");
        }
    }
}
