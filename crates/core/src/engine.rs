
use crate::buffer::WordBuffer;
use crate::dict::{self, Dictionary};
use crate::filename;
use crate::layout::{self, Lang};
use crate::morph;
use crate::ngram::Context;
use crate::split;
use crate::typo;
use crate::userdict::UserWords;
use std::collections::HashSet;

const SIGN_WORD_MAX_RANK: u32 = 1000;

const SINGLE_WORD_REASON: &str = "lone letter is a word only in the other language";

fn is_single_letter_word(lang: Lang, c: char) -> bool {
    match lang {
        Lang::En => matches!(c, 'a' | 'i'),
        Lang::Ru => matches!(c, 'а' | 'в' | 'и' | 'к' | 'о' | 'с' | 'у' | 'я'),
    }
}

pub const SHORT_WORD_MAX_RANK: u32 = 3000;
pub const SPLIT_VS_TYPO: f64 = 2.0;
pub const SPLIT_MIN_COUNT: f64 = 8.0;
pub const LM_SPLIT_OVER_TYPO: f64 = 1.0;
pub const REVERT_SINGLE_NEXT_RANK: u32 = 5_000;

pub const HYPHEN_PART_MAX_RANK: u32 = 20_000;

const HYPHEN_PARTICLES: [&str; 6] = ["то", "либо", "нибудь", "таки", "ка", "де"];
pub const FULL_FORM_MIN_LEN: usize = 4;
pub const FULL_FORM_CONVERT_MIN_LEN: usize = 5;
pub const FULL_FORM_YIELD_RANK: u32 = 3_000;
pub const SHORT_SWITCH_SRC_RANK: u32 = 2_000;
pub const SHORT_SWITCH_DST_RANK: u32 = 50;
const SHORT_SWITCH_KEEP: [&str; 1] = ["vs"];
pub const READING_CORPUS_RATIO: f64 = 10.0;
pub const SIGNED_READING_MAX_RANK: u32 = 1_000;

fn reading_likelier(typed: &str, src: Lang, conv: &str, dst: Lang) -> bool {
    let freq = |w: &str, l: Lang| {
        let g = crate::ngram::get(l);
        let n = Dictionary::normalize(w);
        if g.total() > 0.0 { g.count(&[n.as_str()]) / g.total() } else { 0.0 }
    };
    let (fc, ft) = (freq(conv, dst), freq(typed, src));
    fc > 0.0 && fc > READING_CORPUS_RATIO * ft
}

fn form_freq(word: &str, lang: Lang) -> f64 {
    let g = crate::ngram::get(lang);
    let n = Dictionary::normalize(word);
    if g.total() > 0.0 { g.count(&[n.as_str()]) / g.total() } else { 0.0 }
}

fn typo_swap_beats_layout(swap: &str, src: Lang, layout: &str, dst: Lang) -> bool {
    let swap_n = Dictionary::normalize(trim_non_letters(swap));
    let lay_n = Dictionary::normalize(trim_non_letters(layout));
    if swap_n.is_empty() || !swap_n.chars().all(char::is_alphabetic) {
        return false;
    }
    if lay_n.is_empty() || !lay_n.chars().all(char::is_alphabetic) {
        return true;
    }
    if dst == src {
        return false;
    }
    let fs = form_freq(&swap_n, src);
    let fl = form_freq(&lay_n, dst);
    let frequent = dict::get(src).rank(&swap_n).is_some_and(|r| r < typo::MAX_RANK);
    frequent && (fl <= 0.0 || fs >= fl)
}

pub const SPLIT_VS_DEDUPE: f64 = 10.0;

pub const NC_ERROR_RATE: f64 = 0.01;
pub const NC_UNSEEN: f64 = 0.5;
pub const NC_UNKNOWN_RATE: f64 = 0.01;

fn attested_form_wins(token: &str, text: &str, target: Lang) -> bool {
    let Some(src) = layout::detect_script(token) else { return false };
    let core = trim_non_letters(token);
    if !core.chars().all(char::is_alphabetic) {
        return false;
    }
    let lower = Dictionary::normalize(core);
    let d = dict::get(src);
    if d.rank(&lower).is_some() {
        return false;
    }
    let alt_core = trim_non_letters(text);
    if alt_core.contains(' ') || !alt_core.chars().all(char::is_alphabetic) {
        return false;
    }
    if target != src && alt_core.chars().count() != core.chars().count() {
        return false;
    }
    let seen = crate::ngram::get(src).count(&[&lower]);
    let letters: Vec<char> = core.chars().collect();
    let full = d.in_full_forms(&lower);
    let rate = if full {
        if !title_or_all_caps(&letters) && (seen <= 0.0 || target != src) {
            return false;
        }
        NC_ERROR_RATE
    } else {
        if seen <= 0.0 || target != src {
            return false;
        }
        NC_UNKNOWN_RATE
    };
    let typed = seen.max(NC_UNSEEN);
    let alt = crate::ngram::get(target).count(&[&Dictionary::normalize(alt_core)]);
    let scale = |l: Lang| crate::ngram::get(l).total().max(1.0);
    alt / scale(target) * rate < typed / scale(src)
}

#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub autocorrect: bool,
    pub convert_single_letters: bool,
    pub preferred_lang: Option<Lang>,
    pub skip_urls_emails: bool,
    pub skip_tokens_with_digits: bool,
    pub min_word_len: usize,
    pub user_words: UserWords,
    pub fix_typos: bool,
    pub never_correct: HashSet<String>,
    pub undo_on_backspace: bool,
    pub fix_missing_spaces: bool,
    pub typo_level: typo::Level,
    pub never_typo: HashSet<String>,
    pub undone_once: HashSet<String>,
    pub use_context: bool,
}

pub const TYPO_REASON: &str = "typo fix";
pub const MIXED_REASON: &str = "layout switched mid-word";
const MIXED_MIN_LETTERS: usize = 3;
const CASE_ENDING_LETTERS: &str = "аеёиоуыюяй";
const MIXED_SHORT_MAX_RANK: u32 = 5_000;
const BRAND_PREFIX_RANK: u32 = 20_000;
pub const LAYOUT_TYPO_REASON: &str = "wrong layout and a typo";
pub const HAND_SHIFT_REASON: &str = "home-row hand shift";
pub const SPACE_REASON: &str = "missing space after a function word";

