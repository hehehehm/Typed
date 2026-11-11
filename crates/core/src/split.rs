//! Missing space after a short Russian function word: `снейросеткой` → `с нейросеткой`,
//! `вофисе` → `в офисе`, `незнаю` → `не знаю`.
//!
//! Rules (all must hold):
//! 1. the token is not a word: not in the ranked list, extras, full-form set, brand list,
//!    and not a plausible inflection of a listed word ([`morph::is_known_form`]) —
//!    except the curated glued forms the lists carry anyway ([`is_glued`]: `незнаю`);
//! 2. it starts with a preposition, conjunction or particle from [`PREFIXES`];
//! 3. the rest is a word: a ranked word of at least [`MIN_REST`] letters, or a word of
//!    the brand / new-word list of at least [`MIN_REST_FULL`] letters (`нейросеткой`);
//! 4. after a preposition the rest must have an ending that suits it (`с` + -ом/-ой/-ами
//!    or genitive, `в` + prepositional/accusative…), after a conjunction or particle it
//!    must be 5+ letters long;
//! 5. the rest is not a name (`камаро` is not `к амаро`) and, except after `не`, not a
//!    verb form (`порулить`, `задолбал` are words): tags in `data/ru_split_tags.txt`;
//! 6. among several splits a ranked rest beats a full-form one, then the longest wins.
//!
//! Only the space is inserted, so the typed casing stays (`Вофисе` → `В офисе`).
//! English is not handled: `a`/`I`/`to`/`in` glued to words collide with too many real
//! words and names.

use crate::dict::Dictionary;
use crate::layout::Lang;
use crate::morph;
use std::collections::HashSet;
use std::sync::OnceLock;

static RU_GLUED: &str = include_str!("../data/ru_glued.txt");
static RU_TAGS: &str = include_str!("../data/ru_split_tags.txt");

/// Tag of a frequent word from `data/ru_split_tags.txt`: `n` name / no parse, `v` verb form.
fn tag(word: &str) -> Option<char> {
    static MAP: OnceLock<std::collections::HashMap<String, char>> = OnceLock::new();
    MAP.get_or_init(|| {
        RU_TAGS
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let (t, w) = l.split_once(' ')?;
                Some((w.trim().to_string(), t.chars().next()?))
            })
            .collect()
    })
    .get(word)
    .copied()
}

/// Prepositions: the rest must have an ending that fits them.
const PREPOSITIONS: [&str; 22] = [
    "с", "в", "к", "у", "о", "на", "по", "за", "из", "от", "до", "со", "во", "ко", "об", //
    "для", "без", "под", "над", "при", "про", "о",
];

/// Glued forms that the frequency lists carry as if they were words (`незнаю`,
/// `вобщем`): split them anyway. See `data/ru_glued.txt`.
pub fn is_glued(token: &str) -> bool {
    static SET: OnceLock<HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| {
        RU_GLUED
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .flat_map(str::split_whitespace)
            .map(Dictionary::normalize)
            .collect()
    })
    .contains(&Dictionary::normalize(token))
}

/// Short function words that get glued to the next word.
/// (`же` and `ли` follow a word, so they are not glued to the next one.)
pub const PREFIXES: [&str; 29] = [
    "с", "в", "к", "у", "о", "и", "а", "я", //
    "на", "по", "за", "из", "от", "до", "не", "со", "во", "ко", "об", //
    "для", "без", "под", "над", "при", "про", "что", "как", //
    "то", "но",
];
/// A ranked rest must be at least this frequent (the whole 40 000-word list, tagged).
pub const MAX_RANK: u32 = 40_000;
/// Shortest ranked rest (3-letter rests clash with typos: `чтокой` is `точкой`, not `что кой`).
pub const MIN_REST: usize = 4;
/// Shortest rest that is only in the full-form set or the brand list.
pub const MIN_REST_FULL: usize = 5;
/// Shortest token at all.
pub const MIN_TOKEN: usize = 5;
/// After `не`, a rest that is not a verb form must be this frequent.
pub const NEG_OTHER_RANK: u32 = 5_000;
/// After a conjunction or particle (`а`, `и`, `но`, `то`, `что`…) the rest must be this frequent.
pub const CONJ_RANK: u32 = 12_000;
/// Shortest rest after a one-letter preposition (`с тайн`, `о роль` are names / words),
/// pronouns excepted (`у тебя`).
pub const MIN_REST_ONE: usize = 5;
/// Shortest rest after a conjunction or particle (`а`, `и`, `но`, `что`…; not `не`).
pub const MIN_REST_CONJ: usize = 5;

