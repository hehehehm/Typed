//! Lists corpus words missing from the dictionaries that the typo fixer would change
//! without context but keeps after their most frequent previous word:
//! `ctx_scan <ngrams.tsv> <ru|en> [limit]` (debug helper for the context model).
use layout_keeper_core::{dict, dict::Dictionary, layout::Lang, ngram::Context, typo, Engine, EngineOptions};
use std::collections::HashMap;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let lang = if a[1] == "en" { Lang::En } else { Lang::Ru };
    let limit: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);
    let text = std::fs::read_to_string(&a[0]).unwrap();
    let d = dict::get(lang);
    let mut uni: Vec<(String, u64)> = vec![];
    let mut best_prev: HashMap<String, (String, u64)> = HashMap::new();
    for l in text.lines() {
        let Some((k, c)) = l.split_once('\t') else { continue };
        let c: u64 = c.parse().unwrap_or(0);
        let ws: Vec<&str> = k.split(' ').collect();
        match ws.len() {
            1 if ws[0] != "<s>" && !d.is_word(ws[0]) => uni.push((ws[0].to_string(), c)),
            2 if ws[0] != "<s>" => {
                let e = best_prev.entry(ws[1].to_string()).or_insert((String::new(), 0));
                if c > e.1 { *e = (ws[0].to_string(), c); }
            }
            _ => {}
        }
    }
    uni.sort_by(|x, y| y.1.cmp(&x.1));
    let e = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
    let _ = e;
    let (mut n, mut fixed, mut kept) = (0, 0, 0);
    for (w, c) in &uni {
        let Some((p, pc)) = best_prev.get(w) else { continue };
        n += 1;
        let off = typo::suggest_level(w, lang, typo::Level::Normal);
        if off.is_none() { continue; }
        fixed += 1;
        let on = typo::suggest_ctx(w, lang, typo::Level::Normal, &Context::of(&[p]));
        if on.is_none() {
            kept += 1;
            if kept <= limit { println!("{w} ({c}) after `{p}` ({pc}): no ctx → {:?}", off.unwrap()); }
        }
        let _ = Dictionary::normalize(w);
    }
    println!("corpus words outside dictionaries with a previous word: {n}; changed without context {fixed}; kept with context {kept}");
}
