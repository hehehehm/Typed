use layout_keeper_core::dict::{self, Dictionary};
use layout_keeper_core::layout::Lang;
use std::collections::{HashMap, HashSet};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lang = if args[1] == "ru" { Lang::Ru } else { Lang::En };
    let list = std::fs::read_to_string(&args[2]).unwrap();
    let freqs = std::fs::read_to_string(&args[3]).unwrap();
    let min_zipf: f64 = args[4].parse().unwrap();
    let min_len: usize = args[5].parse().unwrap();
    let d = dict::get(lang);
    let zipf: HashMap<&str, f64> = freqs
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter_map(|(w, f)| f.parse::<f64>().ok().map(|f| (w, (f * 1e9).log10())))
        .collect();
    let base: Vec<&str> = list.lines().map(str::trim).filter(|w| !w.is_empty() && !w.starts_with('#')).collect();
    let have: HashSet<&str> = base.iter().copied().collect();
    let raw: Vec<f64> = base.iter().map(|w| zipf.get(w).copied().unwrap_or(f64::NAN)).collect();
    let window = 200;
    let mut curve = vec![0.0; base.len()];
    for i in 0..base.len() {
        let lo = i.saturating_sub(window);
        let hi = (i + window).min(base.len());
        let mut v: Vec<f64> = raw[lo..hi].iter().copied().filter(|z| !z.is_nan()).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        curve[i] = if v.is_empty() { 0.0 } else { v[v.len() / 2] };
    }
    for i in 1..curve.len() {
        if curve[i] > curve[i - 1] {
            curve[i] = curve[i - 1];
        }
    }
    let mut added: Vec<(&str, f64)> = zipf
        .iter()
        .filter(|(w, z)| **z >= min_zipf && !have.contains(**w) && w.chars().count() >= min_len)
        .filter(|(w, _)| d.in_full_forms(w) && !d.is_brand(w) && !d.is_name_form(w) && Dictionary::normalize(w) == **w)
        .map(|(w, z)| (*w, *z))
        .collect();
    added.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(b.0)));
    let mut out = Vec::with_capacity(base.len() + added.len());
    let mut j = 0;
    for (i, w) in base.iter().enumerate() {
        while j < added.len() && added[j].1 > curve[i] {
            out.push(added[j].0);
            j += 1;
        }
        out.push(w);
    }
    for (w, _) in &added[j..] {
        out.push(w);
    }
    eprintln!("base {} added {} inserted_before_end {}", base.len(), added.len(), j);
    for w in out {
        println!("{w}");
    }
}
