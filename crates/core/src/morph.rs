
use crate::dict::Dictionary;

pub const MIN_STEM: usize = 5;
const MIN_STEM_SHORT: usize = 3;
const MIN_STEM_DIRECT: usize = 4;
const STRICT_POINTS: usize = 3;

#[derive(Clone, Copy)]
enum StemEnd {
    Any,
    Hard,
    Velar,
    Hush,
    Consonant,
    NotHush,
    Vowel,
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

fn strong_ending(end: &str, last: char, class: Class) -> bool {
    end.chars().count() >= 2 && only_class(end, last, class)
}

fn only_class(end: &str, last: char, class: Class) -> bool {
    GROUPS.iter().filter(|g| g.stem.ok(last) && g.ends.contains(&end)).all(|g| g.class == class)
}

use StemEnd::*;

const GROUPS: &[Group] = &[
    n(Consonant, &["ия", "ии", "ию", "ией", "иею", "ий", "иям", "иями", "иях"]),
    n(Consonant, &["ие", "ия", "ию", "ием", "ии", "ий", "иям", "иями", "иях"]),
    n(Hard, &["а", "ы", "е", "у", "ой", "ою", "ам", "ами", "ах"]),
    n(Velar, &["а", "и", "е", "у", "ой", "ою", "ам", "ами", "ах"]),
    n(Hush, &["а", "и", "е", "у", "ой", "ей", "ею", "ам", "ами", "ах"]),
    n(Consonant, &["я", "и", "е", "ю", "ей", "ею", "ям", "ями", "ях"]),
    n(Consonant, &["ь", "и", "ью", "ей", "ям", "ями", "ях"]),
    n(Hard, &["", "а", "у", "ом", "е", "ы", "ов", "ам", "ами", "ах"]),
    n(Velar, &["", "а", "у", "ом", "е", "и", "ов", "ам", "ами", "ах"]),
    n(Hush, &["", "а", "у", "ом", "ем", "е", "и", "ей", "ев", "ам", "ами", "ах"]),
    n(Consonant, &["ь", "я", "ю", "ем", "е", "и", "ей", "ям", "ями", "ях"]),
    n(VowelNotI, &["й", "я", "ю", "ем", "е", "и", "ев", "ям", "ями", "ях"]),
    n(Hard, &["о", "", "а", "у", "ом", "е", "ам", "ами", "ах"]),
    n(Velar, &["о", "", "а", "у", "ом", "е", "и", "ам", "ами", "ах"]),
    n(Consonant, &["е", "я", "ю", "ем", "ей", "ям", "ями", "ях"]),
    a(Hard, &["ый", "ой", "ая", "ое", "ые", "ого", "ому", "ым", "ую", "ых", "ыми", "ом", "ою"]),
    a(Velar, &["ий", "ой", "ая", "ое", "ие", "ого", "ому", "им", "ую", "их", "ими", "ом", "ою"]),
    a(Hush, &["ий", "ая", "ее", "ие", "его", "ему", "им", "ую", "их", "ими", "ей", "ем"]),
    a(Hush, &["ой", "ая", "ое", "ие", "ого", "ому", "им", "ую", "их", "ими", "ом"]),
    a(Consonant, &["ий", "яя", "ее", "ие", "его", "ему", "им", "юю", "их", "ими", "ей", "ем"]),
    v(NotHush, &["ить", "ю", "ишь", "ит", "им", "ите", "ят", "ил", "ила", "ило", "или", "и", "ь", "ьте", "ящий", "ивший", "енный"]),
    v(Hush, &["ить", "у", "ишь", "ит", "им", "ите", "ат", "ил", "ила", "ило", "или", "и", "ь", "ьте", "ащий", "ивший", "енный"]),
    v(Consonant, &["еть", "ю", "у", "ишь", "ит", "им", "ите", "ят", "ат", "ел", "ела", "ело", "ели", "и"]),
    v(Consonant, &["еть", "ею", "еешь", "еет", "еем", "еете", "еют", "ел", "ела", "ело", "ели", "ей", "ейте"]),
    v(Any, &["ать", "аю", "аешь", "ает", "аем", "аете", "ают", "ал", "ала", "ало", "али", "ай", "айте", "ая", "ающий", "авший", "анный"]),
    v(Any, &["ять", "яю", "яешь", "яет", "яем", "яете", "яют", "ял", "яла", "яло", "яли", "яй", "яйте", "яя", "яющий", "явший", "янный"]),
    v(Any, &["овать", "ую", "уешь", "ует", "уем", "уете", "уют", "овал", "овала", "овало", "овали", "уй", "уйте"]),
    v(Any, &["евать", "юю", "юешь", "юет", "юем", "юете", "юют", "евал", "евала", "евало", "евали", "юй", "юйте"]),
    v(Any, &["нуть", "ну", "нешь", "нет", "нем", "нете", "нут", "нул", "нула", "нуло", "нули", "ни", "ните"]),
    v(Consonant, &["у", "ешь", "ет", "ем", "ете", "ут", "и", "ите"]),
    v(Vowel, &["ю", "ешь", "ет", "ем", "ете", "ют", "й", "йте"]),
];

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

fn looks_infinitive(stem: &str, end: &str) -> bool {
    let f = format!("{stem}{end}");
    f.ends_with("ть") || f.ends_with("ти") || f.ends_with("чь")
}

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

pub fn only_ending_differs(a: &str, b: &str) -> bool {
    let (av, bv): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let common = av.iter().zip(&bv).take_while(|(x, y)| x == y).count();
    (4..=common).any(|k| {
        let ta: String = av[k..].iter().collect();
        let tb: String = bv[k..].iter().collect();
        ta != tb && is_ending(&ta) && is_ending(&tb)
    })
}

const MAT_STEMS: &[&str] = &[
    "еб", "еба", "ебн", "ебуч", "ебл", "ебан",
    "бляд", "бля", "блят",
    "хуй", "хуя", "хуе", "хуи", "хуйн", "хуев",
    "пизд", "пизж",
    "сук", "суч",
    "муд", "мудак", "мудо",
    "гандон", "гондон",
    "пидор", "пидар", "пидр", "пидарас", "пидорас",
    "хер", "херо", "херов",
    "жоп", "говн", "срак",
];

const MAT_PREFIXES: &[&str] = &[
    "недо", "пере", "раз", "рас", "под", "при", "про", "изо", "ото", "обо", "без",
    "из", "ис", "от", "за", "вы", "на", "до", "во", "со", "об", "по", "у", "с", "в", "о",
    "",
];

const MAT_EXTRA_ENDS: &[&str] = &[
    "аный", "аная", "аное", "аные", "аных", "аным", "аными", "аном", "аную",
    "еный", "еная", "еное", "еные",
    "ть", "ти", "чь", "ться", "тись",
    "ец", "ца", "цу", "цом", "це",
    "ня", "ни", "не", "ню", "ней", "нею", "ней",
    "ак", "ака", "аки", "аку", "аком", "аков", "аками",
    "ин", "ина", "ины", "ине", "ину",
    "ло", "ла", "лы",
    "ота", "оты", "оте",
    "ись", "ища", "ище", "ищи",
    "уч", "учий", "учая", "учка",
    "ный", "ная", "ное", "ные",
    "ывать", "ываю", "ываешь", "ывает", "ываем", "ываете", "ывают",
    "ывал", "ывала", "ывало", "ывали", "ывай", "ывайте", "ывая",
    "ивать", "иваю", "иваешь", "ивает", "иваем", "иваете", "ивают",
    "ивал", "ивала", "ивало", "ивали", "ивай",
];

fn mat_ending_ok(end: &str) -> bool {
    end.is_empty() || is_ending(end) || MAT_EXTRA_ENDS.contains(&end)
}

pub fn is_mat_form(word: &str) -> bool {
    let w = Dictionary::normalize(word);
    if w.chars().count() < 3 || !w.chars().all(|c| matches!(c, 'а'..='я' | 'ё') || c == 'ъ' || c == 'ь') {
        return false;
    }
    let bases = [
        w.as_str(),
        w.strip_suffix("ся").unwrap_or(w.as_str()),
        w.strip_suffix("сь").unwrap_or(w.as_str()),
    ];
    for base in bases {
        for prefix in MAT_PREFIXES {
            let Some(mut rest) = base.strip_prefix(prefix) else { continue };
            if rest.starts_with('ъ') || rest.starts_with('ь') {
                rest = &rest[rest.chars().next().unwrap().len_utf8()..];
            }
            if rest.is_empty() {
                continue;
            }
            if mat_stem_with_ending(rest) {
                return true;
            }
        }
    }
    false
}

fn mat_stem_with_ending(rest: &str) -> bool {
    for stem in MAT_STEMS {
        if let Some(end) = rest.strip_prefix(stem) {
            if !mat_ending_ok(end) {
                continue;
            }
            let stem_n = stem.chars().count();
            let end_n = end.chars().count();
            if stem_n <= 2 && end_n == 1 {
                continue;
            }
            return true;
        }
    }
    false
}

pub fn mat_forms() -> Vec<String> {
    let mut ends: Vec<&str> = all_endings().iter().copied().collect();
    ends.extend_from_slice(MAT_EXTRA_ENDS);
    ends.sort_unstable();
    ends.dedup();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for prefix in MAT_PREFIXES {
        for stem in MAT_STEMS {
            for end in &ends {
                for refl in ["", "ся", "сь"] {
                    if !refl.is_empty() && end.is_empty() {
                        continue;
                    }
                    if stem.chars().count() <= 2 && end.chars().count() == 1 {
                        continue;
                    }
                    let signs: &[&str] = if !prefix.is_empty()
                        && !"аеёиоуыэюя".contains(prefix.chars().last().unwrap_or(' '))
                        && "аеёиоуыэюя".contains(stem.chars().next().unwrap_or(' '))
                    {
                        &["", "ъ"]
                    } else {
                        &[""]
                    };
                    for s in signs {
                        let w = format!("{prefix}{s}{stem}{end}{refl}");
                        if w.chars().count() >= 3 && seen.insert(w.clone()) {
                            out.push(w);
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn is_known_form(d: &Dictionary, word: &str) -> bool {
    let w = Dictionary::normalize(word);
    d.is_word(&w)
        || d.is_word(&format!("{w}ся"))
        || d.is_word(&format!("{w}сь"))
        || is_mat_form(&w)
        || evidence_any(d, &w).is_some_and(|(n, _, _)| n >= 2)
}

pub fn candidate_rank(d: &Dictionary, w: &str) -> Option<u32> {
    match d.rank(w) {
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
                if group.class != Class::Verb && looks_infinitive(&stem_s, other) {
                    continue;
                }
                if let Some(r) = sibling_rank(d, &stem_s, other) {
                    score += if strong_ending(other, last, group.class) { 2 } else { 1 };
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
            winners.push((group.class != Class::Verb && group.ends.contains(&end), rank));
        }
        if top >= STRICT_POINTS {
            if winners.iter().all(|(has, _)| *has) {
                let r = winners.iter().map(|(_, r)| *r).min().unwrap_or(u32::MAX);
                best = Some(best.map_or(r, |b| b.min(r)));
            }
        }
    }
    best
}

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

pub fn max_end_len() -> usize {
    static MAX: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MAX.get_or_init(|| GROUPS.iter().filter(|g| g.class != Class::Verb).flat_map(|g| g.ends.iter()).map(|e| e.chars().count()).max().unwrap_or(0))
}

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
        for w in ["оцени", "оцените", "оценю", "проверь", "исправьте"] {
            assert!(is_known_form(d, w), "{w}");
        }
    }

    #[test]
    fn classes_do_not_mix() {
        let d = dict::get(Lang::Ru);
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
        assert!(!ok("системой", "ситстемрым", "системым"));
    }

    #[test]
    fn mat_forms_are_known_and_not_ordinary_words() {
        let d = dict::get(Lang::Ru);
        for w in [
            "заебался",
            "ЗАЕБАЛСЯ",
            "ебаный",
            "ЕБАНЫЙ",
            "отъебись",
            "охуеть",
            "нахуя",
            "похуй",
            "пиздец",
            "блядь",
            "блять",
            "сука",
            "мудак",
            "гандон",
            "пидор",
            "выебывается",
            "заебись",
            "распидорасило",
        ] {
            assert!(is_mat_form(w), "mat form {w}");
            assert!(is_known_form(d, w), "known {w}");
        }
        for w in ["забрался", "собрался", "мудрый", "мудрость", "хлеб", "небо", "характер"] {
            assert!(!is_mat_form(w), "false mat {w}");
        }
        let generated = mat_forms();
        assert!(generated.len() > 10_000, "productive expansion too small: {}", generated.len());
        assert!(generated.iter().any(|w| w == "заебался"), "заебался must be generated");
        for w in generated.iter().filter(|w| w.chars().count() >= 4).take(200) {
            assert!(is_mat_form(w), "generated form not recognised: {w}");
        }
    }
}

const DERIV_VERBAL: &[&str] = &["лк", "льщик", "тель"];
const DERIV_NOMINAL: &[&str] = &["щик", "чик", "ник", "шк", "ечк", "очк", "ушк", "юшк", "ищ", "изм", "ист", "ств", "ниц", "овщин", "ёнок", "онок", "ирова", "изирова"];
const DERIV_ADJECTIVAL: &[&str] = &["ость", "еньк", "оньк"];
const DERIV_ENDINGS: &[&str] = &[
    "", "а", "я", "и", "ы", "у", "ю", "е", "о", "ой", "ей", "ою", "ам", "ям", "ами", "ями", "ах", "ях", "ом", "ем", "ов", "ев", "ью", "ть", "ться", "л", "ла", "ли", "ло", "ет", "ют", "ешь", "ете", "ая", "ый", "ое", "ые",
];
const DERIV_PREFIXES: &[&str] = &[
    "пере", "недо", "пред", "раз", "рас", "под", "над", "при", "про", "анти", "супер", "псевдо", "мега", "микро", "полу", "само", "сверх",
];
const DERIV_VERB_FORMS: &[&str] = &["ть", "ться", "л", "ла", "ли", "ло", "лся", "лась", "лись"];

fn is_vowel(c: char) -> bool {
    "аеёиоуыэюя".contains(c)
}

pub fn looks_derived(d: &Dictionary, w: &str) -> bool {
    derivation(d, w).is_some()
}

pub fn keeps_derivation(d: &Dictionary, w: &str, cand: &str) -> bool {
    match derivation(d, w) {
        None => true,
        Some((_, affix, true)) => cand.starts_with(affix),
        Some((base, affix, false)) => {
            let stem = ["ть", "ый", "ой", "ий", "а", "я"].iter().find_map(|e| base.strip_suffix(e)).unwrap_or(&base);
            let head: String = stem.chars().take(stem.chars().count() - 1).collect();
            cand.starts_with(&head) && cand.contains(affix)
        }
    }
}

pub fn derivation(d: &Dictionary, w: &str) -> Option<(String, &'static str, bool)> {
    if w.chars().count() < 6 {
        return None;
    }
    let known = |s: &str| s.chars().count() >= 5 && d.is_word(s);
    for end in DERIV_ENDINGS {
        let Some(rest) = w.strip_suffix(end) else { continue };
        let tries: [(&[&str], &[&str]); 3] = [
            (DERIV_VERBAL, &["ть"]),
            (DERIV_NOMINAL, &["", "а", "я"]),
            (DERIV_ADJECTIVAL, &["ый", "ой", "ий"]),
        ];
        for (sufs, stem_ends) in tries {
            for suf in sufs {
                let Some(stem) = rest.strip_suffix(suf) else { continue };
                let Some(last) = stem.chars().last() else { continue };
                if stem.chars().count() < 5 {
                    continue;
                }
                for e in stem_ends {
                    let ok_shape = match *e {
                        "ть" => is_vowel(last),
                        "ий" => "гкхжшчщ".contains(last),
                        _ => !is_vowel(last) && last != 'ь' && last != 'ъ',
                    };
                    let base = format!("{stem}{e}");
                    if ok_shape && known(&base) {
                        return Some((base, *suf, false));
                    }
                }
            }
        }
    }
    DERIV_PREFIXES.iter().find_map(|p| {
        let rest = w.strip_prefix(p)?;
        (DERIV_VERB_FORMS.iter().any(|f| rest.ends_with(f)) && known(rest)).then(|| (rest.to_string(), *p, true))
    })
}
