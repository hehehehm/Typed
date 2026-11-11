//! Lightweight Russian inflection: the frequency list has only some word forms
//! (`коррекция` but not `коррекцию`). A form counts as known when the same stem with
//! another ending of the same paradigm is in the dictionary.
//!
//! No real morphology: paradigms are lists of endings that alternate in one declension
//! or conjugation, each limited to stems ending in the right kind of letter (`точк-и`
//! but `стол-ы`), so endings of different word classes never mix: `систем-ой` (noun)
//! does not make `систем-ым` (adjective) a word.
//!
//! Evidence is counted in "siblings": a dictionary form with a telling ending (two+
//! letters, one word class: `-ить`, `-ый`, `-ией`) counts 2, any other counts 1.
//! - [`is_known_form`] protects what the user typed from being "fixed" (2 points);
//! - [`candidate_rank`] decides whether a fix may produce a form that is not in the
//!   list: 2 points and a 5+ letter stem.

use crate::dict::Dictionary;

/// Shortest stem for a fix that produces a form missing from the list.
pub const MIN_STEM: usize = 5;
/// Shortest stem at all (with two siblings).
const MIN_STEM_SHORT: usize = 3;
/// Shorter stems may only improve the rank of a word that is itself in the list
/// (`точкой`, rare in subtitles, gets the rank of `точка`).
const MIN_STEM_DIRECT: usize = 4;
/// Points (a telling ending 2, another 1, the dictionary form +1) an unlisted fix needs.
const STRICT_POINTS: usize = 3;

#[derive(Clone, Copy)]
enum StemEnd {
    Any,
    /// Hard consonant other than к г х ж ш ч щ ц й.
    Hard,
    /// к г х
    Velar,
    /// ж ш ч щ ц
    Hush,
    /// Any consonant but й.
    Consonant,
    /// Consonant that is not ж ш ч щ.
    NotHush,
    /// Vowel.
    Vowel,
    /// Vowel other than и (`музе-й`, but `истори-й` belongs to `история`).
    VowelNotI,
}

const VOWELS: &str = "аеёиоуыэюя";

impl StemEnd {
    fn ok(self, last: char) -> bool {
        let vowel = VOWELS.contains(last);
        let consonant = !vowel && last != 'й' && last != 'ь' && last != 'ъ';
        match self {
            StemEnd::Any => true,
            StemEnd::Hard => consonant && !"кгхжшчщц".contains(last),
            StemEnd::Velar => "кгх".contains(last),
            StemEnd::Hush => "жшчщц".contains(last),
            StemEnd::Consonant => consonant,
            StemEnd::NotHush => consonant && !"жшчщ".contains(last),
            StemEnd::Vowel => vowel,
            StemEnd::VowelNotI => vowel && last != 'и',
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Noun,
    Adj,
    Verb,
}

struct Group {
    class: Class,
    stem: StemEnd,
    ends: &'static [&'static str],
}

const fn n(stem: StemEnd, ends: &'static [&'static str]) -> Group {
    Group { class: Class::Noun, stem, ends }
}
const fn a(stem: StemEnd, ends: &'static [&'static str]) -> Group {
    Group { class: Class::Adj, stem, ends }
}
const fn v(stem: StemEnd, ends: &'static [&'static str]) -> Group {
    Group { class: Class::Verb, stem, ends }
}

/// An ending that identifies the paradigm well enough for one sibling to back a new
/// form: two+ letters, one word class. Bare stems (`ошибок` is a plural, not a
/// masculine noun) and one-letter endings (`ошибался` is not a noun in -я) are too weak.
fn strong_ending(end: &str, last: char, class: Class) -> bool {
    end.chars().count() >= 2 && only_class(end, last, class)
}

/// `end` after a stem ending in `last` belongs to word class `class` only
/// (`-ый` is always an adjective; `-ой` may be a noun or an adjective).
fn only_class(end: &str, last: char, class: Class) -> bool {
    GROUPS.iter().filter(|g| g.stem.ok(last) && g.ends.contains(&end)).all(|g| g.class == class)
}

use StemEnd::*;

