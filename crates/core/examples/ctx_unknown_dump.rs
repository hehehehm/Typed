use layout_keeper_core::{dict, dict::Dictionary, layout, layout::Lang, ngram, typo};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (lang, tag) = if a.get(1).map(String::as_str) == Some("en") { (Lang::En, "en") } else { (Lang::Ru, "ru") };
    let level = if a.get(2).map(String::as_str) == Some("aggressive") { typo::Level::Aggressive } else { typo::Level::Normal };
    let text = std::fs::read_to_string(format!("{}/tests/data/heldout_{tag}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let d = dict::get(lang);
    let mut total = 0usize;
    for s in text.lines().filter(|l| !l.trim().is_empty()).take(3000) {
        let chars: Vec<char> = s.chars().collect();
        let mut ws = vec![];
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_alphabetic() {
                let j = (i..chars.len()).find(|&j| !chars[j].is_alphabetic()).unwrap_or(chars.len());
                ws.push((i, j));
                i = j;
            } else {
                i += 1;
            }
        }
        for k in 0..ws.len() {
            let w: String = chars[ws[k].0..ws[k].1].iter().collect();
            let n = Dictionary::normalize(&w);
            if w.chars().count() < 3 || layout::detect_script(&w) != Some(lang) || !d.is_word(&n) || d.rank(&n).is_some_and(|r| r < 20_000) {
                continue;
            }
            total += 1;
            let prev = (k > 0 && chars[ws[k - 1].1..ws[k].0].iter().all(|c| c.is_whitespace() || *c == ',')).then(|| chars[ws[k - 1].0..ws[k - 1].1].iter().collect::<String>());
            let ctx = match &prev { Some(p) => ngram::Context::of(&[p.as_str()]), None => ngram::Context::of(&[ngram::START]) };
            if let Some(f) = typo::suggest_as_unknown_ctx(d, &w, lang, level, &ctx) {
                println!("{w}\t{}\t{f}", prev.unwrap_or_default());
            }
        }
    }
    eprintln!("rare words: {total}");
}
