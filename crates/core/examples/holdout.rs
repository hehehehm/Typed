//! Hold-out audit: dictionary from the top N words, then correct words ranked below N
//! (absent from that dictionary) must not be "fixed".
use layout_keeper_core::{dict::Dictionary, layout::Lang, typo};
fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20000);
    let take: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
    for (lang, path) in [(Lang::Ru, "crates/core/data/ru.txt"), (Lang::En, "crates/core/data/en.txt")] {
        let all = std::fs::read_to_string(path).unwrap();
        let words: Vec<&str> = all.lines().map(str::trim).filter(|w| !w.is_empty()).collect();
        let top = words[..n].join("\n");
        // `full` as a third argument: also attach the full-form set (as shipped).
        let with_full = std::env::args().nth(3).as_deref() == Some("full");
        let mut d = Dictionary::from_list(lang, &top);
        if with_full {
            d = d.with_known_set(layout_keeper_core::dict::full_forms_fst(lang));
        }
        let mut tested = 0;
        let mut fp = 0;
        let mut ex = Vec::new();
        let mut nonword = 0;
        let mut nw_ex = Vec::new();
        let full = layout_keeper_core::dict::get(lang);
        for w in words[n..].iter().step_by(((words.len() - n) / take).max(1)) {
            if w.chars().count() < typo::MIN_LEN || d.contains(w) { continue; }
            tested += 1;
            if let Some(f) = typo::suggest_in(&d, w, lang) {
                fp += 1;
                let fl = f.to_lowercase();
                if !full.contains(&fl) {
                    nonword += 1;
                    if nw_ex.len() < 15 { nw_ex.push(format!("{w}→{f}")); }
                }
                if ex.len() < 25 { ex.push(format!("{w}→{f}")); }
            }
        }
        println!("{lang:?}: tested {tested}, changed {fp} ({:.1}%)", 100.0 * fp as f64 / tested as f64);
        println!("  {}", ex.join(", "));
        println!("  fixes that are not in the full 40k list: {nonword}: {}", nw_ex.join(", "));
    }
}
