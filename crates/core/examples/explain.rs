//! `cargo run --release --example explain -- word ...`: typo suggestions per level
//! (`AS_UNKNOWN=1`: treat real words as unknown, like the FP audit).
use layout_keeper_core::{dict, layout, typo};
fn main() {
    let unknown = std::env::var_os("AS_UNKNOWN").is_some();
    for w in std::env::args().skip(1) {
        let Some(l) = layout::detect_script(&w) else { continue };
        for lv in [typo::Level::Cautious, typo::Level::Normal, typo::Level::Aggressive] {
            let r = if unknown { typo::suggest_as_unknown_level(dict::get(l), &w, l, lv) } else { typo::suggest_level(&w, l, lv) };
            println!("{w} {lv:?}: {r:?}");
        }
    }
}
