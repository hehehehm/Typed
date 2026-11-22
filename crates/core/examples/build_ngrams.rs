use layout_keeper_core::dict::key_bytes;
use layout_keeper_core::ngram::quantise;

fn key(ws: &[&str]) -> Option<Vec<u8>> {
    let mut k = Vec::new();
    for (i, w) in ws.iter().enumerate() {
        if i > 0 {
            k.push(b' ');
        }
        if *w == "<s>" {
            k.push(0x01);
        } else {
            let b = key_bytes(w)?;
            if b.is_empty() {
                return None;
            }
            k.extend(b);
        }
    }
    Some(k)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (src, dst) = (args.next().expect("ngrams.tsv"), args.next().expect("out.fst"));
    let max_bi: usize = args.next().map_or(1_000_000, |s| s.parse().unwrap());
    let max_tri: usize = args.next().map_or(500_000, |s| s.parse().unwrap());
    let text = std::fs::read_to_string(&src).unwrap();
    let mut by_n: [Vec<(Vec<u8>, u64)>; 3] = [vec![], vec![], vec![]];
    let mut total = 0u64;
    for line in text.lines() {
        let Some((g, c)) = line.split_once('\t') else { continue };
        let c: u64 = c.parse().unwrap();
        let ws: Vec<&str> = g.split(' ').collect();
        if ws.len() == 1 && ws[0] != "<s>" {
            total += c;
        }
        if let Some(k) = key(&ws) {
            by_n[ws.len() - 1].push((k, c));
        }
    }
    let mut all: Vec<(Vec<u8>, u64)> = std::mem::take(&mut by_n[0]);
    for (n, max) in [(1usize, max_bi), (2, max_tri)] {
        let mut v = std::mem::take(&mut by_n[n]);
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let min = v.get(max.min(v.len()).saturating_sub(1)).map_or(0, |x| x.1);
        v.truncate(max);
        eprintln!("{}-grams kept {} (count >= {min})", n + 1, v.len());
        all.extend(v);
    }
    all.push((vec![0x02], total));
    let mut all: Vec<(Vec<u8>, u64)> = all.into_iter().map(|(k, c)| (k, quantise(c))).collect();
    all.sort();
    all.dedup_by(|a, b| a.0 == b.0);
    let map = fst::Map::from_iter(all.iter().map(|(k, v)| (k.as_slice(), *v))).unwrap();
    let bytes = map.as_fst().as_bytes();
    std::fs::write(&dst, bytes).unwrap();
    println!("{dst}: {} keys, {} bytes, {total} tokens", all.len(), bytes.len());
}
