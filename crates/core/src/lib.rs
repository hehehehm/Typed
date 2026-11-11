//! # layout_keeper_core
//!
//! Platform-independent brain of LayoutKeeper:
//!
//! * [`layout`] — physical-key maps between US QWERTY and Russian ЙЦУКЕН;
//! * [`buffer`] — in-memory buffer of the word currently being typed;
//! * [`dict`] — embedded English/Russian frequency word lists and bigram stats;
//! * [`engine`] — decides whether a finished word was typed in the wrong layout;
//! * [`split`] — inserts a missing space after a glued Russian preposition (`вофисе`);
//! * [`config`] — local JSON config (per-app layouts, exceptions, hotkey).
//!
//! The crate never performs network I/O and never persists typed text.

pub mod buffer;
pub mod config;
pub mod dict;
pub mod engine;
pub mod ffi;
pub mod filename;
pub mod layout;
pub mod morph;
pub mod split;
pub mod typo;
pub mod userdict;
pub mod vault;
pub use dict::get as dictionary;

pub use config::{Config, Hotkey, HotkeyCode, LogoStyle};
pub use engine::{Decision, Engine, EngineOptions, Key, Replacement};
pub use layout::Lang;
pub use userdict::UserWords;