/// Paradigms ("" = bare stem, e.g. `стол`, `слов`); the first ending of each is its
/// dictionary form.
const GROUPS: &[Group] = &[
    // nouns in -ия / -ие (коррекция, коррекцию…; приложение, приложений…)
    n(Consonant, &["ия", "ии", "ию", "ией", "иею", "ий", "иям", "иями", "иях"]),
    n(Consonant, &["ие", "ия", "ию", "ием", "ии", "ий", "иям", "иями", "иях"]),
    // feminine -а (система, системы…), velar (точка, точки…), hushing (задача…)
    n(Hard, &["а", "ы", "е", "у", "ой", "ою", "ам", "ами", "ах"]),
    n(Velar, &["а", "и", "е", "у", "ой", "ою", "ам", "ами", "ах"]),
    n(Hush, &["а", "и", "е", "у", "ой", "ей", "ею", "ам", "ами", "ах"]),
    // feminine -я (неделя…) and -ь (тетрадь…)
    n(Consonant, &["я", "и", "е", "ю", "ей", "ею", "ям", "ями", "ях"]),
    n(Consonant, &["ь", "и", "ью", "ей", "ям", "ями", "ях"]),
    // masculine hard (файл, файла…), velar (список, кнопок…), hushing (падеж…)
    n(Hard, &["", "а", "у", "ом", "е", "ы", "ов", "ам", "ами", "ах"]),
    n(Velar, &["", "а", "у", "ом", "е", "и", "ов", "ам", "ами", "ах"]),
    n(Hush, &["", "а", "у", "ом", "ем", "е", "и", "ей", "ев", "ам", "ами", "ах"]),
    // masculine soft (пароль…) and -й (музей…)
    n(Consonant, &["ь", "я", "ю", "ем", "е", "и", "ей", "ям", "ями", "ях"]),
    n(VowelNotI, &["й", "я", "ю", "ем", "е", "и", "ев", "ям", "ями", "ях"]),
    // neuter -о (слово, окно…) and soft -е (поле…)
    n(Hard, &["о", "", "а", "у", "ом", "е", "ам", "ами", "ах"]),
    n(Velar, &["о", "", "а", "у", "ом", "е", "и", "ам", "ами", "ах"]),
    n(Consonant, &["е", "я", "ю", "ем", "ей", "ям", "ями", "ях"]),
    // adjectives / participles: hard, velar (русский), hushing (хороший), soft (последний)
    a(Hard, &["ый", "ой", "ая", "ое", "ые", "ого", "ому", "ым", "ую", "ых", "ыми", "ом", "ою"]),
    a(Velar, &["ий", "ой", "ая", "ое", "ие", "ого", "ому", "им", "ую", "их", "ими", "ом", "ою"]),
    a(Hush, &["ий", "ая", "ее", "ие", "его", "ему", "им", "ую", "их", "ими", "ей", "ем"]),
    a(Hush, &["ой", "ая", "ое", "ие", "ого", "ому", "им", "ую", "их", "ими", "ом"]),
    a(Consonant, &["ий", "яя", "ее", "ие", "его", "ему", "им", "юю", "их", "ими", "ей", "ем"]),
    // verbs in -ить (оценить: оцени, оценю, оценишь, оценил…), hushing stems take у/ат
    v(NotHush, &["ить", "ю", "ишь", "ит", "им", "ите", "ят", "ил", "ила", "ило", "или", "и", "ь", "ьте", "ящий", "ивший", "енный"]),
    v(Hush, &["ить", "у", "ишь", "ит", "им", "ите", "ат", "ил", "ила", "ило", "или", "и", "ь", "ьте", "ащий", "ивший", "енный"]),
    // verbs in -еть (смотреть: смотрю, смотришь…; уметь: умею…)
    v(Consonant, &["еть", "ю", "у", "ишь", "ит", "им", "ите", "ят", "ат", "ел", "ела", "ело", "ели", "и"]),
    v(Consonant, &["еть", "ею", "еешь", "еет", "еем", "еете", "еют", "ел", "ела", "ело", "ели", "ей", "ейте"]),
    // verbs in -ать / -ять (делать: делай, делаю…; проверять)
    v(Any, &["ать", "аю", "аешь", "ает", "аем", "аете", "ают", "ал", "ала", "ало", "али", "ай", "айте", "ая", "ающий", "авший", "анный"]),
    v(Any, &["ять", "яю", "яешь", "яет", "яем", "яете", "яют", "ял", "яла", "яло", "яли", "яй", "яйте", "яя", "яющий", "явший", "янный"]),
    // verbs in -овать / -евать (рисовать: рисую…)
    v(Any, &["овать", "ую", "уешь", "ует", "уем", "уете", "уют", "овал", "овала", "овало", "овали", "уй", "уйте"]),
    v(Any, &["евать", "юю", "юешь", "юет", "юем", "юете", "юют", "евал", "евала", "евало", "евали", "юй", "юйте"]),
    // verbs in -нуть (нажать is irregular; толкнуть: толкну, толкнешь…)
    v(Any, &["нуть", "ну", "нешь", "нет", "нем", "нете", "нут", "нул", "нула", "нуло", "нули", "ни", "ните"]),
    // present stems of the first conjugation (разбер-у, разбер-ешь…; откро-ю, откро-ешь…)
    v(Consonant, &["у", "ешь", "ет", "ем", "ете", "ут", "и", "ите"]),
    v(Vowel, &["ю", "ешь", "ет", "ем", "ете", "ют", "й", "йте"]),
];

