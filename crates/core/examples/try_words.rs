use layout_keeper_core::{layout, typo, Engine, EngineOptions};
fn main() {
    let e = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
    for w in std::env::args().skip(1) {
        let lang = layout::detect_script(&w);
        let strict = lang.and_then(|l| typo::suggest_level(&w, l, typo::Level::Cautious));
        println!("{w}: {:?} | strict typo: {strict:?}", e.decide(&w));
    }
}
