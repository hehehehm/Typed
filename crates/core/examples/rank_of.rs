use layout_keeper_core::{dict, layout, ngram};
fn main() {
    for w in std::env::args().skip(1) {
        let Some(l) = layout::detect_script(&w) else { continue };
        println!("{w}: rank {:?} count {:.0}", dict::get(l).rank_exact(&w), ngram::get(l).count(&[&w]));
    }
}
