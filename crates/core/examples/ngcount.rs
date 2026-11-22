use layout_keeper_core::{layout, ngram};
fn main() {
    for w in std::env::args().skip(1) {
        let parts: Vec<&str> = w.split(' ').collect();
        let l = layout::detect_script(&w).unwrap();
        println!("{w}: {}", ngram::get(l).count(&parts));
    }
}
