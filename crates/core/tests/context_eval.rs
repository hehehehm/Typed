//! In-sentence evaluation of the context model (run with
//! `cargo test --release -p layout_keeper_core --test context_eval -- --ignored --nocapture`).
//!
//! Held-out sentences (`tests/data/heldout_{ru,en}.txt`, Tatoeba sentences whose id is a
//! multiple of 50, never counted into the n-gram tables) are typed key by key through the
//! engine (`fix_typos` on), with the context model on and off:
//! * `clean` — the sentence as is: any change of the final text is a false positive.
//! * `typo1`, `typo2`, `layout`, `layout+typo` — one dictionary word (3+ letters) of a
//!   sentence that came through clean in both modes is corrupted (realistic slips, the other
//!   layout, or both; corruptions that make a real word are skipped). `right` = the final
//!   text is the original sentence, `wrong` = something else changed, the rest stayed.
//!   `mid` = only targets with a previous word (where context can help).
use layout_keeper_core::{dict, dict::Dictionary, layout, layout::Lang, ngram, typo, Engine, EngineOptions, Key};
use std::fmt::Write as _;

const RU_ROWS: [&str; 3] = ["йцукенгшщзхъ", "фывапролджэ", "ячсмитьбю"];
const EN_ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

fn neighbours(c: char, lang: Lang) -> Vec<char> {
    let rows: Vec<Vec<char>> = match lang { Lang::Ru => RU_ROWS, Lang::En => EN_ROWS }.iter().map(|r| r.chars().collect()).collect();
    let Some((r, p)) = rows.iter().enumerate().find_map(|(r, row)| row.iter().position(|&k| k == c).map(|p| (r as i32, p as i32))) else { return vec![] };
    let mut out = vec![];
    for (dr, dc) in [(0, -1), (0, 1), (1, -1), (1, 0), (-1, 0), (-1, 1)] {
        let (rr, cc) = (r + dr, p + dc);
        if rr >= 0 && (rr as usize) < rows.len() && cc >= 0 {
            if let Some(&k) = rows[rr as usize].get(cc as usize) { out.push(k); }
        }
    }
    out
}

