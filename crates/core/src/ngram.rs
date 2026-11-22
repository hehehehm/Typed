//! Word n-gram context model (since 0.3.72): how well a word fits the one or two words
//! typed before it. Used by the typo fixer to pick between close candidates and to veto a
//! fix that does not fit (or when the typed word itself fits better).
//!
//! Data: `data/{ru,en}_ngrams.fst`, an fst map built by `tools/ngrams/` from Tatoeba
//! sentences (CC BY 2.0 FR) and a Wikipedia dump part (CC BY-SA 4.0), see `data/README.md`.
//! Keys are [`key_bytes`] words joined by a space (`<s>` = sentence start = byte 0x01),
//! values are counts quantised by [`quantise`]. Unigrams, pruned bigrams and trigrams.
//! Memory-mapped from the binary (like the full-form sets): no start-up cost.
//!
//! Scores are evidence-aware log ratios: observed count vs the count expected if the word
//! did not depend on its context (`ln((seen + ½) / (expected + ½))`), so a pair never seen
//! after a rare word means nothing, after a frequent one it counts against the word.

use crate::dict::{from_key_bytes, key_bytes, Dictionary};
use crate::layout::Lang;
use fst::Streamer;
use std::sync::OnceLock;

static EN_NGRAMS: &[u8] = include_bytes!("../data/en_ngrams.fst");
static RU_NGRAMS: &[u8] = include_bytes!("../data/ru_ngrams.fst");

/// Sentence-start token.
pub const START: &str = "<s>";

/// Count → stored value: `round(4 · log2(count)) + 1` (0 = absent).
pub fn quantise(c: u64) -> u64 {
    if c == 0 {
        0
    } else {
        ((c as f64).log2() * 4.0).round() as u64 + 1
    }
}

/// Stored value → approximate count.
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

    /// Approximate count of an n-gram of normalised words (`<s>` allowed first).
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

    /// Total tokens counted.
    pub fn total(&self) -> f64 {
        self.total
    }

    /// Walk every n-gram in the model. Special keys (`<s>`, the total sentinel) are
    /// skipped. Each item is the list of normalised words and the approximate count.
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

/// The context model of `lang`.
pub fn get(lang: Lang) -> &'static Ngrams {
    static EN: OnceLock<Ngrams> = OnceLock::new();
    static RU: OnceLock<Ngrams> = OnceLock::new();
    match lang {
        Lang::En => EN.get_or_init(|| Ngrams::new(EN_NGRAMS)),
        Lang::Ru => RU.get_or_init(|| Ngrams::new(RU_NGRAMS)),
    }
}

/// Left context of the word being judged: the previous word (or [`START`]) and the one
/// before it, normalised (lowercase, ё → е). Empty = unknown (after a click, at the start
/// of the field, after a sign that breaks the sentence such as `)` or a digit).
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

    /// From plain words, oldest first (`["я", "не"]`); `<s>` may come first.
    pub fn of(words: &[&str]) -> Self {
        let n = |w: &&str| if *w == START { START.to_string() } else { Dictionary::normalize(w) };
        let mut it = words.iter().rev();
        let prev = it.next().map(n);
        let prev2 = it.next().map(n);
        Self { prev, prev2 }
    }
}

/// The previous word must be seen at least this often for its context to count.
pub const MIN_PREV: f64 = 20.0;
/// Clamp of [`fit`].
pub const FIT_MAX: f64 = 4.0;

/// How well `w` (normalised) fits `ctx`: positive = seen more often after these words than
/// chance, negative = expected there but not seen. `None` without a usable context.
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

/// A fix never seen after the previous word although chance alone would give it this many
/// times there is vetoed (`в` + `вы`, `the` + `are`). Rarer misses mean little: the
/// tables are pruned and a small corpus misses many good pairs.
pub const VETO_EXPECTED: f64 = 12.0;

/// `w` was never seen after `prev` although at least [`VETO_EXPECTED`] times were expected.
fn absent_after(prev: &str, w: &str, lang: Lang) -> bool {
    let g = get(lang);
    if g.is_empty() {
        return false;
    }
    let cp = g.count(&[prev]);
    let cw = g.count(&[w]);
    cp >= MIN_PREV && cw > 0.0 && g.count(&[prev, w]) == 0.0 && cp * cw / g.total() >= VETO_EXPECTED
}
/// The typed word itself fits this well, and clearly better than the fix: keep it.
pub const TYPED_FIT: f64 = 1.5;

/// Context says no to changing `typed` into `fix` (both any case): the fix is never seen
/// after the previous word where it would be expected often, or the typed word (a name, a code word the corpus knows)
/// fits clearly better. Sentence start alone never vetoes.
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
