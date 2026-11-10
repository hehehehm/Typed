//! Wrong-layout decision engine.
//!
//! Feed it keys of the focused text field; when a word is finished it decides whether
//! the word was typed in the wrong layout and returns a [`Replacement`] for the platform
//! adapter to apply (N backspaces + corrected text + layout switch).
//!
//! ## Decision rules (in order)
//! 1. Shortcut keys (Ctrl/Alt/Super/Cmd held) never reach the buffer — they reset it.
//! 2. URLs, e-mails, paths (`@`, `://`, `www.`, `\`), tokens with digits (configurable),
//!    mixed Latin+Cyrillic tokens and weird casing (`hElLo`, password-like) are kept.
//! 2a. File names: `name.ext` with a known extension typed on the Russian layout
//!     (`мфгдеюишт`, `ю` = the `.` key) → `vault.bin`; correct Latin file names are kept.
//! 3. Too short (< `min_word_len` letters) → keep, except single letters when enabled:
//!    a letter that is a one-letter word only in the other language goes there
//!    (d → в, ф → a); real ones (a, I, в, и, я) stay; letters that are words in neither
//!    language (q/й) follow the assigned app language.
//! 3a. User words (`custom_words`) win: if the converted form is a user word → convert,
//!     even if the typed form is a known built-in word (`учу` → `exe` once `exe` is added).
//!     If the typed form is a user word → keep. No short-word rank limit for user words.
//! 4. The typed word is a known word in its own language → keep. This also covers the
//!    rare case where *both* forms are real words: we never convert automatically then
//!    (the manual hotkey still works).
//! 5. The converted form (optionally without up to 2 trailing punctuation characters,
//!    e.g. `ghbdtn,` → `привет,`) is a known word in the other language → convert.
//!    Words of ≤3 letters must be among the 3000 most frequent to count.
//! 6. Neither is a dictionary word: convert only if the typed word contains letter
//!    bigrams impossible in its language *and* the converted form has none and is at
//!    least 4 letters long. Random strings usually fail both and are kept.

use crate::buffer::WordBuffer;
use crate::dict::{self, Dictionary};
use crate::filename;
use crate::layout::{self, Lang};
use crate::typo;
use crate::userdict::UserWords;
use std::collections::HashSet;

/// Rank limit for short words spelled with sign keys (`b[` → «их»).
const SIGN_WORD_MAX_RANK: u32 = 1000;

const SINGLE_WORD_REASON: &str = "lone letter is a word only in the other language";

/// Letters that make a word on their own (lowercase).
fn is_single_letter_word(lang: Lang, c: char) -> bool {
    match lang {
        Lang::En => matches!(c, 'a' | 'i'),
        Lang::Ru => matches!(c, 'а' | 'в' | 'и' | 'к' | 'о' | 'с' | 'у' | 'я'),
    }
}

/// Words of 1–3 letters only count as dictionary hits if they are this frequent
/// (subtitle-derived lists contain short junk like "ет").
pub const SHORT_WORD_MAX_RANK: u32 = 3000;
/// Shortest token whose membership in the full-form set keeps it unchanged.
pub const FULL_FORM_MIN_LEN: usize = 4;
/// Shortest token converted only because the other layout gives a full-form-set word.
pub const FULL_FORM_CONVERT_MIN_LEN: usize = 5;

/// Tunables, normally filled from [`crate::Config`].
#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub autocorrect: bool,
    pub convert_single_letters: bool,
    /// Assigned language of the current app, supplied by the controller (not persisted).
    pub preferred_lang: Option<Lang>,
    pub skip_urls_emails: bool,
    pub skip_tokens_with_digits: bool,
    pub min_word_len: usize,
    /// User dictionary, refreshed from the config on reload.
    pub user_words: UserWords,
    /// Fix one-letter typos toward a frequent word of the same language.
    pub fix_typos: bool,
    /// Words the user undid (normalised: lowercase, ё → е). Never autocorrected again.
    pub never_correct: HashSet<String>,
    /// Backspace right after an autocorrection (before any other key) undoes it.
    pub undo_on_backspace: bool,
}

/// `Decision::Convert` reason for a typo fix (kept for undo).
pub const TYPO_REASON: &str = "typo fix";

