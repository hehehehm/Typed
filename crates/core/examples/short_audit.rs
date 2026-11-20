//! `cargo run --release --example short_audit -- [src_min_rank] [dst_max_rank]`: 2–3 letter
//! words of each frequency list whose other-layout reading is a top word of the other list
//! (the short-token rule of 0.3.74): `src rank → reading rank`, with the engine's decision.
use layout_keeper_core::{dict, engine::Decision, layout, layout::Lang, Engine, EngineOptions};
fn main() {
    let a: Vec<u32> = std::env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    let (smin, dmax) = (a.first().copied().unwrap_or(1000), a.get(1).copied().unwrap_or(300));
    let e = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
    for lang in [Lang::En, Lang::Ru] {
        let (d, o) = (dict::get(lang), dict::get(lang.other()));
        let mut v: Vec<(String, u32, String, u32)> = d
            .iter()
            .filter(|(w, r)| (2..=3).contains(&w.chars().count()) && *r >= smin && w.chars().all(char::is_alphabetic))
            .filter_map(|(w, r)| {
                let c = layout::convert(w, lang);
                (c.chars().all(char::is_alphabetic) && layout::detect_script(&c) == Some(lang.other()))
                    .then(|| o.rank(&c).filter(|&cr| cr < dmax).map(|cr| (w.to_string(), r, c, cr)))
                    .flatten()
            })
            .collect();
        v.sort_by_key(|x| x.3);
        for (w, r, c, cr) in v {
            let now = match e.decide(&w) { Decision::Convert { text, .. } => text, Decision::Keep(_) => "-".into() };
            println!("{lang:?}\t{w}\t{r}\t{c}\t{cr}\t{now}");
        }
    }
}
