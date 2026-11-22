use layout_keeper_core::{dict, layout, typo, Config, Decision, Engine};
fn main() {
    let mut o = Config::default().engine_options();
    o.typo_level = typo::Level::Normal;
    o.fix_typos = true;
    o.fix_missing_spaces = true;
    let e = Engine::new(o);
    for w in std::env::args().skip(1) {
        let Some(l) = layout::detect_script(&w) else { println!("{w}: no script"); continue };
        let c = layout::convert(&w, l);
        let lw = w.to_lowercase();
        let lc = c.to_lowercase();
        let d = e.decide(&w);
        let s = match d { Decision::Convert { text, reason, .. } => format!("CONVERT {text} ({reason})"), Decision::Keep(r) => format!("KEEP ({r})") };
        println!("{w} -> {s} | src rank {:?} full {} | conv {c} rank {:?} full {}", dict::get(l).rank(&lw), dict::get(l).in_full_forms(&lw), dict::get(l.other()).rank(&lc), dict::get(l.other()).in_full_forms(&lc));
    }
}