impl Default for EngineOptions {
    fn default() -> Self {
        Self { autocorrect: true, convert_single_letters: true, preferred_lang: None, skip_urls_emails: true, skip_tokens_with_digits: true, min_word_len: 2, user_words: UserWords::default(), fix_typos: false, never_correct: HashSet::new(), undo_on_backspace: true }
    }
}

/// A key as seen by the engine (already translated to the character it produced).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A printable character. Whitespace and pure punctuation act as word boundaries.
    Char(char),
    Backspace,
    /// Enter / Tab and similar: finish the word, the boundary is that character.
    Boundary(char),
    /// Cursor moved, mouse click, focus change, Escape, … — forget everything.
    Reset,
}

/// Edit for the platform adapter: erase `delete` characters before the caret,
/// type `insert`, and make `target` the active layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub delete: usize,
    pub insert: String,
    pub target: Lang,
}

/// Outcome of judging a finished token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Keep(&'static str),
    Convert { text: String, target: Lang, reason: &'static str },
}

#[derive(Debug, Default)]
struct LastWord {
    word: String,
    boundary: String,
    /// Set when this word is an autocorrection (wrong layout or typo): what was typed.
    /// Undo (hotkey, conversion hotkey, Backspace) puts it back.
    fix: Option<AutoFix>,
    /// No key since the word was finished: the Backspace-undo window.
    fresh: bool,
}

#[derive(Debug, Clone)]
struct AutoFix {
    /// The word exactly as typed.
    typed: String,
    /// The first boundary character as typed (before punctuation conversion).
    typed_boundary: String,
}

/// The last autocorrection, kept while the user erases it by hand: retyping exactly
/// what was typed means "this was right" (the stubborn-repeat case).
#[derive(Debug, Clone)]
struct RecentFix {
    typed: String,
    erased: bool,
}

impl Decision {
}

/// Stateful engine for one text-input stream.
#[derive(Debug, Default)]
pub struct Engine {
    pub options: EngineOptions,
    buf: WordBuffer,
    last: Option<LastWord>,
    recent_fix: Option<RecentFix>,
    /// Undone with Backspace: remembered once the user moves on (not if they erase it).
    pending_learn: Option<String>,
    /// Newly remembered never-correct words for the controller to save.
    learned: Vec<String>,
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

/// Casing a human produces: all lower, all upper, or Capitalised.
fn natural_case(letters: &[char]) -> bool {
    let up = letters.iter().filter(|c| c.is_uppercase()).count();
    up == 0 || up == letters.len() || (up == 1 && letters[0].is_uppercase())
}

fn trim_non_letters(s: &str) -> &str {
    s.trim_matches(|c: char| !c.is_alphabetic())
}

impl Engine {
    pub fn new(options: EngineOptions) -> Self {
        Self { options, ..Default::default() }
    }

    /// Forget the current and last word (focus change, click, cursor movement).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.last = None;
        self.recent_fix = None;
        if let Some(w) = self.pending_learn.take() {
            self.remember(&w);
        }
    }

    /// Never-correct words remembered since the last call (for the config).
    pub fn take_learned(&mut self) -> Vec<String> {
        std::mem::take(&mut self.learned)
    }

    /// Add `word` to the never-correct list (this session and, via `take_learned`, the config).
    fn remember(&mut self, word: &str) {
        let key = never_key(word);
        if !key.is_empty() && self.options.never_correct.insert(key.clone()) {
            log::debug!("never-correct: remembered a word of {} chars", key.chars().count());
            self.learned.push(key);
        }
    }

    /// Undo the last autocorrection (dedicated hotkey): the word exactly as typed comes
    /// back with its boundary, the layout it was typed in is activated, and the word is
    /// never corrected again. `None` when the last word was not autocorrected or the
    /// caret may have moved since.
    pub fn undo_last(&mut self, current_layout: Lang) -> Option<Replacement> {
        if !self.buf.is_empty() {
            return None;
        }
        let rep = self.restore_last(current_layout)?;
        self.recent_fix = None;
        if let Some(w) = self.last.as_ref().map(|l| l.word.clone()) {
            self.remember(&w);
        }
        Some(rep)
    }

    /// Put the typed word back in place of the autocorrected last word.
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

