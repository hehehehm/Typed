
use crate::dict::Dictionary;
use crate::layout::Lang;
use crate::morph;
use std::collections::HashSet;
use std::sync::OnceLock;

static RU_GLUED: &str = include_str!("../data/ru_glued.txt");
static RU_TAGS: &str = include_str!("../data/ru_split_tags.txt");

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

const PREPOSITIONS: [&str; 22] = [
    "с", "в", "к", "у", "о", "на", "по", "за", "из", "от", "до", "со", "во", "ко", "об",
    "для", "без", "под", "над", "при", "про", "о",
];

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

pub const PREFIXES: [&str; 29] = [
    "с", "в", "к", "у", "о", "и", "а", "я",
    "на", "по", "за", "из", "от", "до", "не", "со", "во", "ко", "об",
    "для", "без", "под", "над", "при", "про", "что", "как",
    "то", "но",
];
pub const MAX_RANK: u32 = 40_000;
pub const MIN_REST: usize = 4;
pub const MIN_REST_FULL: usize = 5;
pub const MIN_TOKEN: usize = 5;
pub const NEG_OTHER_RANK: u32 = 5_000;
pub const CONJ_RANK: u32 = 12_000;
pub const MIN_REST_ONE: usize = 5;
pub const MIN_REST_CONJ: usize = 5;

pub const PARTICLES: [&str; 3] = ["бы", "же", "ли"];
pub const PARTICLE_STEM_RANK: u32 = 2_000;
pub const MIN_PARTICLE_STEM: usize = 3;
pub const SHORT_PARTICLE_STEM_RANK: u32 = 600;
const PREPOSITION_STEMS: [&str; 6] = ["обо", "ото", "изо", "подо", "предо", "передо"];

const MIN_COMPOUND_PART: usize = 4;
const MIN_COMPOUND_TOKEN: usize = 10;
const COMPOUND_SHORT_RANK: u32 = 8_000;

fn split_particle(d: &Dictionary, chars: &[char], lower: &[char], min_stem: usize) -> Option<String> {
    for p in PARTICLES {
        let pv: Vec<char> = p.chars().collect();
        if !lower.ends_with(&pv) || lower.len() < pv.len() + min_stem {
            continue;
        }
        let n = lower.len() - pv.len();
        let stem: String = lower[..n].iter().collect();
        let max_rank = if n <= 3 { SHORT_PARTICLE_STEM_RANK } else { PARTICLE_STEM_RANK };
        if tag(&stem) == Some('n')
            || PREPOSITION_STEMS.contains(&stem.as_str())
            || !d.rank_exact(&stem).is_some_and(|r| r < max_rank)
        {
            continue;
        }
        let head: String = chars[..n].iter().collect();
        let tail: String = chars[n..].iter().collect();
        return Some(format!("{head} {tail}"));
    }
    None
}

const PRONOUNS: [&str; 30] = [
    "него", "нее", "ней", "них", "ним", "ними", "нему", "нем", "меня", "мне", "мной", "тебя",
    "тебе", "тобой", "себя", "себе", "собой", "нас", "нам", "нами", "вас", "вам", "вами",
    "всех", "всем", "всеми", "всего", "всему", "этого", "этом",
];

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
        _ => true,
    }
}

pub fn split_glued(d: &Dictionary, token: &str, lang: Lang) -> Option<String> {
    if lang != Lang::Ru {
        return None;
    }
    let chars: Vec<char> = token.chars().collect();
    if chars.len() < MIN_TOKEN || !chars.iter().all(|c| c.is_alphabetic()) {
        return None;
    }
    let lower = Dictionary::normalize(token);
    let glued = is_glued(&lower);
    if !glued && d.is_word(&lower) {
        return None;
    }
    if !glued && morph::is_known_form(d, &lower) {
        let lv: Vec<char> = lower.chars().collect();
        return split_particle(d, &chars, &lv, MIN_PARTICLE_STEM + 1);
    }
    let lv: Vec<char> = lower.chars().collect();
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
        if prep && PRONOUNS.contains(&rest.as_str()) {
            offer((true, rl));
            continue;
        }
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
        let rc: Vec<char> = rest.chars().collect();
        let consonant = |c: char| !"аеиоуыэюяйьъ".contains(c);
        let cluster = consonant(rc[0]) && rc.len() > 1 && consonant(rc[1]);
        if (matches!(p, "со" | "во" | "ко") && !cluster) || (p == "об" && consonant(rc[0])) || (p == "о" && !consonant(rc[0])) {
            continue;
        }
        let t = tag(&rest);
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
        let full = !ranked && (prep || neg) && rl >= MIN_REST_FULL && d.is_brand(&rest);
        if !ranked && !full {
            continue;
        }
        offer((ranked, rl));
    }
    let Some((_, pl)) = best else {
        return split_particle(d, &chars, &lv, MIN_PARTICLE_STEM)
            .or_else(|| split_compound(d, &chars, &lv));
    };
    let head: String = chars[..pl].iter().collect();
    let tail: String = chars[pl..].iter().collect();
    Some(format!("{head} {tail}"))
}

