use fst::{IntoStreamer, Streamer};
use layout_keeper_core::{dict, dict::Dictionary, layout, morph, typo, Lang};
fn main() {
    let level = match std::env::args().nth(1).as_deref() { Some("aggressive") => typo::Level::Aggressive, _ => typo::Level::Normal };
    let lang = Lang::Ru;
    let d = dict::get(lang);
    let ranked = dict::ranked_only(lang);
    let set = fst::Set::new(dict::full_forms_fst(lang)).unwrap();
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(97);
    let mut s = set.into_stream();
    let (mut k, mut n, mut der, mut fp, mut fp_der) = (0usize, 0, 0, 0, 0);
    while let Some(key) = s.next() {
        k += 1;
        if k % step != 0 { continue; }
        let w = dict::key_to_word(key);
        let c = w.chars().count();
        if !(3..=12).contains(&c) || ranked.contains(&w) || !w.chars().all(char::is_alphabetic) || layout::detect_script(&w) != Some(lang) { continue; }
        n += 1;
        let an = morph::derivation(d, &Dictionary::normalize(&w));
        let fix = typo::suggest_as_unknown_level(d, &w, lang, level).or_else(|| layout_keeper_core::typo_wide::layout_typo_as_unknown_level(&w, lang, level));
        if an.is_some() { der += 1; }
        if fix.is_some() { fp += 1; if an.is_some() { fp_der += 1; } }
        if an.is_some() || std::env::var("ALL").is_ok() { println!("{w}\t{:?}\t{}", an, fix.unwrap_or_default()); }
        if n >= 3000 * 97 / step { break; }
    }
    eprintln!("words {n}, derived {der}, changed {fp}, changed among derived {fp_der}");
}