    /// Backspace right after an autocorrection: undo it instead of erasing the boundary
    /// (the platform swallows that Backspace). Only for a single space / sign boundary;
    /// after Enter or Tab the text may already be sent.
    fn backspace_undo(&mut self, current_layout: Lang) -> Option<Replacement> {
        let last = self.last.as_ref()?;
        if !self.options.undo_on_backspace
            || !last.fresh
            || last.fix.is_none()
            || last.boundary.chars().count() != 1
            || last.boundary.contains(['\n', '\t', '\r'])
        {
            return None;
        }
        let rep = self.restore_last(current_layout)?;
        let typed = self.last.as_ref().map(|l| l.word.clone()).unwrap_or_default();
        // If they erase the restored word and type it again, that still counts.
        self.recent_fix = Some(RecentFix { typed: typed.clone(), erased: false });
        self.pending_learn = Some(typed);
        Some(rep)
    }

    /// Length of the word in progress (for debug logging — never its content).
    pub fn current_len(&self) -> usize {
        self.buf.len()
    }

    /// Process a key. `modifiers_held` = Ctrl/Alt/Super/Cmd were down (a shortcut).
    pub fn feed(&mut self, key: Key, modifiers_held: bool) -> Option<Replacement> {
        if modifiers_held {
            if key == Key::Backspace {
                // Ctrl/Alt+Backspace erases a word: keep watching for a retyped original.
                self.mark_erased();
                self.buf.clear();
                self.last = None;
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
                }
                if !self.buf.backspace() {
                    // Erasing into the previous word: we no longer know what is on screen.
                    self.mark_erased();
                    self.last = None;
                }
                None
            }
            Key::Char(c) if !is_boundary(c) => {
                if !self.buf.push(c) {
                    self.last = None;
                }
                None
            }
            // Char: the boundary is already on screen (space, punctuation).
            // Boundary: Enter/Tab has not been delivered; the platform swallows it
            // and we inject it after the corrected word (delete does not include it).
            Key::Char(c) => self.finish_word(c, true),
            Key::Boundary(c) => self.finish_word(c, false),
        }
    }

    /// The user erases into the previous word (Backspace with an empty buffer).
    fn mark_erased(&mut self) {
        // Erasing the word just restored with Backspace: they did not want it either.
        if self.last.is_some() {
            self.pending_learn = None;
        }
        if let Some(rf) = self.recent_fix.as_mut() {
            rf.erased = true;
        }
    }

    fn finish_word(&mut self, boundary: char, already_typed: bool) -> Option<Replacement> {
        let token = self.buf.take();
        if token.is_empty() {
            if let Some(last) = self.last.as_mut() {
                last.fresh = false;
                if last.boundary.chars().count() < 4 {
                    last.boundary.push(boundary);
                    return None;
                }
            }
            self.last = None;
            return None;
        }
        if let Some(w) = self.pending_learn.take() {
            self.remember(&w);
        }
        // Erased an autocorrection by hand and typed the original again: keep it, forever.
        if self.recent_fix.take().is_some_and(|rf| rf.erased && never_key(&rf.typed) == never_key(&token)) {
            self.remember(&token);
        }
        // Glued to the previous word by a non-space sign (`З!К)П`, `a)b`): that is a
        // password or code, not prose, so it is never autocorrected.
        let glued = self.last.as_ref().is_some_and(|l| {
            !l.boundary.is_empty() && !l.boundary.chars().any(char::is_whitespace)
        });
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
                let fix = AutoFix { typed: token.clone(), typed_boundary: boundary.to_string() };
                self.recent_fix = Some(RecentFix { typed: token, erased: false });
                self.last = Some(LastWord { word: text, boundary: b, fix: Some(fix), fresh: true });
                Some(rep)
            }
            Decision::Keep(reason) => {
                log::debug!("autocorrect: keep word of {} chars ({})", token.chars().count(), reason);
                self.last = Some(LastWord { word: token, boundary: b, fix: None, fresh: false });
                None
            }
        }
    }

    /// Manual conversion (hotkey): flips the word in progress, or else the last finished
    /// word, regardless of dictionaries. Pressing it again flips back (undo).
    pub fn manual_convert(&mut self, current_layout: Lang) -> Option<Replacement> {
        if !self.buf.is_empty() {
            let word = self.buf.as_string();
            let from = layout::detect_script(&word).unwrap_or(current_layout);
            let conv = layout::convert(&word, from);
            self.buf.set(&conv);
            return Some(Replacement { delete: word.chars().count(), insert: conv, target: from.other() });
        }
        if self.last.as_ref().is_some_and(|l| l.fix.is_some()) {
            // Right after an autocorrection the conversion hotkey is an undo too.
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

    /// Pure decision for a finished token (no state change). See module docs for the rules.
    pub fn decide(&self, token: &str) -> Decision {
        let o = &self.options;
        if !o.never_correct.is_empty() && o.never_correct.contains(&never_key(token)) {
            return Decision::Keep("never correct (undone by the user)");
        }
        if let Some(d) = self.user_word_decision(token) {
            return d;
        }
        if o.skip_urls_emails && looks_like_url_or_email(token) {
            return Decision::Keep("url/email/path");
        }
        // File names: `vault.bin` stays, `мфгдеюишт` (the same keys on RU) → `vault.bin`.
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
                    // A lone letter that is a word in only one language goes there:
                    // d → в, z → я, ф → a, ш → i; real ones (a, I; в, и, я) stay.
                    if conv_word && !typed_word {
                        return Decision::Convert { text: converted, target: dst, reason: SINGLE_WORD_REASON };
                    }
                    // Neither is a word (q/й, x/ч): follow the app's assigned language.
                    if !typed_word && !conv_word && o.preferred_lang == Some(dst) {
                        return Decision::Convert {
                            text: converted, target: dst, reason: "single letter to app language",
                        };
                    }
                }
            }
            // Signs that are letters in the other layout: `b[` → «их», `e;` → «уж».
            // Only very common words; the dictionary folds ё into е, so `t`` must not
            // pass as «тё» = «те» (её is the one real ё word here).
            let chars: Vec<char> = token.chars().collect();
            let dst_dict = dict::get(src.other());
            if let Some(d) = Self::try_convert(&chars, src, o.min_word_len, |w| {
                let lower = w.to_lowercase();
                (!lower.contains('ё') || lower == "её")
                    && dst_dict.rank(w).is_some_and(|r| r < SIGN_WORD_MAX_RANK)
            }) {
                return d;
            }
            return Decision::Keep("too short");
        }
        if !natural_case(&letters) {
            return Decision::Keep("unnatural casing (password-like)");
        }
        let dst = src.other();
        let src_dict = dict::get(src);
        let dst_dict = dict::get(dst);

        let chars: Vec<char> = token.chars().collect();

        if src_dict.contains(core) {
            return Decision::Keep("known word in current layout");
        }

        if let Some(d) = Self::try_convert(&chars, src, o.min_word_len, |w| {
            dst_dict.rank(w).is_some_and(|r| w.chars().count() > 3 || r < SHORT_WORD_MAX_RANK)
        }) {
            return d;
        }

        // Full-form sets (every case, number, tense): a rare but real form in the typed
        // layout stays, unless its other-layout reading is a ranked word (checked above:
        // `еру` is a form in the big set, but `the` wins). Short tokens are left to the
        // ranked lists, where the big sets are full of 2–3 letter abbreviations.
        let n_letters = letters.len();
        if n_letters >= FULL_FORM_MIN_LEN && src_dict.in_full_forms(core) {
            return Decision::Keep("known word form in current layout");
        }
        // Unknown in the typed layout, a real (rare) form in the other one: convert.
        if n_letters >= FULL_FORM_CONVERT_MIN_LEN {
            if let Some(d) = Self::try_convert(&chars, src, o.min_word_len, |w| {
                w.chars().count() >= FULL_FORM_CONVERT_MIN_LEN && dst_dict.in_full_forms(w)
            }) {
                return d;
            }
        }

        if o.fix_typos {
            if core.chars().all(char::is_alphabetic) {
                if let Some(fix) = typo::suggest(core, src) {
                    return Decision::Convert { text: token.replacen(core, &fix, 1), target: src, reason: TYPO_REASON };
                }
            }
        }

        // Heuristic fallback for words missing from both dictionaries.
        if core.chars().all(char::is_alphabetic) && letters.len() >= 4 {
            let conv = layout::convert(core, src);
            if conv.chars().all(char::is_alphabetic)
                && layout::detect_script(&conv) == Some(dst)
                && src_dict.unseen_bigrams(core) >= 1
                && dst_dict.unseen_bigrams(&conv) == 0
            {
                let text = layout::convert(token, src);
                return Decision::Convert { text, target: dst, reason: "letter bigrams fit other language" };
            }
        }
        Decision::Keep("not confident")
    }

    /// User words win over every other rule, digits and signs included (`ьз3` → `mp3`,
    /// `сш.св` → `ci/cd`). Up to two trailing signs may follow the word.
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

    /// Convert the whole token, then without up to two trailing punctuation chars, and
    /// accept the first converted core that `known` approves.
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

