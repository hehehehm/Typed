//! Exhaustive typist false-positive / typo-recovery sweep across ranked words,
//! all full forms (RU+EN), productive mat forms, n-grams (clean + space/merge
//! errors), hyphenated words (clean + segment typos), single-edit typo types
//! (subst/omit/insert/double/transpose across lower/Title/CAPS), hand-shift ±1,
//! and layout conversion across all cases.
//!
//! ```text
//! SWEEP_OUT=/tmp/typist_sweep.txt cargo test --release -p layout_keeper_core --test typist_sweep -- --ignored --nocapture
//! ```
use layout_keeper_core::{
    dict, engine::Decision, layout, morph, ngram, split, typo, Config, Engine, EngineOptions, Lang,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

const PREPS: &[&str] = &["в", "с", "к", "у", "о", "и", "а", "я"];
const HYPHEN_PARTICLES: &[&str] = &["то", "либо", "нибудь", "таки", "ка", "де"];
const EXAMPLES: usize = 25;

fn n_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).max(1)
}

fn par_chunks<T: Sync, F>(items: &[T], chunk: usize, f: F)
where
    F: Fn(&[T]) + Sync,
{
    if items.is_empty() {
        return;
    }
    let threads = n_threads();
    let size = (items.len() + threads - 1) / threads;
    let size = size.max(chunk);
    std::thread::scope(|scope| {
        for part in items.chunks(size) {
            let f = &f;
            scope.spawn(move || f(part));
        }
    });
}

fn opts() -> EngineOptions {
    let mut o = Config::default().engine_options();
    o.typo_level = typo::Level::Normal;
    o.fix_typos = true;
    o.fix_missing_spaces = true;
    o
}

fn decide(e: &mut Engine, token: &str) -> String {
    e.reset();
    match e.decide(token) {
        Decision::Convert { text, .. } => text,
        Decision::Keep(_) => token.to_string(),
    }
}

fn field_after_with(e: &mut Engine, text: &str) -> String {
    use layout_keeper_core::Key;
    e.reset();
    let mut out: Vec<char> = Vec::new();
    for k in text.chars().map(Key::Char).chain([Key::Boundary('\n')]) {
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
    out.into_iter().collect::<String>().trim_end().to_string()
}

fn title(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) => c.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
        None => String::new(),
    }
}

fn caps(s: &str) -> String {
    s.chars().flat_map(|c| c.to_uppercase()).collect()
}

fn case_variants(s: &str) -> Vec<String> {
    let low = s.to_lowercase();
    let up = caps(s);
    let tit = title(s);
    let mut v = vec![low.clone()];
    if tit != low {
        v.push(tit);
    }
    if up != low && up != v[v.len() - 1] {
        v.push(up);
    }
    v
}

struct Bucket {
    name: &'static str,
    ran: AtomicU64,
    fail: AtomicU64,
    examples: Mutex<Vec<(String, String, String)>>,
    classes: Mutex<BTreeMap<String, (u64, Vec<String>)>>,
}

impl Bucket {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            ran: AtomicU64::new(0),
            fail: AtomicU64::new(0),
            examples: Mutex::new(Vec::new()),
            classes: Mutex::new(BTreeMap::new()),
        }
    }
    fn check(&self, typed: &str, got: &str, expected: &str) {
        self.ran.fetch_add(1, Ordering::Relaxed);
        if got != expected {
            self.fail.fetch_add(1, Ordering::Relaxed);
            let mut g = self.examples.lock().unwrap();
            if g.len() < EXAMPLES {
                g.push((typed.to_string(), got.to_string(), expected.to_string()));
            }
        }
    }
    fn check_classified(&self, typed: &str, got: &str, expected: &str, cls: &str) {
        self.ran.fetch_add(1, Ordering::Relaxed);
        if got != expected {
            self.fail.fetch_add(1, Ordering::Relaxed);
            let mut g = self.examples.lock().unwrap();
            if g.len() < EXAMPLES {
                g.push((typed.to_string(), got.to_string(), expected.to_string()));
            }
            drop(g);
            let mut m = self.classes.lock().unwrap();
            let e = m.entry(cls.to_string()).or_insert((0, Vec::new()));
            e.0 += 1;
            if e.1.len() < 12 {
                e.1.push(format!("{typed}→{got}"));
            }
        }
    }
    fn report(&self) -> String {
        let n = self.ran.load(Ordering::Relaxed);
        let f = self.fail.load(Ordering::Relaxed);
        let pass = n.saturating_sub(f);
        let pct = if n == 0 { 0.0 } else { 100.0 * pass as f64 / n as f64 };
        let mut s = format!(
            "{:<24} ran={:<10} pass={:<10} fail={:<8} ({:.3}%)\n",
            self.name, n, pass, f, pct
        );
        let m = self.classes.lock().unwrap();
        if !m.is_empty() {
            let mut rows: Vec<_> = m.iter().collect();
            rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
            for (k, (c, ex)) in rows {
                s.push_str(&format!("  class {:>6}  {:<42} {:?}\n", c, k, ex));
            }
        }
        let g = self.examples.lock().unwrap();
        for (t, got, exp) in g.iter() {
            s.push_str(&format!("  FAIL {t:?} → {got:?} (wanted {exp:?})\n"));
        }
        s
    }
}

