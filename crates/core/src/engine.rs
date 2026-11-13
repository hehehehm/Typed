//! Wrong-layout decision engine.
//!
//! Feed it keys of the focused text field; when a word is finished it decides whether
//! the word was typed in the wrong layout and returns a [`Replacement`] for the platform
//! adapter to apply (N backspaces + corrected text + layout switch).
//!
//! ## Decision rules (in order)
//! 1. Shortcut keys (Ctrl/Alt/Super/Cmd held) never reach the buffer — they reset it.
//! 2. URLs, e-mails, paths (`@`, `://`, `www.`, `\`), tokens with digits (configurable),
//!    and weird casing (`hElLo`, password-like) are kept. Mixed Latin+Cyrillic tokens
//!    are kept unless re-reading one script in the other layout gives a known word (or,
//!    with typo fixes, a unique fix): `приdtn` → `привет`, see `mixed_decision`.
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
//! 5a. Missing space (`fix_missing_spaces`): an unknown Russian token that is a glued
//!     preposition / «не» + a frequent word becomes two words (`вофисе` → `в офисе`),
//!     see [`crate::split`]; curated glued forms (`незнаю`) are split before rule 4.
//! 6. Neither is a dictionary word: convert only if the typed word contains letter
//!    bigrams impossible in its language *and* the converted form has none and is at
//!    least 4 letters long. Random strings usually fail both and are kept.

use crate::buffer::WordBuffer;
use crate::dict::{self, Dictionary};
use crate::filename;
use crate::layout::{self, Lang};
use crate::split;
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

/// Each half of a hyphenated word typed in the wrong layout (`dct-nfrb` = `все-таки`)
/// must be at least this frequent (4+ letters; shorter halves use [`SHORT_WORD_MAX_RANK`]).
pub const HYPHEN_PART_MAX_RANK: u32 = 20_000;

/// Russian particles that only follow a hyphen (`кто-то`, `дай-ка`, `все-таки`).
const HYPHEN_PARTICLES: [&str; 6] = ["то", "либо", "нибудь", "таки", "ка", "де"];
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
    /// Insert a missing space after a glued Russian function word (`вофисе` → `в офисе`).
    pub fix_missing_spaces: bool,
}

/// `Decision::Convert` reason for a typo fix (kept for undo).
pub const TYPO_REASON: &str = "typo fix";
/// Layout switched in the middle of a word (`приdtn` → `привет`).
pub const MIXED_REASON: &str = "layout switched mid-word";
/// Letters a mixed Latin+Cyrillic word needs before it is read as one word.
const MIXED_MIN_LETTERS: usize = 3;
/// One-letter Russian case endings (`iPhoneа`, `sheы`, `Windowsе`): such a letter after a
/// Latin word is kept as an ending, never re-read on the Latin layout.
const CASE_ENDING_LETTERS: &str = "аеёиоуыюяй";
/// A 3-letter mixed token (`prщ` → `pro`) converts only into a word this frequent.
const MIXED_SHORT_MAX_RANK: u32 = 5_000;
/// English words this frequent, followed by a short Cyrillic ending, are read as a name
/// with a Russian case ending (`keyами`), never re-read as Russian.
const BRAND_PREFIX_RANK: u32 = 20_000;
/// `Decision::Convert` reason for an inserted missing space (undoable like a typo fix).
pub const SPACE_REASON: &str = "missing space after a function word";