impl Default for EngineOptions {
    fn default() -> Self {
        Self { autocorrect: true, convert_single_letters: true, preferred_lang: None, skip_urls_emails: true, skip_tokens_with_digits: true, min_word_len: 2, user_words: UserWords::default(), fix_typos: false, never_correct: HashSet::new(), undo_on_backspace: true, fix_missing_spaces: true, typo_level: typo::Level::default(), never_typo: HashSet::new(), undone_once: HashSet::new(), use_context: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    Boundary(char),
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub delete: usize,
    pub insert: String,
    pub target: Lang,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Keep(&'static str),
    Convert { text: String, target: Lang, reason: &'static str },
}

#[derive(Debug, Default)]
struct LastWord {
    word: String,
    boundary: String,
    fix: Option<AutoFix>,
    fresh: bool,
}

#[derive(Debug, Clone)]
struct AutoFix {
    typed: String,
    typed_boundary: String,
    steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Learned {
    NeverCorrect(String),
    NeverTypo(String),
    UndoneOnce(String),
}

pub const UNDO_HISTORY: usize = 12;

#[derive(Debug, Clone)]
struct RecentFix {
    typed: String,
    erased: bool,
}

impl Decision {
}

#[derive(Debug, Default)]
pub struct Engine {
    pub options: EngineOptions,
    buf: WordBuffer,
    last: Option<LastWord>,
    recent_fix: Option<RecentFix>,
    pending_learn: Option<(String, bool)>,
    learned: Vec<Learned>,
    history: Vec<LastWord>,
    edited_fix: bool,
    layout_nudge: Option<Lang>,
}

fn is_boundary(c: char) -> bool {
    c.is_whitespace() || !(layout::is_word_char(c) || is_url_char(c))
}

fn is_url_char(c: char) -> bool {
    matches!(c, '/' | ':' | '@' | '.' | '-' | '_' | '=' | '&' | '%' | '#' | '+' | '~' | '\\')
}

fn looks_like_url_or_email(t: &str) -> bool {
    let l = t.to_ascii_lowercase();
    l.contains('@') || l.contains("://") || l.starts_with("www.") || l.contains('\\')
        || (l.contains('/') && l.split('/').filter(|s| !s.is_empty()).count() > 1)
}

fn natural_case(letters: &[char]) -> bool {
    let up = letters.iter().filter(|c| c.is_uppercase()).count();
    up == 0 || up == letters.len() || (up == 1 && letters[0].is_uppercase())
}

fn title_or_all_caps(letters: &[char]) -> bool {
    if letters.is_empty() {
        return false;
    }
    let up = letters.iter().filter(|c| c.is_uppercase()).count();
    up == letters.len() || (up == 1 && letters[0].is_uppercase())
}

fn dst_rank_ok(w: &str, r: u32, letters: &[char]) -> bool {
    let n = w.chars().count();
    n > 3
        || r < SHORT_WORD_MAX_RANK
        || (n == 3 && r < HYPHEN_PART_MAX_RANK && title_or_all_caps(letters))
}

fn edge_signs_trimmed(letters: &[char], src: Lang) -> &[char] {
    let sign = |c: &char| !layout::convert_char(*c, src).is_alphabetic();
    let start = letters.iter().take_while(|c| sign(c)).count();
    let end = letters.len() - letters[start..].iter().rev().take_while(|c| sign(c)).count();
    if start < end { &letters[start..end] } else { letters }
}

fn trim_non_letters(s: &str) -> &str {
    s.trim_matches(|c: char| !c.is_alphabetic())
}

impl Engine {
    pub fn new(options: EngineOptions) -> Self {
        Self { options, ..Default::default() }
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.drop_context();
        self.recent_fix = None;
        self.edited_fix = false;
        self.layout_nudge = None;
        self.commit_pending_learn();
    }

    pub fn take_layout_nudge(&mut self) -> Option<Lang> {
        self.layout_nudge.take()
    }

    fn drop_context(&mut self) {
        self.last = None;
        self.history.clear();
    }

    fn commit_pending_learn(&mut self) {
        if let Some((w, typo_only)) = self.pending_learn.take() {
            self.count_undo(&w, typo_only);
        }
    }

    pub fn take_learned(&mut self) -> Vec<Learned> {
        std::mem::take(&mut self.learned)
    }

    fn remember(&mut self, word: &str) {
        let key = never_key(word);
        self.options.undone_once.remove(&key);
        if !key.is_empty() && self.options.never_correct.insert(key.clone()) {
            log::debug!("never-correct: remembered a word of {} chars", key.chars().count());
            self.learned.push(Learned::NeverCorrect(key));
        }
    }

    fn count_undo(&mut self, word: &str, typo_only: bool) {
        let key = never_key(word);
        if key.is_empty() {
            return;
        }
        let once = if typo_only { format!("typo:{key}") } else { key.clone() };
        if !self.options.undone_once.remove(&once) {
            self.options.undone_once.insert(once.clone());
            self.learned.push(Learned::UndoneOnce(once));
            return;
        }
        if typo_only {
            if self.options.never_typo.insert(key.clone()) {
                self.learned.push(Learned::NeverTypo(key));
            }
        } else {
            self.remember(&key);
        }
    }

    pub fn undo_last(&mut self, current_layout: Lang) -> Option<Replacement> {
        if self.buf.is_empty() && self.last.as_ref().is_some_and(|l| l.fix.is_some()) {
            let rep = self.restore_last(current_layout)?;
            self.recent_fix = None;
            self.pending_learn = None;
            if let Some(w) = self.last.as_ref().map(|l| l.word.clone()) {
                self.remember(&w);
            }
            return Some(rep);
        }
        self.undo_earlier(current_layout)
    }

    fn undo_earlier(&mut self, current_layout: Lang) -> Option<Replacement> {
        let last = self.last.take()?;
        let mut all = std::mem::take(&mut self.history);
        all.push(last);
        let sent = |w: &LastWord| w.boundary.contains(['\n', '\t', '\r']);
        let mut target = None;
        for i in (0..all.len()).rev() {
            if sent(&all[i]) {
                break;
            }
            if all[i].fix.is_some() {
                target = Some(i);
                break;
            }
        }
        let rep = target.map(|i| {
            let mut rest = String::new();
            for w in &all[i + 1..] {
                rest.push_str(&w.word);
                rest.push_str(&w.boundary);
            }
            rest.push_str(&self.buf.as_string());
            let w = &mut all[i];
            let fix = w.fix.take().expect("target has a fix");
            let boundary = format!("{}{}", fix.typed_boundary, w.boundary.chars().skip(1).collect::<String>());
            let delete = w.word.chars().count() + w.boundary.chars().count() + rest.chars().count();
            let insert = format!("{}{boundary}{rest}", fix.typed);
            w.word = fix.typed;
            w.boundary = boundary;
            w.fresh = false;
            (w.word.clone(), Replacement { delete, insert, target: current_layout })
        });
        self.last = all.pop();
        self.history = all;
        let (word, rep) = rep?;
        self.recent_fix = None;
        self.remember(&word);
        Some(rep)
    }

    fn restore_last(&mut self, current_layout: Lang) -> Option<Replacement> {
        let last = self.last.as_mut()?;
        let fix = last.fix.take()?;
        let rest: String = last.boundary.chars().skip(1).collect();
        let boundary = format!("{}{rest}", fix.typed_boundary);
        let rep = Replacement {
            delete: last.word.chars().count() + last.boundary.chars().count(),
            insert: format!("{}{boundary}", fix.typed),
            target: layout::detect_script(&fix.typed).unwrap_or(current_layout),
        };
        last.word = fix.typed;
        last.boundary = boundary;
        last.fresh = false;
        Some(rep)
    }

    fn backspace_undo(&mut self, current_layout: Lang) -> Option<Replacement> {
        let last = self.last.as_mut()?;
        if !self.options.undo_on_backspace
            || !last.fresh
            || last.fix.is_none()
            || last.boundary.chars().count() != 1
            || last.boundary.contains(['\n', '\t', '\r'])
        {
            return None;
        }
        if let Some(step) = last.fix.as_mut().and_then(|f| f.steps.pop()) {
            let rep = Replacement {
                delete: last.word.chars().count() + last.boundary.chars().count(),
                insert: format!("{step}{}", last.boundary),
                target: layout::detect_script(&step).unwrap_or(current_layout),
            };
            last.word = step.clone();
            self.pending_learn = Some((step, true));
            return Some(rep);
        }
        let rep = self.restore_last(current_layout)?;
        let typed = self.last.as_ref().map(|l| l.word.clone()).unwrap_or_default();
        self.recent_fix = Some(RecentFix { typed: typed.clone(), erased: false });
        self.pending_learn = Some((typed, false));
        Some(rep)
    }

    pub fn current_len(&self) -> usize {
        self.buf.len()
    }

    pub fn feed(&mut self, key: Key, modifiers_held: bool) -> Option<Replacement> {
        if modifiers_held {
            if key == Key::Backspace {
                self.mark_erased();
                self.buf.clear();
                self.drop_context();
                return None;
            }
            self.reset();
            return None;
        }
        if let Some(l) = self.last.as_mut() {
            if key != Key::Backspace {
                l.fresh = false;
            }
        }
        match key {
            Key::Reset => {
                self.reset();
                None
            }
            Key::Backspace => {
                if self.buf.is_empty() {
                    let layout = self.options.preferred_lang.unwrap_or(Lang::En);
                    if let Some(rep) = self.backspace_undo(layout) {
                        return Some(rep);
                    }
                    if self.reopen_last() {
                        return None;
                    }
                    self.mark_erased();
                    self.drop_context();
                    return None;
                }
                self.buf.backspace();
                if self.buf.is_empty() {
                    self.edited_fix = false;
                    self.mark_erased();
                }
                None
            }
            Key::Char(c) if !self.is_word_end(c) => {
                if !self.buf.push(c) {
                    self.edited_fix = false;
                    self.drop_context();
                }
                None
            }
            Key::Char(c) => self.finish_word(c, true),
            Key::Boundary(c) => self.finish_word(c, false),
        }
    }

    fn mark_erased(&mut self) {
        if self.last.is_some() {
            self.pending_learn = None;
        }
        if let Some(rf) = self.recent_fix.as_mut() {
            rf.erased = true;
        }
    }

    fn is_word_end(&self, c: char) -> bool {
        if is_boundary(c) {
            return true;
        }
        if c.is_alphanumeric() || self.buf.is_empty() {
            return false;
        }
        let token = self.buf.as_string();
        let Some(script) = layout::detect_script(&token) else {
            return false;
        };
        let other = layout::convert_char(c, script.other());
        let same = layout::convert_char(c, script);
        let punct_here = other.is_alphabetic() && other != c;
        let punct_other = !c.is_alphabetic() && same != c && !same.is_alphabetic();
        if !punct_here && !punct_other {
            return false;
        }
        matches!(self.decide(&token), Decision::Convert { .. })
    }

    fn reopen_last(&mut self) -> bool {
        let last = match self.last.as_mut() {
            Some(l) => l,
            None => return false,
        };
        if last.word.is_empty() && last.boundary.is_empty() {
            return false;
        }
        if !last.boundary.is_empty() {
            let mut chs: Vec<char> = last.boundary.chars().collect();
            chs.pop();
            last.boundary = chs.into_iter().collect();
            last.fresh = false;
            if !last.boundary.is_empty() {
                self.layout_nudge = layout::detect_script(&last.word);
                return true;
            }
        }
        let word = std::mem::take(&mut last.word);
        let was_fix = last.fix.is_some();
        self.last = self.history.pop();
        if word.is_empty() {
            return true;
        }
        self.buf.set(&word);
        self.edited_fix = was_fix;
        self.layout_nudge = layout::detect_script(&word);
        true
    }

    fn finish_word(&mut self, boundary: char, already_typed: bool) -> Option<Replacement> {
        let token = self.buf.take();
        let leave_alone = std::mem::take(&mut self.edited_fix);
        if token.is_empty() {
            if let Some(last) = self.last.as_mut() {
                last.fresh = false;
                if last.boundary.chars().count() < 4 {
                    last.boundary.push(boundary);
                    return None;
                }
            }
            self.drop_context();
            return None;
        }
        self.commit_pending_learn();
        if leave_alone {
            self.recent_fix = None;
            let mut b = String::new();
            b.push(boundary);
            self.push_last(LastWord { word: token, boundary: b, fix: None, fresh: false });
            return None;
        }
        if self.recent_fix.take().is_some_and(|rf| rf.erased && never_key(&rf.typed) == never_key(&token)) {
            self.remember(&token);
        }
        let glued = self.last.as_ref().is_some_and(|l| {
            !l.boundary.is_empty() && !l.boundary.chars().any(char::is_whitespace)
        });
        if self.options.autocorrect && !glued {
            if let Some(rep) = self.merge_split_word(&token, boundary, already_typed) {
                return Some(rep);
            }
        }
        let decision = if !self.options.autocorrect {
            Decision::Keep("autocorrect disabled")
        } else if glued {
            Decision::Keep("glued to previous word (password-like)")
        } else {
            match self.decide(&token) {
                Decision::Convert { reason: "single letter to app language" | SINGLE_WORD_REASON, .. }
                    if !(boundary.is_whitespace() || matches!(boundary, ',' | '.' | '?')) =>
                {
                    Decision::Keep("single letter before a sign (password-like)")
                }
                d => d,
            }
        };
        if let Some(rep) = self.revert_single_letter(&token, &decision, boundary, already_typed) {
            return Some(rep);
        }
        let mut b = String::new();
        b.push(boundary);
        match decision {
            Decision::Convert { text, target, reason } => {
                log::debug!("autocorrect: convert word of {} chars to {} ({})", token.chars().count(), target, reason);
                if let Some(src) = layout::detect_script(&token).filter(|&s| s != target) {
                    b = layout::convert_punctuation(&b, src);
                }
                let typed = if already_typed { 1 } else { 0 };
                let rep = Replacement {
                    delete: token.chars().count() + typed,
                    insert: format!("{text}{b}"),
                    target,
                };
                let mut steps = Vec::new();
                if reason == LAYOUT_TYPO_REASON {
                    let chars: Vec<char> = token.chars().collect();
                    if let Some((_, plain)) = layout::detect_script(&token).and_then(|src| self.layout_typo_decision(&chars, src)) {
                        steps.push(plain);
                    }
                }
                let fix = AutoFix { typed: token.clone(), typed_boundary: boundary.to_string(), steps };
                self.recent_fix = Some(RecentFix { typed: token, erased: false });
                self.push_last(LastWord { word: text, boundary: b, fix: Some(fix), fresh: true });
                Some(rep)
            }
            Decision::Keep(reason) => {
                log::debug!("autocorrect: keep word of {} chars ({})", token.chars().count(), reason);
                self.push_last(LastWord { word: token, boundary: b, fix: None, fresh: false });
                None
            }
        }
    }

    fn revert_single_letter(&mut self, token: &str, decision: &Decision, boundary: char, already_typed: bool) -> Option<Replacement> {
        let last = self.last.as_ref()?;
        let fix = last.fix.as_ref()?;
        if !last.fresh && last.boundary != " " {
            return None;
        }
        if last.boundary != " " || fix.typed.chars().count() != 1 || last.word.chars().count() != 1 {
            return None;
        }
        let orig = layout::detect_script(&fix.typed)?;
        if layout::detect_script(&last.word) != Some(orig.other()) {
            return None;
        }
        let (text, target) = match decision {
            Decision::Keep(_) => (token.to_string(), layout::detect_script(token)?),
            Decision::Convert { text, target, .. } => (text.clone(), *target),
        };
        if target != orig || text.chars().count() < 2 || !text.chars().all(|c| c.is_alphabetic() && c.is_lowercase()) {
            return None;
        }
        if !dict::get(orig).rank(&text).is_some_and(|r| r < REVERT_SINGLE_NEXT_RANK) {
            return None;
        }
        if self.history.last().is_some_and(|h| !h.boundary.contains(['.', '!', '?', '\n']) && layout::detect_script(&h.word) == Some(orig.other())) {
            return None;
        }
        let typed_len = if already_typed { 1 } else { 0 };
        let delete = last.word.chars().count() + last.boundary.chars().count() + token.chars().count() + typed_len;
        let letter = fix.typed.clone();
        let mut bnd = String::new();
        bnd.push(boundary);
        if target != layout::detect_script(token).unwrap_or(target) {
            bnd = layout::convert_punctuation(&bnd, target.other());
        }
        let rep = Replacement { delete, insert: format!("{letter} {text}{bnd}"), target: orig };
        let lw = self.last.as_mut()?;
        lw.word = letter;
        lw.fix = None;
        lw.fresh = false;
        let fix = if text != token {
            Some(AutoFix { typed: token.to_string(), typed_boundary: boundary.to_string(), steps: Vec::new() })
        } else {
            None
        };
        self.push_last(LastWord { word: text, boundary: bnd, fix, fresh: false });
        Some(rep)
    }

    fn merge_split_word(&mut self, token: &str, boundary: char, already_typed: bool) -> Option<Replacement> {
        if !self.options.fix_missing_spaces || !self.options.fix_typos || self.options.typo_level == typo::Level::Cautious {
            return None;
        }
        let last = self.last.as_ref()?;
        if last.boundary != " " {
            return None;
        }
        let b = token;
        let src = layout::detect_script(b)?;
        if !b.chars().all(char::is_alphabetic) {
            return None;
        }
        let a_typed = match &last.fix {
            Some(f) if f.steps.is_empty() && layout::detect_script(&f.typed) == Some(src) && !f.typed.contains(' ') => f.typed.clone(),
            Some(_) => return None,
            None => last.word.clone(),
        };
        if layout::detect_script(&last.word) != Some(src) || !a_typed.chars().all(char::is_alphabetic) {
            return None;
        }
        let merged = format!("{a_typed}{b}");
        let mletters: Vec<char> = merged.chars().collect();
        if !natural_case(&mletters) || merged.chars().count() < 4 {
            return None;
        }
        let d = dict::get(src);
        let al = Dictionary::normalize(&a_typed);
        let bl = Dictionary::normalize(b);
        let ml = Dictionary::normalize(&merged);
        if self.options.never_correct.contains(&ml) {
            return None;
        }
        let known = |w: &str| match w.chars().count() {
            1 => is_single_letter_word(src, w.chars().next().unwrap_or(' ')),
            2 | 3 => d.rank(w).is_some_and(|r| r < split::LM_SHORT_PART_RANK),
            _ => d.is_word(w) || (src == Lang::Ru && morph::is_known_form(d, w)),
        };
        let m_ok = d.rank(&ml).is_some() || (ml.chars().count() >= 6 && d.in_full_forms(&ml));
        if !m_ok {
            return None;
        }
        let ka = known(&al);
        let kb = known(&bl);
        if bl.chars().count() < 2 || (al.chars().count() < 2 && !ka) {
            return None;
        }
        if ka && kb {
            return None;
        }
        if kb && bl.chars().count() <= 2 {
            return None;
        }
        if !ka && !kb && d.rank(&ml).is_none() {
            return None;
        }
        if typo::is_title_case(b) && !ka {
            return None;
        }
        let mut bnd = String::new();
        bnd.push(boundary);
        let typed_len = if already_typed { 1 } else { 0 };
        let delete = last.word.chars().count() + last.boundary.chars().count() + b.chars().count() + typed_len;
        let typed_all = format!("{a_typed}{}{b}", last.boundary);
        let rep = Replacement { delete, insert: format!("{merged}{bnd}"), target: src };
        let fix = AutoFix { typed: typed_all.clone(), typed_boundary: boundary.to_string(), steps: Vec::new() };
        self.recent_fix = Some(RecentFix { typed: typed_all, erased: false });
        self.last = Some(LastWord { word: merged, boundary: bnd, fix: Some(fix), fresh: true });
        Some(rep)
    }

    fn push_last(&mut self, w: LastWord) {
        if let Some(prev) = self.last.replace(w) {
            self.history.push(prev);
            if self.history.len() > UNDO_HISTORY {
                self.history.remove(0);
            }
        }
    }

    pub fn manual_convert(&mut self, current_layout: Lang) -> Option<Replacement> {
        if !self.buf.is_empty() {
            let word = self.buf.as_string();
            let from = layout::detect_script(&word).unwrap_or(current_layout);
            let conv = layout::convert(&word, from);
            self.buf.set(&conv);
            return Some(Replacement { delete: word.chars().count(), insert: conv, target: from.other() });
        }
        if self.last.as_ref().is_some_and(|l| l.fix.is_some()) {
            return self.undo_last(current_layout);
        }
        let last = self.last.as_mut()?;
        last.fresh = false;
        let from = layout::detect_script(&last.word).unwrap_or(current_layout);
        let conv = layout::convert(&last.word, from);
        let boundary = layout::convert_punctuation(&last.boundary, from);
        let rep = Replacement {
            delete: last.word.chars().count() + last.boundary.chars().count(),
            insert: format!("{conv}{boundary}"),
            target: from.other(),
        };
        last.word = conv;
        last.boundary = boundary;
        Some(rep)
    }

    pub fn decide(&self, token: &str) -> Decision {
        let d = self.decide_core(token);
        if let Decision::Convert { text, target, reason } = &d {
            if *reason != SPACE_REASON && attested_form_wins(token, text, *target) {
                let typo_swap = *reason == TYPO_REASON
                    && typo::is_adjacent_swap(trim_non_letters(token), trim_non_letters(text));
                if !typo_swap {
                    return Decision::Keep("typed form attested in the corpus");
                }
            }
        }
        d
    }

    fn decide_core(&self, token: &str) -> Decision {
        let o = &self.options;
        if !o.never_correct.is_empty() && o.never_correct.contains(&never_key(token)) {
            return Decision::Keep("never correct (undone by the user)");
        }
        if let Some(d) = self.user_word_decision(token) {
            return d;
        }
        if is_cli_flag(token) {
            return Decision::Keep("cli flag");
        }
        if o.skip_urls_emails && looks_like_url_or_email(token) {
            return Decision::Keep("url/email/path");
        }
        if filename::is_en_filename(token) {
            return Decision::Keep("file name");
        }
        if let Some(text) = filename::from_ru_layout(token) {
            return Decision::Convert { text, target: Lang::En, reason: "file name typed in the other layout" };
        }
        if o.skip_tokens_with_digits && token.chars().any(|c| c.is_ascii_digit()) {
            return Decision::Keep("contains digits");
        }
        let Some(src) = layout::detect_script(token) else {
            if let Some(d) = self.mixed_decision(token) {
                return d;
            }
            return Decision::Keep("no letters or mixed scripts");
        };
        let core = trim_non_letters(token);
        let letters: Vec<char> = core.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() < o.min_word_len.max(1) {
            if o.convert_single_letters && letters.len() == 1 {
                let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
                let converted = layout::convert(token, src);
                let conv_letters: Vec<char> = converted.chars().filter(|c| c.is_alphabetic()).collect();
                let dst = src.other();
                if layout::detect_script(&converted) == Some(dst) && conv_letters.len() == 1 {
                    let typed_word = is_single_letter_word(src, lower(letters[0]));
                    let conv_word = is_single_letter_word(dst, lower(conv_letters[0]));
                    let ctx = self.left_context(src);
                    let lc = core.to_lowercase();
                    let ok_ctx = |p: &str| {
                        p != crate::ngram::START
                            && p.chars().count() >= 2
                            && (dict::get(src).contains(p)
                                || crate::ngram::get(src).count(&[p, &lc]) > 0.0)
                    };
                    let multi_ctx = o.preferred_lang != Some(dst)
                        && (ctx.prev.as_deref().is_some_and(ok_ctx)
                            || ctx.prev2.as_deref().is_some_and(ok_ctx));
                    if conv_word && !typed_word && !multi_ctx {
                        return Decision::Convert { text: converted, target: dst, reason: SINGLE_WORD_REASON };
                    }
                    if !typed_word && !conv_word && o.preferred_lang == Some(dst) {
                        return Decision::Convert {
                            text: converted, target: dst, reason: "single letter to app language",
                        };
                    }
                }
            }
            let chars: Vec<char> = token.chars().collect();
            let dst_dict = dict::get(src.other());
            let all_caps_word = chars.len() >= 3
                && chars.iter().all(|c| layout::convert_char(*c, src).is_alphabetic());
            if let Some(d) = Self::try_convert(&chars, src, o.min_word_len, |w| {
                let lower = w.to_lowercase();
                if lower.contains('ё') && lower != "её" {
                    return false;
                }
                let wletters: Vec<char> = w.chars().collect();
                let whole_caps = all_caps_word
                    && wletters.len() == chars.len()
                    && title_or_all_caps(&wletters);
                dst_dict.rank(w).is_some_and(|r| whole_caps || r < SIGN_WORD_MAX_RANK)
            }) {
                return d;
            }
            return Decision::Keep("too short");
        }
        if !natural_case(edge_signs_trimmed(&letters, src)) {
            return Decision::Keep("unnatural casing (password-like)");
        }
        if src == Lang::Ru && morph::is_mat_form(core) {
            return Decision::Keep("known mat form");
        }
        let dst = src.other();
        let src_dict = dict::get(src);
        let dst_dict = dict::get(dst);

        let chars: Vec<char> = token.chars().collect();

        let full_short = letters.len() == 3 && src_dict.in_full_forms(core);
        let caps_short = title_or_all_caps(&letters);
        let ranked_convert = || {
            Self::try_convert(&chars, src, o.min_word_len, |w| {
                if full_short && w.chars().count() < letters.len() {
                    return false;
                }
                dst_dict.rank(w).is_some_and(|r| {
                    if full_short && caps_short && w.chars().count() == letters.len() {
                        r < SHORT_WORD_MAX_RANK
                    } else {
                        dst_rank_ok(w, r, &letters)
                    }
                })
            })
        };
        if !natural_case(&letters) {
            return Self::try_convert(&chars, src, o.min_word_len, |w| {
                dst_dict.rank(w).is_some_and(|r| w.chars().count() > 3 || r < SHORT_WORD_MAX_RANK)
                    || (w.chars().count() >= FULL_FORM_CONVERT_MIN_LEN && dst_dict.in_full_forms(w))
            })
            .unwrap_or(Decision::Keep("unnatural casing (password-like)"));
        }
        if o.fix_missing_spaces && src == Lang::Ru && split::is_glued(core) {
            if let Some(d) = ranked_convert() {
                return self.prefer_own_layout_swap(token, core, src, d);
            }
            if let Some(d) = self.typo_swap_decision(token, core, src) {
                return d;
            }
            if let Some(d) = self.split_decision(token, core, src) {
                return d;
            }
        }

        if src == Lang::En && (2..=3).contains(&letters.len()) && !self.typed_language_context(src) {
            let lower = Dictionary::normalize(core);
            let rare = src_dict.rank(&lower).is_some_and(|r| r > SHORT_SWITCH_SRC_RANK) && !SHORT_SWITCH_KEEP.contains(&lower.as_str());
            if rare {
                let whole: String = chars.iter().collect();
                let whole_conv = layout::convert(&whole, src);
                let whole_core = trim_non_letters(&whole_conv);
                if whole_core.chars().count() > letters.len()
                    && layout::detect_script(whole_core) == Some(dst)
                    && whole_core.chars().all(char::is_alphabetic)
                    && dst_dict.rank(whole_core).is_some_and(|r| r < FULL_FORM_YIELD_RANK)
                {
                    return Decision::Convert { text: whole_conv, target: dst, reason: "converted form is a known word" };
                }
                if let Some(d) = Self::try_convert(&chars, src, o.min_word_len, |w| dst_dict.rank(w).is_some_and(|r| r < SHORT_SWITCH_DST_RANK)) {
                    return d;
                }
            }
        }
        if src_dict.contains(core) {
            return Decision::Keep("known word in current layout");
        }
        if let Some(d) = Self::hyphen_converted(&chars, src) {
            return self.prefer_own_layout_swap(token, core, src, d);
        }

        let src_full = letters.len() >= FULL_FORM_MIN_LEN && src_dict.in_full_forms(core);
        let ranked = if src_full {
            let lead_letter = layout::convert(&letters[0].to_string(), src).chars().all(char::is_alphabetic);
            Self::try_convert(&chars, src, o.min_word_len, |w| {
                let whole = w.chars().count() == letters.len();
                dst_dict.rank(w).is_some_and(|r| {
                    dst_rank_ok(w, r, &letters)
                        && if whole {
                            r < FULL_FORM_YIELD_RANK || reading_likelier(core, src, w, dst)
                        } else {
                            lead_letter && r < SIGNED_READING_MAX_RANK && reading_likelier(core, src, w, dst)
                        }
                })
            })
        } else if letters.len() <= 3 && src_dict.in_full_forms(core) && self.typed_language_context(src) {
            let lower = Dictionary::normalize(core);
            if crate::ngram::get(src).count(&[lower.as_str()]) > 0.0 {
                None
            } else {
                ranked_convert()
            }
        } else {
            ranked_convert()
        };
        if let Some(d) = ranked {
            return self.prefer_own_layout_swap(token, core, src, d);
        }

        let n_letters = letters.len();
        if n_letters >= FULL_FORM_MIN_LEN && src_dict.in_full_forms(core) {
            return Decision::Keep("known word form in current layout");
        }
        if n_letters >= FULL_FORM_CONVERT_MIN_LEN {
            if let Some(d) = Self::try_convert(&chars, src, o.min_word_len, |w| {
                w.chars().count() >= FULL_FORM_CONVERT_MIN_LEN && dst_dict.in_full_forms(w)
            }) {
                return self.prefer_own_layout_swap(token, core, src, d);
            }
        }

        if o.fix_missing_spaces && dst == Lang::Ru {
            if let Some(d) = Self::split_converted(token, src) {
                return d;
            }
        }
        if o.fix_missing_spaces {
            if let Some(d) = self.split_decision(token, core, src) {
                if o.fix_typos && core.chars().all(char::is_alphabetic) {
                    if let Some(fix) = self.typo_fix(core, src) {
                        let (n, m) = (core.chars().count(), fix.chars().count());
                        let mut a: Vec<char> = Dictionary::normalize(core).chars().collect();
                        let mut b: Vec<char> = Dictionary::normalize(&fix).chars().collect();
                        a.sort_unstable();
                        b.sort_unstable();
                        let vowel_slip = {
                            let (x, y): (Vec<char>, Vec<char>) = (core.chars().collect(), fix.chars().collect());
                            let diff: Vec<usize> = (0..n.min(m)).filter(|&i| x[i] != y[i]).collect();
                            let v = |c: char| "аеёиоуыэюяAEIOUaeiou".contains(c.to_lowercase().next().unwrap_or(c));
                            n == m && diff.len() == 1 && v(x[diff[0]]) && v(y[diff[0]])
                        };
                        let split_text = match &d {
                            Decision::Convert { text, .. } => text.as_str(),
                            Decision::Keep(_) => "",
                        };
                        let ng = crate::ngram::get(src);
                        let parts: Vec<String> = split_text.split_whitespace().map(Dictionary::normalize).collect();
                        let split_count = if parts.len() == 2 { ng.count(&[&parts[0], &parts[1]]) } else { 0.0 };
                        let fix_count = ng.count(&[&Dictionary::normalize(&fix)]);
                        let split_likelier = split_count >= SPLIT_MIN_COUNT && split_count * SPLIT_VS_TYPO >= fix_count;
                        let lm_split = split::split_glued(src_dict, core, src).is_none();
                        let lm_loses = lm_split && !is_single_letter_prep_split(split_text) && split_count < fix_count * LM_SPLIT_OVER_TYPO;
                        let dedupe = typo::is_dedupe_of(core, &fix) && split_count * SPLIT_VS_DEDUPE < fix_count;
                        let unseen_glued = !lm_split && split_count <= 0.0 && !is_single_letter_prep_split(split_text);
                        if typo::is_adjacent_swap(core, &fix) || dedupe || unseen_glued || lm_loses || (!is_single_letter_prep_split(split_text) && !split_likelier && (m > n || a == b || vowel_slip)) {
                            return Decision::Convert { text: token.replacen(core, &fix, 1), target: src, reason: TYPO_REASON };
                        }
                    }
                }
                return d;
            }
        }

        if o.fix_typos {
            if let Some(d) = self.typo_swap_decision(token, core, src) {
                return d;
            }
            let split_like = single_prep_plus_word(core, src).is_some()
                || (core.chars().all(char::is_alphabetic) && split::lm_segment(src_dict, core, src).is_some());
            if !split_like {
                if let Some(d) = self.hand_shift_decision(token, src) {
                    return d;
                }
            }
            if let Some((prep, rest)) = single_prep_plus_word(core, src) {
                let mid = typo::is_title_case(core)
                    && self
                        .left_context(src)
                        .prev
                        .as_deref()
                        .is_some_and(|p| p != crate::ngram::START);
                let unseen = crate::ngram::get(src).count(&[&Dictionary::normalize(&prep), &Dictionary::normalize(&rest)]) <= 0.0;
                let weak_rest = unseen && src_dict.rank(&Dictionary::normalize(&rest)).is_none();
                if o.fix_missing_spaces && !mid && !weak_rest {
                    let fixed = format!("{prep} {rest}");
                    return Decision::Convert {
                        text: token.replacen(core, &fixed, 1),
                        target: src,
                        reason: SPACE_REASON,
                    };
                }
            }
            if core.chars().all(char::is_alphabetic) {
                if let Some(fix) = self.typo_fix(core, src) {
                    return Decision::Convert { text: token.replacen(core, &fix, 1), target: src, reason: TYPO_REASON };
                }
            }
            if let Some(d) = self.hyphen_typo_decision(token, src) {
                return d;
            }
            if let Some((d, _)) = self.layout_typo_decision(&chars, src) {
                return d;
            }
        }

        if (2..=3).contains(&letters.len())
            && caps_short
            && core.chars().all(char::is_alphabetic)
            && src_dict.rank(&Dictionary::normalize(core)).is_none()
            && !self.typed_language_context(src)
        {
            if !src_dict.in_full_forms(core) {
                if let Some(d) = Self::try_convert(&chars, src, o.min_word_len, |w| {
                    w.chars().count() == letters.len()
                        && w.chars().all(char::is_alphabetic)
                        && dst_dict.rank(w).is_some_and(|r| letters.len() == 3 || r < HYPHEN_PART_MAX_RANK)
                }) {
                    return d;
                }
            }
        }
        if core.chars().all(char::is_alphabetic) && letters.len() >= 4 {
            let conv = layout::convert(core, src);
            if conv.chars().all(char::is_alphabetic)
                && layout::detect_script(&conv) == Some(dst)
                && src_dict.unseen_bigrams(core) >= 1
                && dst_dict.unseen_bigrams(&conv) == 0
            {
                if o.fix_typos && self.typed_language_context(src) {
                    return Decision::Keep("unknown either way inside a sentence of the typed language");
                }
                let text = layout::convert(token, src);
                return Decision::Convert { text, target: dst, reason: "letter bigrams fit other language" };
            }
        }
        Decision::Keep("not confident")
    }

    fn mixed_decision(&self, token: &str) -> Option<Decision> {
        let o = &self.options;
        let core = trim_non_letters(token);
        let letters: Vec<char> = core.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() < MIXED_MIN_LETTERS || !natural_case(&letters) {
            return None;
        }
        let script = |c: char| layout::detect_script(c.encode_utf8(&mut [0; 4]));
        let mut runs: Vec<(Lang, usize)> = Vec::new();
        for &c in &letters {
            let Some(l) = script(c) else { continue };
            match runs.last_mut() {
                Some((rl, n)) if *rl == l => *n += 1,
                _ => runs.push((l, 1)),
            }
        }
        let mut typos_ok = o.fix_typos && letters.iter().all(|c| c.is_lowercase());
        let mut brand_ending = false;
        if let [(first, n), (_, tail)] = runs[..] {
            let prefix: String = letters[..n].iter().collect();
            brand_ending = first == Lang::En
                && tail <= 3
                && dict::get(first).rank(&prefix.to_lowercase()).is_some_and(|r| r < BRAND_PREFIX_RANK);
            if tail <= 2 {
                let prefix: String = letters[..n].iter().collect();
                if dict::get(first).is_word(&prefix.to_lowercase()) {
                    let last = letters[letters.len() - 1].to_lowercase().next().unwrap_or(' ');
                    if tail == 1 && CASE_ENDING_LETTERS.contains(last) {
                        return None;
                    }
                    typos_ok = false;
                }
            }
        }
        let mut only: Option<Lang> = None;
        if letters.len() < MIXED_MIN_LETTERS + 1 {
            if runs.len() != 2 {
                return None;
            }
            let (major, start) = if runs[0].1 > runs[1].1 { (runs[0].0, 0) } else { (runs[1].0, runs[0].1) };
            let pair: String = letters[start..start + 2].iter().collect::<String>().to_lowercase();
            if dict::get(major).contains(&pair) {
                only = Some(major);
            }
        }
        let known = |d: &Dictionary, w: &str| {
            let n = w.chars().count();
            if n <= 3 {
                d.rank(w).is_some_and(|r| r < MIXED_SHORT_MAX_RANK)
            } else {
                d.contains(w) || (n >= FULL_FORM_MIN_LEN && d.in_full_forms(w))
            }
        };
        let mut found: Vec<(bool, String, Lang)> = Vec::new();
        for lang in [Lang::Ru, Lang::En] {
            if (brand_ending && lang == Lang::Ru) || only.is_some_and(|l| l != lang) {
                continue;
            }
            let whole = layout::convert(token, lang.other());
            let d = dict::get(lang);
            if whole != core
                && whole.chars().all(char::is_alphabetic)
                && layout::detect_script(&whole) == Some(lang)
                && known(d, &whole)
            {
                found.push((true, whole, lang));
                continue;
            }
            let cand = layout::convert(core, lang.other());
            if !cand.chars().all(char::is_alphabetic) || layout::detect_script(&cand) != Some(lang) {
                continue;
            }
            let fixed = token.replacen(core, &cand, 1);
            if known(d, &cand) {
                found.push((true, fixed, lang));
            } else if typos_ok {
                if let Some(fix) = self.typo_fix(&cand, lang) {
                    found.push((false, token.replacen(core, &fix, 1), lang));
                }
            }
        }
        let exact: Vec<_> = found.iter().filter(|f| f.0).collect();
        let pick = match (exact.len(), found.len()) {
            (1, _) => exact[0],
            (0, 1) => &found[0],
            _ => return None,
        };
        Some(Decision::Convert { text: pick.1.clone(), target: pick.2, reason: MIXED_REASON })
    }

    fn split_converted(token: &str, src: Lang) -> Option<Decision> {
        let dst = src.other();
        let chars: Vec<char> = token.chars().collect();
        for k in 0..=2.min(chars.len().saturating_sub(1)) {
            let (head, tail) = chars.split_at(chars.len() - k);
            if tail.iter().any(|c| c.is_alphanumeric()) {
                break;
            }
            let conv = layout::convert(&head.iter().collect::<String>(), src);
            if !conv.chars().all(char::is_alphabetic) || layout::detect_script(&conv) != Some(dst) {
                continue;
            }
            if let Some(fixed) = split::split_glued(dict::get(dst), &conv, dst) {
                let tail: String = tail.iter().collect();
                return Some(Decision::Convert { text: fixed + &layout::convert_punctuation(&tail, src), target: dst, reason: SPACE_REASON });
            }
        }
        None
    }

    fn hyphen_converted(chars: &[char], src: Lang) -> Option<Decision> {
        let dst = src.other();
        let (src_dict, dst_dict) = (dict::get(src), dict::get(dst));
        let frequent = |w: &str, after: bool| {
            let lower = Dictionary::normalize(w);
            (after && dst == Lang::Ru && HYPHEN_PARTICLES.contains(&lower.as_str()))
                || dst_dict.rank(&lower).is_some_and(|r| {
                    r < if lower.chars().count() <= 3 { SHORT_WORD_MAX_RANK } else { HYPHEN_PART_MAX_RANK }
                })
                || (lower.chars().count() >= FULL_FORM_CONVERT_MIN_LEN && dst_dict.in_full_forms(&lower))
        };
        let whole = layout::convert(&chars.iter().collect::<String>(), src);
        let whole_last_known = whole.rsplit_once('-').is_some_and(|(_, b)| {
            let b = Dictionary::normalize(b);
            !b.is_empty() && b.chars().all(char::is_alphabetic) && (dst_dict.contains(&b) || dst_dict.in_full_forms(&b))
        });
        for k in 0..=2.min(chars.len().saturating_sub(1)) {
            let (head, tail) = chars.split_at(chars.len() - k);
            if tail.iter().any(|c| c.is_alphanumeric()) || (k > 0 && whole_last_known) {
                break;
            }
            let head: String = head.iter().collect();
            let Some((a, b)) = head.split_once('-') else { continue };
            if a.chars().count() < 2 || b.chars().count() < 2 || b.contains('-') {
                continue;
            }
            if src_dict.contains(&Dictionary::normalize(a)) && src_dict.contains(&Dictionary::normalize(b)) {
                return None;
            }
            let conv = layout::convert(&head, src);
            let (ca, cb) = conv.split_once('-')?;
            let letters = |w: &str| w.chars().all(char::is_alphabetic) && layout::detect_script(w) == Some(dst);
            if !letters(ca) || !letters(cb) || !natural_case(&conv.chars().filter(|c| c.is_alphabetic()).collect::<Vec<_>>()) {
                continue;
            }
            let long_half = ca.chars().count().max(cb.chars().count()) >= 4;
            if (dst == Lang::Ru || long_half) && frequent(ca, false) && frequent(cb, true) {
                let tail: String = tail.iter().collect();
                return Some(Decision::Convert {
                    text: conv + &layout::convert_punctuation(&tail, src),
                    target: dst,
                    reason: "converted form is a known word",
                });
            }
        }
        None
    }

    fn hyphen_typo_decision(&self, token: &str, src: Lang) -> Option<Decision> {
        let o = &self.options;
        if !o.fix_typos || o.typo_level == typo::Level::Cautious {
            return None;
        }
        let core = trim_non_letters(token);
        if !core.contains('-') || !core.chars().all(|c| c.is_alphabetic() || c == '-') {
            return None;
        }
        let (a, b) = core.split_once('-')?;
        if a.is_empty() || b.is_empty() || b.contains('-') {
            return None;
        }
        if !a.chars().all(char::is_alphabetic) || !b.chars().all(char::is_alphabetic) {
            return None;
        }
        if hyphen_keep(a, b, src) {
            return None;
        }
        let mut accepted: Vec<(String, String)> = Vec::new();
        let push = |acc: &mut Vec<(String, String)>, fa: String, fb: String| {
            if (fa != a || fb != b) && hyphen_accept(&fa, &fb, src) && !acc.iter().any(|(x, y)| *x == fa && *y == fb)
            {
                acc.push((fa, fb));
            }
        };
        if let Some(p) = particle_typo(b) {
            push(&mut accepted, a.to_string(), typo::match_case(b, p));
        }
        if !dict::get(src).is_word(b) {
            if let Some(fix) = self.typo_fix_segment(b, src, true) {
                push(&mut accepted, a.to_string(), typo::match_case(b, &fix));
            }
            for fix in doubled_letter_drops(b) {
                push(&mut accepted, a.to_string(), typo::match_case(b, &fix));
            }
        }
        let b_norm = Dictionary::normalize(b);
        let b_valid_word = dict::get(src).is_word(b) && !(src == Lang::Ru && HYPHEN_PARTICLES.contains(&b_norm.as_str()));
        let skip_left_stem = b_valid_word && is_plausible_compound_stem(a, src);
        if !dict::get(src).is_word(a) {
            if let Some(fix) = self.typo_fix_segment(a, src, true) {
                if !skip_left_stem || hyphen_whole_known(&fix, b, src) {
                    push(&mut accepted, typo::match_case(a, &fix), b.to_string());
                }
                if let Some(p) = particle_typo(b) {
                    push(&mut accepted, typo::match_case(a, &fix), typo::match_case(b, p));
                }
            }
            for fix in doubled_letter_drops(a) {
                if !skip_left_stem
                    || hyphen_whole_known(&fix, b, src)
                    || dict::get(src).rank(&fix).is_some_and(|r| r < HYPHEN_PART_MAX_RANK)
                {
                    push(&mut accepted, typo::match_case(a, &fix), b.to_string());
                }
            }
        }
        if accepted.len() > 1 {
            accepted.retain(|(fa, fb)| hyphen_keep(fa, fb, src));
        }
        if accepted.len() != 1 {
            return None;
        }
        let (fa, fb) = accepted.pop()?;
        let fixed = format!("{fa}-{fb}");
        Some(Decision::Convert {
            text: token.replacen(core, &fixed, 1),
            target: src,
            reason: TYPO_REASON,
        })
    }

    fn typo_fix_segment(&self, part: &str, lang: Lang, assume_unknown: bool) -> Option<String> {
        let o = &self.options;
        if !o.never_typo.is_empty() && o.never_typo.contains(&never_key(part)) {
            return None;
        }
        let ctx = self.left_context(lang);
        if assume_unknown {
            typo::suggest_as_unknown_ctx(dict::get(lang), part, lang, o.typo_level, &ctx)
        } else {
            typo::suggest_ctx(part, lang, o.typo_level, &ctx)
        }
    }

    fn typo_swap_decision(&self, token: &str, core: &str, lang: Lang) -> Option<Decision> {
        if !self.options.fix_typos {
            return None;
        }
        if !core.chars().all(char::is_alphabetic) {
            return None;
        }
        let fix = self.typo_fix(core, lang)?;
        typo::is_adjacent_swap(core, &fix).then(|| Decision::Convert {
            text: token.replacen(core, &fix, 1),
            target: lang,
            reason: TYPO_REASON,
        })
    }

    fn prefer_own_layout_swap(&self, token: &str, core: &str, src: Lang, layout: Decision) -> Decision {
        let Some(swap) = self.typo_swap_decision(token, core, src) else {
            return layout;
        };
        let (layout_text, dst) = match &layout {
            Decision::Convert { text, target, .. } => (text.as_str(), *target),
            Decision::Keep(_) => return layout,
        };
        let Decision::Convert { text: swap_text, .. } = &swap else {
            return layout;
        };
        if typo_swap_beats_layout(swap_text, src, layout_text, dst) {
            swap
        } else {
            layout
        }
    }

    fn typo_fix(&self, core: &str, lang: Lang) -> Option<String> {
        let o = &self.options;
        if !o.never_typo.is_empty() && o.never_typo.contains(&never_key(core)) {
            return None;
        }
        typo::suggest_ctx(core, lang, o.typo_level, &self.left_context(lang))
    }

    fn prev_hand_shift(&self) -> Option<(i32, Lang)> {
        let last = self.last.as_ref()?;
        let fix = last.fix.as_ref()?;
        typo::hand_shift_which(trim_non_letters(&fix.typed), trim_non_letters(&last.word))
    }

    fn hand_shift_decision(&self, token: &str, src: Lang) -> Option<Decision> {
        let o = &self.options;
        if !o.fix_typos || o.typo_level == typo::Level::Cautious {
            return None;
        }
        let core = trim_non_letters(token);
        if typo::is_title_case(core) {
            let mid = self
                .left_context(src)
                .prev
                .as_deref()
                .is_some_and(|p| p != crate::ngram::START);
            if mid {
                return None;
            }
        }
        if !o.never_typo.is_empty() && o.never_typo.contains(&never_key(core)) {
            return None;
        }
        let whole_word_chars = token != core && token.chars().all(|c| !c.is_whitespace()) && token.chars().any(|c| !c.is_alphabetic());
        if whole_word_chars {
            if let Some((fix, dest)) = typo::hand_shift_fix(token, src, o.typo_level, self.prev_hand_shift()) {
                return Some(Decision::Convert { text: fix, target: dest, reason: HAND_SHIFT_REASON });
            }
        }
        let (fix, dest) = typo::hand_shift_fix(core, src, o.typo_level, self.prev_hand_shift())?;
        Some(Decision::Convert {
            text: token.replacen(core, &fix, 1),
            target: dest,
            reason: HAND_SHIFT_REASON,
        })
    }

    fn typed_language_context(&self, lang: Lang) -> bool {
        let ctx = self.left_context(lang);
        ctx.prev.as_deref().is_some_and(|p| p != crate::ngram::START && dict::get(lang).contains(p))
    }

    pub fn left_context(&self, lang: Lang) -> Context {
        let mut words: Vec<String> = Vec::new();
        if !self.options.use_context {
            return Context::none();
        }
        for w in self.last.iter().chain(self.history.iter().rev()) {
            let b = &w.boundary;
            let tail: String = w.word.chars().rev().take_while(|c| !c.is_alphabetic()).collect();
            if b.contains(['.', '!', '?', '\n', '\r']) || tail.contains(['.', '!', '?']) {
                words.push(crate::ngram::START.to_string());
                break;
            }
            let gap_ok = |c: char| c.is_whitespace() && c != '\t' || matches!(c, ',' | ';' | ':' | '-' | '—' | '–' | '"' | '«' | '»');
            if !b.chars().chain(tail.chars()).all(gap_ok) {
                break;
            }
            if w.word.chars().any(|c| c.is_ascii_digit()) {
                break;
            }
            let Some(core) = w.word.split(|c: char| !c.is_alphabetic()).filter(|p| !p.is_empty()).last() else { break };
            if layout::detect_script(core) != Some(lang) {
                break;
            }
            words.push(Dictionary::normalize(core));
            if words.len() == 2 {
                break;
            }
        }
        Context { prev: words.first().cloned(), prev2: words.get(1).cloned() }
    }

    fn layout_typo_decision(&self, chars: &[char], src: Lang) -> Option<(Decision, String)> {
        let o = &self.options;
        let dst = src.other();
        for k in 0..=2.min(chars.len().saturating_sub(1)) {
            let (head, tail) = chars.split_at(chars.len() - k);
            if tail.iter().any(|c| c.is_alphanumeric()) {
                break;
            }
            let head: String = head.iter().collect();
            let conv = layout::convert(&head, src);
            let conv_core = trim_non_letters(&conv);
            let letters: Vec<char> = conv_core.chars().collect();
            if letters.is_empty()
                || !letters.iter().all(|c| c.is_alphabetic())
                || layout::detect_script(conv_core) != Some(dst)
                || !natural_case(&letters)
            {
                continue;
            }
            let typed_letters: String = head.chars().filter(|c| c.is_alphabetic()).collect();
            if !crate::typo_wide::looks_foreign(&Dictionary::normalize(&typed_letters), src) {
                continue;
            }
            if !o.never_typo.is_empty() && o.never_typo.contains(&never_key(conv_core)) {
                continue;
            }
            if let Some(fix) = crate::typo_wide::layout_typo_ctx(conv_core, dst, o.typo_level, &self.left_context(dst)) {
                let tail: String = tail.iter().collect();
                let tail = layout::convert_punctuation(&tail, src);
                let plain = format!("{conv}{tail}");
                let text = format!("{}{tail}", conv.replacen(conv_core, &fix, 1));
                return Some((Decision::Convert { text, target: dst, reason: LAYOUT_TYPO_REASON }, plain));
            }
        }
        None
    }

    fn split_decision(&self, token: &str, core: &str, src: Lang) -> Option<Decision> {
        if !core.chars().all(char::is_alphabetic) {
            return None;
        }
        let d = dict::get(src);
        let fixed = match split::split_glued(d, core, src) {
            Some(f) => f,
            None => {
                let lower = Dictionary::normalize(core);
                if d.is_word(&lower) || morph::is_known_form(d, &lower) || d.is_name_form(&lower) {
                    return None;
                }
                let seg = split::lm_segment(d, core, src)?;
                if seg.dropped && !(self.options.fix_typos && self.options.typo_level != typo::Level::Cautious) {
                    return None;
                }
                seg.text
            }
        };
        let lower = Dictionary::normalize(core);
        if !split::is_glued(&lower) {
            let ng = crate::ngram::get(src);
            let whole = ng.count(&[&lower]);
            if whole > 0.0 {
                let parts: Vec<String> = fixed.split_whitespace().map(Dictionary::normalize).collect();
                let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
                if ng.count(&refs) < whole * split::LM_WHOLE_RATIO {
                    return None;
                }
            }
        }
        if typo::is_title_case(core) {
            let mid = self
                .left_context(src)
                .prev
                .as_deref()
                .is_some_and(|p| p != crate::ngram::START);
            if mid {
                let prefix = fixed.split_whitespace().next().unwrap_or("");
                if prefix.chars().count() >= 2 {
                    return None;
                }
            }
        }
        Some(Decision::Convert { text: token.replacen(core, &fixed, 1), target: src, reason: SPACE_REASON })
    }

    fn user_word_decision(&self, token: &str) -> Option<Decision> {
        let words = &self.options.user_words;
        if words.is_empty() {
            return None;
        }
        let src = layout::detect_script(token)?;
        if token.chars().filter(|c| c.is_alphabetic()).count() < self.options.min_word_len.max(1) {
            return None;
        }
        let dst = src.other();
        let chars: Vec<char> = token.chars().collect();
        for k in 0..=2.min(chars.len().saturating_sub(1)) {
            let (head, tail) = chars.split_at(chars.len() - k);
            if tail.iter().any(|c| c.is_alphanumeric()) {
                break;
            }
            let head: String = head.iter().collect();
            if words.contains(&head, src) {
                return Some(Decision::Keep("user word in current layout"));
            }
            let conv = layout::convert(&head, src);
            if layout::detect_script(&conv) == Some(dst) && words.contains(&conv, dst) {
                let tail: String = tail.iter().collect();
                return Some(Decision::Convert {
                    text: conv + &layout::convert_punctuation(&tail, src),
                    target: dst,
                    reason: "converted form is a user word",
                });
            }
        }
        None
    }

    fn try_convert(chars: &[char], src: Lang, min_len: usize, known: impl Fn(&str) -> bool) -> Option<Decision> {
        let dst = src.other();
        for k in 0..=2.min(chars.len().saturating_sub(1)) {
            let (head, tail) = chars.split_at(chars.len() - k);
            if tail.iter().any(|c| c.is_alphanumeric()) {
                break;
            }
            let head: String = head.iter().collect();
            let conv = layout::convert(&head, src);
            let conv_core = trim_non_letters(&conv);
            if conv_core.chars().count() >= min_len.max(1)
                && layout::detect_script(conv_core) == Some(dst)
                && conv_core.chars().all(char::is_alphabetic)
                && known(conv_core)
            {
                let tail: String = tail.iter().collect();
                return Some(Decision::Convert { text: conv + &layout::convert_punctuation(&tail, src), target: dst, reason: "converted form is a known word" });
            }
        }
        None
    }
}

const SINGLE_LETTER_PREPS: &[char] = &['в', 'с', 'к', 'у', 'о', 'и', 'а', 'я'];

fn is_single_letter_prep_split(text: &str) -> bool {
    let mut parts = text.split_whitespace();
    let Some(prep) = parts.next() else { return false };
    if parts.next().is_none() {
        return false;
    }
    let p = Dictionary::normalize(prep);
    let mut cs = p.chars();
    matches!(cs.next(), Some(c) if SINGLE_LETTER_PREPS.contains(&c)) && cs.next().is_none()
}

fn single_prep_plus_word(core: &str, lang: Lang) -> Option<(String, String)> {
    if lang != Lang::Ru {
        return None;
    }
    let lower = Dictionary::normalize(core);
    let mut cs = lower.chars();
    let first = cs.next()?;
    if !SINGLE_LETTER_PREPS.contains(&first) {
        return None;
    }
    let rest: String = cs.collect();
    if rest.chars().count() < 3 {
        return None;
    }
    let d = dict::get(lang);
    if d.is_word(&lower) || morph::is_known_form(d, &lower) {
        return None;
    }
    let rest_ok = if rest.chars().count() < 5 {
        d.rank(&rest).is_some_and(|r| r < HYPHEN_PART_MAX_RANK)
    } else {
        d.is_word(&rest) || morph::is_known_form(d, &rest)
    };
    if rest_ok {
        let n = core.chars().next()?.len_utf8();
        return Some((core[..n].to_string(), core[n..].to_string()));
    }
    None
}

fn hyphen_part_ok(w: &str, lang: Lang, after: bool) -> bool {
    let d = dict::get(lang);
    let lower = Dictionary::normalize(w);
    (after && lang == Lang::Ru && HYPHEN_PARTICLES.contains(&lower.as_str()))
        || d.rank(&lower).is_some_and(|r| {
            r < if lower.chars().count() <= 3 { SHORT_WORD_MAX_RANK } else { HYPHEN_PART_MAX_RANK }
        })
        || (lower.chars().count() >= FULL_FORM_CONVERT_MIN_LEN && d.in_full_forms(&lower))
}

fn hyphen_whole_known(a: &str, b: &str, lang: Lang) -> bool {
    let d = dict::get(lang);
    let whole = format!("{}-{}", Dictionary::normalize(a), Dictionary::normalize(b));
    d.contains(&whole) || d.in_full_forms(&whole)
}

fn hyphen_keep(a: &str, b: &str, lang: Lang) -> bool {
    if hyphen_whole_known(a, b, lang) {
        return true;
    }
    let nb = Dictionary::normalize(b);
    hyphen_part_ok(a, lang, false) && lang == Lang::Ru && HYPHEN_PARTICLES.contains(&nb.as_str())
}

fn is_plausible_compound_stem(a: &str, lang: Lang) -> bool {
    let d = dict::get(lang);
    let la = Dictionary::normalize(a);
    let len = la.chars().count();
    let max_unseen = if lang == Lang::Ru && len >= 5 && la.ends_with(['о', 'е']) { 1 } else { 0 };
    len >= 4
        && d.unseen_bigrams(&la) <= max_unseen
        && ((lang == Lang::Ru && la.ends_with(['о', 'е'])) || morph::looks_derived(d, &la))
}

fn hyphen_accept(a: &str, b: &str, lang: Lang) -> bool {
    hyphen_keep(a, b, lang)
        || (hyphen_part_ok(a, lang, false) && hyphen_part_ok(b, lang, true))
}

fn particle_typo(typed: &str) -> Option<&'static str> {
    let lower = Dictionary::normalize(typed);
    if HYPHEN_PARTICLES.contains(&lower.as_str()) {
        return None;
    }
    let d = dict::get(Lang::Ru);
    let known = d.is_word(&lower);
    let drops = doubled_letter_drops(&lower);
    let hits: Vec<&'static str> = HYPHEN_PARTICLES
        .iter()
        .copied()
        .filter(|p| {
            if !is_close_typo(&lower, p) {
                return false;
            }
            if known && lower.chars().count() != p.chars().count() {
                let rare_double = drops.iter().any(|x| x == p)
                    && d.rank(&lower).is_none_or(|r| r >= SHORT_WORD_MAX_RANK);
                if !rare_double {
                    return false;
                }
            }
            true
        })
        .collect();
    (hits.len() == 1).then(|| hits[0])
}

fn is_close_typo(a: &str, b: &str) -> bool {
    let (av, bv): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if av == bv {
        return false;
    }
    let mut asort = av.clone();
    asort.sort_unstable();
    let mut bsort = bv.clone();
    bsort.sort_unstable();
    if asort == bsort && av.len() <= 6 {
        return true;
    }
    if av.len() == bv.len() {
        let diff: Vec<usize> = (0..av.len()).filter(|&i| av[i] != bv[i]).collect();
        if diff.len() != 1 {
            return false;
        }
        if av.len() <= 2 {
            return typo::adjacent_keys(av[diff[0]], bv[diff[0]], Lang::Ru);
        }
        return true;
    }
    if av.len() + 1 == bv.len() {
        return is_char_subseq(&av, &bv);
    }
    if bv.len() + 1 == av.len() {
        return is_char_subseq(&bv, &av);
    }
    false
}

fn doubled_letter_drops(s: &str) -> Vec<String> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < cs.len() {
        if cs[i] == cs[i + 1] {
            let mut t = cs.clone();
            t.remove(i);
            let w: String = t.into_iter().collect();
            if !out.iter().any(|x| x == &w) {
                out.push(w);
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

fn is_char_subseq(short: &[char], long: &[char]) -> bool {
    let mut i = 0;
    for &c in long {
        if i < short.len() && short[i] == c {
            i += 1;
        }
    }
    i == short.len()
}

fn is_cli_flag(token: &str) -> bool {
    token.starts_with('-')
}

pub fn never_key(token: &str) -> String {
    Dictionary::normalize(trim_non_letters(token))
}

pub fn is_known(word: &str, lang: Lang) -> bool {
    dict::get(lang).is_word(&Dictionary::normalize(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        Engine::new(EngineOptions::default())
    }

    fn type_str(e: &mut Engine, s: &str) -> Vec<Replacement> {
        s.chars().filter_map(|c| e.feed(Key::Char(c), false)).collect()
    }

    fn conv(t: &str) -> Option<String> {
        match engine().decide(t) {
            Decision::Convert { text, .. } => Some(text),
            Decision::Keep(_) => None,
        }
    }

    #[test]
    fn shifted_sign_at_the_edge_is_not_password_casing() {
        assert_eq!(conv("тучесдщгвЖ").as_deref(), Some("nextcloud:"));
        assert_eq!(conv("ТучесдщгвЖ").as_deref(), Some("Nextcloud:"));
        assert_eq!(conv("руддщЭ").as_deref(), Some("hello\""));
        assert_eq!(conv("ЭруддщЭ").as_deref(), Some("\"hello\""));
        assert_eq!(conv("ЭРуддщ").as_deref(), Some("\"Hello"));
        assert_eq!(conv("руддщЮ").as_deref(), Some("hello>"));
        for t in ["тучесДщгвЖ", "тУчесдщгвЖ", "ТУчесдщгвЖ", "пароЛьЖ", "ЖпАроль"] {
            assert_eq!(engine().decide(t), Decision::Keep("unnatural casing (password-like)"), "{t}");
        }
        let typos = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
        assert_eq!(typos.decide("ПаркуЖ"), Decision::Keep("unnatural casing (password-like)"));
        assert_eq!(conv("тучесдщгв").as_deref(), Some("nextcloud"));
        assert_eq!(conv("реезы").as_deref(), Some("https"));
    }

    #[test]
    fn first_letter_slip_before_dots() {
        let typos = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
        let fix = |w: &str| match typos.decide(w) {
            Decision::Convert { text, .. } => Some(text),
            _ => None,
        };
        assert_eq!(fix("жлементы").as_deref(), Some("элементы"));
        assert_eq!(fix("жлементы....").as_deref(), Some("элементы...."));
        assert_eq!(fix("Жлементы,").as_deref(), Some("Элементы,"));
        for w in ["жители", "железный", "элементы", "этажи"] {
            assert_eq!(fix(w), None, "{w}");
        }
    }

    #[test]
    fn hyphenated_word_in_the_wrong_layout() {
        assert_eq!(conv("dct-nfrb").as_deref(), Some("все-таки"));
        assert_eq!(conv("Dct-nfrb").as_deref(), Some("Все-таки"));
        assert_eq!(conv("dct-nfrb,").as_deref(), Some("все-таки,"));
        assert_eq!(conv("rnj-nj").as_deref(), Some("кто-то"));
        assert_eq!(conv("bp-pf").as_deref(), Some("из-за"));
        assert_eq!(conv("rfr-yb,elm").as_deref(), Some("как-нибудь"));
        assert_eq!(conv("lfq-rf").as_deref(), Some("дай-ка"));
        assert_eq!(conv("цудд-лтщцт").as_deref(), Some("well-known"));
        assert_eq!(conv("ghtcc-rjyathtywbz[").as_deref(), Some("пресс-конференциях"));
        for w in ["well-known", "up-to", "e-mail", "x-ray", "кто-то", "все-таки", "гы-гы", "цу-цу", "туц-туц", "rnj-nj-nj"] {
            assert_eq!(conv(w), None, "{w}");
        }
    }

    fn typo_fix_at(level: typo::Level, w: &str) -> Option<String> {
        let e = Engine::new(EngineOptions { fix_typos: true, typo_level: level, ..Default::default() });
        match e.decide(w) {
            Decision::Convert { text, .. } => Some(text),
            Decision::Keep(_) => None,
        }
    }

    #[test]
    fn adjacent_transposition_of_a_known_word_is_fixed() {
        let fix = |w: &str| typo_fix_at(typo::Level::Normal, w);
        assert_eq!(fix("заню").as_deref(), Some("знаю"));
        assert_eq!(fix("Заню").as_deref(), Some("Знаю"));
        assert_eq!(fix("ЗАНЮ").as_deref(), Some("ЗНАЮ"));
        assert_eq!(fix("тиак").as_deref(), Some("итак"));
        assert_eq!(fix("уверне").as_deref(), Some("уверен"));
        assert_eq!(fix("ивдеть").as_deref(), Some("видеть"));
        assert_eq!(fix("впорос").as_deref(), Some("вопрос"));
        assert_eq!(fix("кгода").as_deref(), Some("когда"));
        assert_eq!(fix("опмню").as_deref(), Some("помню"));
        assert_eq!(typo_fix_at(typo::Level::Aggressive, "заню").as_deref(), Some("знаю"));
        assert_eq!(typo_fix_at(typo::Level::Cautious, "заню"), None);
        let off = Engine::new(EngineOptions { fix_typos: false, typo_level: typo::Level::Normal, ..Default::default() });
        assert!(matches!(off.decide("заню"), Decision::Keep(_)));
        assert_eq!(fix("знаю"), None, "must keep знаю");
        assert_eq!(fix("daer").as_deref(), Some("dear"));
        assert_eq!(fix("waek").as_deref(), Some("weak"));
        assert_eq!(fix("faer").as_deref(), Some("fear"));
        assert_eq!(fix("онги").as_deref(), Some("ноги"));
        assert_eq!(fix("номре").as_deref(), Some("номер"));
        assert_eq!(fix("bolw").as_deref(), Some("blow"));
        assert_eq!(fix("esat").as_deref(), Some("seat"));
        assert_eq!(fix("stra").as_deref(), Some("star"));
        assert_eq!(fix("бдуу").as_deref(), Some("буду"));
        assert_eq!(fix("вшаа").as_deref(), Some("ваша"));
        assert_eq!(fix("огду").as_deref(), Some("году"));
        assert_eq!(fix("дувх").as_deref(), Some("двух"));
        assert_eq!(fix("игыр").as_deref(), Some("игры"));
        assert_eq!(fix("aftre").as_deref(), Some("after"));
        assert_eq!(fix("nmae").as_deref(), Some("name"));
        assert_eq!(fix("beign").as_deref(), Some("being"));
        assert_eq!(fix("tiems").as_deref(), Some("times"));
        assert_eq!(fix("braek").as_deref(), Some("break"));
        assert_eq!(fix("onse").as_deref(), Some("ones"));
        assert_eq!(fix("trian").as_deref(), Some("train"));
        assert_eq!(fix("hsan").as_deref(), Some("hasn"));
        assert_eq!(fix("amry").as_deref(), Some("army"));
        assert_eq!(fix("Маск"), None, "must keep Маск");
        assert_eq!(fix("маск").as_deref(), Some("макс"));
    }

    #[test]
    fn short_unattested_en_form_converts_to_frequent_ru_word() {
        let e = Engine::new(EngineOptions { fix_typos: true, typo_level: typo::Level::Normal, ..Default::default() });
        match e.decide("ult") {
            Decision::Convert { text, target, .. } => {
                assert_eq!(text, "где");
                assert_eq!(target, Lang::Ru);
            }
            other => panic!("isolated ult: {other:?}"),
        }
        let mut e = Engine::new(EngineOptions { fix_typos: true, typo_level: typo::Level::Normal, ..Default::default() });
        for ch in "это ".chars() {
            let _ = e.feed(Key::Char(ch), false);
        }
        match e.decide("ult") {
            Decision::Convert { text, .. } => assert_eq!(text, "где"),
            other => panic!("after RU ult: {other:?}"),
        }
        let mut e = Engine::new(EngineOptions { fix_typos: true, typo_level: typo::Level::Normal, ..Default::default() });
        for ch in "the ".chars() {
            let _ = e.feed(Key::Char(ch), false);
        }
        match e.decide("ult") {
            Decision::Convert { text, .. } => assert_eq!(text, "где"),
            other => panic!("after EN ult: {other:?}"),
        }
        let off = Engine::new(EngineOptions { fix_typos: false, ..Default::default() });
        match off.decide("ult") {
            Decision::Convert { text, .. } => assert_eq!(text, "где"),
            other => panic!("typo off ult: {other:?}"),
        }
    }

    #[test]
    fn hyphenated_typos_are_fixed() {
        let fix = |w: &str| typo_fix_at(typo::Level::Normal, w);
        assert_eq!(fix("все-атки").as_deref(), Some("все-таки"));
        assert_eq!(fix("Все-атки").as_deref(), Some("Все-таки"));
        assert_eq!(fix("ВСЕ-АТКИ").as_deref(), Some("ВСЕ-ТАКИ"));
        assert_eq!(fix("все-такии").as_deref(), Some("все-таки"));
        assert_eq!(fix("как-от").as_deref(), Some("как-то"));
        assert_eq!(fix("кто-от").as_deref(), Some("кто-то"));
        assert_eq!(fix("где-нибудть").as_deref(), Some("где-нибудь"));
        assert_eq!(fix("из-заа").as_deref(), Some("из-за"));
        assert_eq!(fix("по-моемуу").as_deref(), Some("по-моему"));
        assert_eq!(fix("все-тоо").as_deref(), Some("все-то"));
        assert_eq!(fix("яркко-голубом").as_deref(), Some("ярко-голубом"));
        for w in ["все-таки", "всё-таки", "кто-то", "из-за", "по-моему", "как-нибудь", "дай-ка", "кое-как", "во-первых", "по-новому", "когда-нибудь", "кошко-мальчик", "Кошко-мальчик", "КОШКО-МАЛЬЧИК", "красно-белый", "нейро-сеть", "Нейро-сеть", "НЕЙРО-СЕТЬ"] {
            assert_eq!(fix(w), None, "must keep {w}");
        }
        assert_eq!(typo_fix_at(typo::Level::Cautious, "все-атки"), None);
        let off = Engine::new(EngineOptions { fix_typos: false, typo_level: typo::Level::Normal, ..Default::default() });
        assert!(matches!(off.decide("все-атки"), Decision::Keep(_)));
    }

    #[test]
    fn preposition_plus_word_is_not_merged() {
        assert_eq!(field_after(typo::Level::Normal, "врот"), "в рот");
        assert_eq!(field_after(typo::Level::Aggressive, "врот"), "в рот");
        assert_eq!(field_after(typo::Level::Normal, "ВРОТ"), "В РОТ");
        assert_eq!(field_after(typo::Level::Normal, "в рот"), "в рот");
        assert_eq!(field_after(typo::Level::Normal, "В РОТ"), "В РОТ");
        assert_eq!(field_after(typo::Level::Normal, "ЕБАНЫЙ В РОТ"), "ЕБАНЫЙ В РОТ");
        assert_eq!(field_after(typo::Level::Normal, "ЕБАНЫЙ ВРОТ"), "ЕБАНЫЙ В РОТ");
        assert_eq!(field_after(typo::Level::Normal, "ебаный в рот"), "ебаный в рот");
        assert_eq!(field_after(typo::Level::Normal, "отебе"), "о тебе");
        assert_eq!(field_after(typo::Level::Normal, "отсуда"), "отсюда");
    }

    #[test]
    fn caps_short_wrong_layout_converts() {
        assert_eq!(layout::convert("AJY", Lang::En), "ФОН");
        assert_eq!(layout::convert("Ajy", Lang::En), "Фон");
        assert_eq!(layout::convert("ajy", Lang::En), "фон");
        assert_eq!(conv("AJY").as_deref(), Some("ФОН"));
        assert_eq!(conv("Ajy").as_deref(), Some("Фон"));
        assert_eq!(conv("ajy"), None, "lowercase short switch is unchanged");
        assert_eq!(field_after(typo::Level::Normal, "AJY"), "ФОН");
        assert_eq!(layout::convert("YOU", Lang::En), "НЩГ");
        assert_eq!(conv("НЩГ").as_deref(), Some("YOU"));
        assert_eq!(conv("Нщг").as_deref(), Some("You"));
        assert_eq!(conv("ВСЕ"), None);
        assert_eq!(conv("THE"), None);
        assert_eq!(conv("VS"), None);
        assert_eq!(conv("ОК"), None);
        assert_eq!(conv("МГУ"), None);
        assert_eq!(conv("FP"), None);
        assert_eq!(conv("СЗ"), None);
        assert_eq!(conv("F{{").as_deref(), Some("АХХ"));
        assert_eq!(conv("J<}").as_deref(), Some("ОБЪ"));
        assert_eq!(field_after(typo::Level::Normal, "that he d"), "that he d");
    }

    #[test]
    fn caps_mat_is_not_corrected() {

        for level in [typo::Level::Normal, typo::Level::Aggressive] {
            assert_eq!(field_after(level, "ЗАЕБАЛСЯ"), "ЗАЕБАЛСЯ", "{level:?}");
            assert_eq!(field_after(level, "заебался"), "заебался", "{level:?}");
            assert_eq!(field_after(level, "ЕБАНЫЙ"), "ЕБАНЫЙ", "{level:?}");
        }
        assert_eq!(field_after(typo::Level::Normal, "услушу"), "услугу");
        assert_eq!(field_after(typo::Level::Normal, "УСЛУШУ"), "УСЛУГУ");
    }

    #[test]
    fn wrong_layout_with_a_missing_space() {
        assert_eq!(engine().decide("yflj,s"), Decision::Convert { text: "надо бы".into(), target: Lang::Ru, reason: SPACE_REASON });
        assert_eq!(conv("Yflj,s").as_deref(), Some("Надо бы"));
        assert_eq!(conv("yflj,s!").as_deref(), Some("надо бы!"));
        assert_eq!(conv("dJabct").as_deref(), None);
        assert_eq!(conv("djabct").as_deref(), Some("в офисе"));
        assert_eq!(conv("yflj").as_deref(), Some("надо"));
        assert_eq!(conv("xnj,s").as_deref(), Some("чтобы"));
        assert_eq!(conv("nfr;t").as_deref(), Some("также"));
        assert_eq!(conv("чцшлш").as_deref(), Some("xwiki"));
        assert_eq!(conv(",skjkb").as_deref(), Some("было ли"));
        assert_eq!(conv("'njn;t").as_deref(), Some("этот же"));
    }

    #[test]
    fn layout_switched_mid_word_0_3_69() {
        assert_eq!(conv("приdtn").as_deref(), Some("привет"));
        assert_eq!(conv("ghbвет").as_deref(), Some("привет"));
        assert_eq!(conv("Приdtn").as_deref(), Some("Привет"));
        assert_eq!(conv("спаcb,j").as_deref(), Some("спасибо"));
        assert_eq!(conv("гkfpf[").as_deref(), Some("глазах"));
        assert_eq!(conv("приdtn.").as_deref(), Some("привет."));
        assert_eq!(conv("wщrld").as_deref(), Some("world"));
        assert_eq!(conv("prщ").as_deref(), Some("pro"));
        assert_eq!(conv("Prщ").as_deref(), Some("Pro"));
        assert_eq!(conv("hellщ").as_deref(), Some("hello"));
        assert_eq!(conv("неn").as_deref(), Some("нет"));
        assert_eq!(conv("toщ").as_deref(), Some("too"));
        for t in ["даg", "изm", "dты", "сif", "мby"] {
            assert_eq!(conv(t), None, "{t}");
        }
        assert_eq!(conv("ыщund").as_deref(), Some("sound"));
        let typos = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
        let fix = |t: &str| match typos.decide(t) {
            Decision::Convert { text, .. } => Some(text),
            Decision::Keep(_) => None,
        };
        assert_eq!(fix("приdtnn").as_deref(), Some("привет"));
        assert_eq!(fix("wщrlds").as_deref(), Some("worlds"));
        assert_eq!(fix("beaгtfiul").as_deref(), Some("beautiful"));
        assert_eq!(conv("приdtnn"), None);
        for t in ["Gitом", "iPhoneа", "gitом", "PRы", "VPNом", "Wi-Fiем", "abcабв", "оотjbmь", "sheы", "youе", "Windowsе", "Heyом", "keyами", "gitа", "macу", "Appleй", "linuxе"] {
            assert_eq!(fix(t), None, "{t}");
        }
        let mut e = engine();
        assert_eq!(type_str(&mut e, "приdtn "), vec![Replacement { delete: 7, insert: "привет ".into(), target: Lang::Ru }]);
    }

    #[test]
    fn zufsu_to_peace() {
        assert_eq!(conv("зуфсу").as_deref(), Some("peace"));
        let mut e = engine();
        let reps = type_str(&mut e, "зуфсу ");
        assert_eq!(reps, vec![Replacement { delete: 6, insert: "peace ".into(), target: Lang::En }]);
    }

    #[test]
    fn capitalized_second_word_converts() {
        assert_eq!(conv("Rfr").as_deref(), Some("Как"));
        assert_eq!(conv("Зуфсу").as_deref(), Some("Peace"));
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn Rfr "), vec![
            Replacement { delete: 7, insert: "привет ".into(), target: Lang::Ru },
            Replacement { delete: 4, insert: "Как ".into(), target: Lang::Ru },
        ]);
        let mut e = engine();
        assert_eq!(type_str(&mut e, "зуфсу Зуфсу "), vec![
            Replacement { delete: 6, insert: "peace ".into(), target: Lang::En },
            Replacement { delete: 6, insert: "Peace ".into(), target: Lang::En },
        ]);
    }

    #[test]
    fn user_examples() {
        assert_eq!(conv("руддщ").as_deref(), Some("hello"));
        assert_eq!(conv("ghbdtn").as_deref(), Some("привет"));
        assert_eq!(conv("ltkf").as_deref(), Some("дела"));
        assert_eq!(conv("Ghbdtn").as_deref(), Some("Привет"));
        assert_eq!(conv("[jhjij").as_deref(), Some("хорошо"));
    }

    #[test]
    fn trailing_punctuation() {
        assert_eq!(conv("ghbdtn,").as_deref(), Some("привет,"));
        let mut e = engine();
        let reps = type_str(&mut e, "руддщ!");
        assert_eq!(reps, vec![Replacement { delete: 6, insert: "hello!".into(), target: Lang::En }]);
    }

    #[test]
    fn real_words_are_kept() {
        for w in ["peace", "hello", "привет", "дела", "the", "что", "Hello"] {
            assert_eq!(conv(w), None, "{w}");
        }
    }

    #[test]
    fn junk_is_kept() {
        for w in ["hElLo", "xK9#qz", "p4ssw0rd", "ab", "https://example.com", "me@mail.ru", "qzx", "здфтуеpeace"] {
            assert_eq!(conv(w), None, "{w}");
        }
    }

    #[test]
    fn digits_configurable() {
        let mut e = engine();
        e.options.skip_tokens_with_digits = false;
                assert!(matches!(e.decide("ghbdtn2"), Decision::Convert { .. }));
        assert_eq!(engine().decide("ghbdtn2"), Decision::Keep("contains digits"));
    }

    #[test]
    fn both_words_collision_not_converted() {
        let en = dict::get(Lang::En);
        let ru = dict::get(Lang::Ru);
        let _ = (en, ru);
        for w in ["ytn", "rfr"] {
            assert!(conv(w).is_some(), "{w}");
        }
        let mut found = None;
        for line in include_str!("../data/en.txt").lines().take(20000) {
            let c = layout::convert(line, Lang::En);
            if line.len() >= 2 && layout::detect_script(&c) == Some(Lang::Ru) && c.chars().all(char::is_alphabetic) && is_known(&c, Lang::Ru) {
                found = Some(line.to_string());
                break;
            }
        }
        if let Some(w) = found {
            assert_eq!(engine().decide(&w), Decision::Keep("known word in current layout"), "{w}");
        }
    }

    #[test]
    fn modifiers_reset() {
        let mut e = engine();
        type_str(&mut e, "ghbd");
        assert_eq!(e.feed(Key::Char('c'), true), None);
        assert_eq!(e.current_len(), 0);
        assert_eq!(type_str(&mut e, "tn "), vec![]);
    }

    #[test]
    fn backspace_editing() {
        let mut e = engine();
        type_str(&mut e, "ghbdtx");
        e.feed(Key::Backspace, false);
        let r = type_str(&mut e, "n ");
        assert_eq!(r[0].insert, "привет ");
        assert_eq!(r[0].delete, 7);
    }

    #[test]
    fn enter_and_tab_are_boundaries() {
        let mut e = engine();
        type_str(&mut e, "ghbdtn");
        let r = e.feed(Key::Boundary('\n'), false).unwrap();
        assert_eq!(r.insert, "привет\n");
        assert_eq!(r.delete, 6);
        let mut e = engine();
        type_str(&mut e, "ghbdtn");
        let r = e.feed(Key::Boundary('\t'), false).unwrap();
        assert_eq!(r.insert, "привет\t");
        assert_eq!(r.delete, 6);
        let mut e = engine();
        let r = type_str(&mut e, "ghbdtn ");
        assert_eq!(r[0].delete, 7);
        assert_eq!(r[0].insert, "привет ");
    }

    #[test]
    fn reset_on_focus_change() {
        let mut e = engine();
        type_str(&mut e, "ghbd");
        e.feed(Key::Reset, false);
        assert_eq!(type_str(&mut e, "tn "), vec![]);
        assert_eq!(e.manual_convert(Lang::En).unwrap().insert, "ет ");
    }

    #[test]
    fn plus_does_not_end_a_word() {
        let mut e = engine();
        assert!(type_str(&mut e, "ctrl+shift+k").is_empty());
        assert_eq!(e.current_len(), "ctrl+shift+k".chars().count());
        assert!(matches!(engine().decide("ctrl+shift+к"), Decision::Keep(_)));
    }

    #[test]
    fn manual_convert_current_and_last() {
        let mut e = engine();
        type_str(&mut e, "qzx");
        let r = e.manual_convert(Lang::En).unwrap();
        assert_eq!(r, Replacement { delete: 3, insert: "йяч".into(), target: Lang::Ru });
        assert_eq!(e.manual_convert(Lang::Ru).unwrap().insert, "qzx");
        type_str(&mut e, " ");
        let r = e.manual_convert(Lang::En).unwrap();
        assert_eq!(r, Replacement { delete: 4, insert: "йяч ".into(), target: Lang::Ru });
    }

    #[test]
    fn manual_undo_of_autocorrect() {
        let mut e = engine();
        type_str(&mut e, "ghbdtn ");
        let r = e.manual_convert(Lang::Ru).unwrap();
        assert_eq!(r.insert, "ghbdtn ");
        assert_eq!(r.delete, 7);
    }

    #[test]
    fn autocorrect_off() {
        let mut e = engine();
        e.options.autocorrect = false;
        assert!(type_str(&mut e, "зуфсу ").is_empty());
        assert_eq!(e.manual_convert(Lang::Ru).unwrap().insert, "peace ");
    }

    #[test]
    fn comma_inside_ru_word_typed_on_en() {
        let mut e = engine();
        let r = type_str(&mut e, ",eltn ");
        assert_eq!(r[0].insert, "будет ");
    }

    fn engine_with(words: &[&str]) -> Engine {
        Engine::new(EngineOptions { user_words: UserWords::from_words(words), ..Default::default() })
    }

    #[test]
    fn single_letter_too_short() {
        assert_eq!(engine().decide("q"), Decision::Keep("too short"));
        assert_eq!(engine().decide("a"), Decision::Keep("too short"));
        assert_eq!(engine_with(&["й"]).decide("q"), Decision::Keep("too short"));
    }

    #[test]
    fn letter_plus_sign_keys_make_short_words() {
        let mut e = engine();
        assert_eq!(conv("b[").as_deref(), Some("их"));
        assert_eq!(conv("B[").as_deref(), Some("Их"));
        assert_eq!(type_str(&mut e, "b[ "), vec![Replacement { delete: 3, insert: "их ".into(), target: Lang::Ru }]);
        e.options.preferred_lang = Some(Lang::En);
        assert_eq!(type_str(&mut e, "b[ ")[0].insert, "их ");
        assert_eq!(conv("a,"), None);
        assert_eq!(conv("x["), None);
        assert_eq!(conv("t`").as_deref(), Some("её"));
        for keep in ["`y", "l`", "h,", "v,", "g;"] {
            assert_eq!(conv(keep), None, "{keep}");
        }
    }

    #[test]
    fn lone_letters_follow_real_one_letter_words() {
        let mut e = engine();
        for (typed, expected) in [("d", "в"), ("b", "и"), ("z", "я"), ("D", "В"), ("ф", "a"), ("ш", "i"), ("Ш", "I")] {
            assert!(matches!(e.decide(typed), Decision::Convert { ref text, .. } if text == expected), "{typed}");
        }
        for keep in ["a", "I", "i", "в", "и", "я", "q", "й", "x"] {
            assert!(matches!(e.decide(keep), Decision::Keep(_)), "{keep}");
        }
        assert_eq!(type_str(&mut e, "ghbdtn d "), vec![
            Replacement { delete: 7, insert: "привет ".into(), target: Lang::Ru },
            Replacement { delete: 2, insert: "в ".into(), target: Lang::Ru },
        ]);
        assert!(type_str(&mut e, "d!").is_empty());
        e.options.convert_single_letters = false;
        assert!(matches!(e.decide("d"), Decision::Keep(_)));
    }

    #[test]
    fn uchu_is_known_russian_word() {
        assert_eq!(engine().decide("учу"), Decision::Keep("known word in current layout"));
    }

    #[test]
    fn user_word_overrides_known_word() {
        let e = engine_with(&["exe"]);
        assert!(matches!(e.decide("учу"), Decision::Convert { ref text, target: Lang::En, .. } if text == "exe"));
        assert!(matches!(e.decide("Учу"), Decision::Convert { ref text, .. } if text == "Exe"));
        let mut e = engine_with(&["EXE"]);
        let r = type_str(&mut e, "учу ");
        assert_eq!(r, vec![Replacement { delete: 4, insert: "exe ".into(), target: Lang::En }]);
    }

    #[test]
    fn user_word_on_typed_side_is_kept() {
        assert_eq!(engine_with(&["ghbdtn"]).decide("ghbdtn"), Decision::Keep("user word in current layout"));
        assert_eq!(engine_with(&["rfr"]).decide("rfr"), Decision::Keep("user word in current layout"));
    }

    #[test]
    fn builtin_extras_convert_but_do_not_override() {
        assert_eq!(conv("дщд").as_deref(), Some("lol"));
        assert_eq!(conv("сшсв").as_deref(), Some("cicd"));
        assert_eq!(conv("шьрщ").as_deref(), Some("imho"));
        assert_eq!(conv("bv[j").as_deref(), Some("имхо"));
        assert!(dict::get(Lang::En).contains("exe"));
        assert_eq!(conv("учу"), None);
    }
    #[test]
    fn single_letters_to_assigned_language() {
        let mut e = engine();
        e.options.preferred_lang = Some(Lang::Ru);
        assert_eq!(type_str(&mut e, "q "), vec![Replacement { delete: 2, insert: "й ".into(), target: Lang::Ru }]);
        assert!(type_str(&mut e, "a ").is_empty());
        assert_eq!(type_str(&mut e, "d ")[0].insert, "в ");
        e.options.preferred_lang = Some(Lang::En);
        assert_eq!(type_str(&mut e, "й ")[0].insert, "q ");
        assert!(type_str(&mut e, "в ").is_empty());
        assert_eq!(type_str(&mut e, "ф ")[0].insert, "a ");
        let mut e2 = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
        assert!(type_str(&mut e2, "that i d ").is_empty());
    }

    #[test]
    fn single_letter_guards() {
        let mut e = engine();
        assert!(type_str(&mut e, "a ").is_empty());
        e.options.preferred_lang = Some(Lang::Ru);
        assert!(type_str(&mut e, "в ").is_empty());
        assert!(matches!(e.decide("qz"), Decision::Keep(_)));
        e.options.convert_single_letters = false;
        assert!(type_str(&mut e, "d ").is_empty());
        e.options.convert_single_letters = true;
        e.options.autocorrect = false;
        assert!(type_str(&mut e, "b ").is_empty());
    }

    #[test]
    fn converted_word_boundary_and_manual_roundtrip() {
        let mut e = engine();
        assert_eq!(conv("yfghbvth?").as_deref(), Some("например,"));
        assert_eq!(type_str(&mut e, "yfghbvth?"), vec![Replacement {
            delete: 9, insert: "например,".into(), target: Lang::Ru,
        }]);
        assert_eq!(e.manual_convert(Lang::Ru), Some(Replacement {
            delete: 9, insert: "yfghbvth?".into(), target: Lang::En,
        }));
        assert_eq!(e.manual_convert(Lang::En).unwrap().insert, "например,");
    }

    #[test]
    fn punctuation_in_token_tails() {
        assert!(!is_boundary(','));
        assert!(is_boundary('?'));
        assert_eq!(conv("ghbdtn,").as_deref(), Some("привет,"));
        assert_eq!(conv("ghbdtn.").as_deref(), Some("привет."));
        assert_eq!(conv("руддщ,").as_deref(), Some("hello?"));
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn, "), vec![Replacement {
            delete: 8, insert: "привет, ".into(), target: Lang::Ru,
        }]);
        assert_eq!(type_str(&mut e, "руддщ, "), vec![Replacement {
            delete: 6, insert: "hello?".into(), target: Lang::En,
        }]);
        assert_eq!(type_str(&mut engine(), "руддщ,"), vec![Replacement {
            delete: 6, insert: "hello?".into(), target: Lang::En,
        }]);
        assert_eq!(type_str(&mut engine(), "руддщ."), vec![Replacement {
            delete: 6, insert: "hello/".into(), target: Lang::En,
        }]);
        assert_eq!(type_str(&mut engine(), "ghbdtn/"), vec![Replacement {
            delete: 7, insert: "привет.".into(), target: Lang::Ru,
        }]);
        assert_eq!(type_str(&mut engine(), "ghbdtn,"), vec![]);
        assert_eq!(type_str(&mut engine(), "ghbdtn."), vec![]);
        assert_eq!(type_str(&mut engine(), "k.,jdm "), vec![Replacement {
            delete: 7, insert: "любовь ".into(), target: Lang::Ru,
        }]);
    }

    #[test]
    fn passwords_with_signs_are_left_alone() {
        for pref in [Lang::Ru, Lang::En] {
            let mut e = engine();
            e.options.preferred_lang = Some(pref);
            assert!(type_str(&mut e, "З!К)П ").is_empty(), "pref {pref:?}");
            let mut e = engine();
            e.options.preferred_lang = Some(pref);
            assert!(type_str(&mut e, "P!R)G ").is_empty(), "pref {pref:?}");
        }
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn! ").len(), 1);
    }

    #[test]
    fn user_words_with_digits_and_signs() {
        let mut e = Engine::new(EngineOptions { user_words: UserWords::from_words(&["mp3", "ci/cd"]), ..Default::default() });
        assert!(matches!(e.decide("ьз3"), Decision::Convert { ref text, .. } if text == "mp3"));
        assert!(matches!(e.decide("сш.св"), Decision::Convert { ref text, .. } if text == "ci/cd"));
        assert!(matches!(e.decide("mp3"), Decision::Keep(_)));
        let reps = type_str(&mut e, "ьз3 ");
        assert_eq!(reps[0].insert, "mp3 ");
    }

    #[test]
    fn typo_fix_and_undo() {
        let mut e = engine();
        assert!(type_str(&mut e, "поечаток ").is_empty(), "off by default");
        e.options.fix_typos = true;
        let r = type_str(&mut e, "пирвет, ");
        assert_eq!(r, vec![Replacement { delete: 7, insert: "привет,".into(), target: Lang::Ru }]);
        let r = type_str(&mut e, "Becuase ");
        assert_eq!(r[0].insert, "Because ");
        let undo = e.manual_convert(Lang::En).unwrap();
        assert_eq!(undo, Replacement { delete: 8, insert: "Becuase ".into(), target: Lang::En });
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        let mut e = Engine::new(EngineOptions { user_words: UserWords::from_words(&["пирвет"]), fix_typos: true, ..Default::default() });
        assert!(type_str(&mut e, "пирвет ").is_empty());
    }

    #[test]
    fn abbreviations_convert() {
        assert_eq!(conv("пкфзрйд").as_deref(), Some("graphql"));
        assert_eq!(conv("vxc").as_deref(), Some("мчс"));
        assert_eq!(conv("cybkc").as_deref(), Some("снилс"));
        assert_eq!(conv("мчс"), None);
        assert_eq!(conv("oauth"), None);
    }

    #[test]
    fn file_names_in_wrong_layout() {
        assert_eq!(conv("мфгдеюишт").as_deref(), Some("vault.bin"));
        assert_eq!(conv("ензув-0ю3ю59юефкюпя").as_deref(), Some("typed-0.3.59.tar.gz"));
        let mut e = engine();
        assert_eq!(type_str(&mut e, "мфгдеюишт "), vec![Replacement { delete: 10, insert: "vault.bin ".into(), target: Lang::En }]);
        for keep in ["vault.bin", "typed-0.3.59.tar.gz", "README.md", "люблю", "сюда", "плюс"] {
            assert_eq!(conv(keep), None, "{keep}");
        }
        let mut e = engine();
        assert!(type_str(&mut e, "vault.bin ").is_empty());
        assert_eq!(conv("k.,jdm").as_deref(), Some("любовь"));
        assert_eq!(conv("yfghbvth?").as_deref(), Some("например,"));
    }

    #[test]
    fn typo_fix_keeps_case() {
        let mut e = engine();
        e.options.fix_typos = true;
        assert_eq!(type_str(&mut e, "делать кореркцию "), vec![Replacement { delete: 10, insert: "коррекцию ".into(), target: Lang::Ru }]);
        assert_eq!(type_str(&mut e, "чтокой ")[0].insert, "точкой ");
        assert!(type_str(&mut e, "коррекцию ").is_empty());
    }

    #[test]
    fn hand_shift_fixes_home_row_misplacement() {
        let e = Engine::new(EngineOptions { fix_typos: true, typo_level: typo::Level::Normal, ..Default::default() });
        assert_eq!(e.decide("ротань"), Decision::Convert { text: "привет".into(), target: Lang::Ru, reason: HAND_SHIFT_REASON });
        let left = layout::shift_token("привет", Lang::Ru, Lang::Ru, -1).unwrap();
        assert_eq!(e.decide(&left), Decision::Convert { text: "привет".into(), target: Lang::Ru, reason: HAND_SHIFT_REASON });
        assert_eq!(e.decide("jr;;p"), Decision::Convert { text: "hello".into(), target: Lang::En, reason: HAND_SHIFT_REASON });
        let en_left = layout::shift_token("hello", Lang::En, Lang::En, -1).unwrap();
        assert_eq!(e.decide(&en_left), Decision::Convert { text: "hello".into(), target: Lang::En, reason: HAND_SHIFT_REASON });
        assert_eq!(e.decide("окжжз"), Decision::Convert { text: "hello".into(), target: Lang::En, reason: HAND_SHIFT_REASON });
        assert_eq!(e.decide("hjnfym"), Decision::Convert { text: "привет".into(), target: Lang::Ru, reason: HAND_SHIFT_REASON });
        assert_eq!(e.decide("Ротань"), Decision::Convert { text: "Привет".into(), target: Lang::Ru, reason: HAND_SHIFT_REASON });
        let cautious = Engine::new(EngineOptions { fix_typos: true, typo_level: typo::Level::Cautious, ..Default::default() });
        assert!(matches!(cautious.decide("ротань"), Decision::Keep(_)));
        assert!(matches!(cautious.decide("jr;;p"), Decision::Keep(_)));
        assert!(matches!(e.decide("привет"), Decision::Keep(_)));
        assert!(matches!(e.decide("hello"), Decision::Keep(_)));
        assert!(matches!(e.decide("загран"), Decision::Keep(_)));
        assert!(matches!(e.decide("struct"), Decision::Keep(_)));
        assert_eq!(e.decide("програма"), Decision::Convert { text: "программа".into(), target: Lang::Ru, reason: TYPO_REASON });
        let off = engine();
        assert!(matches!(off.decide("ротань"), Decision::Keep(_)), "fix_typos is off by default");

        let mut e = Engine::new(EngineOptions { fix_typos: true, typo_level: typo::Level::Normal, ..Default::default() });
        let a = layout::shift_token("привет", Lang::Ru, Lang::Ru, 1).unwrap();
        let b = layout::shift_token("хорошо", Lang::Ru, Lang::Ru, 1).unwrap();
        assert_eq!(type_str(&mut e, &format!("{a} "))[0].insert, "привет ");
        assert_eq!(type_str(&mut e, &format!("{b} "))[0].insert, "хорошо ");
    }

    fn bs(e: &mut Engine) -> Option<Replacement> {
        e.feed(Key::Backspace, false)
    }

    fn never(e: &mut Engine) -> Vec<String> {
        e.take_learned().into_iter().filter_map(|l| if let Learned::NeverCorrect(w) = l { Some(w) } else { None }).collect()
    }

    #[test]
    fn undo_hotkey_restores_and_remembers() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        let undo = e.undo_last(Lang::Ru).unwrap();
        assert_eq!(undo, Replacement { delete: 7, insert: "ghbdtn ".into(), target: Lang::En });
        assert_eq!(never(&mut e), vec!["ghbdtn".to_string()]);
        assert!(type_str(&mut e, "ghbdtn ").is_empty());
        assert!(type_str(&mut e, "Ghbdtn, ").is_empty());
        assert_eq!(type_str(&mut e, "pltcm ")[0].insert, "здесь ");
        assert!(type_str(&mut e, "hello ").is_empty());
        assert_eq!(e.undo_last(Lang::En), Some(Replacement { delete: 12, insert: "pltcm hello ".into(), target: Lang::En }));
        assert_eq!(e.undo_last(Lang::En), None);
        type_str(&mut e, "ghbd");
        assert_eq!(e.undo_last(Lang::En), None);
    }

    #[test]
    fn undo_restores_typed_punctuation_and_typo() {
        let mut e = engine();
        e.options.fix_typos = true;
        assert_eq!(type_str(&mut e, "ghbdtn?")[0].insert, "привет,");
        assert_eq!(e.undo_last(Lang::Ru).unwrap().insert, "ghbdtn?");
        assert_eq!(type_str(&mut e, " пирвет ")[0].insert, "привет ");
        assert_eq!(e.undo_last(Lang::Ru).unwrap(), Replacement { delete: 7, insert: "пирвет ".into(), target: Lang::Ru });
        assert!(type_str(&mut e, "пирвет ").is_empty());
        let mut learned = never(&mut e);
        learned.sort();
        assert_eq!(learned, vec!["ghbdtn".to_string(), "пирвет".to_string()]);
    }

    #[test]
    fn conversion_hotkey_right_after_fix_is_undo() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "зуфсу ")[0].insert, "peace ");
        let r = e.manual_convert(Lang::En).unwrap();
        assert_eq!(r, Replacement { delete: 6, insert: "зуфсу ".into(), target: Lang::Ru });
        assert_eq!(never(&mut e), vec!["зуфсу".to_string()]);
        assert_eq!(e.manual_convert(Lang::Ru).unwrap().insert, "peace ");
    }

    #[test]
    fn backspace_right_after_fix_undoes() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "зуфсу ")[0].insert, "peace ");
        assert_eq!(bs(&mut e), Some(Replacement { delete: 6, insert: "зуфсу ".into(), target: Lang::Ru }));
        assert!(e.take_learned().is_empty());
        assert!(type_str(&mut e, "привет ").is_empty());
        assert_eq!(e.take_learned(), vec![Learned::UndoneOnce("зуфсу".into())]);
        assert_eq!(type_str(&mut e, "зуфсу ")[0].insert, "peace ");
        assert!(bs(&mut e).is_some());
        assert!(type_str(&mut e, "привет ").is_empty());
        assert_eq!(never(&mut e), vec!["зуфсу".to_string()]);
        assert!(type_str(&mut e, "зуфсу ").is_empty());
    }

    #[test]
    fn backspace_undo_only_first_key_and_not_after_enter() {
        let mut e = engine();
        type_str(&mut e, "ghbdtn ");
        type_str(&mut e, "a");
        assert_eq!(bs(&mut e), None);
        assert_eq!(bs(&mut e), None);
        let mut e = engine();
        type_str(&mut e, "ghbdtn ");
        assert!(bs(&mut e).is_some());
        assert_eq!(bs(&mut e), None);
        let mut e = engine();
        assert!(e.feed(Key::Char('g'), false).is_none());
        type_str(&mut e, "hbdtn");
        assert!(e.feed(Key::Boundary('\n'), false).is_some());
        assert_eq!(bs(&mut e), None);
        let mut e = engine();
        e.options.undo_on_backspace = false;
        type_str(&mut e, "ghbdtn ");
        assert_eq!(bs(&mut e), None);
    }

    #[test]
    fn erased_and_retyped_original_is_kept_and_remembered() {
        let mut e = engine();
        e.options.undo_on_backspace = false;
        e.options.fix_typos = true;
        assert_eq!(type_str(&mut e, "пирвет ")[0].insert, "привет ");
        for _ in 0..7 {
            assert_eq!(bs(&mut e), None);
        }
        assert!(type_str(&mut e, "пирвет ").is_empty(), "retyped original is kept");
        assert_eq!(never(&mut e), vec!["пирвет".to_string()]);
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        assert!(e.feed(Key::Backspace, true).is_none());
        assert!(type_str(&mut e, "ghbdtn ").is_empty());
        assert_eq!(never(&mut e), vec!["ghbdtn".to_string()]);
        assert_eq!(type_str(&mut e, "pltcm ")[0].insert, "здесь ");
        for _ in 0..6 {
            bs(&mut e);
        }
        assert_eq!(type_str(&mut e, "ntcn ")[0].insert, "тест ");
        assert!(never(&mut e).is_empty());
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        assert!(bs(&mut e).is_some());
        for _ in 0..7 {
            bs(&mut e);
        }
        assert!(type_str(&mut e, "ghbdtn ").is_empty());
        assert_eq!(never(&mut e), vec!["ghbdtn".to_string()]);
    }

    fn ctx_engine(use_context: bool) -> Engine {
        Engine::new(EngineOptions { fix_typos: true, use_context, ..Default::default() })
    }

    #[test]
    fn left_context_follows_the_sentence() {
        let mut e = ctx_engine(true);
        type_str(&mut e, "я не ");
        assert_eq!(e.left_context(Lang::Ru), Context::of(&["я", "не"]));
        assert_eq!(e.left_context(Lang::En), Context::none());
        type_str(&mut e, "знаю. ");
        assert_eq!(e.left_context(Lang::Ru), Context::of(&[crate::ngram::START]));
        type_str(&mut e, "Ну, ");
        assert_eq!(e.left_context(Lang::Ru), Context::of(&["<s>", "ну"]));
        e.feed(Key::Reset, false);
        assert_eq!(e.left_context(Lang::Ru), Context::none());
        type_str(&mut e, "мы");
        e.feed(Key::Boundary('\n'), false);
        assert_eq!(e.left_context(Lang::Ru), Context::of(&[crate::ngram::START]));
        type_str(&mut e, "в 5 ");
        assert_eq!(e.left_context(Lang::Ru), Context::none());
        type_str(&mut e, "i'm ");
        assert_eq!(e.left_context(Lang::En), Context::of(&["m"]));
        assert_eq!(ctx_engine(false).left_context(Lang::Ru), Context::none());
    }

    #[test]
    fn context_keeps_names_it_knows() {
        for s in ["нельсон мандела ", "ален делон "] {
            assert_eq!(type_str(&mut ctx_engine(true), s), vec![], "{s}");
        }
    }

    #[test]
    fn context_picks_between_close_candidates() {
        for (s, fix) in [("я бы хотл ", "хотел "), ("со мпой ", "мной ")] {
            let reps = type_str(&mut ctx_engine(true), s);
            assert_eq!(reps.last().map(|r| r.insert.as_str()), Some(fix), "{s}");
        }
    }

    #[test]
    fn mangled_word_inside_a_sentence_keeps_its_layout() {
        assert_eq!(conv("vfry").as_deref(), Some("макн"));
        assert_eq!(type_str(&mut ctx_engine(true), "so vfry "), vec![]);
        assert!(!type_str(&mut ctx_engine(false), "so vfry ").is_empty());
    }

    #[test]
    fn cli_flags_are_not_corrected() {
        let e = typo_engine();
        assert!(
            matches!(e.decide("--oneline"), Decision::Keep("cli flag")),
            "{:?}",
            e.decide("--oneline")
        );
        assert!(
            matches!(e.decide("-D"), Decision::Keep("cli flag")),
            "{:?}",
            e.decide("-D")
        );
        assert!(!matches!(e.decide("все-таки"), Decision::Keep("cli flag")));
    }

    #[test]
    fn capitalised_mid_sentence_is_not_split() {
        let mut e = typo_engine();
        let _ = type_str(&mut e, "сказал ");
        assert!(
            matches!(e.decide("Додома"), Decision::Keep(_)),
            "Додома mid-sentence: {:?}",
            e.decide("Додома")
        );
        assert!(
            matches!(e.decide("Изаура"), Decision::Keep(_)),
            "Изаура mid-sentence: {:?}",
            e.decide("Изаура")
        );
        let e = typo_engine();
        match e.decide("Вофисе") {
            Decision::Convert { text, reason, .. } => {
                assert_eq!(text, "В офисе");
                assert_eq!(reason, SPACE_REASON);
            }
            other => panic!("Вофисе at sentence start should split, got {other:?}"),
        }
    }

    fn typo_engine() -> Engine {
        let mut e = engine();
        e.options.fix_typos = true;
        e
    }

    #[test]
    fn wrong_layout_and_typo_0_3_72() {
        let mut e = typo_engine();
        assert_eq!(type_str(&mut e, "xnjj "), vec![Replacement { delete: 5, insert: "что ".into(), target: Lang::Ru }]);
        assert_eq!(type_str(&mut e, "ghbdnt ")[0].insert, "привет ");
        let mut e = engine();
        assert!(type_str(&mut e, "ghbdnt ").first().map_or(true, |r| r.insert != "привет "));
        let mut e = typo_engine();
        e.options.typo_level = typo::Level::Cautious;
        assert!(type_str(&mut e, "xnjj ").first().map_or(true, |r| r.insert != "что "));
        assert!(type_str(&mut e, "teh ").is_empty());
        e.options.typo_level = typo::Level::Normal;
        assert_eq!(type_str(&mut e, "teh ")[0].insert, "the ");
    }

    #[test]
    fn glued_content_words_and_leading_neighbour_insert() {
        let e = typo_engine();
        let out = |w: &str| match e.decide(w) {
            Decision::Convert { text, reason, .. } => Some((text, reason)),
            d => panic!("{w}: {d:?}"),
        };
        assert_eq!(out("отменаисправления").as_ref().map(|(t, r)| (t.as_str(), *r)), Some(("отмена исправления", SPACE_REASON)));
        assert_eq!(out("лдва").as_ref().map(|(t, r)| (t.as_str(), *r)), Some(("два", TYPO_REASON)));
        let mut c = typo_engine();
        c.options.typo_level = typo::Level::Cautious;
        assert!(matches!(c.decide("отменаисправления"), Decision::Convert { .. }));
        assert!(matches!(c.decide("лдва"), Decision::Keep(_)));
        for w in ["два", "отмена", "исправления", "чтобы", "также", "благополучие"] {
            assert!(matches!(e.decide(w), Decision::Keep(_)), "{w}");
        }
    }

    #[test]
    fn partial_undo_keeps_the_layout() {
        let mut e = typo_engine();
        assert_eq!(type_str(&mut e, "xnjj ")[0].insert, "что ");
        assert_eq!(bs(&mut e), Some(Replacement { delete: 4, insert: "чтоо ".into(), target: Lang::Ru }));
        assert_eq!(bs(&mut e), Some(Replacement { delete: 5, insert: "xnjj ".into(), target: Lang::En }));
        assert_eq!(bs(&mut e), None);
        let mut e = typo_engine();
        type_str(&mut e, "xnjj ");
        assert_eq!(e.undo_last(Lang::Ru), Some(Replacement { delete: 4, insert: "xnjj ".into(), target: Lang::En }));
    }

    #[test]
    fn partial_undo_twice_stops_typo_fixes_only() {
        let mut e = typo_engine();
        for round in 0..2 {
            assert_eq!(type_str(&mut e, "xnjj ")[0].insert, "что ", "round {round}");
            assert!(bs(&mut e).is_some());
            assert!(type_str(&mut e, "дальше ").is_empty());
        }
        let learned = e.take_learned();
        assert_eq!(learned, vec![Learned::UndoneOnce("typo:чтоо".into()), Learned::NeverTypo("чтоо".into())]);
        assert!(type_str(&mut e, "xnjj ").first().map_or(true, |r| r.insert != "что "));
        assert_eq!(type_str(&mut e, "превт ")[0].insert, "привет ");
    }

    #[test]
    fn undo_hotkey_walks_back_word_by_word() {
        let mut e = typo_engine();
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        assert_eq!(type_str(&mut e, "vbh ")[0].insert, "мир ");
        assert!(type_str(&mut e, "и ").is_empty());
        assert_eq!(e.undo_last(Lang::Ru), Some(Replacement { delete: 6, insert: "vbh и ".into(), target: Lang::Ru }));
        assert_eq!(e.undo_last(Lang::Ru), Some(Replacement { delete: 13, insert: "ghbdtn vbh и ".into(), target: Lang::Ru }));
        assert_eq!(e.undo_last(Lang::Ru), None);
        assert_eq!(never(&mut e), vec!["vbh".to_string(), "ghbdtn".to_string()]);
        let mut e = typo_engine();
        type_str(&mut e, "ghbdtn ");
        type_str(&mut e, "ми");
        assert_eq!(e.undo_last(Lang::Ru), Some(Replacement { delete: 9, insert: "ghbdtn ми".into(), target: Lang::Ru }));
        let mut e = typo_engine();
        type_str(&mut e, "ghbdtn");
        e.feed(Key::Boundary('\n'), false);
        type_str(&mut e, "ok ");
        assert_eq!(e.undo_last(Lang::En), None);
        let mut e = typo_engine();
        type_str(&mut e, "ghbdtn ");
        e.feed(Key::Reset, false);
        type_str(&mut e, "ok ");
        assert_eq!(e.undo_last(Lang::En), None);
    }

    #[test]
    fn never_correct_from_options() {
        let mut e = engine();
        e.options.never_correct.insert("зуфсу".into());
        assert!(type_str(&mut e, "Зуфсу. ").is_empty());
        assert_eq!(conv("ghbdtn").as_deref(), Some("привет"));
    }

    #[test]
    fn full_form_words_are_kept_and_fixes_still_work() {
        for w in ["скидывать", "скидываешь", "скидывавшимися", "оцени", "вкладками", "раскладками"] {
            assert!(is_known(w, Lang::Ru), "{w}");
            assert_eq!(typo::suggest(w, Lang::Ru), None, "{w}");
        }
        for w in ["uninstallers", "reconfiguring", "overengineered"] {
            assert_eq!(typo::suggest(w, Lang::En), None, "{w}");
        }
        assert_eq!(typo::suggest("кореркцию", Lang::Ru).as_deref(), Some("коррекцию"));
        assert_eq!(typo::suggest("влкдаки", Lang::Ru).as_deref(), Some("вкладки"));
        assert_eq!(typo::suggest("ситстемрым", Lang::Ru).as_deref(), Some("системным"));
        assert_eq!(typo::suggest("чтокой", Lang::Ru).as_deref(), Some("точкой"));
        assert_eq!(conv("зуфсу").as_deref(), Some("peace"));
        assert_eq!(conv("мфгдеюишт").as_deref(), Some("vault.bin"));
        assert_eq!(conv("еру").as_deref(), Some("the"));
        assert_eq!(conv("цуку").as_deref(), Some("were"));
        assert_eq!(conv("crblsdfdibvcz").as_deref(), Some("скидывавшимся"));
        assert_eq!(conv("скидывавшимся"), None);
    }

    #[test]
    fn brands_0_3_65() {
        let mut e = engine();
        e.options.fix_typos = true;
        assert!(type_str(&mut e, "realme ").is_empty());
        assert!(type_str(&mut e, "Realme ").is_empty());
        for w in ["realme", "tecno", "infinix", "oneplus"] {
            assert_eq!(e.decide(w), Decision::Keep("known word form in current layout"), "{w}");
        }
        assert_eq!(conv("куфдьу").as_deref(), Some("realme"));
        assert_eq!(conv("шташтшч").as_deref(), Some("infinix"));
        for w in ["реалми", "сяоми", "айфоном", "самсунга", "хуавея"] {
            assert_eq!(conv(w), None, "{w}");
            assert_eq!(e.decide(w), Decision::Keep("known word form in current layout"), "{w}");
        }
        assert_eq!(conv("fqajyjv").as_deref(), Some("айфоном"));
        assert_eq!(conv("зуфсу").as_deref(), Some("peace"));
        assert_eq!(conv("мфгдеюишт").as_deref(), Some("vault.bin"));
        assert_eq!(typo::suggest("скидывать", Lang::Ru), None);
        assert_eq!(typo::suggest("оцени", Lang::Ru), None);
    }

    #[test]
    fn missing_space_after_function_word() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "снейросеткой "), vec![Replacement { delete: 13, insert: "с нейросеткой ".into(), target: Lang::Ru }]);
        assert_eq!(type_str(&mut e, "Вофисе, ")[0].insert, "В офисе,");
        assert_eq!(type_str(&mut e, "незнаю ")[0].insert, "не знаю ");
        assert_eq!(type_str(&mut e, "кврачу ")[0].insert, "к врачу ");
        assert!(type_str(&mut e, "сделать вопрос сколько ").is_empty());
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        assert_eq!(type_str(&mut e, "нафайле ")[0].insert, "на файле ");
        assert_eq!(bs(&mut e), Some(Replacement { delete: 9, insert: "нафайле ".into(), target: Lang::Ru }));
        assert!(type_str(&mut e, "дальше ").is_empty());
        assert_eq!(type_str(&mut e, "нафайле ")[0].insert, "на файле ");
        assert!(bs(&mut e).is_some());
        assert!(type_str(&mut e, "дальше ").is_empty());
        assert_eq!(never(&mut e), vec!["нафайле".to_string()]);
        assert!(type_str(&mut e, "нафайле ").is_empty());
        let mut e = engine();
        assert_eq!(type_str(&mut e, "вофисе ")[0].insert, "в офисе ");
        assert_eq!(e.undo_last(Lang::Ru).unwrap(), Replacement { delete: 8, insert: "вофисе ".into(), target: Lang::Ru });
        assert_eq!(never(&mut e), vec!["вофисе".to_string()]);
        let mut e = engine();
        e.options.fix_typos = true;
        assert_eq!(type_str(&mut e, "отсуда ")[0].insert, "отсюда ");
        assert_eq!(type_str(&mut e, "котрое ")[0].insert, "которое ");
        assert_eq!(type_str(&mut e, "наработе ")[0].insert, "на работе ");
        assert_eq!(type_str(&mut e, "отебе ")[0].insert, "о тебе ");
        assert_eq!(type_str(&mut e, "чтокой ")[0].insert, "точкой ");
        let mut e = engine();
        e.options.fix_missing_spaces = false;
        assert!(type_str(&mut e, "вофисе незнаю ").is_empty());
        let mut e = engine();
        e.options.autocorrect = false;
        assert!(type_str(&mut e, "вофисе ").is_empty());
        let mut e = engine_with(&["вофисе"]);
        assert!(type_str(&mut e, "вофисе ").is_empty());
    }

    fn field_after(level: typo::Level, text: &str) -> String {
        let mut e = typo_engine();
        e.options.typo_level = level;
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
    fn real_forms_short_tokens_and_new_words_0_3_74() {
        assert_eq!(conv("шару"), None);
        assert_eq!(conv("иную"), None);
        assert_eq!(conv("цуку").as_deref(), Some("were"));
        assert_eq!(conv("lf").as_deref(), Some("да"));
        assert_eq!(conv("lf.").as_deref(), Some("даю"));
        assert_eq!(conv("Lf").as_deref(), Some("Да"));
        assert_eq!(conv("ns").as_deref(), Some("ты"));
        for keep in ["vs", "ok", "js", "id", "cd", "ls", "db", "ui", "pr", "dj", "us"] {
            assert_eq!(conv(keep), None, "{keep}");
        }
        let mut e = typo_engine();
        assert!(type_str(&mut e, "get ns ").is_empty());
        for level in [typo::Level::Normal, typo::Level::Aggressive] {
            let s = "есть зранилка типа nas, образ лежит на хосте -- зас сделают шару? я смогу напрямик скачать на нее? и надо почистить boot -- как лучше сделать?";
            assert_eq!(field_after(level, s), s, "{level:?}");
            assert_eq!(field_after(level, "есть хранилка типа nas"), "есть хранилка типа nas", "{level:?}");
        }
        assert_eq!(field_after(typo::Level::Normal, "lf, ns ghfd"), "да, ты прав");
    }

    #[test]
    fn punctuation_triggers_wrong_layout_word() {
        assert_eq!(type_str(&mut engine(), "ghbdtn?"), vec![Replacement {
            delete: 7, insert: "привет,".into(), target: Lang::Ru,
        }]);
        assert_eq!(type_str(&mut engine(), "ghbdtn!"), vec![Replacement {
            delete: 7, insert: "привет!".into(), target: Lang::Ru,
        }]);
        assert_eq!(type_str(&mut engine(), "руддщ!"), vec![Replacement {
            delete: 6, insert: "hello!".into(), target: Lang::En,
        }]);
        let mut e = engine();
        type_str(&mut e, "ghbdtn");
        assert_eq!(e.feed(Key::Boundary('\n'), false), Some(Replacement {
            delete: 6, insert: "привет\n".into(), target: Lang::Ru,
        }));
        let mut e = engine();
        type_str(&mut e, "руддщ");
        assert_eq!(e.feed(Key::Boundary('\t'), false), Some(Replacement {
            delete: 5, insert: "hello\t".into(), target: Lang::En,
        }));
    }

    #[test]
    fn edited_correction_is_left_alone_and_stays_in_its_layout() {
        let mut e = engine();
        e.options.undo_on_backspace = false;
        assert_eq!(type_str(&mut e, "jpdex, "), vec![Replacement {
            delete: 7, insert: "озвучб ".into(), target: Lang::Ru,
        }]);
        assert_eq!(e.feed(Key::Backspace, false), None);
        assert_eq!(e.take_layout_nudge(), Some(Lang::Ru));
        assert_eq!(e.current_len(), 6);
        assert_eq!(e.feed(Key::Backspace, false), None);
        assert_eq!(e.take_layout_nudge(), None);
        assert_eq!(e.feed(Key::Char('ь'), false), None);
        assert_eq!(e.feed(Key::Char(' '), false), None);
        assert_eq!(e.current_len(), 0);

        let mut e = Engine::new(EngineOptions { fix_typos: true, typo_level: typo::Level::Normal, undo_on_backspace: false, ..Default::default() });
        assert_eq!(type_str(&mut e, "пирвет ")[0].insert, "привет ");
        assert_eq!(e.feed(Key::Backspace, false), None);
        assert_eq!(e.take_layout_nudge(), Some(Lang::Ru));
        for _ in 0..5 {
            assert_eq!(e.feed(Key::Backspace, false), None);
        }
        assert_eq!(type_str(&mut e, "ирвет "), vec![], "edited typo fix must not be applied again");
    }

    #[test]
    fn backspace_undo_still_restores_when_enabled() {
        let mut e = engine();
        type_str(&mut e, "ghbdtn ");
        let u = e.feed(Key::Backspace, false).unwrap();
        assert_eq!(u, Replacement { delete: 7, insert: "ghbdtn ".into(), target: Lang::En });
        assert_eq!(e.take_layout_nudge(), None);
    }
}