struct Recall {
    name: &'static str,
    ran: AtomicU64,
    hit: AtomicU64,
    wrong: AtomicU64,
    examples: Mutex<Vec<(String, String, String)>>,
}

impl Recall {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            ran: AtomicU64::new(0),
            hit: AtomicU64::new(0),
            wrong: AtomicU64::new(0),
            examples: Mutex::new(Vec::new()),
        }
    }
    fn check(&self, typed: &str, got: &str, expected: &str) {
        self.ran.fetch_add(1, Ordering::Relaxed);
        if got == expected {
            self.hit.fetch_add(1, Ordering::Relaxed);
        } else {
            if got != typed {
                self.wrong.fetch_add(1, Ordering::Relaxed);
            }
            let mut g = self.examples.lock().unwrap();
            if g.len() < EXAMPLES {
                g.push((typed.to_string(), got.to_string(), expected.to_string()));
            }
        }
    }
    fn report(&self) -> String {
        let n = self.ran.load(Ordering::Relaxed);
        let h = self.hit.load(Ordering::Relaxed);
        let w = self.wrong.load(Ordering::Relaxed);
        let miss = n.saturating_sub(h);
        let pct = if n == 0 { 0.0 } else { 100.0 * h as f64 / n as f64 };
        let wpct = if n == 0 { 0.0 } else { 100.0 * w as f64 / n as f64 };
        let mut s = format!(
            "{:<28} ran={:<10} recall={:>6.2}%  wrong={:>5.2}%  hit={:<9} miss={}\n",
            self.name, n, pct, wpct, h, miss
        );
        let g = self.examples.lock().unwrap();
        for (t, got, exp) in g.iter() {
            s.push_str(&format!("  MISS {t:?} → {got:?} (wanted {exp:?})\n"));
        }
        s
    }
}

fn hyphen_from_corpora() -> Vec<String> {
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in ["typist_chat.txt", "heldout_ru.txt", "heldout_en.txt"] {
        let Ok(text) = std::fs::read_to_string(format!("{data}/{name}")) else {
            continue;
        };
        for line in text.lines() {
            if line.starts_with('#') {
                continue;
            }
            for w in line.split(|c: char| c.is_whitespace() || ".,!?;:()[]{}\"'«»".contains(c)) {
                if w.contains('-')
                    && !w.starts_with('-')
                    && !w.ends_with('-')
                    && w.chars().all(|c| c.is_alphabetic() || c == '-')
                    && w.chars().any(|c| c.is_alphabetic())
                    && seen.insert(w.to_string())
                {
                    out.push(w.to_string());
                }
            }
        }
    }
    let heads = [
        "все", "всё", "как", "кто", "что", "где", "когда", "куда", "откуда", "зачем", "почему",
        "какой", "чей", "дай", "ну", "кое", "во", "по",
    ];
    for h in heads {
        for p in HYPHEN_PARTICLES {
            let w = format!("{h}-{p}");
            if seen.insert(w.clone()) {
                out.push(w);
            }
        }
    }
    // Plausible Russian/English compounds (must stay untouched).
    for w in [
        "кошко-мальчик", "кошко-девочка", "мальчик-с-пальчик", "красно-белый", "нейро-сеть",
        "псевдо-код", "мини-офис", "веб-сайт", "онлайн-курс", "видео-звонок", "аудио-книга",
        "медиа-файл", "крипто-кошелёк", "авто-обновление", "полу-финал", "супер-герой",
        "state-of-the-art", "well-known", "user-facing", "real-time", "end-to-end",
    ] {
        if seen.insert(w.to_string()) {
            out.push(w.to_string());
        }
    }
    out
}

fn strided<T: Clone>(items: &[T], step: usize) -> Vec<T> {
    if step <= 1 {
        return items.to_vec();
    }
    items.iter().step_by(step).cloned().collect()
}

fn typo_particle(p: &str) -> Vec<String> {
    let cs: Vec<char> = p.chars().collect();
    let mut out = Vec::new();
    if cs.len() >= 2 {
        let mut t = cs.clone();
        t.swap(0, 1);
        out.push(t.into_iter().collect());
    }
    if let Some(&last) = cs.last() {
        let mut t = cs.clone();
        t.push(last);
        out.push(t.into_iter().collect());
    }
    out
}

