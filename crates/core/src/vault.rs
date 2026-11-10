//! Local encrypted vault for logins and payment cards.
//!
//! File format (`vault.bin`, next to `config.json`):
//! `b"LKV1" | m_cost u32 LE | t_cost u32 LE | p_cost u32 LE | salt[16] | nonce[24] | ciphertext+tag`
//!
//! * key = Argon2id(master password, salt) → 32 bytes;
//! * AEAD = XChaCha20-Poly1305, the header is authenticated as associated data;
//! * plaintext = JSON of [`VaultData`]; it only ever exists in memory and is zeroized after use.
//!
//! Nothing here touches the network or logs secrets. Entries only come from explicit user
//! input in the GUI — the keyboard hook never writes here.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const MAGIC: &[u8; 4] = b"LKV1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const HEADER_LEN: usize = 4 + 12 + SALT_LEN + NONCE_LEN;

#[derive(Clone, Default, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Login {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub notes: String,
    /// Case-insensitive substring of the focused app id (Windows: lowercase exe path) or URL.
    #[serde(default)]
    pub app_match: String,
}

#[derive(Clone, Default, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Card {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub number: String,
    /// MM/YY
    #[serde(default)]
    pub expiry: String,
    #[serde(default)]
    pub holder: String,
    /// Optional, extra-sensitive.
    #[serde(default)]
    pub cvc: String,
}

impl Card {
    /// "•••• 1234" — the only form of the number ever shown in lists.
    pub fn masked(&self) -> String {
        let digits: Vec<char> = self.number.chars().filter(|c| c.is_ascii_digit()).collect();
        let n = digits.len();
        let tail: String = digits[n.saturating_sub(4)..].iter().collect();
        format!("•••• {tail}")
    }
}

impl Login {
    pub fn matches_app(&self, app_id: &str) -> bool {
        let m = self.app_match.trim().to_lowercase();
        !m.is_empty() && app_id.to_lowercase().contains(&m)
    }
}

#[derive(Clone, Default, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct VaultData {
    #[serde(default)]
    pub logins: Vec<Login>,
    #[serde(default)]
    pub cards: Vec<Card>,
}

// Never print secrets, even by accident through `{:?}`.
impl fmt::Debug for VaultData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VaultData {{ {} logins, {} cards }}", self.logins.len(), self.cards.len())
    }
}

#[derive(Debug)]
pub enum VaultError {
    /// Wrong master password or tampered/corrupted file (AEAD can't tell them apart).
    BadPasswordOrCorrupt,
    Format(&'static str),
    Io(io::Error),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VaultError::BadPasswordOrCorrupt => write!(f, "неверный мастер-пароль или файл повреждён"),
            VaultError::Format(m) => write!(f, "неверный формат сейфа: {m}"),
            VaultError::Io(e) => write!(f, "ошибка файла: {e}"),
        }
    }
}

impl From<io::Error> for VaultError {
    fn from(e: io::Error) -> Self {
        VaultError::Io(e)
    }
}

#[derive(Clone, Copy)]
struct KdfParams {
    m_kib: u32,
    t: u32,
    p: u32,
}

/// 64 MiB, 3 passes — OWASP-level Argon2id. Tests use cheap params.
const DEFAULT_KDF: KdfParams = KdfParams { m_kib: 64 * 1024, t: 3, p: 1 };

fn derive_key(password: &str, salt: &[u8], k: KdfParams) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let params = Params::new(k.m_kib, k.t, k.p, Some(32)).map_err(|_| VaultError::Format("kdf params"))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|_| VaultError::Format("kdf"))?;
    Ok(key)
}

/// `<config dir>/vault.bin` (same folder as config.json).
pub fn default_path() -> PathBuf {
    let cfg = crate::Config::default_path();
    cfg.parent().map(|d| d.join("vault.bin")).unwrap_or_else(|| PathBuf::from("vault.bin"))
}

/// An unlocked vault. Dropping it zeroizes the key and all entries.
pub struct Vault {
    key: Zeroizing<[u8; 32]>,
    salt: [u8; SALT_LEN],
    kdf: KdfParams,
    pub data: VaultData,
}

impl Vault {
    /// New empty vault for `password` (not yet written).
    pub fn create(password: &str) -> Result<Vault, VaultError> {
        Self::create_with(password, DEFAULT_KDF)
    }

    fn create_with(password: &str, kdf: KdfParams) -> Result<Vault, VaultError> {
        let mut salt = [0u8; SALT_LEN];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        let key = derive_key(password, &salt, kdf)?;
        Ok(Vault { key, salt, kdf, data: VaultData::default() })
    }