/// Key for the never-correct list: the token's letters core, lowercase, `ё` → `е`.
pub fn never_key(token: &str) -> String {
    Dictionary::normalize(trim_non_letters(token))
}

/// Convenience: normalised dictionary check used by tests and the CLI.
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
        // Find a real collision in the vendored lists and make sure we keep it.
        let en = dict::get(Lang::En);
        let ru = dict::get(Lang::Ru);
        let _ = (en, ru);
        for w in ["ytn", "rfr"] {
            // "ytn"→"нет", "rfr"→"как": EN side not words → converted.
            assert!(conv(w).is_some(), "{w}");
        }
        // A token that is a word in its own layout is kept even if its conversion is a word too.
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
        // Enter has not been typed into the field yet — only the word is deleted.
        assert_eq!(r.delete, 6);
        let mut e = engine();
        type_str(&mut e, "ghbdtn");
        let r = e.feed(Key::Boundary('\t'), false).unwrap();
        assert_eq!(r.insert, "привет\t");
        assert_eq!(r.delete, 6);
        // A typed space *is* already on screen.
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
        // '+' stays inside the token so `ctrl+shift+k` (and a single letter in the
        // other layout, e.g. `ctrl+shift+к`) is one mixed or punctuated word, not three.
        let mut e = engine();
        assert!(type_str(&mut e, "ctrl+shift+k").is_empty());
        assert_eq!(e.current_len(), "ctrl+shift+k".chars().count());
        assert!(matches!(engine().decide("ctrl+shift+к"), Decision::Keep(_)));
    }

    #[test]
    fn manual_convert_current_and_last() {
        let mut e = engine();
        type_str(&mut e, "qzx");
        // Current word, dictionary-unsure: still converts.
        let r = e.manual_convert(Lang::En).unwrap();
        assert_eq!(r, Replacement { delete: 3, insert: "йяч".into(), target: Lang::Ru });
        // Pressing again flips back.
        assert_eq!(e.manual_convert(Lang::Ru).unwrap().insert, "qzx");
        type_str(&mut e, " ");
        // Last committed word incl. its boundary.
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
        // ",eltn" = "будет": ',' is the б key, so it must stay part of the word.
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
        // Not a word in the other language: left alone.
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
        // Password-like: a letter glued to a sign stays.
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
        // "ghbdtn" → "привет" normally; as a user word it stays.
        assert_eq!(engine_with(&["ghbdtn"]).decide("ghbdtn"), Decision::Keep("user word in current layout"));
        // Unknown 3-letter typed form: user word on the typed side wins over a dictionary hit.
        assert_eq!(engine_with(&["rfr"]).decide("rfr"), Decision::Keep("user word in current layout"));
    }

    #[test]
    fn builtin_extras_convert_but_do_not_override() {
        assert_eq!(conv("дщд").as_deref(), Some("lol"));
        assert_eq!(conv("сшсв").as_deref(), Some("cicd"));
        assert_eq!(conv("шьрщ").as_deref(), Some("imho"));
        assert_eq!(conv("bv[j").as_deref(), Some("имхо"));
        // "exe" ships as a built-in extra, but must not beat the real Russian word.
        assert!(dict::get(Lang::En).contains("exe"));
        assert_eq!(conv("учу"), None);
    }
    #[test]
    fn single_letters_to_assigned_language() {
        let mut e = engine();
        e.options.preferred_lang = Some(Lang::Ru);
        // Not a word anywhere: the app language decides.
        assert_eq!(type_str(&mut e, "q "), vec![Replacement { delete: 2, insert: "й ".into(), target: Lang::Ru }]);
        // A real English word stays even in a Russian app.
        assert!(type_str(&mut e, "a ").is_empty());
        assert_eq!(type_str(&mut e, "d ")[0].insert, "в ");
        e.options.preferred_lang = Some(Lang::En);
        assert_eq!(type_str(&mut e, "й ")[0].insert, "q ");
        assert!(type_str(&mut e, "в ").is_empty());
        assert_eq!(type_str(&mut e, "ф ")[0].insert, "a ");
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
        // US keys for «например» are yfghbvth (not ntghbvth).
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
            delete: 7, insert: "hello? ".into(), target: Lang::En,
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
        // Prose with punctuation still converts.
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
        assert_eq!(r, vec![Replacement { delete: 8, insert: "привет, ".into(), target: Lang::Ru }]);
        let r = type_str(&mut e, "Becuase ");
        assert_eq!(r[0].insert, "Because ");
        let undo = e.manual_convert(Lang::En).unwrap();
        assert_eq!(undo, Replacement { delete: 8, insert: "Becuase ".into(), target: Lang::En });
        // Wrong layout still wins over typo fixing.
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        // User words are never "fixed".
        let mut e = Engine::new(EngineOptions { user_words: UserWords::from_words(&["пирвет"]), fix_typos: true, ..Default::default() });
        assert!(type_str(&mut e, "пирвет ").is_empty());
    }


    #[test]
    fn abbreviations_convert() {
        assert_eq!(conv("пкфзрйд").as_deref(), Some("graphql"));
        assert_eq!(conv("vxc").as_deref(), Some("мчс"));
        assert_eq!(conv("cybkc").as_deref(), Some("снилс"));
        // And are not dragged into the other layout.
        assert_eq!(conv("мчс"), None);
        assert_eq!(conv("oauth"), None);
    }


    #[test]
    fn file_names_in_wrong_layout() {
        assert_eq!(conv("мфгдеюишт").as_deref(), Some("vault.bin"));
        assert_eq!(conv("ензув-0ю3ю59юефкюпя").as_deref(), Some("typed-0.3.59.tar.gz"));
        let mut e = engine();
        assert_eq!(type_str(&mut e, "мфгдеюишт "), vec![Replacement { delete: 10, insert: "vault.bin ".into(), target: Lang::En }]);
        // Correct names never go to Russian, Russian words with ю/б stay.
        for keep in ["vault.bin", "typed-0.3.59.tar.gz", "README.md", "люблю", "сюда", "плюс"] {
            assert_eq!(conv(keep), None, "{keep}");
        }
        let mut e = engine();
        assert!(type_str(&mut e, "vault.bin ").is_empty());
        // Russian words typed on EN with the ю key still convert, punctuation as before.
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

    fn bs(e: &mut Engine) -> Option<Replacement> {
        e.feed(Key::Backspace, false)
    }

    #[test]
    fn undo_hotkey_restores_and_remembers() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        let undo = e.undo_last(Lang::Ru).unwrap();
        assert_eq!(undo, Replacement { delete: 7, insert: "ghbdtn ".into(), target: Lang::En });
        assert_eq!(e.take_learned(), vec!["ghbdtn".to_string()]);
        // Never again, in any casing; other words still convert.
        assert!(type_str(&mut e, "ghbdtn ").is_empty());
        assert!(type_str(&mut e, "Ghbdtn, ").is_empty());
        assert_eq!(type_str(&mut e, "pltcm ")[0].insert, "здесь ");
        // Nothing to undo after a kept word or in the middle of a word.
        assert!(type_str(&mut e, "hello ").is_empty());
        assert_eq!(e.undo_last(Lang::En), None);
        type_str(&mut e, "ghbd");
        assert_eq!(e.undo_last(Lang::En), None);
    }

    #[test]
    fn undo_restores_typed_punctuation_and_typo() {
        let mut e = engine();
        e.options.fix_typos = true;
        // `?` on the EN layout is `,` on the RU one; undo puts back what was typed.
        assert_eq!(type_str(&mut e, "ghbdtn?")[0].insert, "привет,");
        assert_eq!(e.undo_last(Lang::Ru).unwrap().insert, "ghbdtn?");
        assert_eq!(type_str(&mut e, " пирвет ")[0].insert, "привет ");
        assert_eq!(e.undo_last(Lang::Ru).unwrap(), Replacement { delete: 7, insert: "пирвет ".into(), target: Lang::Ru });
        assert!(type_str(&mut e, "пирвет ").is_empty());
        let mut learned = e.take_learned();
        learned.sort();
        assert_eq!(learned, vec!["ghbdtn".to_string(), "пирвет".to_string()]);
    }

    #[test]
    fn conversion_hotkey_right_after_fix_is_undo() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "зуфсу ")[0].insert, "peace ");
        let r = e.manual_convert(Lang::En).unwrap();
        assert_eq!(r, Replacement { delete: 6, insert: "зуфсу ".into(), target: Lang::Ru });
        assert_eq!(e.take_learned(), vec!["зуфсу".to_string()]);
        // Pressed again: a plain flip, as before.
        assert_eq!(e.manual_convert(Lang::Ru).unwrap().insert, "peace ");
    }

    #[test]
    fn backspace_right_after_fix_undoes() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "зуфсу ")[0].insert, "peace ");
        assert_eq!(bs(&mut e), Some(Replacement { delete: 6, insert: "зуфсу ".into(), target: Lang::Ru }));
        // Remembered only once the user moves on.
        assert!(e.take_learned().is_empty());
        assert!(type_str(&mut e, "привет ").is_empty());
        assert_eq!(e.take_learned(), vec!["зуфсу".to_string()]);
        assert!(type_str(&mut e, "зуфсу ").is_empty());
    }

    #[test]
    fn backspace_undo_only_first_key_and_not_after_enter() {
        let mut e = engine();
        // Another key in between: Backspace just erases.
        type_str(&mut e, "ghbdtn ");
        type_str(&mut e, "a");
        assert_eq!(bs(&mut e), None);
        assert_eq!(bs(&mut e), None);
        // Second Backspace after an undo erases normally.
        let mut e = engine();
        type_str(&mut e, "ghbdtn ");
        assert!(bs(&mut e).is_some());
        assert_eq!(bs(&mut e), None);
        // After Enter the message may be sent already.
        let mut e = engine();
        assert!(e.feed(Key::Char('g'), false).is_none());
        type_str(&mut e, "hbdtn");
        assert!(e.feed(Key::Boundary('\n'), false).is_some());
        assert_eq!(bs(&mut e), None);
        // Switched off in the settings.
        let mut e = engine();
        e.options.undo_on_backspace = false;
        type_str(&mut e, "ghbdtn ");
        assert_eq!(bs(&mut e), None);
    }

    #[test]
    fn erased_and_retyped_original_is_kept_and_remembered() {
        // The «stubbornly repeats the mistake» case: erase the fix, type it again.
        let mut e = engine();
        e.options.undo_on_backspace = false;
        e.options.fix_typos = true;
        assert_eq!(type_str(&mut e, "пирвет ")[0].insert, "привет ");
        for _ in 0..7 {
            assert_eq!(bs(&mut e), None);
        }
        assert!(type_str(&mut e, "пирвет ").is_empty(), "retyped original is kept");
        assert_eq!(e.take_learned(), vec!["пирвет".to_string()]);
        // Ctrl+Backspace works the same.
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        assert!(e.feed(Key::Backspace, true).is_none());
        assert!(type_str(&mut e, "ghbdtn ").is_empty());
        assert_eq!(e.take_learned(), vec!["ghbdtn".to_string()]);
        // A different word after erasing is judged normally and nothing is learned.
        assert_eq!(type_str(&mut e, "pltcm ")[0].insert, "здесь ");
        for _ in 0..6 {
            bs(&mut e);
        }
        assert_eq!(type_str(&mut e, "ntcn ")[0].insert, "тест ");
        assert!(e.take_learned().is_empty());
        // With Backspace-undo on: the first Backspace restores, the user keeps erasing
        // and retypes the original: still kept and remembered.
        let mut e = engine();
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        assert!(bs(&mut e).is_some());
        for _ in 0..7 {
            bs(&mut e);
        }
        assert!(type_str(&mut e, "ghbdtn ").is_empty());
        assert_eq!(e.take_learned(), vec!["ghbdtn".to_string()]);
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
        // Real forms missing from the ranked lists are never "fixed".
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
        // `еру` is a form in the full Russian set, but `the` is a ranked word: convert.
        assert_eq!(conv("еру").as_deref(), Some("the"));
        assert_eq!(conv("цуку").as_deref(), Some("were"));
        // A rare Russian form typed on the EN layout converts via the full set.
        assert_eq!(conv("crblsdfdibvcz").as_deref(), Some("скидывавшимся"));
        // A rare real Russian form typed correctly stays.
        assert_eq!(conv("скидывавшимся"), None);
    }
}