fn is_ru_vowel(c: char) -> bool {
    matches!(c, 'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я')
}

fn is_en_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

fn neighbour_char(c: char, lang: Lang) -> Option<char> {
    let low = c.to_lowercase().next()?;
    let pick = layout::shift_char(low, lang, lang, 1)
        .filter(|x| x.is_alphabetic())
        .or_else(|| layout::shift_char(low, lang, lang, -1).filter(|x| x.is_alphabetic()))?;
    if c.is_uppercase() {
        pick.to_uppercase().next()
    } else {
        Some(pick)
    }
}

#[derive(Copy, Clone)]
enum EditKind {
    SubstNb,
    Omit,
    InsertNb,
    Double,
    Transpose,
}

/// Apply one deterministic realistic typo near the middle of `w`.
/// Returns `None` if `w` is too short or the mutated form happens to be a valid word.
fn make_edit(w: &str, lang: Lang, kind: EditKind) -> Option<String> {
    let cs: Vec<char> = w.chars().collect();
    let n = cs.len();
    if n < 4 {
        return None;
    }
    let mid = n / 2;
    let out: String = match kind {
        EditKind::SubstNb => {
            let mut v = cs.clone();
            let idx = mid.min(n - 1);
            let sub = neighbour_char(v[idx], lang)?;
            if sub == v[idx] {
                return None;
            }
            v[idx] = sub;
            v.into_iter().collect()
        }
        EditKind::Omit => {
            if n < 5 {
                return None;
            }
            let mut v = cs.clone();
            v.remove(mid);
            v.into_iter().collect()
        }
        EditKind::InsertNb => {
            let mut v = cs.clone();
            let ins = neighbour_char(v[mid], lang)?;
            v.insert(mid, ins);
            v.into_iter().collect()
        }
        EditKind::Double => {
            let mut v = cs.clone();
            let ch = v[mid];
            v.insert(mid, ch);
            v.into_iter().collect()
        }
        EditKind::Transpose => {
            if n < 4 {
                return None;
            }
            let mut v = cs.clone();
            let i = mid.saturating_sub(1);
            if v[i].to_lowercase().next() == v[i + 1].to_lowercase().next() {
                return None;
            }
            v.swap(i, i + 1);
            v.into_iter().collect()
        }
    };
    let lower = out.to_lowercase();
    let d = dict::get(lang);
    if d.is_word(&lower) || morph::is_known_form(d, &lower) {
        return None;
    }
    // Also skip if converted form is a valid word of the other language.
    let other_conv = layout::convert(&lower, lang);
    if dict::get(lang.other()).is_word(&other_conv) {
        return None;
    }
    let _ = (is_ru_vowel, is_en_vowel);
    Some(out)
}

fn case_label(s: &str) -> &'static str {
    let letters: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return "other";
    }
    let up = letters.iter().filter(|c| c.is_uppercase()).count();
    if up == 0 {
        "lower"
    } else if up == letters.len() {
        "CAPS"
    } else if up == 1 && letters[0].is_uppercase() {
        "Title"
    } else {
        "other"
    }
}

fn sweep_tokens(bucket: &Bucket, words: &[String], variants: bool) {
    par_chunks(words, 256, |chunk| {
        let mut e = Engine::new(opts());
        for w in chunk {
            if w.is_empty() {
                continue;
            }
            let vars = if variants { case_variants(w) } else { vec![w.clone()] };
            for v in vars {
                let got = decide(&mut e, &v);
                bucket.check(&v, &got, &v);
            }
        }
    });
}

fn sweep_layout_convert(rows: &[&Recall], src: Lang, words: &[String]) {
    let dst = src.other();
    par_chunks(words, 256, |chunk| {
        let mut e = Engine::new(opts());
        for w in chunk {
            if !w.chars().all(|c| c.is_alphabetic()) {
                continue;
            }
            if layout::detect_script(w) != Some(src) {
                continue;
            }
            let short = w.chars().count() <= 3;
            for variant in case_variants(w) {
                let typed = layout::convert(&variant, src);
                if layout::detect_script(&typed) != Some(dst) {
                    continue;
                }
                let got = decide(&mut e, &typed);
                let label = case_label(&variant);
                let i = match label {
                    "lower" => 0,
                    "Title" => 1,
                    "CAPS" => 2,
                    _ => continue,
                };
                rows[i].check(&typed, &got, &variant);
                if short {
                    rows[3 + i].check(&typed, &got, &variant);
                }
            }
        }
    });
}

