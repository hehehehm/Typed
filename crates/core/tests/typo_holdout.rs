//! Holdout measurement of the typo fixer (run with `--ignored --nocapture`).
//!
//! * Synthetic slips: every 7th ranked word of 5+ letters gets one realistic slip (neighbour
//!   key, swap, dropped or doubled letter, never in the first letter). Counted: fixed right,
//!   fixed WRONG, left alone. Slips that are themselves real words are skipped.
//! * Rare-word slips (since 0.3.69): the same for forms of the full-form set that are not
//!   in the 40 000-word list (`приветственный`).
//! * Unknown words: real forms outside the ranked list treated as unknown
//!   (`typo::suggest_as_unknown`, as if they were names / slang we don't know). Every
//!   change is a false correction.
use fst::{IntoStreamer, Streamer};
use layout_keeper_core::{dict, layout::Lang, typo};

const RU_ROWS: [&str; 3] = ["йцукенгшщзхъ", "фывапролджэ", "ячсмитьбю"];
const EN_ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

fn neighbours(c: char, lang: Lang) -> Vec<char> {
    let rows: Vec<Vec<char>> = match lang { Lang::Ru => RU_ROWS, Lang::En => EN_ROWS }.iter().map(|r| r.chars().collect()).collect();
    let Some((r, p)) = rows.iter().enumerate().find_map(|(r, row)| row.iter().position(|&k| k == c).map(|p| (r as i32, p as i32))) else { return vec![] };
    let mut out = vec![];
    for (dr, dc) in [(0, -1), (0, 1), (1, -1), (1, 0), (-1, 0), (-1, 1)] {
        let (rr, cc) = (r + dr, p + dc);
        if rr >= 0 && (rr as usize) < rows.len() && cc >= 0 {
            if let Some(&k) = rows[rr as usize].get(cc as usize) { out.push(k); }
        }
    }
    out
}

