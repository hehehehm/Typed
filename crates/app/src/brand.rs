//! User-facing product name. Since 0.3.68 the Windows binary is `typed.exe` (was
//! `layout-keeper.exe`; the installer removes the old one). Config and vault live in the `typed` folder.

pub const APP_NAME: &str = "Typed";
/// Window title (and settings footer): `Typed (ver. 0.3.67)`. The Windows adapter finds
/// the settings window by this exact title.
pub const WINDOW_TITLE: &str = concat!("Typed (ver. ", env!("CARGO_PKG_VERSION"), ")");
