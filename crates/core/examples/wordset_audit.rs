//! Before/after audit of the full-form sets (`dict::set_full_forms_enabled`).
//! `cargo run --release -p layout_keeper_core --example wordset_audit -- <ru_full.txt> <en_full.txt>`
//! (hermitdave FrequencyWords `*_full.txt`: "word count" lines; not bundled.)
//! 1. Real-looking rare words (frequency list ranks 40k+, count >= 5) must not be "fixed".
//! 2. Synthetic typos of ranked words (neighbour-key slip or swap) should still be fixed.
//! 3. Engine: rare correct words must not be converted to the other layout; frequent
//!    words typed in the wrong layout must still be converted.
use layout_keeper_core::{dict::{self, Dictionary}, engine::{Decision, Engine, EngineOptions}, layout::{self, Lang}, typo};

const ROWS_EN: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
const ROWS_RU: [&str; 3] = ["йцукенгшщзхъ", "фывапролджэ", "ячсмитьбю"];

fn neighbours(c: char, lang: Lang) -> Vec<char> {
    let rows = if lang == Lang::En { ROWS_EN } else { ROWS_RU };
    for (r, row) in rows.iter().enumerate() {
        if let Some(p) = row.chars().position(|k| k == c) {
            let mut out = Vec::new();
            for (rr, rw) in rows.iter().enumerate() {
                if (rr as i32 - r as i32).abs() > 1 { continue; }
                for (pp, k) in rw.chars().enumerate() {
                    if k != c && (pp as i32 - p as i32).abs() <= 1 { out.push(k); }
                }
            }
            return out;
        }
    }
    Vec::new()
}

struct Rng(u64);
impl Rng { fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 } }

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = [(Lang::Ru, args.first().cloned()), (Lang::En, args.get(1).cloned())];
    for (lang, path) in paths {
        let d = dict::get(lang);
        let Some(path) = path else { continue };
        let text = std::fs::read_to_string(path).unwrap();
        let rare: Vec<String> = text.lines().skip(40_000).filter_map(|l| {
            let mut it = l.split_whitespace();
            let (w, n) = (it.next()?, it.next()?.parse::<u32>().ok()?);
            let w = Dictionary::normalize(w);
            (n >= 5 && w.chars().count() >= 4 && layout::detect_script(&w) == Some(lang) && w.chars().all(char::is_alphabetic) && !d.contains(&w)).then_some(w)
        }).collect();
        let sample: Vec<&String> = rare.iter().step_by((rare.len() / 5000).max(1)).collect();
        let mut ranked: Vec<(&str, u32)> = d.iter().filter(|&(w, r)| (1000..20_000).contains(&r) && w.chars().count() >= 5).collect();
        ranked.sort_by_key(|&(_, r)| r);
        let mut rng = Rng(0x9E3779B97F4A7C15);
        let mut typos = Vec::new();
        for (w, _) in ranked.iter().step_by(4) {
            let mut c: Vec<char> = w.chars().collect();
            let i = 1 + (rng.next() as usize) % (c.len() - 1);
            if rng.next() % 2 == 0 && i + 1 < c.len() { c.swap(i, i + 1); } else {
                let nb = neighbours(c[i], lang);
                if nb.is_empty() { continue; }
                c[i] = nb[(rng.next() as usize) % nb.len()];
            }
            let t: String = c.into_iter().collect();
            if !d.contains(&t) { typos.push((t, w.to_string())); }
        }
        let wrong: Vec<(String, String)> = ranked.iter().step_by(4).map(|(w, _)| (layout::convert(w, lang), w.to_string())).collect();
        println!("== {lang:?}: full-form set {} words; {} rare list words, sampled {}; {} synthetic typos", d.known_len(), rare.len(), sample.len(), typos.len());
        for on in [false, true] {
            dict::set_full_forms_enabled(on);
            let changed: Vec<String> = sample.iter().filter_map(|w| typo::suggest(w, lang).map(|f| format!("{w}→{f}"))).collect();
            let fixed = typos.iter().filter(|(t, w)| typo::suggest(t, lang).as_deref() == Some(w.as_str())).count();
            let wrongfix = typos.iter().filter(|(t, w)| typo::suggest(t, lang).is_some_and(|f| f != *w)).count();
            let mut eo = EngineOptions::default();
            eo.fix_typos = true;
            let e = Engine::new(eo);
            let conv_rare: Vec<String> = sample.iter().filter_map(|w| match e.decide(w) { Decision::Convert { text, target, .. } if target != lang => Some(format!("{w}→{text}")), _ => None }).collect();
            let conv_ok = wrong.iter().filter(|(t, w)| matches!(e.decide(t), Decision::Convert { ref text, .. } if text == w)).count();
            println!("  full forms {}: rare words changed by typo fix {}/{} ({:.1}%), e.g. {}", if on { "ON " } else { "OFF" }, changed.len(), sample.len(), 100.0 * changed.len() as f64 / sample.len() as f64, changed.iter().take(12).cloned().collect::<Vec<_>>().join(", "));
            println!("                 synthetic typos fixed right {}/{} ({:.1}%), fixed wrong {}", fixed, typos.len(), 100.0 * fixed as f64 / typos.len() as f64, wrongfix);
            println!("                 engine: rare words sent to other layout {} ({:.2}%) e.g. {}; wrong-layout words converted {}/{} ({:.1}%)", conv_rare.len(), 100.0 * conv_rare.len() as f64 / sample.len() as f64, conv_rare.iter().take(8).cloned().collect::<Vec<_>>().join(", "), conv_ok, wrong.len(), 100.0 * conv_ok as f64 / wrong.len() as f64);
        }
    }
}
