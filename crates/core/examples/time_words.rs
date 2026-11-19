use layout_keeper_core::{dict, layout, typo, typo_wide};
use std::time::Instant;
fn main() {
    for w in std::env::args().skip(1) {
        let l = layout::detect_script(&w).unwrap();
        let _ = typo::suggest(&w, l);
        let t = Instant::now(); let a = typo::suggest_level(&w, l, typo::Level::Cautious); let t1 = t.elapsed();
        let t = Instant::now(); let b = typo_wide::suggest_wide(dict::get(l), &w, l, false, typo::Level::Normal, &layout_keeper_core::ngram::Context::none()); let t2 = t.elapsed(); let t = Instant::now(); let _ = typo_wide::suggest_wide(dict::get(l), &w, l, true, typo::Level::Normal, &layout_keeper_core::ngram::Context::none()); println!("  assume_unknown {:?}", t.elapsed());
        println!("{w}: strict {t1:?} {a:?} | wide {t2:?} {b:?}");
    }
}