/// All productive mat forms (3 cases = ~2.12M) + hyphenated corpus words.
#[test]
#[ignore]
fn typist_sweep_mat_and_hyphen_smoke() {
    let mat = morph::mat_forms();
    assert!(mat.len() >= 10_000, "mat_forms too small: {}", mat.len());
    let b_mat = Bucket::new("fp_mat");
    let b_hy = Bucket::new("fp_hyphen");
    sweep_tokens(&b_mat, &mat, true);
    sweep_tokens(&b_hy, &hyphen_from_corpora(), true);
    print!("{}", b_mat.report());
    print!("{}", b_hy.report());
    let fail = b_mat.fail.load(Ordering::Relaxed) + b_hy.fail.load(Ordering::Relaxed);
    assert_eq!(fail, 0, "mat/hyphen smoke found {fail} FPs");
}

#[test]
#[ignore]
fn typist_sweep_layout_case() {
    let t0 = Instant::now();
    let ru_words: Vec<String> = dict::get(Lang::Ru).iter().map(|(w, _)| w.to_string()).collect();
    let en_words: Vec<String> = dict::get(Lang::En).iter().map(|(w, _)| w.to_string()).collect();
    let ru_l = Recall::new("layout RU→EN lower");
    let ru_t = Recall::new("layout RU→EN Title");
    let ru_c = Recall::new("layout RU→EN CAPS");
    let ru_sl = Recall::new("layout RU→EN 2–3 lower");
    let ru_st = Recall::new("layout RU→EN 2–3 Title");
    let ru_sc = Recall::new("layout RU→EN 2–3 CAPS");
    let en_l = Recall::new("layout EN→RU lower");
    let en_t = Recall::new("layout EN→RU Title");
    let en_c = Recall::new("layout EN→RU CAPS");
    let en_sl = Recall::new("layout EN→RU 2–3 lower");
    let en_st = Recall::new("layout EN→RU 2–3 Title");
    let en_sc = Recall::new("layout EN→RU 2–3 CAPS");
    sweep_layout_convert(&[&ru_l, &ru_t, &ru_c, &ru_sl, &ru_st, &ru_sc], Lang::Ru, &ru_words);
    sweep_layout_convert(&[&en_l, &en_t, &en_c, &en_sl, &en_st, &en_sc], Lang::En, &en_words);
    println!("layout-conversion sweep: wall {:.1}s", t0.elapsed().as_secs_f64());
    for r in [
        &ru_l, &ru_t, &ru_c, &ru_sl, &ru_st, &ru_sc,
        &en_l, &en_t, &en_c, &en_sl, &en_st, &en_sc,
    ] {
        print!("{}", r.report());
    }
}

