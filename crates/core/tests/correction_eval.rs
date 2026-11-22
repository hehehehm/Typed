use layout_keeper_core::{dict, dict::Dictionary, engine::Decision, layout, layout::Lang, typo, Engine, EngineOptions};
use std::collections::BTreeMap;
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
    match rng.next(5) {
        0 => { let nb = neighbours(v[i], lang); if nb.is_empty() { return false; } v[i] = nb[rng.next(nb.len())]; }
        1 => { if i + 1 >= n || v[i] == v[i + 1] { return false; } v.swap(i, i + 1); }
        2 => { if n < 3 { return false; } v.remove(i); }
        3 => { let nb = neighbours(v[i], lang); if nb.is_empty() { return false; } v.insert(i + rng.next(2), nb[rng.next(nb.len())]); }
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

fn capitalise(w: &str) -> String {
    let mut c = w.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn level() -> typo::Level {
    match std::env::var("EVAL_LEVEL").as_deref() {
        Ok("cautious") => typo::Level::Cautious,
        Ok("aggressive") => typo::Level::Aggressive,
        _ => typo::Level::Normal,
    }
}

fn engine() -> Engine {
    Engine::new(EngineOptions { fix_typos: true, typo_level: level(), ..Default::default() })
}

static EXPECT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
fn expect(w: &str) { *EXPECT.lock().unwrap() = w.to_string(); }
fn out_text(e: &Engine, token: &str) -> Option<String> {
    let r = match e.decide(token) {
        Decision::Convert { text, .. } => Some(text),
        Decision::Keep(_) => None,
    };
    if let Ok(p) = std::env::var("EVAL_DUMP") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{token}\t{}\t{}", r.clone().unwrap_or_default(), EXPECT.lock().unwrap());
        }
    }
    r
}

fn pool(lang: Lang, len: usize, n: usize) -> Vec<String> {
    let d = dict::get(lang);
    let limit = if len <= 3 { 3_000 } else { 20_000 };
    let mut v: Vec<(String, u32)> = d
        .iter()
        .filter(|(w, r)| *r < limit && w.chars().count() == len && w.chars().all(char::is_alphabetic) && layout::detect_script(w) == Some(lang))
        .map(|(w, r)| (w.to_string(), r))
        .collect();
    v.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    if v.len() <= n { return v.into_iter().map(|x| x.0).collect(); }
    (0..n).map(|i| v[i * v.len() / n].0.clone()).collect()
}

#[derive(Default, Clone, Copy)]
struct Cell { total: u32, right: u32, partial: u32, wrong: u32 }
impl Cell {
    fn fmt(&self) -> String {
        if self.total == 0 { return "—".into(); }
        let p = |x: u32| 100.0 * x as f64 / self.total as f64;
        if self.partial > 0 {
            return format!("{:.0}% (+{:.0}%) / {:.1}%", p(self.right), p(self.partial), p(self.wrong));
        }
        format!("{:.0}% / {:.1}%", p(self.right), p(self.wrong))
    }
}

const PER_CELL: usize = 120;

