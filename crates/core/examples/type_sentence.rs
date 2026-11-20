//! `cargo run --release --example type_sentence -- "text" ...`: types each text key by key
//! through the engine (typo fixing on, context on) at every level and prints the result.
use layout_keeper_core::{typo, Engine, EngineOptions, Key};
fn type_text(e: &mut Engine, text: &str) -> String {
    e.reset();
    let mut out: Vec<char> = Vec::new();
    for k in text.chars().map(Key::Char).chain([Key::Boundary('\n')]) {
        if let Key::Char(c) = k { out.push(c); }
        match e.feed(k, false) {
            Some(r) => {
                let keep = out.len().saturating_sub(r.delete);
                out.truncate(keep);
                out.extend(r.insert.chars());
            }
            None => if let Key::Boundary(c) = k { out.push(c); },
        }
    }
    let mut s: String = out.into_iter().collect();
    if s.ends_with('\n') { s.pop(); }
    s
}
fn main() {
    for t in std::env::args().skip(1) {
        for lv in [typo::Level::Cautious, typo::Level::Normal, typo::Level::Aggressive] {
            let mut e = Engine::new(EngineOptions { fix_typos: true, typo_level: lv, ..Default::default() });
            println!("{lv:?}: {}", type_text(&mut e, &t));
        }
    }
}
