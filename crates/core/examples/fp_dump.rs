//! `cargo run --release --example fp_dump -- ru|en [normal|aggressive]`: the unknown-word
//! FP proxy of `correction_eval` (every 97th unranked full form, or every `STRIDE`th), one
//! line per change.
use layout_keeper_core::{dict, layout, layout::Lang, typo};
fn main() {
    use fst::{IntoStreamer, Streamer};
    let a: Vec<String> = std::env::args().collect();
    let lang = if a.get(1).map(String::as_str) == Some("en") { Lang::En } else { Lang::Ru };
    let level = if a.get(2).map(String::as_str) == Some("aggressive") { typo::Level::Aggressive } else { typo::Level::Normal };
    let d = dict::get(lang);
    let stride: usize = std::env::var("STRIDE").ok().and_then(|s| s.parse().ok()).unwrap_or(97);
    let ranked = dict::ranked_only(lang);
    let set = fst::Set::new(dict::full_forms_fst(lang)).unwrap();
    let mut s = set.into_stream();
    let (mut k, mut n) = (0usize, 0usize);
    while let Some(key) = s.next() {
        k += 1;
        if k % stride != 0 { continue; }
        let w = dict::key_to_word(key);
        let len = w.chars().count();
        if !(3..=12).contains(&len) || ranked.contains(&w) || !w.chars().all(char::is_alphabetic) || layout::detect_script(&w) != Some(lang) { continue; }
        n += 1;
        if n > 3000 { break; }
        if let Some(f) = typo::suggest_as_unknown_level(d, &w, lang, level).or_else(|| layout_keeper_core::typo_wide::layout_typo_as_unknown_level(&w, lang, level)) {
            println!("{w}\t{f}");
        }
    }
}
