use layout_keeper_core::{dict, layout, split};
fn main() {
    for w in std::env::args().skip(1) {
        let Some(l) = layout::detect_script(&w) else { continue };
        let d = dict::get(l);
        let g = split::split_glued(d, &w, l);
        let m = split::lm_segment(d, &w, l).map(|s| format!("{} c={} drop={}", s.text, s.count, s.dropped));
        println!("{w}: glued={g:?} lm={m:?}");
    }
}