#[test]
#[ignore]
fn correction_eval() {
    let label = std::env::var("EVAL_LABEL").unwrap_or_else(|_| "run".into());
    let mut report = format!("# Correction eval: {label}\n\nCells: recall (fixed right) / wrong fix, per word length; for layout+typo `(+N%)` = layout switched but the slip left. Typo 2 from 4 letters, typo 3 from 6.\n\n");
    let e = engine();
    let mut wrong_ex: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut miss_ex: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let kinds = ["typo 1", "typo 2", "typo 3", "layout", "layout+typo"];
    let mut times: Vec<(f64, String)> = Vec::new();
    for lang in [Lang::Ru, Lang::En] {
        let d = dict::get(lang);
        let mut rng = Rng(7);
        let mut ex_rng = Rng(11);
        let mut table: BTreeMap<(&str, usize), Cell> = BTreeMap::new();
        let mut totals: BTreeMap<&str, Cell> = BTreeMap::new();
        for len in 2..=12 {
            for w in pool(lang, len, PER_CELL) {
                let cap = rng.next(6) == 0;
                for kind in kinds {
                    if (kind == "typo 2" && len < 4) || (kind == "typo 3" && len < 6) { continue; }
                    let typed = match kind {
                        "typo 1" => slips(&w, 1, lang, &mut rng),
                        "typo 2" => slips(&w, 2, lang, &mut rng),
                        "typo 3" => slips(&w, 3, lang, &mut rng),
                        "layout" => Some(w.clone()),
                        _ => slips(&w, 1, lang, &mut rng),
                    };
                    let Some(typed) = typed else { continue };
                    let is_layout = kind.starts_with("layout");
                    if !is_layout && d.is_word(&typed) { continue; }
                    let typed = if cap { capitalise(&typed) } else { typed };
                    let typed = if is_layout {
                        let t = layout::convert(&typed, lang);
                        if dict::get(lang.other()).is_word(&t) { continue; }
                        if kind == "layout+typo" && d.is_word(&Dictionary::normalize(&layout::convert(&t, lang.other()))) { continue; }
                        t
                    } else { typed };
                    let t0 = std::time::Instant::now();
                    expect(&format!("{kind}|{w}"));
                    let got = out_text(&e, &typed);
                    times.push((t0.elapsed().as_secs_f64() * 1000.0, typed.clone()));
                    let half = is_layout && got.as_deref().is_some_and(|g| Dictionary::normalize(g) != w && Dictionary::normalize(g) == Dictionary::normalize(&layout::convert(&typed, lang.other())));
                    let cell = table.entry((kind, len)).or_default();
                    let tot = totals.entry(kind).or_default();
                    cell.total += 1; tot.total += 1;
                    match got {
                        Some(g) if Dictionary::normalize(&g) == w => { cell.right += 1; tot.right += 1; }
                        Some(_) if half => { cell.partial += 1; tot.partial += 1; }
                        Some(g) => {
                            cell.wrong += 1; tot.wrong += 1;
                            let ex = wrong_ex.entry(format!("{lang:?} {kind}")).or_default();
                            if ex.len() < 12 { ex.push(format!("{typed}→{g} ({w})")); }
                        }
                        None => {
                            let ex = miss_ex.entry(format!("{lang:?} {kind}")).or_default();
                            if ex.len() < 12 && ex_rng.next(4) == 0 { ex.push(format!("{typed} ({w})")); }
                        }
                    }
                }
            }
        }
        let _ = writeln!(report, "## {lang:?}: recall / wrong\n\n| kind | {} | all |", (2..=12).map(|l| l.to_string()).collect::<Vec<_>>().join(" | "));
        let _ = writeln!(report, "|---|{}---|", "---|".repeat(11));
        for kind in kinds {
            let cells: Vec<String> = (2..=12).map(|l| table.get(&(kind, l)).copied().unwrap_or_default().fmt()).collect();
            let t = totals.get(kind).copied().unwrap_or_default();
            let _ = writeln!(report, "| {kind} | {} | **{}** (n={}) |", cells.join(" | "), t.fmt(), t.total);
        }
        report.push('\n');
    }

    report += "## False positives: unknown-word proxy (changed %)\n\n| set | 2-3 | 4 | 5-6 | 7-8 | 9-12 | all |\n|---|---|---|---|---|---|---|\n";
    let bucket = |n: usize| match n { 0..=3 => 0, 4 => 1, 5..=6 => 2, 7..=8 => 3, _ => 4 };
    for lang in [Lang::Ru, Lang::En] {
        let d = dict::get(lang);
        let ranked = dict::ranked_only(lang);
        let words = full_only(lang, &ranked);
        for (name, cap) in [("lower", false), ("Capitalised", true)] {
            let mut b = [(0u32, 0u32); 5];
            let mut ex = vec![];
            for w in &words {
                let n = w.chars().count();
                let t = if cap { capitalise(w) } else { w.clone() };
                let changed = typo::suggest_as_unknown_level(d, &t, lang, level()).or_else(|| layout_keeper_core::typo_wide::layout_typo_as_unknown_level(&t, lang, level()));
                let k = bucket(n);
                b[k].0 += 1;
                if let Some(f) = &changed {
                    if let Ok(p) = std::env::var("EVAL_DUMP_FP") {
                        use std::io::Write as _;
                        if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open(p) { let _ = writeln!(fh, "{lang:?} {name}\t{t}\t{f}"); }
                    }
                }
                if let Some(f) = changed { b[k].1 += 1; if ex.len() < 12 { ex.push(format!("{t}→{f}")); } }
            }
            let all = b.iter().fold((0, 0), |a, x| (a.0 + x.0, a.1 + x.1));
            let pc = |x: (u32, u32)| if x.0 == 0 { "—".to_string() } else { format!("{:.1}%", 100.0 * x.1 as f64 / x.0 as f64) };
            let _ = writeln!(report, "| {lang:?} {name} | {} | **{}** (n={}) |", b.iter().map(|x| pc(*x)).collect::<Vec<_>>().join(" | "), pc(all), all.0);
            wrong_ex.insert(format!("FP unknown {lang:?} {name}"), ex);
        }
    }
    let mut changed = vec![];
    let curated = curated();
    for t in &curated {
        expect("curated|");
        if let Some(g) = out_text(&e, t) { changed.push(format!("{t}→{g}")); }
    }
    let _ = writeln!(report, "\n## Curated FP set (names, brands, code, URLs, slang): changed {}/{} ({:.1}%)\n\n{:?}\n", changed.len(), curated.len(), 100.0 * changed.len() as f64 / curated.len() as f64, changed);
    let mut known_changed = vec![];
    let mut n_known = 0;
    for lang in [Lang::Ru, Lang::En] {
        for len in 2..=12 {
            for w in pool(lang, len, 60) {
                n_known += 1;
                expect(&format!("known|{w}"));
                if let Some(g) = out_text(&e, &w) { known_changed.push(format!("{w}→{g}")); }
            }
        }
    }
    let _ = writeln!(report, "## Known frequent words changed: {}/{}\n\n{:?}\n", known_changed.len(), n_known, known_changed);
    times.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let n = times.len().max(1);
    let mean = times.iter().map(|t| t.0).sum::<f64>() / n as f64;
    let _ = writeln!(report, "## Engine::decide latency (recall cases): mean {mean:.2} ms, p99 {:.2} ms, max {:.2} ms ({})\n", times[n * 99 / 100].0, times[n - 1].0, times[n - 1].1);
    report += "## Wrong-fix examples\n\n";
    for (k, v) in &wrong_ex { let _ = writeln!(report, "* {k}: {}", v.join(", ")); }
    report += "\n## Miss examples\n\n";
    for (k, v) in &miss_ex { let _ = writeln!(report, "* {k}: {}", v.join(", ")); }
    println!("{report}");
    if let Ok(p) = std::env::var("EVAL_OUT") { std::fs::write(p, &report).unwrap(); }
}