/// Best evidence that `w` (normalised) is `stem + ending` of some paradigm:
/// `(siblings found, best sibling rank, stem length)`, the most siblings first.
/// A sibling whose ending names the word class unambiguously (`системный` for
/// `системным`) counts as two.
fn evidence(d: &Dictionary, w: &str) -> Option<(usize, u32, usize)> {
    let chars: Vec<char> = w.chars().collect();
    let mut best: Option<(usize, u32, usize)> = None;
    for group in GROUPS {
        for end in group.ends {
            let n = end.chars().count();
            if chars.len() < MIN_STEM_SHORT + n {
                continue;
            }
            let (stem, tail) = chars.split_at(chars.len() - n);
            if tail.iter().collect::<String>() != *end || !group.stem.ok(stem[stem.len() - 1]) {
                continue;
            }
            let stem_s: String = stem.iter().collect();
            if !d.has_prefix(&stem_s) {
                continue;
            }
            let mut count = 0;
            let mut rank = u32::MAX;
            for other in group.ends.iter().filter(|o| *o != end) {
                if group.class != Class::Verb && looks_infinitive(&stem_s, other) {
                    continue;
                }
                let r = sibling_rank(d, &stem_s, other);
                if let Some(r) = r {
                    count += if strong_ending(other, stem[stem.len() - 1], group.class) { 2 } else { 1 };
                    rank = rank.min(r);
                }
            }
            if count == 0 {
                continue;
            }
            let e = (count, rank, stem.len());
            best = Some(match best {
                Some(b) if (b.0, std::cmp::Reverse(b.1)) >= (e.0, std::cmp::Reverse(e.1)) => b,
                _ => e,
            });
        }
    }
    best
}

/// Also try the word without the reflexive `-ся`/`-сь` (исправился → исправил).
fn evidence_any(d: &Dictionary, w: &str) -> Option<(usize, u32, usize)> {
    let mut best = evidence(d, w);
    for refl in ["ся", "сь"] {
        if let Some(base) = w.strip_suffix(refl) {
            if base.chars().count() >= 4 {
                let direct = d.rank(base).map(|r| (2, r, base.chars().count()));
                for e in [direct, evidence(d, base)].into_iter().flatten() {
                    if best.is_none_or(|b| e.0 > b.0) {
                        best = Some(e);
                    }
                }
            }
        }
    }
    best
}

/// Every ending of every paradigm, "" included, sorted and deduplicated.
fn all_endings() -> &'static [&'static str] {
    static ENDS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    ENDS.get_or_init(|| {
        let mut v: Vec<&str> = GROUPS.iter().flat_map(|g| g.ends.iter().copied()).collect();
        v.sort_unstable();
        v.dedup();
        v
    })
}

fn is_ending(s: &str) -> bool {
    all_endings().binary_search(&s).is_ok()
}

/// `stem + end` ends like an infinitive (`-ть`, `-ти`, `-чь`), so it backs no noun.
fn looks_infinitive(stem: &str, end: &str) -> bool {
    let f = format!("{stem}{end}");
    f.ends_with("ть") || f.ends_with("ти") || f.ends_with("чь")
}

/// Best rank of `stem + end`, also as a reflexive verb (`разберу` is backed by
/// `разберусь`).
fn sibling_rank(d: &Dictionary, stem: &str, end: &str) -> Option<u32> {
    let mut f = String::with_capacity(stem.len() + end.len() + 4);
    f.push_str(stem);
    f.push_str(end);
    let mut best = d.rank_exact(&f);
    let n = f.len();
    for refl in ["ся", "сь"] {
        f.truncate(n);
        f.push_str(refl);
        if let Some(r) = d.rank_exact(&f) {
            best = Some(best.map_or(r, |b| b.min(r)));
        }
    }
    best
}

