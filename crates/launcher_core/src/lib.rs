//! Launcher privato: logica senza interfaccia.
//!
//! - [`catalog`]: le app che il launcher sa installare (per ora HEIDENHAIN-SIEMENS CONVERTER);
//! - [`source`]: da dove arrivano i rilasci. Trait [`source::ReleaseSource`] con l'implementazione
//!   GitHub (Release di un repository privato, token di sola lettura); una sorgente "cartella di
//!   rete" (`\\server\cnc\...`) si aggiunge implementando lo stesso trait;
//! - [`manifest`]: il `manifest.json` di ogni rilascio (piattaforme, file, SHA-256);
//! - [`install`]: verifica SHA-256, estrazione sicura, installazione per utente, avvio;
//! - [`secret`]: il token nel portachiavi del sistema (mai su disco in chiaro, mai nei log).

pub mod catalog;
pub mod install;
pub mod manifest;
pub mod secret;
pub mod source;
pub mod version;

use std::fmt;

/// Errore del launcher, con un messaggio per l'utente in italiano.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LauncherError {
  pub message: String,
}

impl LauncherError {
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: message.into() }
  }
}

impl fmt::Display for LauncherError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

impl std::error::Error for LauncherError {}

pub type Result<T> = std::result::Result<T, LauncherError>;

/// Piattaforma corrente nel formato dei manifest (`windows-x64`, `macos-arm64`, ...).
pub fn current_platform() -> &'static str {
  match (std::env::consts::OS, std::env::consts::ARCH) {
    ("windows", "x86_64") => "windows-x64",
    ("windows", "aarch64") => "windows-arm64",
    ("macos", "aarch64") => "macos-arm64",
    ("macos", "x86_64") => "macos-x64",
    ("linux", "aarch64") => "linux-arm64",
    _ => "linux-x64",
  }
}
