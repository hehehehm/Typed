//! User-facing product name. The Windows binary stays `layout-keeper.exe` so existing
//! installs keep upgrading. Config and vault live in the `typed` folder.

pub const APP_NAME: &str = "Typed";
/// Window title (and settings footer): `Typed (ver. 0.3.67)`. The Windows adapter finds
/// the settings window by this exact title.
pub const WINDOW_TITLE: &str = concat!("Typed (ver. ", env!("CARGO_PKG_VERSION"), ")");