#[test]
#[ignore]
fn typist_sweep() {
    let t_all = Instant::now();
    let mut report = String::new();

    // 1. All ranked words (RU+EN) × 3 cases
    let t0 = Instant::now();
    let mut ranked: Vec<(Lang, String, u32)> = Vec::new();
    for lang in [Lang::Ru, Lang::En] {
        for (w, r) in dict::get(lang).iter() {
            ranked.push((lang, w.to_string(), r));
        }
    }
    let ranked_words: Vec<String> = ranked.iter().map(|(_, w, _)| w.clone()).collect();
    let b_ranked = Bucket::new("fp_ranked");
    let b_ranked_glued = Bucket::new("fp_ranked_intentional");
    par_chunks(&ranked, 256, |chunk| {
        let mut e = Engine::new(opts());
        for (lang, w, r) in chunk {
            let low = w.to_lowercase();
            let intentional_split = *lang == Lang::Ru && split::is_glued(&low);
            let intentional_short_switch = *lang == Lang::En
                && w.chars().count() <= 3
                && *r >= 2000
                && low != "vs"
                && dict::get(Lang::Ru).rank(&layout::convert(&low, Lang::En)).is_some_and(|rr| rr < 50);
            for v in case_variants(w) {
                let got = decide(&mut e, &v);
                if (intentional_split || intentional_short_switch) && got != v {
                    b_ranked_glued.check(&v, &got, &v);
                } else {
                    b_ranked.check_classified(
                        &v,
                        &got,
                        &v,
                        &format!("{:?}:{}", lang, case_label(&v)),
                    );
                }
            }
        }
    });
    println!("fp_ranked done in {:.1}s", t0.elapsed().as_secs_f64());

    // 2. All full forms (RU+EN, all morph forms) × 3 cases (~11.92M)
    let t0 = Instant::now();
    let mut forms: Vec<(Lang, String)> = Vec::new();
    for lang in [Lang::Ru, Lang::En] {
        dict::get(lang).visit_full_forms(|w| forms.push((lang, w.to_string())));
    }
    let forms_total = forms.len();
    let b_forms = Bucket::new("fp_full_forms");
    par_chunks(&forms, 1024, |chunk| {
        let mut e = Engine::new(opts());
        for (lang, w) in chunk {
            let len = w.chars().count();
            let lcls = if len <= 2 { "len2" } else if len == 3 { "len3" } else { "len4+" };
            for v in case_variants(w) {
                e.reset();
                if let Decision::Convert { text, reason, .. } = e.decide(&v) {
                    if text != v {
                        b_forms.check_classified(
                            &v,
                            &text,
                            &v,
                            &format!("{lcls}:{lang:?}:{reason}"),
                        );
                        continue;
                    }
                }
                b_forms.check(&v, &v, &v);
            }
        }
    });
    println!("fp_full_forms ({forms_total} lemmas) done in {:.1}s", t0.elapsed().as_secs_f64());

    // 3. All productive mat forms × 3 cases (~2.12M)
    let t0 = Instant::now();
    let mat = morph::mat_forms();
    let mat_total = mat.len();
    let b_mat = Bucket::new("fp_mat");
    sweep_tokens(&b_mat, &mat, true);
    println!("fp_mat ({mat_total} forms) done in {:.1}s", t0.elapsed().as_secs_f64());

    // 4. Hyphenated words (corpora + particles + compound stems) × 3 cases
    let t0 = Instant::now();
    let hy = hyphen_from_corpora();
    let b_hy = Bucket::new("fp_hyphen");
    sweep_tokens(&b_hy, &hy, true);
    println!("fp_hyphen ({} words) done in {:.1}s", hy.len(), t0.elapsed().as_secs_f64());

    // 5. All corpus bi/trigrams clean FP (~783k)
    let t0 = Instant::now();
    let mut ngrams_all: Vec<(Lang, Vec<String>)> = Vec::new();
    for lang in [Lang::Ru, Lang::En] {
        ngram::get(lang).visit(|parts, _| {
            if parts.iter().any(|w| w == "<s>") || parts.len() < 2 {
                return;
            }
            ngrams_all.push((lang, parts.to_vec()));
        });
    }
    let ngrams_total = ngrams_all.len();
    let b_ng_fp = Bucket::new("fp_ngram_phrase");
    par_chunks(&ngrams_all, 256, |chunk| {
        let mut e = Engine::new(opts());
        for (lang, parts) in chunk {
            let p = parts.join(" ");
            let got = field_after_with(&mut e, &p);
            if got != p {
                let single = parts.iter().any(|w| w.chars().count() == 1);
                b_ng_fp.check_classified(
                    &p,
                    &got,
                    &p,
                    &format!("{lang:?}:{}", if single { "single-letter" } else { "multi-letter" }),
                );
            } else {
                b_ng_fp.check(&p, &got, &p);
            }
        }
    });
    println!("fp_ngram_phrase ({ngrams_total}) done in {:.1}s", t0.elapsed().as_secs_f64());

    // 6. Single-letter prep valid & glued (lower + CAPS) + all RU prep bigrams
    let t0 = Instant::now();
    let b_space_g = Recall::new("space_glued_prep");
    let b_space_v = Bucket::new("space_prep_valid");
    let b_space_g_caps = Recall::new("space_glued_prep_CAPS");
    let b_space_v_caps = Bucket::new("space_prep_valid_CAPS");
    let mut rests = Vec::new();
    for (w, r) in dict::get(Lang::Ru).iter() {
        if r < 5_000 && w.chars().count() >= 3 && w.chars().all(|c| c.is_alphabetic()) {
            rests.push(w.to_string());
        }
    }
    par_chunks(&rests, 256, |chunk| {
        let mut e = Engine::new(opts());
        for rest in chunk {
            for prep in PREPS {
                let glued = format!("{prep}{rest}");
                let is_real = dict::get(Lang::Ru).is_word(&glued)
                    || morph::is_known_form(dict::get(Lang::Ru), &glued);
                if !is_real {
                    let exp = format!("{prep} {rest}");
                    let got = decide(&mut e, &glued);
                    b_space_g.check(&glued, &got, &exp);
                    let glued_c = caps(&glued);
                    let exp_c = caps(&exp);
                    let got_c = decide(&mut e, &glued_c);
                    b_space_g_caps.check(&glued_c, &got_c, &exp_c);
                }
                let spaced = format!("{prep} {rest}");
                let got2 = field_after_with(&mut e, &spaced);
                b_space_v.check(&spaced, &got2, &spaced);
                let spaced_c = caps(&spaced);
                let got2_c = field_after_with(&mut e, &spaced_c);
                b_space_v_caps.check(&spaced_c, &got2_c, &spaced_c);
            }
        }
    });
    // Also check all RU corpus bigrams starting with single-letter prep when glued.
    let ru_prep_bi: Vec<(String, String)> = ngrams_all
        .iter()
        .filter(|(l, p)| *l == Lang::Ru && p.len() == 2 && PREPS.contains(&p[0].as_str()) && p[1].chars().count() >= 3)
        .filter_map(|(_, p)| {
            let glued = format!("{}{}", p[0], p[1]);
            if dict::get(Lang::Ru).is_word(&glued) || morph::is_known_form(dict::get(Lang::Ru), &glued) {
                None
            } else {
                Some((glued, format!("{} {}", p[0], p[1])))
            }
        })
        .collect();
    let b_prep_bi = Recall::new("space_glued_prep_corpus_bi");
    par_chunks(&ru_prep_bi, 256, |chunk| {
        let mut e = Engine::new(opts());
        for (glued, exp) in chunk {
            let got = decide(&mut e, glued);
            b_prep_bi.check(glued, &got, exp);
        }
    });
    println!("space_prep done in {:.1}s", t0.elapsed().as_secs_f64());

    // 7. Corpus bigrams/trigrams for space/merge errors (sampled 10% across all bi/tri)
    let t0 = Instant::now();
    let bi_all: Vec<&Vec<String>> = ngrams_all.iter().filter(|(_, p)| p.len() == 2).map(|(_, p)| p).collect();
    let tri_all: Vec<&Vec<String>> = ngrams_all.iter().filter(|(_, p)| p.len() == 3).map(|(_, p)| p).collect();
    let bi_sample: Vec<(Lang, Vec<String>)> = ngrams_all
        .iter()
        .filter(|(_, p)| p.len() == 2)
        .step_by(10)
        .cloned()
        .collect();
    let tri_sample: Vec<(Lang, Vec<String>)> = ngrams_all
        .iter()
        .filter(|(_, p)| p.len() == 3)
        .step_by(10)
        .cloned()
        .collect();
    let r_bi_glued = Recall::new("space_glued_bigram (10%)");
    let r_bi_nb = Recall::new("space_nb_bigram (10%)");
    let r_tri_glued = Recall::new("space_glued_trigram (10%)");
    par_chunks(&bi_sample, 256, |chunk| {
        let mut e = Engine::new(opts());
        for (lang, p) in chunk {
            let glued = format!("{}{}", p[0], p[1]);
            let d = dict::get(*lang);
            if !d.is_word(&glued) && !morph::is_known_form(d, &glued) {
                let exp = format!("{} {}", p[0], p[1]);
                let got = decide(&mut e, &glued);
                r_bi_glued.check(&glued, &got, &exp);
            }
            let nb = format!("{}x{}", p[0], p[1]);
            if p[0].chars().all(|c| c.is_alphabetic()) && p[1].chars().all(|c| c.is_alphabetic()) {
                let exp = format!("{} {}", p[0], p[1]);
                let got = decide(&mut e, &nb);
                r_bi_nb.check(&nb, &got, &exp);
            }
        }
    });
    par_chunks(&tri_sample, 256, |chunk| {
        let mut e = Engine::new(opts());
        for (lang, p) in chunk {
            let glued = format!("{}{}", p[0], p[1]);
            let d = dict::get(*lang);
            if !d.is_word(&glued) && !morph::is_known_form(d, &glued) {
                let typed = format!("{glued} {}", p[2]);
                let exp = format!("{} {} {}", p[0], p[1], p[2]);
                let got = field_after_with(&mut e, &typed);
                r_tri_glued.check(&typed, &got, &exp);
            }
        }
    });
    println!(
        "space_merge bi={}/{} tri={}/{} done in {:.1}s",
        bi_sample.len(),
        bi_all.len(),
        tri_sample.len(),
        tri_all.len(),
        t0.elapsed().as_secs_f64()
    );

    // 8. Hand-shift ±1: 100% ranked words (lower) + 25% Title/CAPS + 5% mat_forms
    let t0 = Instant::now();
    let r_shift_ranked = Recall::new("typo_hand_shift_ranked (100% lower)");
    let r_shift_title = Recall::new("typo_hand_shift_ranked (25% Title)");
    let r_shift_caps = Recall::new("typo_hand_shift_ranked (25% CAPS)");
    let r_shift_mat = Recall::new("typo_hand_shift_mat (5%)");
    let run_shift = |bucket: &Recall, items: &[String]| {
        par_chunks(items, 256, |chunk| {
            let mut e = Engine::new(opts());
            for w in chunk {
                let Some(lang) = layout::detect_script(w) else { continue };
                for dir in [-1, 1] {
                    let Some(shifted) = layout::shift_token(w, lang, lang, dir) else { continue };
                    if shifted == *w {
                        continue;
                    }
                    let got = decide(&mut e, &shifted);
                    bucket.check(&shifted, &got, w);
                }
            }
        });
    };
    run_shift(&r_shift_ranked, &ranked_words);
    let ranked_25 = strided(&ranked_words, 4);
    let ranked_title: Vec<String> = ranked_25.iter().map(|w| title(w)).collect();
    let ranked_caps: Vec<String> = ranked_25.iter().map(|w| caps(w)).collect();
    run_shift(&r_shift_title, &ranked_title);
    run_shift(&r_shift_caps, &ranked_caps);
    let mat_sample = strided(&mat, 20);
    run_shift(&r_shift_mat, &mat_sample);
    println!("hand_shift done in {:.1}s", t0.elapsed().as_secs_f64());

    // 9. Single-edit typo types (all 5: SubstNb, Omit, InsertNb, Double, Transpose)
    //    Stratified 25% of all 80,455 ranked words × 3 cases (lower/Title/CAPS)
    let t0 = Instant::now();
    let ranked_edit = strided(&ranked, 4);
    let r_sub = Recall::new("typo1_subst_nb (25%x3)");
    let r_omit = Recall::new("typo1_omit (25%x3)");
    let r_ins = Recall::new("typo1_insert_nb (25%x3)");
    let r_dbl = Recall::new("typo1_double (25%x3)");
    let r_trn = Recall::new("typo1_transpose (25%x3)");
    let r_case_l = Recall::new("typo1_all5_lower");
    let r_case_t = Recall::new("typo1_all5_Title");
    let r_case_c = Recall::new("typo1_all5_CAPS");
    let kinds = [
        (EditKind::SubstNb, &r_sub),
        (EditKind::Omit, &r_omit),
        (EditKind::InsertNb, &r_ins),
        (EditKind::Double, &r_dbl),
        (EditKind::Transpose, &r_trn),
    ];
    par_chunks(&ranked_edit, 128, |chunk| {
        let mut e = Engine::new(opts());
        for (lang, w, _) in chunk {
            for (kind, bucket) in kinds {
                let Some(mutated) = make_edit(w, *lang, kind) else { continue };
                for (exp, m) in [
                    (w.to_lowercase(), mutated.to_lowercase()),
                    (title(w), title(&mutated)),
                    (caps(w), caps(&mutated)),
                ] {
                    let got = decide(&mut e, &m);
                    bucket.check(&m, &got, &exp);
                    match case_label(&exp) {
                        "lower" => r_case_l.check(&m, &got, &exp),
                        "Title" => r_case_t.check(&m, &got, &exp),
                        "CAPS" => r_case_c.check(&m, &got, &exp),
                        _ => {}
                    }
                }
            }
        }
    });
    println!("typo_single_edit done in {:.1}s", t0.elapsed().as_secs_f64());

    // 10. Hyphen typos (particles + corpus hyphen segments, 3 cases)
    let t0 = Instant::now();
    let r_hy_typo = Recall::new("typo_hyphen (3 cases)");
    let heads = ["все", "как", "кто", "где", "когда", "дай", "кое"];
    let mut hy_typos = Vec::new();
    for h in heads {
        for p in HYPHEN_PARTICLES {
            let expected = format!("{h}-{p}");
            for typ in typo_particle(p) {
                if typ != *p {
                    for (t, ex) in [
                        (format!("{h}-{typ}"), expected.clone()),
                        (title(&format!("{h}-{typ}")), title(&expected)),
                        (caps(&format!("{h}-{typ}")), caps(&expected)),
                    ] {
                        hy_typos.push((t, ex));
                    }
                }
            }
        }
    }
    // Left-segment typos on hyphen_from_corpora (Russian/English len>=4 heads).
    for hw in &hy {
        let Some((a, b)) = hw.split_once('-') else { continue };
        let Some(lang) = layout::detect_script(a) else { continue };
        if a.chars().count() >= 4 {
            if let Some(ma) = make_edit(a, lang, EditKind::Double) {
                hy_typos.push((format!("{ma}-{b}"), hw.clone()));
            }
        }
    }
    par_chunks(&hy_typos, 64, |chunk| {
        let mut e = Engine::new(opts());
        for (typed, exp) in chunk {
            let got = decide(&mut e, typed);
            r_hy_typo.check(typed, &got, exp);
        }
    });
    println!("typo_hyphen done in {:.1}s", t0.elapsed().as_secs_f64());

    // 11. Layout conversion recall (100% ranked RU+EN, lower/Title/CAPS + 2-3 letter)
    let t0 = Instant::now();
    let ru_words: Vec<String> = dict::get(Lang::Ru).iter().map(|(w, _)| w.to_string()).collect();
    let en_words: Vec<String> = dict::get(Lang::En).iter().map(|(w, _)| w.to_string()).collect();
    let ru_l = Recall::new("layout RU→EN lower");
    let ru_t = Recall::new("layout RU→EN Title");
    let ru_c = Recall::new("layout RU→EN CAPS");
    let ru_sl = Recall::new("layout RU→EN 2–3 lower");
    let ru_st = Recall::new("layout RU→EN 2–3 Title");
    let ru_sc = Recall::new("layout RU→EN 2–3 CAPS");
    let en_l = Recall::new("layout EN→RU lower");
    let en_t = Recall::new("layout EN→RU Title");
    let en_c = Recall::new("layout EN→RU CAPS");
    let en_sl = Recall::new("layout EN→RU 2–3 lower");
    let en_st = Recall::new("layout EN→RU 2–3 Title");
    let en_sc = Recall::new("layout EN→RU 2–3 CAPS");
    sweep_layout_convert(&[&ru_l, &ru_t, &ru_c, &ru_sl, &ru_st, &ru_sc], Lang::Ru, &ru_words);
    sweep_layout_convert(&[&en_l, &en_t, &en_c, &en_sl, &en_st, &en_sc], Lang::En, &en_words);
    println!("layout_conversion done in {:.1}s", t0.elapsed().as_secs_f64());

    report.push_str(&format!(
        "typist_sweep coverage (total wall {:.1}s, threads={}):\n\
         - fp_ranked: {}/{} (100%) × 3 cases\n\
         - fp_full_forms: {}/{} (100%) × 3 cases\n\
         - fp_mat: {}/{} (100%) × 3 cases\n\
         - fp_hyphen: {} words × 3 cases\n\
         - fp_ngram_phrase: {}/{} (100% non-<s> bi/tri)\n\
         - space_prep: {} RU frequent rests × 8 preps (lower+CAPS) + {} RU prep bigrams\n\
         - space_merge: bi {}/{} (10.0%), tri {}/{} (10.0%)\n\
         - typo_hand_shift: ranked lower {}/{} (100%), Title/CAPS {}/{} (25.0%), mat {}/{} (5.0%)\n\
         - typo_single_edit: {}/{} (25.0%) × 5 edit types × 3 cases\n\
         - layout_conversion: RU {}/{}, EN {}/{} (100%) × 3 cases\n\n",
        t_all.elapsed().as_secs_f64(),
        n_threads(),
        ranked.len(), ranked.len(),
        forms_total, forms_total,
        mat_total, mat_total,
        hy.len(),
        ngrams_total, ngrams_total,
        rests.len(), ru_prep_bi.len(),
        bi_sample.len(), bi_all.len(),
        tri_sample.len(), tri_all.len(),
        ranked_words.len(), ranked_words.len(),
        ranked_25.len(), ranked_words.len(),
        mat_sample.len(), mat_total,
        ranked_edit.len(), ranked.len(),
        ru_words.len(), ru_words.len(),
        en_words.len(), en_words.len(),
    ));

    report.push_str("=== FALSE-POSITIVE SWEEPS ===\n");
    for b in [&b_ranked, &b_ranked_glued, &b_forms, &b_mat, &b_hy, &b_ng_fp, &b_space_v, &b_space_v_caps] {
        report.push_str(&b.report());
        report.push('\n');
    }

    report.push_str("=== TYPO & SPACE RECOVERY SWEEPS ===\n");
    for r in [
        &b_space_g, &b_space_g_caps, &b_prep_bi,
        &r_bi_glued, &r_bi_nb, &r_tri_glued,
        &r_shift_ranked, &r_shift_title, &r_shift_caps, &r_shift_mat,
        &r_sub, &r_omit, &r_ins, &r_dbl, &r_trn,
        &r_case_l, &r_case_t, &r_case_c,
        &r_hy_typo,
    ] {
        report.push_str(&r.report());
    }
    report.push('\n');

    report.push_str("=== LAYOUT-CONVERSION RECALL ===\n");
    for r in [
        &ru_l, &ru_t, &ru_c, &ru_sl, &ru_st, &ru_sc,
        &en_l, &en_t, &en_c, &en_sl, &en_st, &en_sc,
    ] {
        report.push_str(&r.report());
    }

    print!("{report}");
    if let Ok(path) = std::env::var("SWEEP_OUT") {
        let _ = std::fs::write(&path, &report);
        println!("wrote {path}");
    }
    let hard_fail: u64 = [&b_ranked, &b_mat, &b_hy, &b_space_v, &b_space_v_caps]
        .iter()
        .map(|b| b.fail.load(Ordering::Relaxed))
        .sum();
    if hard_fail > 0 {
        panic!("typist_sweep core FP gates failed: {hard_fail}");
    }
}
