use layout_keeper_core::{dict, engine::Decision, layout, layout::Lang, Engine, EngineOptions};
use std::io::Write;
fn main() {
    use fst::{IntoStreamer, Streamer};
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let e = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
    for lang in [Lang::Ru, Lang::En] {
        let tag = match lang { Lang::Ru => "ru", Lang::En => "en" };
        let d = dict::get(lang);
        let mut ranked: Vec<(String, u32)> = d.iter().map(|(w, r)| (w.to_string(), r)).collect();
        ranked.sort_by_key(|x| x.1);
        let mut full = vec![];
        let set = fst::Set::new(dict::full_forms_fst(lang)).unwrap();
        let mut s = set.into_stream();
        while let Some(k) = s.next() {
            let w = dict::key_to_word(k);
            if !d.contains(&w) { full.push(w); }
        }
        for (name, words) in [("ranked", ranked.into_iter().map(|x| x.0).collect::<Vec<_>>()), ("full", full)] {
            let mut f = std::fs::File::create(format!("{out}/{tag}_{name}.tsv")).unwrap();
            let (mut n, mut conv) = (0usize, 0usize);
            for w in &words {
                if !w.chars().all(char::is_alphabetic) || layout::detect_script(w) != Some(lang) || w.chars().count() < 2 { continue; }
                n += 1;
                if let Decision::Convert { text, target, reason } = e.decide(w) {
                    if target != lang {
                        conv += 1;
                        writeln!(f, "{w}\t{text}\t{reason}").unwrap();
                    }
                }
            }
            println!("{tag} {name}: {conv} of {n} real words layout-converted");
        }
    }
}