/// Personal pronouns after a preposition (`у него`, `к ней`, `с ними`): any length.
const PRONOUNS: [&str; 30] = [
    "него", "нее", "ней", "них", "ним", "ними", "нему", "нем", "меня", "мне", "мной", "тебя",
    "тебе", "тобой", "себя", "себе", "собой", "нас", "нам", "нами", "вас", "вам", "вами",
    "всех", "всем", "всеми", "всего", "всему", "этого", "этом",
];

/// Endings that fit after a preposition.
pub fn case_fits(prefix: &str, rest: &str) -> bool {
    let ends = |list: &[&str]| list.iter().any(|e| rest.ends_with(e));
    const INSTR: [&str; 9] = ["ом", "ем", "ой", "ей", "ами", "ями", "им", "ым", "ью"];
    const GEN: [&str; 10] = ["а", "я", "ы", "и", "ов", "ев", "ей", "ого", "его", "ых"];
    const PREP: [&str; 7] = ["е", "и", "ах", "ях", "ом", "ем", "ой"];
    const DAT: [&str; 7] = ["у", "ю", "е", "и", "ам", "ям", "ому"];
    const ACC: [&str; 6] = ["у", "ю", "ь", "ы", "и", "ую"];
    match prefix {
        "с" | "со" => ends(&INSTR) || ends(&GEN),
        "под" | "над" => ends(&INSTR) || ends(&ACC),
        "в" | "во" | "на" | "о" | "об" | "при" => ends(&PREP) || ends(&ACC),
        "к" | "ко" => ends(&DAT),
        "по" => ends(&DAT) || ends(&PREP) || ends(&ACC),
        "у" | "из" | "от" | "до" | "для" | "без" => ends(&GEN),
        "за" => ends(&INSTR) || ends(&ACC),
        "про" => ends(&ACC),
        // Particles, conjunctions, pronouns: any form.
        _ => true,
    }
}

/// `token` with a space after a glued function word, or `None`.
/// The caller has already checked layout, casing, digits and user words.
pub fn split_glued(d: &Dictionary, token: &str, lang: Lang) -> Option<String> {
    if lang != Lang::Ru {
        return None;
    }
    let chars: Vec<char> = token.chars().collect();
    if chars.len() < MIN_TOKEN || !chars.iter().all(|c| c.is_alphabetic()) {
        return None;
    }
    let lower = Dictionary::normalize(token);
    if !is_glued(&lower) && (d.is_word(&lower) || morph::is_known_form(d, &lower)) {
        return None;
    }
    let lv: Vec<char> = lower.chars().collect();
    // (ranked, rest length) — larger is better.
    let mut best: Option<((bool, usize), usize)> = None;
    for p in PREFIXES {
        let pl = p.chars().count();
        if !lv.starts_with(&p.chars().collect::<Vec<_>>()) || lv.len() <= pl {
            continue;
        }
        let rest: String = lv[pl..].iter().collect();
        let rl = lv.len() - pl;
        let prep = PREPOSITIONS.contains(&p);
        let neg = p == "не";
        let mut offer = |score: (bool, usize)| {
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, pl));
            }
        };
        // Personal pronouns fit every preposition (`у тебя`, `со мной`, `об этом`).
        if prep && PRONOUNS.contains(&rest.as_str()) {
            offer((true, rl));
            continue;
        }
        // Prepositions need a fitting ending (`к амаро`, `с андерсон` are names);
        // conjunctions and particles a longer rest. Genitive plural without an ending
        // (`без проблем`, `для книг`) counts for feminine nouns only (`проблема` and
        // `проблемы` listed): `март` / `марта` is not `с март`.
        let listed = |e: &str| d.rank_exact(&format!("{rest}{e}")).is_some_and(|r| r < MAX_RANK);
        let gen_zero = matches!(p, "у" | "из" | "от" | "до" | "для" | "без")
            && rest.chars().last().is_some_and(|c| !"аеиоуыэюяйь".contains(c))
            && listed("а")
            && (listed("ы") || listed("и"));
        if (prep && !case_fits(p, &rest) && !gen_zero) || (!prep && !neg && rl < MIN_REST_CONJ) {
            continue;
        }
        if prep && pl == 1 && rl < MIN_REST_ONE {
            continue;
        }
        // `со` / `во` / `ко` stand before consonant clusters (`со стола`, `во время`),
        // `об` before a vowel (`об игре`), `о` before a consonant.
        let rc: Vec<char> = rest.chars().collect();
        let consonant = |c: char| !"аеиоуыэюяйьъ".contains(c);
        let cluster = consonant(rc[0]) && rc.len() > 1 && consonant(rc[1]);
        if (matches!(p, "со" | "во" | "ко") && !cluster) || (p == "об" && consonant(rc[0])) || (p == "о" && !consonant(rc[0])) {
            continue;
        }
        let t = tag(&rest);
        // Names never, places after prepositions; verb forms only after `не` (`порулить`,
        // `задолбал` are words); after `не` anything else only if very frequent
        // (`не совсем`, but not `не вилла`).
        if t == Some('n') || (t == Some('g') && !prep) || (t == Some('v') && !neg) {
            continue;
        }
        let r = d.rank_exact(&rest);
        if neg && t != Some('v') && !r.is_some_and(|r| r < NEG_OTHER_RANK) {
            continue;
        }
        if !prep && !neg && !r.is_some_and(|r| r < CONJ_RANK) {
            continue;
        }
        let ranked = rl >= MIN_REST && d.rank_exact(&rest).is_some_and(|r| r < MAX_RANK);
        // Outside the ranked list only the brand / new-word list counts (`нейросеткой`):
        // the full-form sets are full of names and verbs that have no tags.
        let full = !ranked && (prep || neg) && rl >= MIN_REST_FULL && d.is_brand(&rest);
        if !ranked && !full {
            continue;
        }
        offer((ranked, rl));
    }
    let (_, pl) = best?;
    let head: String = chars[..pl].iter().collect();
    let tail: String = chars[pl..].iter().collect();
    Some(format!("{head} {tail}"))
}

