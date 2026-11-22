use layout_keeper_core::{dict, layout, ngram, typo, Config, Decision, Engine, Lang};
fn main() {
    let mut o = Config::default().engine_options();
    o.typo_level = typo::Level::Normal;
    o.fix_typos = true;
    o.fix_missing_spaces = true;
    let e = Engine::new(o);
    for lang in [Lang::Ru, Lang::En] {
        let mut forms = Vec::new();
        dict::get(lang).visit_full_forms(|w| forms.push(w.to_string()));
        for w in forms {
            if let Decision::Convert { text, reason, .. } = e.decide(&w) {
                let l2 = layout::detect_script(&text).unwrap_or(lang);
                let t = dict::Dictionary::normalize(&text);
                println!("{w}\t{text}\t{reason}\t{}\t{}\t{:?}\t{:?}", ngram::get(lang).count(&[&w]), ngram::get(l2).count(&[t.trim_matches(|c: char| !c.is_alphabetic())]), dict::get(lang).rank(&w), dict::get(l2).rank(&t));
            }
        }
    }
}