    pub fn encrypt(&self) -> Result<Vec<u8>, VaultError> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let mut header = Vec::with_capacity(HEADER_LEN);
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&self.kdf.m_kib.to_le_bytes());
        header.extend_from_slice(&self.kdf.t.to_le_bytes());
        header.extend_from_slice(&self.kdf.p.to_le_bytes());
        header.extend_from_slice(&self.salt);
        header.extend_from_slice(&nonce);
        let plain = Zeroizing::new(serde_json::to_vec(&self.data).map_err(|_| VaultError::Format("serialize"))?);
        let cipher = XChaCha20Poly1305::new(self.key.as_ref().into());
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: &plain, aad: &header })
            .map_err(|_| VaultError::Format("encrypt"))?;
        header.extend_from_slice(&ct);
        Ok(header)
    }

    pub fn decrypt(bytes: &[u8], password: &str) -> Result<Vault, VaultError> {
        if bytes.len() < HEADER_LEN + 16 || &bytes[..4] != MAGIC {
            return Err(VaultError::Format("header"));
        }
        let u = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        let kdf = KdfParams { m_kib: u(4), t: u(8), p: u(12) };
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&bytes[16..16 + SALT_LEN]);
        let nonce = &bytes[16 + SALT_LEN..HEADER_LEN];
        let key = derive_key(password, &salt, kdf)?;
        let cipher = XChaCha20Poly1305::new(key.as_ref().into());
        let plain = Zeroizing::new(
            cipher
                .decrypt(XNonce::from_slice(nonce), Payload { msg: &bytes[HEADER_LEN..], aad: &bytes[..HEADER_LEN] })
                .map_err(|_| VaultError::BadPasswordOrCorrupt)?,
        );
        let data: VaultData = serde_json::from_slice(&plain).map_err(|_| VaultError::Format("payload"))?;
        Ok(Vault { key, salt, kdf, data })
    }

    pub fn open(path: &Path, password: &str) -> Result<Vault, VaultError> {
        Self::decrypt(&std::fs::read(path)?, password)
    }

    /// Encrypt and write atomically (tmp file + rename). Only ciphertext touches disk.
    pub fn save(&self, path: &Path) -> Result<(), VaultError> {
        let bytes = self.encrypt()?;
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("bin.tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

impl fmt::Debug for Vault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vault({:?})", self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FAST: KdfParams = KdfParams { m_kib: 256, t: 1, p: 1 };

    fn sample() -> Vault {
        let mut v = Vault::create_with("master-test", FAST).unwrap();
        v.data.logins.push(Login {
            label: "test".into(),
            username: "user".into(),
            password: "secret123".into(),
            notes: String::new(),
            app_match: "notepad".into(),
        });
        v.data.cards.push(Card {
            label: "fake".into(),
            number: "0000 0000 0000 4321".into(),
            expiry: "01/30".into(),
            holder: "TEST USER".into(),
            cvc: String::new(),
        });
        v
    }

    #[test]
    fn roundtrip() {
        let bytes = sample().encrypt().unwrap();
        assert!(!bytes.windows(9).any(|w| w == b"secret123"), "plaintext leaked");
        let v = Vault::decrypt(&bytes, "master-test").unwrap();
        assert_eq!(v.data.logins[0].password, "secret123");
        assert_eq!(v.data.cards[0].masked(), "•••• 4321");
        assert!(v.data.logins[0].matches_app(r"c:\windows\system32\notepad.exe"));
    }

    #[test]
    fn wrong_password_fails() {
        let bytes = sample().encrypt().unwrap();
        assert!(matches!(Vault::decrypt(&bytes, "wrong"), Err(VaultError::BadPasswordOrCorrupt)));
    }

    #[test]
    fn tamper_fails() {
        let mut bytes = sample().encrypt().unwrap();
        let n = bytes.len();
        bytes[n - 1] ^= 1;
        assert!(Vault::decrypt(&bytes, "master-test").is_err());
        let mut b2 = sample().encrypt().unwrap();
        b2[20] ^= 1; // salt (authenticated header)
        assert!(Vault::decrypt(&b2, "master-test").is_err());
    }

    #[test]
    fn file_roundtrip() {
        let dir = std::env::temp_dir().join(format!("lk-vault-test-{}", std::process::id()));
        let p = dir.join("vault.bin");
        sample().save(&p).unwrap();
        assert_eq!(Vault::open(&p, "master-test").unwrap().data.logins.len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn debug_hides_secrets() {
        assert!(!format!("{:?}", sample()).contains("secret123"));
    }
}
