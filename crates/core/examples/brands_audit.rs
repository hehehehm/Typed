//! Brand list audit: a brand must never stand in the way of a real word typed in the
//! wrong layout, and brands typed in the wrong layout should convert.
//! `cargo run --release -p layout_keeper_core --example brands_audit`
use layout_keeper_core::{dict, engine::{Decision, Engine, EngineOptions}, layout::{self, Lang}};

fn main() {
    let e = Engine::new(EngineOptions::default());
    for lang in [Lang::Ru, Lang::En] {
        let other = lang.other();
        let od = dict::get(other);
        // 1. every ranked word typed in the other layout: its junk reading is not a brand there,
        //    and the engine still converts it (when it did not hit another rule).
        let list = match lang { Lang::Ru => include_str!("../data/ru.txt"), Lang::En => include_str!("../data/en.txt") };
        let (mut n, mut collide, mut blocked) = (0, Vec::new(), Vec::new());
        for w in list.lines().map(str::trim).filter(|w| w.chars().count() >= 4) {
            let typed = layout::convert(w, lang);
            if layout::detect_script(&typed) != Some(other) || !typed.chars().all(char::is_alphabetic) { continue; }
            n += 1;
            if od.is_brand(&typed) { collide.push(format!("{w}/{typed}")); }
            if od.is_brand(&typed) && !matches!(e.decide(&typed), Decision::Convert { .. }) { blocked.push(format!("{w}/{typed}")); }
        }
        println!("{lang:?} ranked words typed in the {other:?} layout: {n}; reading is a {other:?} brand: {} {:?}; conversion blocked: {} {:?}",
            collide.len(), &collide[..collide.len().min(10)], blocked.len(), &blocked[..blocked.len().min(10)]);
        // 2. brands typed in the wrong layout
        let d = dict::get(lang);
        let brands: Vec<&str> = dict::brands_list(lang).lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
        let (mut tot, mut conv, mut kept) = (0, 0, 0);
        for b in &brands {
            if matches!(e.decide(b), Decision::Keep(_)) { kept += 1; }
            let typed = layout::convert(b, lang);
            if !typed.chars().all(char::is_alphabetic) { continue; }
            tot += 1;
            match e.decide(&typed) { Decision::Convert { text, .. } if text == *b => conv += 1, _ => {} }
        }
        let _ = d;
        println!("{lang:?} brands: {}; typed in the wrong layout (letters only) and converted back: {conv}/{tot} ({:.1}%); typed right and kept: {kept}/{}",
            brands.len(), 100.0 * conv as f64 / tot.max(1) as f64, brands.len());
    }
}