fn full_only(lang: Lang, ranked: &Dictionary) -> Vec<String> {
    use fst::{IntoStreamer, Streamer};
    let set = fst::Set::new(dict::full_forms_fst(lang)).unwrap();
    let mut s = set.into_stream();
    let mut out = vec![];
    let mut k = 0usize;
    while let Some(key) = s.next() {
        k += 1;
        if k % 97 != 0 { continue; }
        let w = dict::key_to_word(key);
        let n = w.chars().count();
        if !(3..=12).contains(&n) || ranked.contains(&w) || !w.chars().all(char::is_alphabetic) || layout::detect_script(&w) != Some(lang) { continue; }
        out.push(w);
        if out.len() >= 3000 { break; }
    }
    out
}

fn curated() -> Vec<String> {
    let s = "
Вася Петя Колян Сашка Женёк Лёха Толик Серёга Наташка Ксюха Иваныч Михалыч Петрович Кузьмич
Шмидт Цукерберг Маск Илон Джобс Гейтс Безос Тинькофф Сбер Яндекс Озон Авито Вайлдберриз Хабр
Пелевин Достоевский Тургенев Акунин Шнуров Моргенштерн Киркоров Пугачёва Галкин Ургант
Kolya Vasya Petrov Ivanov Sasha Misha Dmitry Sergey Natasha Olga Svetlana Tatyana Zhenya
Musk Elon Zuckerberg Bezos Satya Nadella Pichai Altman Karpathy Torvalds Stroustrup Guido
Google Yandex GitHub GitLab Docker Kubernetes Nginx Postgres Redis Kafka Grafana Jira Figma Notion
Slack Zoom Telegram WhatsApp Viber TikTok Instagram Youtube Netflix Spotify Steam Twitch Discord
iPhone iPad macOS Xiaomi Huawei Samsung Lenovo Asus Acer Realme Oppo Vivo Honor Nokia
npm yarn pnpm cargo rustc rustup clippy serde tokio axum actix webpack vite eslint prettier
println eprintln struct impl enum async await const usize isize fn mut pub mod crate
getElementById addEventListener useState useEffect componentDidMount setTimeout
snake_case some_var my_func user_id is_ok max_len api_key
kebab-case foo-bar x-api-key
https://example.com www.github.com user@mail.ru foo.bar.baz C:\\Users\\me ./src/main.rs
HTTP HTTPS JSON YAML TOML SQL CSS HTML API REST CLI GUI CPU GPU RAM SSD USB VPN DNS TCP UDP
ООО ИП ЗАО США РФ МГУ МФТИ СПб ЖКХ ГИБДД МВД ФСБ НДС ИНН СНИЛС
лол кек хз ща чё чо ничо норм пасиб спс плз пжлст имхо кринж краш рофл хайп изи
сабж оффтоп лайк репост апдейт фикс баг фича коммит пуш пулл мерж деплой
загран универ препод инфа проц видос клава инет конфа собес курсач училка телега общага велик лаба ноут комп
ок окей ага угу неа мда хм эх ох ах ой ай ну
lol omg wtf brb idk imo tbh afaik btw thx pls plz ty np gg wp ez
hElLo WoRlD tEsT PaSsWoRd QwErTy
Kpop Bizon Manul Shmel Kotik Pesik
Ленка Анька Димон Вован Игорёк Славик Жорик Гоша Тёма Сеня Стасик
Мурманск Сыктывкар Нарьян Ханты Мытищи Химки Балашиха Подольск Люберцы Чебоксары
Tallinn Riga Vilnius Kyiv Lviv Minsk Almaty Tashkent Bishkek Tbilisi Yerevan Baku
";
    s.split_whitespace().map(str::to_string).collect()
}

