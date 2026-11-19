//! Worst-case time of one `typo::suggest` call (run with `--ignored --nocapture`, release),
//! without and with a left context (the n-gram model scores every close candidate).
use layout_keeper_core::{dict, layout::Lang, ngram::Context, typo};
use std::time::Instant;

fn measure(lang: Lang, words: &[String], ctx: &Context) -> (f64, f64, f64, String) {
    let mut t: Vec<(f64, &String)> = Vec::new();
    for w in words {
        let s = Instant::now();
        let _ = typo::suggest_ctx(w, lang, typo::Level::default(), ctx);
        t.push((s.elapsed().as_secs_f64() * 1000.0, w));
    }
    t.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let n = t.len();
    let mean = t.iter().map(|x| x.0).sum::<f64>() / n as f64;
    (mean, t[n * 99 / 100].0, t[n - 1].0, t[n - 1].1.clone())
}

#[test]
#[ignore]
fn latency() {
    for lang in [Lang::Ru, Lang::En] {
        let d = dict::get(lang);
        let _ = typo::suggest("warmup", lang);
        let mut words: Vec<String> = Vec::new();
        let mut list: Vec<(String, u32)> = d.iter().map(|(w, r)| (w.to_string(), r)).collect();
        list.sort_by_key(|x| x.1);
        for (k, (w, _)) in list.iter().enumerate() {
            if k % 13 != 0 || w.chars().count() < 4 { continue; }
            let mut v: Vec<char> = w.chars().collect();
            // a few slips + gibberish of the same length
            let i = 1 + k % (v.len() - 1);
            let c = v.remove(i);
            words.push(v.iter().collect());
            v.insert(i, c);
            v.insert(i, c);
            words.push(v.iter().collect());
            words.push(v.iter().rev().collect());
        }
        for long in ["превысокомногорассмотрительствующий", "привественный", "ярдык", "pneumonoultramicroscopic", "antidisestablishmentarianizm"] {
            words.push(long.to_string());
        }
        let with = match lang { Lang::Ru => Context::of(&["я", "не"]), Lang::En => Context::of(&["i", "will"]) };
        for (name, ctx) in [("no context", Context::none()), ("context", with)] {
            let (mean, p99, max, worst) = measure(lang, &words, &ctx);
            eprintln!("{lang:?} {name}: {} words, mean {mean:.3} ms, p99 {p99:.3} ms, max {max:.3} ms ({worst})", words.len());
        }
    }
}