/// Convenience over the process-wide dictionary.
pub fn suggest(token: &str, lang: Lang) -> Option<String> {
    split_glued(crate::dict::get(lang), token, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glued examples that must be split (typed → expected).
    pub const GLUED: [(&str, &str); 30] = [
        ("снейросеткой", "с нейросеткой"),
        ("вофисе", "в офисе"),
        ("кврачу", "к врачу"),
        ("нафайле", "на файле"),
        ("незнаю", "не знаю"),
        ("Незнаю", "Не знаю"),
        ("иприложение", "и приложение"),
        ("вобщем", "в общем"),
        ("спомощью", "с помощью"),
        ("наработе", "на работе"),
        ("поделу", "по делу"),
        ("зарулем", "за рулем"),
        ("издома", "из дома"),
        ("оттебя", "от тебя"),
        ("домосквы", "до москвы"),
        ("немогу", "не могу"),
        ("сдрузьями", "с друзьями"),
        ("кбабушке", "к бабушке"),
        ("утебя", "у тебя"),
        ("отебе", "о тебе"),
        ("дляменя", "для меня"),
        ("безпроблем", "без проблем"),
        ("подстолом", "под столом"),
        ("придрузьях", "при друзьях"),
        ("вконце", "в конце"),
        ("всмысле", "в смысле"),
        ("немогла", "не могла"),
        ("ВОФИСЕ", "В ОФИСЕ"),
        ("пронего", "про него"),
        ("каксобака", "как собака"),
    ];

    #[test]
    fn splits_glued_function_words() {
        let mut bad = Vec::new();
        for (typed, want) in GLUED {
            let got = suggest(typed, Lang::Ru);
            if got.as_deref() != Some(want) {
                bad.push(format!("{typed} → {got:?} (want {want})"));
            }
        }
        assert!(bad.is_empty(), "{bad:?}");
    }

    #[test]
    fn keeps_real_words_and_junk() {
        for w in [
            "сделать", "вода", "книга", "наука", "влог", "сток", "незнакомец", "сколько", "вопрос",
            "скидывать", "нейросеткой", "коррекцию", "оцени", "кврчу", "приложение", "реалми",
            "привет", "абракадабра", "сяоми", "айфоном", "заводом", "причем", "отменя", "небыли",
            "чтокой", "вначале", "неплохо", "немного", "сначала", "наверное", "потому", "итак",
        ] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
        // Too short, English, digits, mixed.
        for w in ["вкот", "inthe", "alot", "в2кот"] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
            assert_eq!(suggest(w, Lang::En), None, "{w}");
        }
    }
}
