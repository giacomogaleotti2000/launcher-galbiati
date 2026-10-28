//! Token di accesso: nel portachiavi del sistema (Gestione credenziali di Windows, Portachiavi di
//! macOS, Secret Service su Linux). Mai scritto su disco in chiaro, mai stampato.

use std::fmt;
use std::sync::Mutex;

use crate::{LauncherError, Result};

const SERVICE: &str = "cnc-launcher";
const ACCOUNT: &str = "github-token";

/// Segreto che non si stampa per sbaglio (`Debug`/`Display` mostrano solo asterischi).
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
  pub fn new(value: impl Into<String>) -> Self {
    Self(value.into().trim().to_owned())
  }

  /// Il valore, solo per l'intestazione HTTP.
  pub fn expose(&self) -> &str {
    &self.0
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

impl fmt::Debug for Secret {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Secret(***)")
  }
}

/// Dove si conserva il token.
pub trait TokenStore: Send + Sync {
  fn load(&self) -> Result<Option<Secret>>;
  fn save(&self, token: &Secret) -> Result<()>;
  fn delete(&self) -> Result<()>;
  /// Descrizione per l'utente (es. "Gestione credenziali di Windows").
  fn describe(&self) -> &'static str;
}

/// Portachiavi del sistema operativo.
#[derive(Debug, Default)]
pub struct KeyringStore;

fn entry() -> Result<keyring::Entry> {
  keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| LauncherError::new(format!("portachiavi non disponibile: {e}")))
}

impl TokenStore for KeyringStore {
  fn load(&self) -> Result<Option<Secret>> {
    match entry()?.get_password() {
      Ok(p) => Ok(Some(Secret::new(p))),
      Err(keyring::Error::NoEntry) => Ok(None),
      Err(e) => Err(LauncherError::new(format!("lettura dal portachiavi: {e}"))),
    }
  }

  fn save(&self, token: &Secret) -> Result<()> {
    entry()?.set_password(token.expose()).map_err(|e| LauncherError::new(format!("salvataggio nel portachiavi: {e}")))
  }

  fn delete(&self) -> Result<()> {
    match entry()?.delete_credential() {
      Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
      Err(e) => Err(LauncherError::new(format!("rimozione dal portachiavi: {e}"))),
    }
  }

  fn describe(&self) -> &'static str {
    match std::env::consts::OS {
      "windows" => "Gestione credenziali di Windows",
      "macos" => "Portachiavi di macOS",
      _ => "Secret Service del sistema",
    }
  }
}

/// Token solo in memoria (test, o portachiavi non disponibile: si richiede a ogni avvio).
#[derive(Debug, Default)]
pub struct MemoryStore(Mutex<Option<Secret>>);

impl TokenStore for MemoryStore {
  fn load(&self) -> Result<Option<Secret>> {
    Ok(self.0.lock().map_err(|_| LauncherError::new("stato interno non valido"))?.clone())
  }

  fn save(&self, token: &Secret) -> Result<()> {
    *self.0.lock().map_err(|_| LauncherError::new("stato interno non valido"))? = Some(token.clone());
    Ok(())
  }

  fn delete(&self) -> Result<()> {
    *self.0.lock().map_err(|_| LauncherError::new("stato interno non valido"))? = None;
    Ok(())
  }

  fn describe(&self) -> &'static str {
    "solo in memoria (va reinserito a ogni avvio)"
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn secret_is_never_printed() {
    let s = Secret::new("github_pat_ABCDEF");
    assert_eq!(format!("{s:?}"), "Secret(***)");
    assert_eq!(s.expose(), "github_pat_ABCDEF");
  }

  #[test]
  fn memory_store_round_trip() {
    let m = MemoryStore::default();
    assert!(m.load().unwrap().is_none());
    m.save(&Secret::new(" tok ")).unwrap();
    assert_eq!(m.load().unwrap().unwrap().expose(), "tok");
    m.delete().unwrap();
    assert!(m.load().unwrap().is_none());
  }
}
