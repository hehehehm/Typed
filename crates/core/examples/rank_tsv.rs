//! stdin: `typed\t...` lines; stdout: line + `\tsrc_rank\tsrc_full\tdst_rank\tdst_full`.
use layout_keeper_core::{dict, layout};
use std::io::BufRead;
fn main() {
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let t = line.split('\t').next().unwrap().to_string();
        let core: String = t.chars().filter(|c| c.is_alphabetic()).collect::<String>().to_lowercase();
        let Some(l) = layout::detect_script(&t) else { continue };
        let c: String = layout::convert(&t, l).chars().filter(|c| c.is_alphabetic()).collect::<String>().to_lowercase();
        let r = |l, w: &str| dict::get(l).rank(w).map(|r| r as i64).unwrap_or(-1);
        println!("{line}\t{}\t{}\t{}\t{}", r(l, &core), dict::get(l).in_full_forms(&core) as u8, r(l.other(), &c), dict::get(l.other()).in_full_forms(&c) as u8);
    }
}
