//! Audit of the missing-space fixer (`split`).
//! `cargo run --release -p layout_keeper_core --example split_audit -- [ru_full.txt] [ru_all.txt]`
//! 1. Real words must never be split: every word of `ru.txt`, every 50th form of the
//!    full-form set (as an unknown word: a dictionary without that form), brand names.
//! 2. Rare tokens of the subtitle list (rank 40k+, count >= 5) unknown to every list:
//!    how many get split (some of them really are glued words).
//! 3. Synthetic glued pairs: preposition + frequent word.
use layout_keeper_core::{dict::{self, Dictionary}, layout::Lang, split};

struct Rng(u64);
impl Rng { fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 } }

fn main() {
    let d = dict::get(Lang::Ru);
    // 1a. ranked words (known, so only the curated glued list may split them)
    let ru = include_str!("../data/ru.txt");
    let words: Vec<&str> = ru.lines().map(str::trim).filter(|w| !w.is_empty()).collect();
    let mut split_known = Vec::new();
    for w in &words {
        if let Some(s) = split::split_glued(d, w, Lang::Ru) { split_known.push(format!("{w}→{s}")); }
    }
    println!("ranked list words: {} tested, split {} ({:.3}%) — only curated glued forms expected: {}",
        words.len(), split_known.len(), 100.0 * split_known.len() as f64 / words.len() as f64,
        split_known.iter().take(30).cloned().collect::<Vec<_>>().join(", "));
    // 1b. real words treated as unknown: the full-form sample, with the full-form set
    //     switched off and words of the ranked list skipped.
    let ranked = dict::ranked_only(Lang::Ru);
    let mut tested = 0; let mut fp = Vec::new();
    dict::set_full_forms_enabled(false);
    let full_txt = std::env::args().nth(2);
    let mut sample: Vec<String> = Vec::new();
    if let Some(p) = full_txt.as_deref() {
        for (i, l) in std::fs::read_to_string(p).unwrap().lines().enumerate() { if i % 50 == 0 { sample.push(l.to_string()); } }
    }
    for w in &sample {
        if w.chars().count() < split::MIN_TOKEN || ranked.contains(w) { continue; }
        tested += 1;
        if let Some(s) = split::split_glued(&ranked, w, Lang::Ru) { fp.push(format!("{w}→{s}")); }
    }
    dict::set_full_forms_enabled(true);
    if tested > 0 {
        println!("full-form set sample (as unknown words, full set OFF): {tested} tested, split {} ({:.2}%), e.g. {}",
            fp.len(), 100.0 * fp.len() as f64 / tested as f64, fp.iter().take(25).cloned().collect::<Vec<_>>().join(", "));
    }
    // ...and the same sample with the shipped dictionary (full set ON): must be 0.
    let mut fp_on = Vec::new();
    for w in &sample {
        if w.chars().count() < split::MIN_TOKEN { continue; }
        if let Some(s) = split::split_glued(d, w, Lang::Ru) { fp_on.push(format!("{w}→{s}")); }
    }
    println!("full-form set sample, shipped dictionary: split {} of {}: {}", fp_on.len(), sample.len(), fp_on.iter().take(20).cloned().collect::<Vec<_>>().join(", "));
    // brands
    let brands: Vec<&str> = dict::brands_list(Lang::Ru).lines().filter(|l| !l.starts_with('#')).collect();
    let bsplit: Vec<&&str> = brands.iter().filter(|w| split::split_glued(d, w, Lang::Ru).is_some()).collect();
    println!("RU brand names: {} tested, split {}", brands.len(), bsplit.len());
    // 2. rare subtitle tokens
    if let Some(p) = std::env::args().nth(1) {
        let text = std::fs::read_to_string(p).unwrap();
        let mut tested = 0; let mut ex = Vec::new(); let mut n = 0;
        for l in text.lines().skip(40_000) {
            let mut it = l.split_whitespace();
            let (Some(w), Some(c)) = (it.next(), it.next()) else { continue };
            if c.parse::<u32>().unwrap_or(0) < 5 { continue; }
            let w = Dictionary::normalize(w);
            if w.chars().count() < split::MIN_TOKEN || !w.chars().all(|c| ('а'..='я').contains(&c)) || d.is_word(&w) { continue; }
            tested += 1;
            if let Some(s) = split::split_glued(d, &w, Lang::Ru) { n += 1; if ex.len() < 400 { ex.push(format!("{w}→{s}")); } }
        }
        println!("rare unknown subtitle tokens: {tested} tested, split {n} ({:.2}%):\n  {}", 100.0 * n as f64 / tested.max(1) as f64, ex.join(", "));
    }
    // 3. synthetic glued pairs
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let preps = ["с", "в", "к", "у", "о", "на", "по", "за", "из", "от", "до", "не", "для", "без", "под", "при", "про"];
    let (mut tot, mut ok, mut wrong) = (0, 0, Vec::new());
    for _ in 0..3000 {
        let w = words[(rng.next() % 10_000) as usize];
        let p = preps[(rng.next() % preps.len() as u64) as usize];
        if w.chars().count() < split::MIN_REST { continue; }
        // Realistic pairs only: the word's ending fits the preposition.
        if !split::case_fits(p, w) && p != "не" { continue; }
        let glued = format!("{p}{w}");
        if d.is_word(&glued) { continue; }
        tot += 1;
        match split::split_glued(d, &glued, Lang::Ru) {
            Some(s) if s == format!("{p} {w}") => ok += 1,
            Some(s) => if wrong.len() < 15 { wrong.push(format!("{glued}→{s}")) },
            None => {}
        }
    }
    println!("synthetic glued pairs (preposition + frequent word with a fitting ending, or не + word): {tot}, split right {ok} ({:.1}%), split wrong: {}", 100.0 * ok as f64 / tot as f64, wrong.join(", "));
}