impl Default for EngineOptions {
    fn default() -> Self {
        Self { autocorrect: true, convert_single_letters: true, preferred_lang: None, skip_urls_emails: true, skip_tokens_with_digits: true, min_word_len: 2, user_words: UserWords::default(), fix_typos: false, never_correct: HashSet::new(), undo_on_backspace: true, fix_missing_spaces: true }
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

/// The letters without the ones at either end that are signs on the other layout
/// (Ж = `:`, Э = `"`, Б = `<`, Ю = `>`, Х = `{`, Ъ = `}`): `тучесдщгвЖ` is `nextcloud:`,
/// and its capital Ж is just Shift for the colon, not password-like casing. Capitals in
/// the middle still count (`пароЛь`). Leaves at least one letter.
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
        if !natural_case(edge_signs_trimmed(&letters, src)) {
            return Decision::Keep("unnatural casing (password-like)");
        }
        let dst = src.other();
        let src_dict = dict::get(src);
        let dst_dict = dict::get(dst);

        let chars: Vec<char> = token.chars().collect();

        let ranked_convert = || {
            Self::try_convert(&chars, src, o.min_word_len, |w| {
                dst_dict.rank(w).is_some_and(|r| w.chars().count() > 3 || r < SHORT_WORD_MAX_RANK)
            })
        };
        // Passed the casing guard only because of a shifted sign at the edge
        // (`тучесдщгвЖ`): the other-layout reading may convert, nothing else (no typo
        // fix, split or guess on a token cased like that).
        if !natural_case(&letters) {
            return Self::try_convert(&chars, src, o.min_word_len, |w| {
                dst_dict.rank(w).is_some_and(|r| w.chars().count() > 3 || r < SHORT_WORD_MAX_RANK)
                    || (w.chars().count() >= FULL_FORM_CONVERT_MIN_LEN && dst_dict.in_full_forms(w))
            })
            .unwrap_or(Decision::Keep("unnatural casing (password-like)"));
        }
        // Glued forms the lists carry as words (`незнаю`): split before "known word".
        if o.fix_missing_spaces && src == Lang::Ru && split::is_glued(core) {
            if let Some(d) = ranked_convert() {
                return d;
            }
            if let Some(d) = Self::split_decision(token, core, src) {
                return d;
            }
        }

        if src_dict.contains(core) {
            return Decision::Keep("known word in current layout");
        }
        // One hyphen inside (`dct-nfrb` = `все-таки`): '-' is the same key in both
        // layouts, so the token arrives whole; each half must read as a frequent word.
        if let Some(d) = Self::hyphen_converted(&chars, src) {
            return d;
        }

        if let Some(d) = ranked_convert() {
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

        // Wrong layout AND a missing space (`yflj,s` = `надобы` → `надо бы`, `dJabct` →
        // `в офисе`): the other-layout reading is not a word, but splits like one.
        if o.fix_missing_spaces && dst == Lang::Ru {
            if let Some(d) = Self::split_converted(token, src) {
                return d;
            }
        }
        // Missing space after a function word (`вофисе` → `в офисе`): the token is in its
        // own layout (no conversion fired above) and unknown in every list.
        if o.fix_missing_spaces {
            if let Some(d) = Self::split_decision(token, core, src) {
                // A typo fix that puts back a missing letter, swaps letters back or fixes
                // one vowel beats the split (`отсуда` → `отсюда`, not `от суда`;
                // `котрое` → `которое`); other fixes lose (`отебе` is `о тебе`, not `отеле`).
                if o.fix_typos && core.chars().all(char::is_alphabetic) {
                    if let Some(fix) = typo::suggest(core, src) {
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
                        if m > n || a == b || vowel_slip {
                            return Decision::Convert { text: token.replacen(core, &fix, 1), target: src, reason: TYPO_REASON };
                        }
                    }
                }
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

    /// A word with both Latin and Cyrillic letters: the layout was switched mid-word
    /// (`приdtn` → `привет`, `hellщ` → `hello`). The letters of one script are re-read in
    /// the other layout; the result must be a known word, or (with typo fixes on, and only
    /// for lowercase words) have a unique typo fix. If both readings qualify, nothing
    /// changes. Mixed identifiers stay: `Gitом`, `iPhoneа` (Capitalised, unknown either
    /// way), `PRы` (casing), anything with digits (checked before).
    fn mixed_decision(&self, token: &str) -> Option<Decision> {
        let o = &self.options;
        let core = trim_non_letters(token);
        let letters: Vec<char> = core.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() < MIXED_MIN_LETTERS || !natural_case(&letters) {
            return None;
        }
        // Script runs of the letters: `gitом` = [git][ом].
        let script = |c: char| layout::detect_script(c.encode_utf8(&mut [0; 4]));
        let mut runs: Vec<(Lang, usize)> = Vec::new();
        for &c in &letters {
            let Some(l) = script(c) else { continue };
            match runs.last_mut() {
                Some((rl, n)) if *rl == l => *n += 1,
                _ => runs.push((l, 1)),
            }
        }
        // A word with a short ending in the other script is usually a Latin name with a
        // Russian case ending (`sheы`, `gitом`, `Windowsе`), not a slip: one letter that can
        // be a case ending never changes; any other single letter (`prщ`, `hellщ`: щ is the
        // `o` key) and two-letter endings change only into an exact word.
        let mut typos_ok = o.fix_typos && letters.iter().all(|c| c.is_lowercase());
        let mut brand_ending = false;
        if let [(first, n), (_, tail)] = runs[..] {
            let prefix: String = letters[..n].iter().collect();
            // A common English word + Russian case ending (`keyами`, `Heyом`) is not re-read
            // as Russian (`Лунами`, `Руном`); `ghbвет` (rare `ghb`) still becomes `привет`.
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
        // Three letters: one switch only (`prщ`, not `аща`) and only into a frequent word.
        // When the two-letter run is a word itself, only the one odd letter is re-read
        // (`неn` → `нет`, `toщ` → `too`), never the word (`даg` is not `lfg`).
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
        // `.` `[` `;` … at the end may be ю х ж typed on the Latin layout: first read the
        // whole token (exact words only), then the letters alone.
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
                if let Some(fix) = typo::suggest(&cand, lang) {
                    found.push((false, token.replacen(core, &fix, 1), lang));
                }
            }
        }
        // An exact reading beats a typo fix in the other script; two of a kind: keep.
        let exact: Vec<_> = found.iter().filter(|f| f.0).collect();
        let pick = match (exact.len(), found.len()) {
            (1, _) => exact[0],
            (0, 1) => &found[0],
            _ => return None,
        };
        Some(Decision::Convert { text: pick.1.clone(), target: pick.2, reason: MIXED_REASON })
    }

    /// [`split::split_glued`] on the other-layout reading of `core`; signs around it are
    /// converted as signs (`yflj,s!` → `надо бы!`).
    fn split_converted(token: &str, src: Lang) -> Option<Decision> {
        let dst = src.other();
        let chars: Vec<char> = token.chars().collect();
        // Like `try_convert`: signs on letter keys count (`,skjkb` = `былоли`), up to two
        // trailing signs stay signs (`yflj,s!` → `надо бы!`).
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

    /// `dct-nfrb` → `все-таки`, `bp-pf` → `из-за`, `rnj-nb,elm` → `кто-нибудь`: exactly one
    /// hyphen between letters, both halves frequent words in the other layout (short
    /// halves very frequent, or a hyphen particle after the hyphen; 5+ letter halves may
    /// be any real form), and the typed halves
    /// not both words of the typed language (`well-known`, `up-to` stay). Into English one
    /// half must have 4+ letters (`цудд-лтщцт` → `well-known`; `гы-гы` stays). Up to two
    /// trailing signs stay signs.
    fn hyphen_converted(chars: &[char], src: Lang) -> Option<Decision> {
        let dst = src.other();
        let (src_dict, dst_dict) = (dict::get(src), dict::get(dst));
        let frequent = |w: &str, after: bool| {
            let lower = Dictionary::normalize(w);
            (after && dst == Lang::Ru && HYPHEN_PARTICLES.contains(&lower.as_str()))
                || dst_dict.rank(&lower).is_some_and(|r| {
                    r < if lower.chars().count() <= 3 { SHORT_WORD_MAX_RANK } else { HYPHEN_PART_MAX_RANK }
                })
                // A rarer form of 5+ letters (`конференциях`), as for whole words.
                || (lower.chars().count() >= FULL_FORM_CONVERT_MIN_LEN && dst_dict.in_full_forms(&lower))
        };
        // The whole token read in the other layout, when its last half is a real (if
        // rare) form: a trailing sign is a letter there (`ghtcc-rjyathtywbz[` is
        // `пресс-конференциях`, not `пресс-конференция[`).
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
            // The typed halves: letters only (signs on letter keys come next).
            if src_dict.contains(&Dictionary::normalize(a)) && src_dict.contains(&Dictionary::normalize(b)) {
                return None;
            }
            let conv = layout::convert(&head, src);
            let (ca, cb) = conv.split_once('-')?;
            let letters = |w: &str| w.chars().all(char::is_alphabetic) && layout::detect_script(w) == Some(dst);
            if !letters(ca) || !letters(cb) || !natural_case(&conv.chars().filter(|c| c.is_alphabetic()).collect::<Vec<_>>()) {
                continue;
            }
            // Into English, one half needs 4+ letters: short Russian reduplications
            // (`гы-гы`, `цу-цу`, `туц-туц`) read as `us-us`, `we-we`, `new-new`.
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

    fn split_decision(token: &str, core: &str, src: Lang) -> Option<Decision> {
        if !core.chars().all(char::is_alphabetic) {
            return None;
        }
        let fixed = split::split_glued(dict::get(src), core, src)?;
        Some(Decision::Convert { text: token.replacen(core, &fixed, 1), target: src, reason: SPACE_REASON })
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
    fn shifted_sign_at_the_edge_is_not_password_casing() {
        // Shift+ж is `:` on the Latin layout: `nextcloud:` typed on the Russian one.
        assert_eq!(conv("тучесдщгвЖ").as_deref(), Some("nextcloud:"));
        assert_eq!(conv("ТучесдщгвЖ").as_deref(), Some("Nextcloud:"));
        // Э = `"`, Б = `<`, Ю = `>` at either end.
        assert_eq!(conv("руддщЭ").as_deref(), Some("hello\""));
        assert_eq!(conv("ЭруддщЭ").as_deref(), Some("\"hello\""));
        assert_eq!(conv("ЭРуддщ").as_deref(), Some("\"Hello"));
        assert_eq!(conv("руддщЮ").as_deref(), Some("hello>"));
        // Capitals in the middle are still password-like; so is odd casing next to the sign.
        for t in ["тучесДщгвЖ", "тУчесдщгвЖ", "ТУчесдщгвЖ", "пароЛьЖ", "ЖпАроль"] {
            assert_eq!(engine().decide(t), Decision::Keep("unnatural casing (password-like)"), "{t}");
        }
        // Only a conversion, never a typo fix, for such casing.
        let typos = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
        assert_eq!(typos.decide("ПаркуЖ"), Decision::Keep("unnatural casing (password-like)"));
        // Without the sign: unchanged rules.
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
        // A trailing sign that is a letter of a real form stays a letter.
        assert_eq!(conv("ghtcc-rjyathtywbz[").as_deref(), Some("пресс-конференциях"));
        // Real hyphenated words in their own layout, and short reduplications, stay.
        for w in ["well-known", "up-to", "e-mail", "x-ray", "кто-то", "все-таки", "гы-гы", "цу-цу", "туц-туц", "rnj-nj-nj"] {
            assert_eq!(conv(w), None, "{w}");
        }
    }

    #[test]
    fn wrong_layout_with_a_missing_space() {
        // `yflj,s` is `надобы` (the comma is the б key): converted and split.
        assert_eq!(engine().decide("yflj,s"), Decision::Convert { text: "надо бы".into(), target: Lang::Ru, reason: SPACE_REASON });
        assert_eq!(conv("Yflj,s").as_deref(), Some("Надо бы"));
        assert_eq!(conv("yflj,s!").as_deref(), Some("надо бы!"));
        assert_eq!(conv("dJabct").as_deref(), None); // odd casing stays
        assert_eq!(conv("djabct").as_deref(), Some("в офисе"));
        assert_eq!(conv("yflj").as_deref(), Some("надо"));
        // Words that end in those particles convert whole.
        assert_eq!(conv("xnj,s").as_deref(), Some("чтобы"));
        assert_eq!(conv("nfr;t").as_deref(), Some("также"));
        // A product name typed on the Russian layout.
        assert_eq!(conv("чцшлш").as_deref(), Some("xwiki"));
        // Letter keys that are signs on the Latin layout: `б` = `,`, `ж` = `;`, `э` = `'`.
        assert_eq!(conv(",skjkb").as_deref(), Some("было ли"));
        assert_eq!(conv("'njn;t").as_deref(), Some("этот же"));
    }

    #[test]
    fn layout_switched_mid_word_0_3_69() {
        // The letters of one script re-read in the other layout give a known word.
        assert_eq!(conv("приdtn").as_deref(), Some("привет"));
        assert_eq!(conv("ghbвет").as_deref(), Some("привет"));
        assert_eq!(conv("Приdtn").as_deref(), Some("Привет"));
        assert_eq!(conv("спаcb,j").as_deref(), Some("спасибо"));
        assert_eq!(conv("гkfpf[").as_deref(), Some("глазах"));
        assert_eq!(conv("приdtn.").as_deref(), Some("привет."));
        assert_eq!(conv("wщrld").as_deref(), Some("world"));
        // One letter after the switch that cannot be a case ending (щ is the `o` key).
        assert_eq!(conv("prщ").as_deref(), Some("pro"));
        assert_eq!(conv("Prщ").as_deref(), Some("Pro"));
        assert_eq!(conv("hellщ").as_deref(), Some("hello"));
        assert_eq!(conv("неn").as_deref(), Some("нет"));
        assert_eq!(conv("toщ").as_deref(), Some("too"));
        // A two-letter word plus one odd letter is not re-read as a whole.
        for t in ["даg", "изm", "dты", "сif", "мby"] {
            assert_eq!(conv(t), None, "{t}");
        }
        assert_eq!(conv("ыщund").as_deref(), Some("sound"));
        // Plus an ordinary typo, when typo fixes are on.
        let typos = Engine::new(EngineOptions { fix_typos: true, ..Default::default() });
        let fix = |t: &str| match typos.decide(t) {
            Decision::Convert { text, .. } => Some(text),
            Decision::Keep(_) => None,
        };
        assert_eq!(fix("приdtnn").as_deref(), Some("привет"));
        assert_eq!(fix("wщrlds").as_deref(), Some("worlds"));
        assert_eq!(fix("beaгtfiul").as_deref(), Some("beautiful"));
        assert_eq!(conv("приdtnn"), None);
        // Mixed names and identifiers stay.
        for t in ["Gitом", "iPhoneа", "gitом", "PRы", "VPNом", "Wi-Fiем", "abcабв", "оотjbmь", "sheы", "youе", "Windowsе", "Heyом", "keyами", "gitа", "macу", "Appleй", "linuxе"] {
            assert_eq!(fix(t), None, "{t}");
        }
        // Through the key buffer, with the replacement in the word's own layout.
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

    #[test]
    fn brands_0_3_65() {
        let mut e = engine();
        e.options.fix_typos = true;
        // Typed in EN: kept (it used to become `realm`).
        assert!(type_str(&mut e, "realme ").is_empty());
        assert!(type_str(&mut e, "Realme ").is_empty());
        for w in ["realme", "tecno", "infinix", "oneplus"] {
            assert_eq!(e.decide(w), Decision::Keep("known word form in current layout"), "{w}");
        }
        // Typed on the RU layout: converted to the brand.
        assert_eq!(conv("куфдьу").as_deref(), Some("realme"));
        assert_eq!(conv("шташтшч").as_deref(), Some("infinix"));
        // Russian spellings stay Russian.
        for w in ["реалми", "сяоми", "айфоном", "самсунга", "хуавея"] {
            assert_eq!(conv(w), None, "{w}");
            assert_eq!(e.decide(w), Decision::Keep("known word form in current layout"), "{w}");
        }
        // And still convert when typed on the EN layout.
        assert_eq!(conv("fqajyjv").as_deref(), Some("айфоном"));
        // Earlier user cases are unchanged.
        assert_eq!(conv("зуфсу").as_deref(), Some("peace"));
        assert_eq!(conv("мфгдеюишт").as_deref(), Some("vault.bin"));
        assert_eq!(typo::suggest("скидывать", Lang::Ru), None);
        assert_eq!(typo::suggest("оцени", Lang::Ru), None);
    }

    #[test]
    fn missing_space_after_function_word() {
        let mut e = engine();
        assert_eq!(type_str(&mut e, "снейросеткой "), vec![Replacement { delete: 13, insert: "с нейросеткой ".into(), target: Lang::Ru }]);
        assert_eq!(type_str(&mut e, "Вофисе, ")[0].insert, "В офисе, ");
        assert_eq!(type_str(&mut e, "незнаю ")[0].insert, "не знаю ");
        assert_eq!(type_str(&mut e, "кврачу ")[0].insert, "к врачу ");
        // Real words and wrong-layout words are not touched.
        assert!(type_str(&mut e, "сделать вопрос сколько ").is_empty());
        assert_eq!(type_str(&mut e, "ghbdtn ")[0].insert, "привет ");
        // Undo restores and remembers.
        assert_eq!(type_str(&mut e, "нафайле ")[0].insert, "на файле ");
        assert_eq!(bs(&mut e), Some(Replacement { delete: 9, insert: "нафайле ".into(), target: Lang::Ru }));
        assert!(type_str(&mut e, "дальше ").is_empty());
        assert_eq!(e.take_learned(), vec!["нафайле".to_string()]);
        assert!(type_str(&mut e, "нафайле ").is_empty());
        let mut e = engine();
        assert_eq!(type_str(&mut e, "вофисе ")[0].insert, "в офисе ");
        assert_eq!(e.undo_last(Lang::Ru).unwrap(), Replacement { delete: 8, insert: "вофисе ".into(), target: Lang::Ru });
        assert_eq!(e.take_learned(), vec!["вофисе".to_string()]);
        // With typo fixing on: a missing letter beats a split, a wrong one does not.
        let mut e = engine();
        e.options.fix_typos = true;
        assert_eq!(type_str(&mut e, "отсуда ")[0].insert, "отсюда ");
        assert_eq!(type_str(&mut e, "котрое ")[0].insert, "которое ");
        assert_eq!(type_str(&mut e, "наработе ")[0].insert, "на работе ");
        assert_eq!(type_str(&mut e, "отебе ")[0].insert, "о тебе ");
        assert_eq!(type_str(&mut e, "чтокой ")[0].insert, "точкой ");
        // Setting off, or autocorrect off (exception apps): nothing happens.
        let mut e = engine();
        e.options.fix_missing_spaces = false;
        assert!(type_str(&mut e, "вофисе незнаю ").is_empty());
        let mut e = engine();
        e.options.autocorrect = false;
        assert!(type_str(&mut e, "вофисе ").is_empty());
        // User words win.
        let mut e = engine_with(&["вофисе"]);
        assert!(type_str(&mut e, "вофисе ").is_empty());
    }
}
