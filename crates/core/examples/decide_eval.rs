use layout_keeper_core::{typo, Decision, Engine, EngineOptions};
fn main() {
    let e = Engine::new(EngineOptions { fix_typos: true, typo_level: typo::Level::Normal, ..Default::default() });
    for w in std::env::args().skip(1) {
        let s = match e.decide(&w) { Decision::Convert { text, reason, .. } => format!("CONVERT {text} ({reason})"), Decision::Keep(r) => format!("KEEP ({r})") };
        println!("{w} -> {s}");
    }
}