/// `a` and `b` share a 4+ letter stem and differ only in paradigm endings
/// (`удаляю` / `удалю`, `файлами` / `файлам`): two forms, not a typo and its fix.
pub fn only_ending_differs(a: &str, b: &str) -> bool {
    let (av, bv): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let common = av.iter().zip(&bv).take_while(|(x, y)| x == y).count();
    (4..=common).any(|k| {
        let ta: String = av[k..].iter().collect();
        let tb: String = bv[k..].iter().collect();
        ta != tb && is_ending(&ta) && is_ending(&tb)
    })
}

/// `word` (any case) is a Russian dictionary word or a plausible form of one: the typed
/// word must not be "fixed". Needs one sibling with a telling ending (`оценить` for
/// `оцени`) or two with short ones; any stem of 3+ letters.
pub fn is_known_form(d: &Dictionary, word: &str) -> bool {
    let w = Dictionary::normalize(word);
    d.is_word(&w)
        || d.is_word(&format!("{w}ся"))
        || d.is_word(&format!("{w}сь"))
        || evidence_any(d, &w).is_some_and(|(n, _, _)| n >= 2)
}

/// Rank to use for a fix candidate (normalised): listed words get the best rank among
/// their forms; unlisted forms count only with [`STRICT_POINTS`] sibling points in one
/// paradigm and a 4+ letter stem, so `системым` (only `системой` backs it) is rejected.
pub fn candidate_rank(d: &Dictionary, w: &str) -> Option<u32> {
    match d.rank(w) {
        // A listed form keeps its own rank unless it is in the rarer half of the list,
        // where the best rank of its forms counts instead.
        Some(r) if r < crate::typo::TAIL_RANK => Some(r),
        Some(r) => Some(match evidence(d, w) {
            Some((_, sr, stem)) if stem >= MIN_STEM_DIRECT => r.min(sr),
            _ => r,
        }),
        None => paradigm_rank(d, w).or_else(|| {
            let base = w.strip_suffix("ся").or_else(|| w.strip_suffix("сь"))?;
            d.rank(base).or_else(|| paradigm_rank(d, base))
        }),
    }
}

/// Rank for an unlisted form `w` = stem + ending when the paradigm that explains the
/// stem's dictionary forms best (most sibling points, at least [`STRICT_POINTS`])
/// has that ending (all of them, on a tie). `автобус-ой` fails: `автобус, автобуса…` fit the masculine
/// paradigm, which has `-ом`, better than the feminine one.
fn paradigm_rank(d: &Dictionary, w: &str) -> Option<u32> {
    let chars: Vec<char> = w.chars().collect();
    let mut best: Option<u32> = None;
    for &end in all_endings() {
        let n = end.chars().count();
        if chars.len() < MIN_STEM + n {
            continue;
        }
        let (stem, tail) = chars.split_at(chars.len() - n);
        if tail.iter().collect::<String>() != end {
            continue;
        }
        let last = stem[stem.len() - 1];
        let stem_s: String = stem.iter().collect();
        if !d.has_prefix(&stem_s) {
            continue;
        }
        let mut top = 0;
        let mut winners: Vec<(bool, u32)> = Vec::new();
        for group in GROUPS.iter().filter(|g| g.stem.ok(last)) {
            let mut score = 0;
            let mut rank = u32::MAX;
            for (i, other) in group.ends.iter().enumerate().filter(|(_, o)| **o != end) {
                // `настроит-ь` is a verb, not a noun in -ь.
                if group.class != Class::Verb && looks_infinitive(&stem_s, other) {
                    continue;
                }
                if let Some(r) = sibling_rank(d, &stem_s, other) {
                    score += if strong_ending(other, last, group.class) { 2 } else { 1 };
                    // The dictionary form (first ending: `коррекция`, `лесбиянка`)
                    // tells the paradigm best.
                    if i == 0 && !other.is_empty() {
                        score += 1;
                    }
                    rank = rank.min(r);
                }
            }
            if score == 0 || score < top {
                continue;
            }
            if score > top {
                top = score;
                winners.clear();
            }
            // Verb forms are only recognised, never produced: their endings depend on
            // stress and conjugation (`спрос-ьте`, `присл-ает` are not words).
            winners.push((group.class != Class::Verb && group.ends.contains(&end), rank));
        }
        if top >= STRICT_POINTS {
            // Every best-fitting paradigm must have the ending: `лесбиянк-о` fails, the
            // feminine paradigm fits as well as the neuter one and has no `-о`.
            if winners.iter().all(|(has, _)| *has) {
                let r = winners.iter().map(|(_, r)| *r).min().unwrap_or(u32::MAX);
                best = Some(best.map_or(r, |b| b.min(r)));
            }
        }
    }
    best
}