struct Rng(u64);
impl Rng {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

fn slip(v: &mut Vec<char>, lang: Lang, rng: &mut Rng) -> bool {
    let n = v.len();
    if n < 2 { return false; }
    let i = 1 + rng.next(n - 1);
    let lower = v[i].to_lowercase().next().unwrap_or(v[i]);
    match rng.next(5) {
        0 => { let nb = neighbours(lower, lang); if nb.is_empty() { return false; } v[i] = nb[rng.next(nb.len())]; }
        1 => { if i + 1 >= n || v[i] == v[i + 1] { return false; } v.swap(i, i + 1); }
        2 => { if n < 3 { return false; } v.remove(i); }
        3 => { let nb = neighbours(lower, lang); if nb.is_empty() { return false; } v.insert(i + rng.next(2), nb[rng.next(nb.len())]); }
        _ => { let c = v[i]; v.insert(i, c); }
    }
    true
}

fn slips(w: &str, k: usize, lang: Lang, rng: &mut Rng) -> Option<String> {
    let mut v: Vec<char> = w.chars().collect();
    for _ in 0..k {
        let mut ok = false;
        for _ in 0..4 { if slip(&mut v, lang, rng) { ok = true; break; } }
        if !ok { return None; }
    }
    let t: String = v.into_iter().collect();
    (t != w).then_some(t)
}

fn level() -> typo::Level {
    match std::env::var("EVAL_LEVEL").as_deref() {
        Ok("cautious") => typo::Level::Cautious,
        Ok("aggressive") => typo::Level::Aggressive,
        _ => typo::Level::Normal,
    }
}

/// Types `text` key by key (then Enter) and returns what the field holds.
fn type_text(e: &mut Engine, text: &str) -> String {
    e.reset();
    let mut out: Vec<char> = Vec::new();
    let keys = text.chars().map(Key::Char).chain([Key::Boundary('\n')]);
    for k in keys {
        // A typed character is in the field before the engine sees it; Enter is not
        // (the replacement types it itself).
        if let Key::Char(c) = k { out.push(c); }
        match e.feed(k, false) {
            Some(r) => {
                let keep = out.len().saturating_sub(r.delete);
                out.truncate(keep);
                out.extend(r.insert.chars());
            }
            None => {
                if let Key::Boundary(c) = k { out.push(c); }
            }
        }
    }
    let mut s: String = out.into_iter().collect();
    if s.ends_with('\n') { s.pop(); }
    s
}

/// Word spans (char offsets) of `s`: runs of letters.
fn words(s: &[char]) -> Vec<(usize, usize)> {
    let mut v = vec![];
    let mut i = 0;
    while i < s.len() {
        if s[i].is_alphabetic() {
            let j = (i..s.len()).find(|&j| !s[j].is_alphabetic()).unwrap_or(s.len());
            v.push((i, j));
            i = j;
        } else {
            i += 1;
        }
    }
    v
}

/// Context of word `i` as the engine would see it: up to two previous words joined by
/// spaces or commas, `<s>` after a sentence end or at the start.
fn sentence_context(chars: &[char], ws: &[(usize, usize)], i: usize) -> ngram::Context {
    let mut v: Vec<String> = vec![];
    let mut j = i;
    loop {
        let gap_start = if j == 0 { 0 } else { ws[j - 1].1 };
        let gap: String = chars[gap_start..ws[j].0].iter().collect();
        if j == 0 || gap.contains(['.', '!', '?']) {
            if j == 0 && !gap.trim().is_empty() && !gap.chars().all(|c| c.is_whitespace() || "\"«—-".contains(c)) { break; }
            v.push(ngram::START.into());
            break;
        }
        if !gap.chars().all(|c| c.is_whitespace() || ",;:-—–\"«»".contains(c)) { break; }
        j -= 1;
        v.push(chars[ws[j].0..ws[j].1].iter().collect());
        if v.len() == 2 { break; }
    }
    v.reverse();
    let refs: Vec<&str> = v.iter().map(String::as_str).collect();
    ngram::Context::of(&refs)
}

#[derive(Default, Clone, Copy)]
struct Cell { total: u32, right: u32, partial: u32, wrong: u32 }
impl Cell {
    fn add(&mut self, right: bool, partial: bool, wrong: bool) { self.total += 1; self.right += right as u32; self.partial += partial as u32; self.wrong += wrong as u32; }
    fn fmt(&self) -> String {
        if self.total == 0 { return "—".into(); }
        let p = |x: u32| 100.0 * x as f64 / self.total as f64;
        if self.partial > 0 {
            return format!("{:.1}% (+{:.1}%) / {:.2}%", p(self.right), p(self.partial), p(self.wrong));
        }
        format!("{:.1}% / {:.2}%", p(self.right), p(self.wrong))
    }
}

const RARE_RANK: u32 = 20_000;
const KINDS: [&str; 4] = ["typo1", "typo2", "layout", "layout+typo"];

#[test]
#[ignore]
fn context_eval() {
    let label = std::env::var("EVAL_LABEL").unwrap_or_else(|_| "run".into());
    let max: usize = std::env::var("EVAL_SENTENCES").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    let dir = std::env::var("EVAL_HELDOUT").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data").into());
    let mut report = String::new();
    writeln!(report, "# Context eval: {label} (level {:?})\n", level()).unwrap();
    writeln!(report, "Cells: right (+ layout fixed, typo kept) / wrong (share of cases). `all` = every target, `mid` = targets after a word.\n").unwrap();
    for lang in [Lang::Ru, Lang::En] {
        let tag = match lang { Lang::Ru => "ru", Lang::En => "en" };
        let Ok(text) = std::fs::read_to_string(format!("{dir}/heldout_{tag}.txt")) else { eprintln!("no held-out sentences for {tag}"); continue };
        let sents: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(max).collect();
        let d = dict::get(lang);
        let mut engines: Vec<Engine> = [true, false]
            .iter()
            .map(|&c| Engine::new(EngineOptions { fix_typos: true, typo_level: level(), use_context: c, ..Default::default() }))
            .collect();
        let mut clean_fp = [0u32; 2];
        let mut fp_examples: [Vec<String>; 2] = [vec![], vec![]];
        let mut cells = [[[Cell::default(); 2]; 2]; 4]; // kind, all/mid, ctx on/off
        let mut diff_examples: Vec<String> = vec![];
        let mut wrong_examples: Vec<String> = vec![];
        let mut unknown = [0u32; 3];
        let mut unknown_rare = [0u32; 3];
        let mut unknown_examples: Vec<String> = vec![];
        let mut ex_rng = Rng(0xe5 ^ lang as u64);
        let mut usable = vec![];
        for s in &sents {
            let mut ok = true;
            for (m, e) in engines.iter_mut().enumerate() {
                let got = type_text(e, s);
                if got != *s {
                    ok = false;
                    clean_fp[m] += 1;
                    if fp_examples[m].len() < 12 { fp_examples[m].push(format!("`{s}` → `{got}`")); }
                }
            }
            if ok { usable.push(*s); }
        }
        for s in &usable {
            // Per-sentence random streams: a sentence dropped from `usable` by a change in
            // the engine must not shift the samples of all the following ones.
            let h = s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
            let mut rng = Rng(0x5eed ^ lang as u64 ^ h);
            let mut pick_rng = Rng(0xe5 ^ lang as u64 ^ h.rotate_left(17));
            let chars: Vec<char> = s.chars().collect();
            let ws = words(&chars);
            let cands: Vec<usize> = (0..ws.len())
                .filter(|&i| {
                    let w: String = chars[ws[i].0..ws[i].1].iter().collect();
                    w.chars().count() >= 3 && layout::detect_script(&w) == Some(lang) && d.is_word(&Dictionary::normalize(&w))
                })
                .collect();
            if cands.is_empty() { continue; }
            // Unknown-word proxy in a real sentence: the word treated as unknown (a name or
            // slang the lists miss) — would it be changed, with and without its context?
            for &ui in cands.iter().filter(|_| pick_rng.next(2) == 0) {
                let (a, b) = ws[ui];
                let w: String = chars[a..b].iter().collect();
                let ctx = sentence_context(&chars, &ws, ui);
                let on = typo::suggest_as_unknown_ctx(d, &w, lang, level(), &ctx);
                let off = typo::suggest_as_unknown_ctx(d, &w, lang, level(), &ngram::Context::none());
                let rare = d.rank(&Dictionary::normalize(&w)).is_none_or(|r| r >= RARE_RANK);
                let u = if rare { &mut unknown_rare } else { &mut unknown };
                u[0] += on.is_some() as u32;
                u[1] += off.is_some() as u32;
                u[2] += 1;
                if rare && on.is_none() && off.is_some() && unknown_examples.len() < 15 {
                    unknown_examples.push(format!("`{w}` (→ `{}` without context) after {:?}", off.unwrap(), ctx.prev));
                }
            }
            let ti = cands[rng.next(cands.len())];
            let (a, b) = ws[ti];
            let w: String = chars[a..b].iter().collect();
            let mid = ti > 0 && !chars[..a].iter().rev().take_while(|c| !c.is_alphabetic()).any(|c| matches!(c, '.' | '!' | '?'));
            for (k, kind) in KINDS.iter().enumerate() {
                let t = match *kind {
                    "typo1" => slips(&w, 1, lang, &mut rng),
                    "typo2" => slips(&w, 2, lang, &mut rng),
                    "layout" => Some(layout::convert(&w, lang)),
                    _ => slips(&w, 1, lang, &mut rng).map(|t| layout::convert(&t, lang)),
                };
                let Some(t) = t else { continue };
                // A wrong-layout word followed by a sign would have that sign typed in the
                // other layout too; keep to words followed by a space.
                if kind.starts_with("layout") && chars.get(b) != Some(&' ') { continue; }
                let back = if kind.starts_with("layout") { layout::convert(&t, lang.other()) } else { t.clone() };
                if d.is_word(&Dictionary::normalize(&back)) && back.to_lowercase() != w.to_lowercase() { continue; }
                if kind.starts_with("layout") && dict::get(lang.other()).is_word(&Dictionary::normalize(&t)) { continue; }
                let typed: String = chars[..a].iter().chain(t.chars().collect::<Vec<_>>().iter()).chain(chars[b..].iter()).collect();
                // Layout fixed, typo kept: half done.
                let half: String = chars[..a].iter().copied().chain(back.chars()).chain(chars[b..].iter().copied()).collect();
                let mut res = [String::new(), String::new()];
                for (m, e) in engines.iter_mut().enumerate() {
                    let got = type_text(e, &typed);
                    // ё and е are the same letter to the dictionaries.
                    let e = |x: &str| x.replace('ё', "е").replace('Ё', "Е");
                    let right = e(&got) == e(s);
                    let partial = !right && *kind == "layout+typo" && got == half;
                    let wrong = !right && !partial && got != typed;
                    cells[k][0][m].add(right, partial, wrong);
                    if m == 0 && wrong && wrong_examples.len() < 60 && ex_rng.next(4) == 0 {
                        wrong_examples.push(format!("{kind}: `{w}`→`{t}`: `{got}`"));
                    }
                    if mid { cells[k][1][m].add(right, partial, wrong); }
                    res[m] = got;
                }
                if res[0] != res[1] && diff_examples.len() < 40 {
                    let mark = |g: &str| if g == *s { "✓" } else if g == typed { "·" } else { "✗" };
                    diff_examples.push(format!("{kind}: `{w}`→`{t}` in `{s}`: ctx {} `{}`, no ctx {} `{}`", mark(&res[0]), res[0], mark(&res[1]), res[1]));
                }
            }
        }
        writeln!(report, "## {tag}: {} sentences, {} clean in both modes\n", sents.len(), usable.len()).unwrap();
        writeln!(report, "| case | ctx on (all) | ctx off (all) | ctx on (mid) | ctx off (mid) | n (all/mid) |").unwrap();
        writeln!(report, "|---|---|---|---|---|---|").unwrap();
        for (k, kind) in KINDS.iter().enumerate() {
            let c = &cells[k];
            writeln!(report, "| {kind} | {} | {} | {} | {} | {}/{} |", c[0][0].fmt(), c[0][1].fmt(), c[1][0].fmt(), c[1][1].fmt(), c[0][0].total, c[1][0].total).unwrap();
        }
        let pu = |u: &[u32; 3], x: u32| 100.0 * x as f64 / u[2].max(1) as f64;
        writeln!(report, "\nUnknown-word proxy in sentences (each word treated as missing from the lists; changed with / without context):").unwrap();
        writeln!(report, "- rare words (rank ≥ {RARE_RANK} or unranked, {} words): {:.2}% / {:.2}%", unknown_rare[2], pu(&unknown_rare, unknown_rare[0]), pu(&unknown_rare, unknown_rare[1])).unwrap();
        writeln!(report, "- frequent words ({} words, flattering: the corpus knows them): {:.2}% / {:.2}%\n", unknown[2], pu(&unknown, unknown[0]), pu(&unknown, unknown[1])).unwrap();
        writeln!(report, "Rare words kept only thanks to context:").unwrap();
        for x in &unknown_examples { writeln!(report, "- {x}").unwrap(); }
        let p = |x: u32| 100.0 * x as f64 / sents.len() as f64;
        writeln!(report, "\nClean sentences changed: ctx on {} ({:.2}%), ctx off {} ({:.2}%)\n", clean_fp[0], p(clean_fp[0]), clean_fp[1], p(clean_fp[1])).unwrap();
        for (m, name) in ["ctx on", "ctx off"].iter().enumerate() {
            writeln!(report, "FP examples ({name}):").unwrap();
            for x in &fp_examples[m] { writeln!(report, "- {x}").unwrap(); }
            writeln!(report).unwrap();
        }
        writeln!(report, "Wrong fixes with context (sample):").unwrap();
        for x in &wrong_examples { writeln!(report, "- {x}").unwrap(); }
        writeln!(report).unwrap();
        writeln!(report, "Cases where context changed the outcome (✓ right, ✗ wrong, · unchanged):").unwrap();
        for x in &diff_examples { writeln!(report, "- {x}").unwrap(); }
        writeln!(report).unwrap();
    }
    println!("{report}");
    if let Ok(p) = std::env::var("EVAL_OUT") { std::fs::write(p, &report).unwrap(); }
}
