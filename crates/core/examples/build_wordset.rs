//! Build a full-form FST set: `build_wordset <words.txt> <out.fst>`.
//! Input: one word per line (any order); words are normalised (lowercase, ё → е),
//! anything that is not purely `a-z` / `а-я` is skipped. See `data/README.md`.
use layout_keeper_core::dict::{key_bytes, Dictionary};
fn main() {
    let mut args = std::env::args().skip(1);
    let (src, dst) = (args.next().expect("words.txt"), args.next().expect("out.fst"));
    let text = std::fs::read_to_string(&src).unwrap();
    let mut keys: Vec<Vec<u8>> = text.lines().filter_map(|l| key_bytes(&Dictionary::normalize(l.trim()))).filter(|k| !k.is_empty()).collect();
    keys.sort_unstable();
    keys.dedup();
    let set = fst::Set::from_iter(keys.iter()).unwrap();
    let bytes = set.as_fst().as_bytes();
    std::fs::write(&dst, bytes).unwrap();
    println!("{dst}: {} words, {} bytes", keys.len(), bytes.len());
}