fn split_compound(d: &Dictionary, chars: &[char], lower: &[char]) -> Option<String> {
    if lower.len() < MIN_COMPOUND_TOKEN {
        return None;
    }
    let mut hits: Vec<(u64, usize)> = Vec::new();
    for i in MIN_COMPOUND_PART..=lower.len() - MIN_COMPOUND_PART {
        let left: String = lower[..i].iter().collect();
        let right: String = lower[i..].iter().collect();
        if !compound_part_ok(d, &left) || !compound_part_ok(d, &right) {
            continue;
        }
        if is_closed_compound_prefix(&left) {
            continue;
        }
        let lr = compound_rank(d, &left);
        let rr = compound_rank(d, &right);
        if lr.is_none() && rr.is_none() {
            continue;
        }
        let score = lr.unwrap_or(50_000) as u64 + rr.unwrap_or(50_000) as u64;
        hits.push((score, i));
    }
    if hits.is_empty() {
        return None;
    }
    hits.sort_unstable();
    if hits.len() >= 2 && hits[1].0 * 2 < hits[0].0 * 3 {
        return None;
    }
    let i = hits[0].1;
    let head: String = chars[..i].iter().collect();
    let tail: String = chars[i..].iter().collect();
    Some(format!("{head} {tail}"))
}

fn compound_rank(d: &Dictionary, part: &str) -> Option<u32> {
    d.rank_exact(part).or_else(|| morph::candidate_rank(d, part)).or_else(|| d.rank(part))
}

const CLOSED_COMPOUND_PREFIXES: &[&str] = &[
    "низко", "высоко", "средне", "мало", "много", "слабо", "сильно", "легко", "трудно", "быстро",
    "медленно", "широко", "узко", "глубоко", "мелко", "добро", "ново", "старо", "ярко", "темно",
    "светло", "само", "взаимо", "внутри", "сверх", "между", "меж", "противо", "после", "перед",
    "двух", "трех", "трёх", "четырех", "четырёх", "пяти", "шести", "семи", "восьми", "девяти",
    "десяти", "одиннадцати", "двенадцати", "двадцати", "тридцати", "сорока", "пятидесяти",
    "шестидесяти", "семидесяти", "восьмидесяти", "девяноста", "полу", "одно", "сто",
];

fn is_closed_compound_prefix(left: &str) -> bool {
    CLOSED_COMPOUND_PREFIXES.contains(&left)
}

fn compound_part_ok(d: &Dictionary, part: &str) -> bool {
    let n = part.chars().count();
    if n < MIN_COMPOUND_PART || tag(part) == Some('n') {
        return false;
    }
    if n == 4 {
        return compound_rank(d, part).is_some_and(|r| r < COMPOUND_SHORT_RANK);
    }
    d.is_word(part) || morph::is_known_form(d, part)
}

pub const LM_MIN_BIGRAM: f64 = 3.0;
pub const LM_MIN_BIGRAM_SHORT: f64 = 20.0;
pub const LM_MIN_TOKEN: usize = 4;
pub const LM_SHORT_PART_RANK: u32 = 20_000;
pub const LM_MARGIN: f64 = 4.0;
pub const LM_WHOLE_RATIO: f64 = 2.0;

pub fn space_neighbours(lang: Lang) -> &'static [char] {
    match lang {
        Lang::En => &['c', 'v', 'b', 'n', 'm'],
        Lang::Ru => &['с', 'м', 'и', 'т', 'ь'],
    }
}