fn spelling_error(w: &str, lang: Lang, rng: &mut Rng) -> Option<String> {
    let v: Vec<char> = w.chars().collect();
    let n = v.len();
    let groups: &[(char, char)] = &[('а', 'о'), ('е', 'и'), ('е', 'а'), ('е', 'я'), ('и', 'я'), ('а', 'я')];
    let mut opts: Vec<Vec<char>> = vec![];
    for i in 1..n {
        if v[i - 1] == v[i] {
            let mut u = v.clone();
            u.remove(i);
            opts.push(u);
        }
        let vowel = "аеиоуыэюяaeiouy".contains(v[i]);
        if !vowel && v[i - 1] != v[i] && v.get(i + 1) != Some(&v[i]) {
            let mut u = v.clone();
            u.insert(i, v[i]);
            opts.push(u);
        }
        if lang == Lang::Ru {
            for &(a, b) in groups {
                for (x, y) in [(a, b), (b, a)] {
                    if v[i] == x {
                        let mut u = v.clone();
                        u[i] = y;
                        opts.push(u);
                    }
                }
            }
        }
    }
    if opts.is_empty() {
        return None;
    }
    let doubles: Vec<&Vec<char>> = opts.iter().filter(|u| u.len() + 1 == n).collect();
    let pick = if !doubles.is_empty() && rng.next(2) == 0 { doubles[rng.next(doubles.len())].clone() } else { opts[rng.next(opts.len())].clone() };
    Some(pick.into_iter().collect())
}

#[test]
#[ignore]
fn spelling_eval() {
    let e = engine();
    let mut report = String::from("# Spelling-error eval\n\n| set | right | wrong | n |\n|---|---|---|---|\n");
    let mut ex: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for lang in [Lang::Ru, Lang::En] {
        let d = dict::get(lang);
        let ranked = dict::ranked_only(lang);
        let mut rare: Vec<String> = full_only(lang, &ranked).into_iter().filter(|w| w.chars().count() >= 5).collect();
        rare.truncate(1500);
        let mut freq: Vec<String> = vec![];
        for len in 5..=12 { freq.extend(pool(lang, len, 120)); }
        for (set, words) in [("frequent", &freq), ("rare form", &rare)] {
            for k in [1usize, 2] {
                let mut rng = Rng(23 + k as u64);
                let (mut n, mut right, mut wrong) = (0u32, 0u32, 0u32);
                for w in words.iter() {
                    let mut t = Some(w.clone());
                    for _ in 0..k { t = t.and_then(|x| spelling_error(&x, lang, &mut rng)); }
                    let Some(t) = t else { continue };
                    if d.is_word(&t) || t == *w { continue; }
                    n += 1;
                    expect(&format!("spell {set} {k}|{w}"));
                    match out_text(&e, &t) {
                        Some(g) if Dictionary::normalize(&g) == *w => right += 1,
                        Some(g) => { wrong += 1; let x = ex.entry(format!("{lang:?} {set} {k} wrong")).or_default(); if x.len() < 10 { x.push(format!("{t}→{g} ({w})")); } }
                        None => { let x = ex.entry(format!("{lang:?} {set} {k} miss")).or_default(); if x.len() < 10 { x.push(format!("{t} ({w})")); } }
                    }
                }
                let pc = |x: u32| if n == 0 { 0.0 } else { 100.0 * x as f64 / n as f64 };
                let _ = writeln!(report, "| {lang:?} {set}, {k} error(s) | {:.0}% | {:.1}% | {n} |", pc(right), pc(wrong));
            }
        }
    }
    report += "\n## Examples\n\n";
    for (k, v) in &ex { let _ = writeln!(report, "* {k}: {}", v.join(", ")); }
    println!("{report}");
    if let Ok(p) = std::env::var("EVAL_OUT") { std::fs::write(p, &report).unwrap(); }
}
