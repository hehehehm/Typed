//! Prints context fit and veto for `prev2 prev word fix` (debug helper):
//! `ctx_fit ru "о том" надт надо`.
use layout_keeper_core::{layout::Lang, ngram};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let lang = if a[0] == "en" { Lang::En } else { Lang::Ru };
    let words: Vec<&str> = a[1].split_whitespace().collect();
    let ctx = ngram::Context::of(&words);
    let g = ngram::get(lang);
    for w in &a[2..] {
        let prev = ctx.prev.as_deref().unwrap_or("");
        println!("{w}: uni {:.0} bi {:.0} prev {:.0} fit {:?}", g.count(&[w]), g.count(&[prev, w]), g.count(&[prev]), ngram::fit(&ctx, w, lang));
    }
    if a.len() >= 4 {
        println!("veto {} -> {}: {}", a[2], a[3], ngram::veto(&ctx, &a[2], &a[3], lang));
    }
}