fn lm_part_ok(d: &Dictionary, lang: Lang, part: &str) -> bool {
    let n = part.chars().count();
    match n {
        0 => false,
        1 => match lang {
            Lang::En => matches!(part, "a" | "i"),
            Lang::Ru => matches!(part, "а" | "в" | "и" | "к" | "о" | "с" | "у" | "я"),
        },
        2 | 3 => d.rank_exact(part).is_some_and(|r| r < LM_SHORT_PART_RANK),
        _ => d.rank_exact(part).is_some() || d.in_full_forms(part),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LmSplit {
    pub text: String,
    pub count: f64,
    pub dropped: bool,
}

pub fn lm_segment(d: &Dictionary, token: &str, lang: Lang) -> Option<LmSplit> {
    let chars: Vec<char> = token.chars().collect();
    if chars.len() < LM_MIN_TOKEN || !chars.iter().all(|c| c.is_alphabetic()) {
        return None;
    }
    let lower: Vec<char> = Dictionary::normalize(token).chars().collect();
    if lower.len() != chars.len() {
        return None;
    }
    let ng = crate::ngram::get(lang);
    let nb = space_neighbours(lang);
    let mut cands: Vec<(f64, usize, usize)> = Vec::new();
    for i in 1..lower.len() {
        let a: String = lower[..i].iter().collect();
        if !lm_part_ok(d, lang, &a) {
            continue;
        }
        let b: String = lower[i..].iter().collect();
        if b.chars().count() >= 2 && lm_part_ok(d, lang, &b) {
            let c = ng.count(&[&a, &b]);
            let need = if a.chars().count() <= 2 || b.chars().count() <= 2 { LM_MIN_BIGRAM_SHORT } else { LM_MIN_BIGRAM };
            if c >= need {
                cands.push((c, i, i));
            }
        }
        if i + 1 < lower.len() && nb.contains(&lower[i]) && lower[i] != lower[i - 1] && lower[i] != lower[i + 1] {
            let b: String = lower[i + 1..].iter().collect();
            if a.chars().count() >= 2 && b.chars().count() >= 2 && lm_part_ok(d, lang, &b) {
                let x: String = lower[i].to_string();
                let c = ng.count(&[&a, &b]);
                let c3 = ng.count(&[&a, &x, &b]);
                if c >= LM_MIN_BIGRAM * 4.0 && c3 < c {
                    cands.push((c / 4.0, i, i + 1));
                }
            }
        }
    }
    if cands.iter().any(|x| x.1 == x.2) {
        cands.retain(|x| x.1 == x.2);
    }
    if cands.is_empty() {
        return None;
    }
    cands.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
    if cands.len() >= 2 && cands[0].0 < cands[1].0 * LM_MARGIN {
        return None;
    }
    let (c, i, j) = cands[0];
    let lw: String = lower.iter().collect();
    let whole = ng.count(&[&lw]);
    if whole > 0.0 && c < whole * LM_WHOLE_RATIO {
        return None;
    }
    let head: String = chars[..i].iter().collect();
    let tail: String = chars[j..].iter().collect();
    Some(LmSplit { text: format!("{head} {tail}"), count: c, dropped: j > i })
}

pub fn suggest(token: &str, lang: Lang) -> Option<String> {
    split_glued(crate::dict::get(lang), token, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn splits_glued_particles() {
        for (typed, want) in [
            ("надобы", "надо бы"), ("Надобы", "Надо бы"), ("какбы", "как бы"),
            ("еслибы", "если бы"), ("чтоли", "что ли"), ("гдеже", "где же"), ("могбы", "мог бы"), ("тотже", "тот же"),
            ("былоли", "было ли"), ("хочули", "хочу ли"), ("похожели", "похоже ли"), ("знаешьли", "знаешь ли"),
        ] {
            assert_eq!(suggest(typed, Lang::Ru).as_deref(), Some(want), "{typed}");
        }
        for w in [
            "чтобы", "также", "даже", "уже", "неужели", "либо", "дали", "были", "могли", "тоже",
            "грибы", "рыбы", "вроде", "ножи", "вели", "пошли", "надоб", "джоли", "барли",
            "рэйли", "силли", "делли", "донли", "барбы", "обоже", "столи", "проли", "развели",
            "почтили", "нимбы",
        ] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
    }

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
        for w in ["вкот", "inthe", "alot", "в2кот"] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
            assert_eq!(suggest(w, Lang::En), None, "{w}");
        }
        for w in [
            "благополучие", "столица", "сухофрукты", "времяпрепровождение",
            "пользователь", "самолет", "видеокарта", "вебсайт", "отмена", "исправления",
            "низкоуглеводные", "восьмидесятипроцентная",
        ] {
            assert_eq!(suggest(w, Lang::Ru), None, "{w}");
        }
    }

    #[test]
    fn splits_glued_content_words() {
        for (typed, want) in [
            ("отменаисправления", "отмена исправления"),
            ("Отменаисправления", "Отмена исправления"),
            ("домашнийофис", "домашний офис"),
            ("времяработы", "время работы"),
            ("интернетмагазин", "интернет магазин"),
            ("сервернаякомната", "серверная комната"),
        ] {
            assert_eq!(suggest(typed, Lang::Ru).as_deref(), Some(want), "{typed}");
        }
        assert_eq!(suggest("missingspace", Lang::En), None);
    }
}
