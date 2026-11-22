use layout_keeper_core::{typo, Config, Engine, Key};

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/typist_real_failures.txt");

struct Case {
    category: String,
    typed: String,
    expected: String,
}

fn load_cases() -> Vec<Case> {
    let text = std::fs::read_to_string(DATA).expect("typist_real_failures.txt");
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut p = l.split('\t');
            Some(Case {
                category: p.next()?.trim().to_string(),
                typed: p.next()?.trim().to_string(),
                expected: p.next()?.trim().to_string(),
            })
        })
        .filter(|c| !c.typed.is_empty() && !c.expected.is_empty())
        .collect()
}

fn field_after(level: typo::Level, text: &str) -> String {
    let mut opt = Config::default().engine_options();
    opt.typo_level = level;
    let mut e = Engine::new(opt);
    let mut out: Vec<char> = Vec::new();
    for k in text.chars().map(Key::Char).chain([Key::Boundary('\n')]) {
        if let Key::Char(c) = k {
            out.push(c);
        }
        match e.feed(k, false) {
            Some(r) => {
                let keep = out.len().saturating_sub(r.delete);
                out.truncate(keep);
                out.extend(r.insert.chars());
            }
            None => {
                if let Key::Boundary(c) = k {
                    out.push(c);
                }
            }
        }
    }
    out.into_iter().collect::<String>().trim_end().to_string()
}

#[test]
fn typist_real_failures_are_100_percent_at_normal() {
    let cases = load_cases();
    assert!(!cases.is_empty(), "typist_real_failures.txt is empty");
    let mut failed: Vec<(String, String, String, String)> = Vec::new();
    let mut by_cat: std::collections::BTreeMap<String, (u32, u32)> = Default::default();
    for c in &cases {
        let got = field_after(typo::Level::Normal, &c.typed);
        let e = by_cat.entry(c.category.clone()).or_insert((0, 0));
        e.0 += 1;
        if got != c.expected {
            e.1 += 1;
            failed.push((c.category.clone(), c.typed.clone(), got, c.expected.clone()));
        }
    }
    println!("typist_real_failures: {} cases at Normal", cases.len());
    for (cat, (n, f)) in &by_cat {
        println!("  {cat}: {}/{} pass ({} fail)", n - f, n, f);
    }
    if !failed.is_empty() {
        for (cat, typed, got, exp) in &failed {
            println!("FAIL [{cat}] {typed:?} → {got:?} (wanted {exp:?})");
        }
        panic!("{} of {} real-failure cases missed", failed.len(), cases.len());
    }
}