struct Rng(u64);
impl Rng {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

fn slip(w: &[char], lang: Lang, rng: &mut Rng) -> Option<String> {
    let n = w.len();
    let i = 1 + rng.next(n - 1);
    let mut v = w.to_vec();
    match rng.next(4) {
        0 => { let nb = neighbours(w[i], lang); if nb.is_empty() { return None; } v[i] = nb[rng.next(nb.len())]; }
        1 => { if i + 1 >= n || w[i] == w[i + 1] { return None; } v.swap(i, i + 1); }
        2 => { v.remove(i); }
        _ => { v.insert(i, w[i]); }
    }
    Some(v.into_iter().collect())
}

fn run_slips(lang: Lang) -> String {
    let d = dict::get(lang);
    let mut words: Vec<(String, u32)> = d.iter().map(|(w, r)| (w.to_string(), r)).collect();
    words.sort_by_key(|x| x.1);
    let mut rng = Rng(42);
    let (mut right, mut wrong, mut none, mut total) = (0, 0, 0, 0);
    let (mut tail_total, mut tail_right) = (0, 0);
    let mut wrong_ex = vec![];
    for (k, (w, r)) in words.iter().enumerate() {
        let cv: Vec<char> = w.chars().collect();
        if k % 7 != 0 || cv.len() < 5 || !cv.iter().all(|c| c.is_alphabetic()) { continue; }
        let Some(t) = slip(&cv, lang, &mut rng) else { continue };
        if d.is_word(&t) { continue; }
        total += 1;
        let tail = *r >= 20_000;
        if tail { tail_total += 1; }
        match typo::suggest(&t, lang) {
            Some(f) if f == *w => { right += 1; if tail { tail_right += 1; } }
            Some(f) => { wrong += 1; if wrong_ex.len() < 15 { wrong_ex.push(format!("{t}->{f} (want {w})")); } }
            None => none += 1,
        }
    }
    let mut out = format!("{lang:?} slips: {total}, fixed right {right} ({:.1}%), WRONG {wrong} ({:.2}%), left {none}; rank 20k-40k targets: {tail_right}/{tail_total}",
        100.0 * right as f64 / total as f64, 100.0 * wrong as f64 / total as f64);
    out += &format!("\n  wrong e.g. {wrong_ex:?}");
    out
}

fn full_words(lang: Lang, every: usize, min_len: usize) -> Vec<String> {
    let ranked = dict::ranked_only(lang);
    let set = fst::Set::new(dict::full_forms_fst(lang)).unwrap();
    let mut stream = set.into_stream();
    let (mut seen, mut out) = (0usize, Vec::new());
    while let Some(k) = stream.next() {
        seen += 1;
        if seen % every != 0 {
            continue;
        }
        let w = dict::key_to_word(k);
        if w.chars().count() >= min_len && !ranked.contains(&w) {
            out.push(w);
        }
    }
    out
}

fn run_full_slips(lang: Lang) -> String {
    let d = dict::get(lang);
    let mut rng = Rng(7);
    let (mut right, mut wrong, mut none, mut total) = (0, 0, 0, 0);
    let mut wrong_ex = vec![];
    for w in full_words(lang, if lang == Lang::Ru { 641 } else { 163 }, 5) {
        let cv: Vec<char> = w.chars().collect();
        let Some(t) = slip(&cv, lang, &mut rng) else { continue };
        if d.is_word(&t) { continue; }
        total += 1;
        match typo::suggest(&t, lang) {
            Some(f) if f == w => right += 1,
            Some(f) => { wrong += 1; if wrong_ex.len() < 15 { wrong_ex.push(format!("{t}->{f} (want {w})")); } }
            None => none += 1,
        }
    }
    format!("{lang:?} rare-word slips: {total}, fixed right {right} ({:.1}%), WRONG {wrong} ({:.2}%), left {none}\n  wrong e.g. {wrong_ex:?}",
        100.0 * right as f64 / total as f64, 100.0 * wrong as f64 / total as f64)
}

fn run_oov(lang: Lang) -> String {
    let d = dict::get(lang);
    let (mut checked, mut changed) = (0usize, 0usize);
    let mut ex = vec![];
    for w in full_words(lang, 97, 4) {
        checked += 1;
        if let Some(f) = typo::suggest_as_unknown(d, &w, lang) {
            changed += 1;
            if ex.len() < 15 { ex.push(format!("{w}->{f}")); }
        }
    }
    format!("{lang:?} unknown words checked {checked}, changed {changed} ({:.2}%)\n  e.g. {ex:?}", 100.0 * changed as f64 / checked as f64)
}

#[test]
#[ignore]
fn holdout() {
    let hs: Vec<_> = [Lang::Ru, Lang::En].into_iter().flat_map(|l| {
        [std::thread::spawn(move || run_slips(l)), std::thread::spawn(move || run_full_slips(l)), std::thread::spawn(move || run_oov(l))]
    }).collect();
    for h in hs { eprintln!("{}", h.join().unwrap()); }
    for (t, l) in [("ярдык", Lang::Ru), ("ярдыка", Lang::Ru), ("ярдыки", Lang::Ru), ("привественный", Lang::Ru)] {
        eprintln!("{t} -> {:?}", typo::suggest(t, l));
    }
}


/// First-letter neighbour-key slips (`шлавный` → `главный`, since 0.3.69): every 3rd ranked
/// word of 5+ letters with its first letter replaced by a neighbour key.
fn run_first_slips(lang: Lang) -> String {
    let d = dict::get(lang);
    let mut words: Vec<(String, u32)> = d.iter().map(|(w, r)| (w.to_string(), r)).collect();
    words.sort_by_key(|x| x.1);
    let mut rng = Rng(3);
    let (mut right, mut wrong, mut none, mut total, mut top) = (0, 0, 0, 0, 0);
    let mut wrong_ex = vec![];
    for (k, (w, r)) in words.iter().enumerate() {
        let mut cv: Vec<char> = w.chars().collect();
        if k % 3 != 0 || cv.len() < 4 || !cv.iter().all(|c| c.is_alphabetic()) { continue; }
        let nb = neighbours(cv[0], lang);
        if nb.is_empty() { continue; }
        cv[0] = nb[rng.next(nb.len())];
        let t: String = cv.into_iter().collect();
        if d.is_word(&t) { continue; }
        total += 1;
        if *r < typo::first_slip_rank(t.chars().count(), lang) { top += 1; }
        match typo::suggest(&t, lang) {
            Some(f) if f == *w => right += 1,
            Some(f) => { wrong += 1; if wrong_ex.len() < 15 { wrong_ex.push(format!("{t}->{f} (want {w})")); } }
            None => none += 1,
        }
    }
    format!("{lang:?} first-letter slips: {total} ({top} of words under the rank limit), fixed right {right}, WRONG {wrong} ({:.2}%), left {none}\n  wrong e.g. {wrong_ex:?}",
        100.0 * wrong as f64 / total as f64)
}

/// Layout switched mid-word (since 0.3.69): every 7th ranked word of 5+ letters with a
/// run from letter 2+ typed in the other layout (`приdtn`). And the false-positive side:
/// Latin words with a Russian case ending (`gitом`, `Windowsе`), which must stay.
fn run_mixed(lang: Lang) -> String {
    use layout_keeper_core::engine::{Decision, Engine, EngineOptions};
    let e = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
    let d = dict::get(lang);
    let mut words: Vec<(String, u32)> = d.iter().map(|(w, r)| (w.to_string(), r)).collect();
    words.sort_by_key(|x| x.1);
    let mut rng = Rng(11);
    let mut slowest = (std::time::Duration::ZERO, String::new());
    let (mut right, mut wrong, mut none, mut total) = (0, 0, 0, 0);
    let mut ex = vec![];
    for (k, (w, _)) in words.iter().enumerate() {
        let cv: Vec<char> = w.chars().collect();
        if k % 7 != 0 || cv.len() < 5 || !cv.iter().all(|c| c.is_alphabetic()) { continue; }
        let a = 1 + rng.next(cv.len() - 2);
        let b = (a + 1 + rng.next(cv.len() - a)).min(cv.len());
        let t: String = cv.iter().enumerate().map(|(i, &c)| if i >= a && i < b { layout_keeper_core::layout::convert_char(c, lang) } else { c }).collect();
        if layout_keeper_core::layout::detect_script(&t).is_some() { continue; }
        total += 1;
        let t0 = std::time::Instant::now();
        let dec = e.decide(&t);
        let el = t0.elapsed();
        if el > slowest.0 { slowest = (el, t.clone()); }
        match dec {
            Decision::Convert { text, .. } if text == *w => right += 1,
            Decision::Convert { text, .. } => { wrong += 1; if ex.len() < 10 { ex.push(format!("{t}->{text} (want {w})")); } }
            Decision::Keep(_) => none += 1,
        }
    }
    let mut out = format!("{lang:?} mid-word layout switches: {total}, fixed right {right} ({:.1}%), WRONG {wrong} ({:.2}%), left {none}; slowest {:?} ({})\n  wrong e.g. {ex:?}",
        100.0 * right as f64 / total as f64, 100.0 * wrong as f64 / total as f64, slowest.0, slowest.1);
    if lang == Lang::En {
        let (mut checked, mut changed) = (0, 0);
        let mut fp = vec![];
        for (k, (w, _)) in words.iter().enumerate() {
            if k % 5 != 0 || w.chars().count() < 3 || !w.chars().all(|c| c.is_ascii_alphabetic()) { continue; }
            for (j, end) in ["ом", "а", "е", "у", "ы", "ов", "ами", "ах", "ой"].iter().enumerate() {
                let base = if (k + j) % 2 == 0 { w.clone() } else { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() };
                let t = format!("{base}{end}");
                checked += 1;
                if let Decision::Convert { text, .. } = e.decide(&t) {
                    changed += 1;
                    if fp.len() < 15 { fp.push(format!("{t}->{text}")); }
                }
            }
        }
        out += &format!("\nLatin word + Russian ending checked {checked}, changed {changed} ({:.2}%)\n  e.g. {fp:?}", 100.0 * changed as f64 / checked as f64);
    }
    out
}

#[test]
#[ignore]
fn mixed() {
    for l in [Lang::Ru, Lang::En] { eprintln!("{}", run_mixed(l)); }
}

#[test]
#[ignore]
fn first_letter() {
    for l in [Lang::Ru, Lang::En] { eprintln!("{}", run_first_slips(l)); }
}
