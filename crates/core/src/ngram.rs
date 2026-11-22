
use crate::dict::{from_key_bytes, key_bytes, Dictionary};
use crate::layout::Lang;
use fst::Streamer;
use std::sync::OnceLock;

static EN_NGRAMS: &[u8] = include_bytes!("../data/en_ngrams.fst");
static RU_NGRAMS: &[u8] = include_bytes!("../data/ru_ngrams.fst");

pub const START: &str = "<s>";

pub fn quantise(c: u64) -> u64 {
    if c == 0 {
        0
    } else {
        ((c as f64).log2() * 4.0).round() as u64 + 1
    }
}

pub fn dequantise(q: u64) -> f64 {
    if q == 0 {
        0.0
    } else {
        2f64.powf((q - 1) as f64 / 4.0)
    }
}

pub struct Ngrams {
    map: Option<fst::Map<&'static [u8]>>,
    total: f64,
}

impl Ngrams {
    fn new(bytes: &'static [u8]) -> Self {
        let map = fst::Map::new(bytes).ok();
        let mut n = Self { map, total: 0.0 };
        n.total = n.raw(&[0x02]);
        n
    }

    fn raw(&self, key: &[u8]) -> f64 {
        self.map.as_ref().and_then(|m| m.get(key)).map_or(0.0, dequantise)
    }

    pub fn count(&self, ws: &[&str]) -> f64 {
        let mut k = Vec::with_capacity(24);
        for (i, w) in ws.iter().enumerate() {
            if i > 0 {
                k.push(b' ');
            }
            if *w == START {
                k.push(0x01);
            } else {
                match key_bytes(w) {
                    Some(b) if !b.is_empty() => k.extend(b),
                    _ => return 0.0,
                }
            }
        }
        self.raw(&k)
    }

    pub fn is_empty(&self) -> bool {
        self.total <= 0.0
    }

    pub fn total(&self) -> f64 {
        self.total
    }

    pub fn visit(&self, mut f: impl FnMut(&[String], f64)) {
        let Some(map) = &self.map else { return };
        let mut stream = map.stream();
        while let Some((k, v)) = stream.next() {
            if k == [0x02] {
                continue;
            }
            let mut parts: Vec<String> = Vec::new();
            let mut ok = true;
            for chunk in k.split(|&b| b == b' ') {
                if chunk == [0x01] {
                    parts.push("<s>".into());
                    continue;
                }
                match from_key_bytes(chunk) {
                    Some(w) if !w.is_empty() => parts.push(w),
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && !parts.is_empty() {
                f(&parts, crate::ngram::dequantise(v));
            }
        }
    }
}

pub fn get(lang: Lang) -> &'static Ngrams {
    static EN: OnceLock<Ngrams> = OnceLock::new();
    static RU: OnceLock<Ngrams> = OnceLock::new();
    match lang {
        Lang::En => EN.get_or_init(|| Ngrams::new(EN_NGRAMS)),
        Lang::Ru => RU.get_or_init(|| Ngrams::new(RU_NGRAMS)),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub prev: Option<String>,
    pub prev2: Option<String>,
}

impl Context {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.prev.is_none()
    }

    pub fn of(words: &[&str]) -> Self {
        let n = |w: &&str| if *w == START { START.to_string() } else { Dictionary::normalize(w) };
        let mut it = words.iter().rev();
        let prev = it.next().map(n);
        let prev2 = it.next().map(n);
        Self { prev, prev2 }
    }
}

pub const MIN_PREV: f64 = 20.0;
pub const FIT_MAX: f64 = 4.0;

pub fn fit(ctx: &Context, w: &str, lang: Lang) -> Option<f64> {
    let g = get(lang);
    let prev = ctx.prev.as_deref()?;
    if g.is_empty() {
        return None;
    }
    let cp = g.count(&[prev]);
    if cp < MIN_PREV {
        return None;
    }
    let cw = g.count(&[w]);
    let p_w = cw.max(0.5) / g.total();
    let c2 = g.count(&[prev, w]);
    let mut s = ((c2 + 0.5) / (cp * p_w + 0.5)).ln();
    if let Some(prev2) = ctx.prev2.as_deref() {
        let c12 = g.count(&[prev2, prev]);
        if c12 >= MIN_PREV / 2.0 {
            let c3 = g.count(&[prev2, prev, w]);
            let expected = c12 * (c2 + 0.5) / (cp + 1.0);
            s += 0.5 * ((c3 + 0.5) / (expected + 0.5)).ln();
        }
    }
    Some(s.clamp(-FIT_MAX, FIT_MAX))
}

pub const VETO_EXPECTED: f64 = 12.0;

fn absent_after(prev: &str, w: &str, lang: Lang) -> bool {
    let g = get(lang);
    if g.is_empty() {
        return false;
    }
    let cp = g.count(&[prev]);
    let cw = g.count(&[w]);
    cp >= MIN_PREV && cw > 0.0 && g.count(&[prev, w]) == 0.0 && cp * cw / g.total() >= VETO_EXPECTED
}
pub const TYPED_FIT: f64 = 1.5;

pub fn veto(ctx: &Context, typed: &str, fix: &str, lang: Lang) -> bool {
    if ctx.prev.as_deref().is_none_or(|p| p == START) {
        return false;
    }
    let (t, f) = (Dictionary::normalize(typed), Dictionary::normalize(fix));
    if f.contains(' ') {
        return false;
    }
    let Some(sf) = fit(ctx, &f, lang) else { return false };
    if ctx.prev.as_deref().is_some_and(|p| absent_after(p, &f, lang)) {
        return true;
    }
    let g = get(lang);
    if g.count(&[&t]) > 0.0 {
        if let Some(st) = fit(ctx, &t, lang) {
            return st >= TYPED_FIT && st > sf + 1.0;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantise_roundtrip() {
        for c in [1u64, 2, 10, 1000, 123_456] {
            let back = dequantise(quantise(c));
            assert!((back / c as f64 - 1.0).abs() < 0.1, "{c} -> {back}");
        }
        assert_eq!(dequantise(0), 0.0);
    }
}