/// Rank of `word` as a dictionary word or a known form of one (lenient), for callers
/// that only ask "is this Russian".
pub fn form_rank(d: &Dictionary, word: &str) -> Option<u32> {
    let w = Dictionary::normalize(word);
    if let Some(r) = d.rank(&w) {
        return Some(r);
    }
    if !is_known_form(d, &w) {
        return None;
    }
    evidence_any(d, &w).map(|e| e.1)
}

/// Same word with the typed word's ending: if `cand` = stem + ending of a paradigm and
/// `typed` ends with another ending of that paradigm, return stem + typed ending.
/// Used to keep the case of a typo fix (`коррекция` + typed `…цию` → `коррекцию`).
///
/// A form not in the list is usable only when `cand`'s ending belongs to that one
/// paradigm for this stem (`системный`: only adjectives end in -ый) or the form has
/// [`STRICT_POINTS`] sibling points: `системой` (noun or adjective?) must not give `системым`.
/// Returns `(form, safe)`; an unsafe form still needs [`reinflect_ok`].
pub fn reinflect(cand: &str, typed: &str) -> Vec<(String, bool)> {
    let c: Vec<char> = cand.chars().collect();
    let mut out = Vec::new();
    for group in GROUPS.iter().filter(|g| g.class != Class::Verb) {
        for end in group.ends {
            let n = end.chars().count();
            if c.len() < MIN_STEM + n || !cand.ends_with(end) || !group.stem.ok(c[c.len() - n - 1]) {
                continue;
            }
            let last = c[c.len() - n - 1];
            if cand.ends_with("ся") || cand.ends_with("сь") {
                return out;
            }
            let stem: String = c[..c.len() - n].iter().collect();
            for other in group.ends.iter().filter(|o| *o != end && !o.is_empty()) {
                if typed.ends_with(other) {
                    // Safe when every paradigm that `cand`'s ending may belong to has the
                    // new ending: `-ый` → `-ым` always; `-ой` (noun or adjective) not.
                    let safe = end.chars().count() >= 2
                        && GROUPS
                            .iter()
                            .filter(|g| g.stem.ok(last) && g.ends.contains(end))
                            .all(|g| g.ends.contains(other));
                    let f = format!("{stem}{other}");
                    if !out.iter().any(|(o, _)| *o == f) {
                        out.push((f, safe));
                    }
                }
            }
        }
    }
    out
}

/// Check for a `(form, safe)` pair from [`reinflect`].
pub fn reinflect_ok(d: &Dictionary, form: &str, safe: bool) -> bool {
    safe || d.contains(form) || candidate_rank(d, form).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict;
    use crate::layout::Lang;

    #[test]
    fn forms_of_known_words() {
        let d = dict::get(Lang::Ru);
        assert!(!d.contains("коррекцию"));
        assert!(is_known_form(d, "коррекцию"));
        assert!(is_known_form(d, "коррекцией"));
        assert!(candidate_rank(d, "точкой").unwrap() < 5000, "via точка");
        assert!(!is_known_form(d, "кореркцию"));
        assert!(!is_known_form(d, "чтокой"));
        // Verb forms: imperative and others of оценить.
        for w in ["оцени", "оцените", "оценю", "проверь", "исправьте"] {
            assert!(is_known_form(d, w), "{w}");
        }
    }

    #[test]
    fn classes_do_not_mix() {
        let d = dict::get(Lang::Ru);
        // система is a noun: no adjective ending on its stem.
        assert_eq!(candidate_rank(d, "системым"), None);
        assert_eq!(candidate_rank(d, "удалиите"), None);
        assert_eq!(candidate_rank(d, "посмеу"), None);
    }

    #[test]
    fn reinflect_keeps_ending() {
        let d = dict::get(Lang::Ru);
        let ok = |cand: &str, typed: &str, f: &str| {
            reinflect(cand, typed).into_iter().any(|(x, safe)| x == f && reinflect_ok(d, &x, safe))
        };
        assert!(ok("коррекция", "кореркцию", "коррекцию"));
        assert!(ok("программа", "прграммой", "программой"));
        assert!(ok("системный", "ситстемрым", "системным"));
        // системой may be a noun: no adjective form from it.
        assert!(!ok("системой", "ситстемрым", "системым"));
    }
}
