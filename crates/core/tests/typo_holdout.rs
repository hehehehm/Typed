//! Holdout measurement of the typo fixer (run with `--ignored --nocapture`).
//!
//! * Synthetic slips: every 7th ranked word of 5+ letters gets one realistic slip (neighbour
//!   key, swap, dropped or doubled letter, never in the first letter). Counted: fixed right,
//!   fixed WRONG, left alone. Slips that are themselves real words are skipped.
//! * Out-of-vocabulary words: real forms from the full-form set that are not in the ranked
//!   list, checked against the ranked list only (as if they were names / slang we don't
//!   know). Every change is a false correction.
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

fn run_oov(lang: Lang) -> String {
    let d = dict::get(lang);
    let mut tail = 0;
    let ranked = dict::ranked_only(lang);
    let set = fst::Set::new(dict::full_forms_fst(lang)).unwrap();
    let mut stream = set.into_stream();
    let (mut seen, mut checked, mut changed) = (0usize, 0usize, 0usize);
    let mut ex = vec![];
    while let Some(k) = stream.next() {
        seen += 1;
        if seen % 97 != 0 { continue; }
        let w: String = k.iter().map(|&b| if b >= 0xE0 { char::from_u32(b as u32 - 0xE0 + 'а' as u32).unwrap() } else { b as char }).collect();
        if w.chars().count() < 4 || ranked.contains(&w) { continue; }
        checked += 1;
        if let Some(f) = typo::suggest_in(&ranked, &w, lang) {
            changed += 1;
            if d.rank(&f).is_some_and(|r| r >= 20_000) { tail += 1; }
            if ex.len() < 15 { ex.push(format!("{w}->{f}")); }
        }
    }
    format!("{lang:?} OOV real words checked {checked}, changed {changed} ({:.2}%), to a rank 20k-40k word: {tail}\n  e.g. {ex:?}", 100.0 * changed as f64 / checked as f64)
}

#[test]
#[ignore]
fn holdout() {
    let hs: Vec<_> = [Lang::Ru, Lang::En].into_iter().flat_map(|l| {
        [std::thread::spawn(move || run_slips(l)), std::thread::spawn(move || run_oov(l))]
    }).collect();
    for h in hs { eprintln!("{}", h.join().unwrap()); }
    for (t, l) in [("ярдык", Lang::Ru), ("ярдыка", Lang::Ru), ("ярдыки", Lang::Ru)] {
        eprintln!("{t} -> {:?}", typo::suggest(t, l));
    }
}
