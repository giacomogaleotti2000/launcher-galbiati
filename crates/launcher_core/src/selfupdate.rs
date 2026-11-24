//! Il launcher si installa e si aggiorna da solo, sempre nella cartella dell'utente: nessuna
//! password di amministratore, ne' al primo avvio ne' agli aggiornamenti.
//!
//! - Installazione per utente (Windows): l'eseguibile scaricato (es. in Download) si copia in
//!   `%LOCALAPPDATA%\cnc-launcher\launcher\cnc-launcher.exe` e crea i collegamenti nel menu Start e
//!   sul Desktop dell'utente. Niente `Program Files`, niente UAC.
//! - Aggiornamento: legge l'ultima Release PUBBLICA di `launcher-galbiati` (nessun token), verifica
//!   l'impronta SHA-256 del file `.sha256` pubblicato accanto all'archivio e sostituisce
//!   l'eseguibile in uso. Su Windows un eseguibile in esecuzione non si sovrascrive ma si puo'
//!   rinominare: diventa `cnc-launcher.old.exe` e viene cancellato al successivo avvio.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::install::{extract_archive, sha256_hex};
use crate::version::Version;
use crate::{LauncherError, Result, current_platform};

/// Repository pubblico del launcher.
pub const LAUNCHER_REPO: &str = "giacomogaleotti2000/launcher-galbiati";
const TAG_PREFIX: &str = "launcher-v";
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;

/// Versione di questo launcher: quella del tag `launcher-vX.Y.Z` (impostata dal workflow di
/// rilascio con `CNC_LAUNCHER_VERSION`), altrimenti quella del Cargo.toml.
pub fn current_version() -> &'static str {
  option_env!("CNC_LAUNCHER_VERSION").filter(|v| !v.is_empty()).unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Nome dell'eseguibile del launcher su questa piattaforma.
pub fn exe_name() -> &'static str {
  if cfg!(windows) { "cnc-launcher.exe" } else { "cnc-launcher" }
}

fn old_name() -> &'static str {
  if cfg!(windows) { "cnc-launcher.old.exe" } else { "cnc-launcher.old" }
}

fn io_err(what: &str, e: impl std::fmt::Display) -> LauncherError {
  LauncherError::new(format!("{what}: {e}"))
}

/// Dove sta il launcher installato per l'utente (`<cartella dati>/launcher/cnc-launcher[.exe]`).
pub fn home_exe(root: &Path) -> PathBuf {
  root.join("launcher").join(exe_name())
}

fn same_file(a: &Path, b: &Path) -> bool {
  match (a.canonicalize(), b.canonicalize()) {
    (Ok(x), Ok(y)) => x == y,
    _ => false,
  }
}

/// Il launcher in esecuzione e' gia' quello installato nella cartella dell'utente?
pub fn running_from_home(root: &Path) -> bool {
  std::env::current_exe().is_ok_and(|exe| same_file(&exe, &home_exe(root)))
}

/// Serve proporre "Installa per questo utente"? Solo su Windows (su macOS il launcher resta dove
/// l'utente lo ha messo) e solo se non si sta gia' usando la copia installata.
pub fn should_offer_install(root: &Path) -> bool {
  cfg!(windows) && !running_from_home(root)
}

/// Cancella gli avanzi di un aggiornamento precedente (eseguibile vecchio, cartella temporanea).
pub fn cleanup_after_update() {
  if let Ok(exe) = std::env::current_exe()
    && let Some(dir) = exe.parent()
  {
    let _ = std::fs::remove_file(dir.join(old_name()));
    let _ = std::fs::remove_dir_all(dir.join(".aggiornamento"));
  }
}

/// Copia il launcher in esecuzione nella cartella dell'utente e crea i collegamenti.
/// Restituisce il percorso del launcher installato (da avviare al posto di questo).
pub fn install_for_user(root: &Path) -> Result<PathBuf> {
  let exe = std::env::current_exe().map_err(|e| io_err("percorso del launcher", e))?;
  let target = home_exe(root);
  if !same_file(&exe, &target) {
    let dir = target.parent().ok_or_else(|| LauncherError::new("cartella del launcher non valida"))?;
    std::fs::create_dir_all(dir).map_err(|e| io_err("cartella del launcher", e))?;
    if target.exists() {
      // una copia installata in uso (aperta in un'altra finestra): la si rinomina
      let old = dir.join(old_name());
      let _ = std::fs::remove_file(&old);
      std::fs::rename(&target, &old).map_err(|e| io_err("sostituzione del launcher installato", e))?;
    }
    std::fs::copy(&exe, &target).map_err(|e| io_err("copia del launcher", e))?;
  }
  create_shortcuts(&target)?;
  Ok(target)
}

/// Collegamenti "CNC Launcher" nel menu Start e sul Desktop dell'utente (Windows).
#[cfg(windows)]
fn create_shortcuts(target: &Path) -> Result<()> {
  use std::os::windows::process::CommandExt;
  const CREATE_NO_WINDOW: u32 = 0x0800_0000;
  // i percorsi passano da variabili d'ambiente: niente problemi di virgolette
  let script = "$w = New-Object -ComObject WScript.Shell; \
    foreach ($d in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) { \
      if ($d) { $s = $w.CreateShortcut((Join-Path $d 'CNC Launcher.lnk')); \
        $s.TargetPath = $env:CNC_TARGET; $s.WorkingDirectory = $env:CNC_DIR; \
        $s.Description = 'CNC LAUNCHER: app Galbiati'; $s.Save() } }";
  let dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
  let status = std::process::Command::new("powershell.exe")
    .args(["-NoProfile", "-NonInteractive", "-Command", script])
    .env("CNC_TARGET", target)
    .env("CNC_DIR", dir)
    .creation_flags(CREATE_NO_WINDOW)
    .status()
    .map_err(|e| io_err("creazione dei collegamenti", e))?;
  if status.success() {
    Ok(())
  } else {
    Err(LauncherError::new("launcher copiato, ma i collegamenti nel menu Start e sul Desktop non sono stati creati"))
  }
}

#[cfg(not(windows))]
fn create_shortcuts(_target: &Path) -> Result<()> {
  Ok(())
}

/// Avvia un altro eseguibile del launcher (dopo installazione o aggiornamento).
pub fn relaunch(exe: &Path) -> Result<()> {
  let mut cmd = std::process::Command::new(exe);
  if let Some(dir) = exe.parent() {
    cmd.current_dir(dir);
  }
  cmd.spawn().map(|_| ()).map_err(|e| io_err("riavvio del launcher", e))
}

// --------------------------------------------------------------------------
// Aggiornamento dalla Release pubblica
// --------------------------------------------------------------------------

/// Un aggiornamento del launcher disponibile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LauncherRelease {
  pub version: Version,
  pub archive_name: String,
  pub archive_url: String,
  pub sha256_url: String,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
  name: String,
  browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
  tag_name: String,
  assets: Vec<GhAsset>,
}

/// Lettore delle Release pubbliche del launcher (nessun token).
#[derive(Debug, Clone)]
pub struct LauncherUpdater {
  api: String,
  repo: String,
}

impl Default for LauncherUpdater {
  fn default() -> Self {
    Self::with_api("https://api.github.com", LAUNCHER_REPO)
  }
}

impl LauncherUpdater {
  /// Con un indirizzo dell'API diverso (test).
  pub fn with_api(api: &str, repo: &str) -> Self {
    Self { api: api.trim_end_matches('/').to_owned(), repo: repo.to_owned() }
  }

  fn get(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url)
      .header("Accept", "application/vnd.github+json")
      .header("User-Agent", "cnc-launcher")
      .call()
      .map_err(|e| match e {
        ureq::Error::StatusCode(404) => LauncherError::new("nessuna Release del launcher pubblicata"),
        ureq::Error::StatusCode(n) => LauncherError::new(format!("risposta inattesa di GitHub ({n})")),
        other => LauncherError::new(format!("rete non raggiungibile: {other}")),
      })?;
    let mut out = Vec::new();
    resp
      .into_body()
      .into_reader()
      .take(MAX_DOWNLOAD)
      .read_to_end(&mut out)
      .map_err(|e| LauncherError::new(format!("scaricamento interrotto: {e}")))?;
    Ok(out)
  }

  /// Ultima versione pubblicata, se piu' recente di `current`.
  pub fn check(&self, current: &str) -> Result<Option<LauncherRelease>> {
    let body = Self::get(&format!("{}/repos/{}/releases/latest", self.api, self.repo))?;
    let rel: GhRelease =
      serde_json::from_slice(&body).map_err(|e| LauncherError::new(format!("risposta di GitHub non valida: {e}")))?;
    let Some(version) = rel.tag_name.strip_prefix(TAG_PREFIX).and_then(Version::parse) else {
      return Ok(None);
    };
    if Version::parse(current).is_some_and(|c| c >= version) {
      return Ok(None);
    }
    let platform = current_platform();
    let ext = if platform.starts_with("windows") { "zip" } else { "tar.gz" };
    let archive_name = format!("cnc-launcher-{}-{platform}.{ext}", version.text);
    let url = |name: &str| rel.assets.iter().find(|a| a.name == name).map(|a| a.browser_download_url.clone());
    let (Some(archive_url), Some(sha256_url)) = (url(&archive_name), url(&format!("{archive_name}.sha256"))) else {
      return Err(LauncherError::new(format!(
        "la Release {} non contiene {archive_name} e la sua impronta",
        rel.tag_name
      )));
    };
    Ok(Some(LauncherRelease { version, archive_name, archive_url, sha256_url }))
  }

  /// Scarica, verifica e mette al posto del launcher in esecuzione (`exe`). Restituisce il
  /// percorso del nuovo eseguibile da avviare.
  pub fn apply(&self, rel: &LauncherRelease, exe: &Path) -> Result<PathBuf> {
    let archive = Self::get(&rel.archive_url)?;
    let sha_file = Self::get(&rel.sha256_url)?;
    let expected = String::from_utf8_lossy(&sha_file).split_whitespace().next().unwrap_or_default().to_owned();
    replace_exe(&archive, &expected, &rel.archive_name, exe)?;
    Ok(exe.to_path_buf())
  }
}

/// Verifica l'impronta, estrae l'eseguibile dall'archivio e sostituisce `exe`.
pub fn replace_exe(archive: &[u8], expected_sha256: &str, archive_name: &str, exe: &Path) -> Result<()> {
  let got = sha256_hex(archive);
  if expected_sha256.is_empty() || !got.eq_ignore_ascii_case(expected_sha256.trim()) {
    return Err(LauncherError::new(format!(
      "impronta SHA-256 diversa da quella pubblicata: aggiornamento annullato (atteso {expected_sha256}, ottenuto {got})"
    )));
  }
  let dir = exe.parent().ok_or_else(|| LauncherError::new("cartella del launcher non valida"))?;
  let staging = dir.join(".aggiornamento");
  let _ = std::fs::remove_dir_all(&staging);
  std::fs::create_dir_all(&staging).map_err(|e| protected(dir, e))?;
  let result = (|| {
    extract_archive(archive_name, archive, &staging)?;
    let new_exe = find_file(&staging, exe_name())
      .ok_or_else(|| LauncherError::new(format!("{} non trovato nell'archivio", exe_name())))?;
    let old = dir.join(old_name());
    let _ = std::fs::remove_file(&old);
    if exe.exists() {
      std::fs::rename(exe, &old).map_err(|e| protected(dir, e))?;
    }
    if let Err(e) = std::fs::rename(&new_exe, exe).or_else(|_| std::fs::copy(&new_exe, exe).map(|_| ())) {
      // si rimette a posto il vecchio
      let _ = std::fs::rename(&old, exe);
      return Err(protected(dir, e));
    }
    make_executable(exe)
  })();
  let _ = std::fs::remove_dir_all(&staging);
  result
}

fn protected(dir: &Path, e: impl std::fmt::Display) -> LauncherError {
  LauncherError::new(format!(
    "non posso scrivere in {} ({e}). Se il launcher e' in una cartella protetta (es. Programmi), \
     usa \"Installa per questo utente\": da li' gli aggiornamenti non chiedono la password.",
    dir.display()
  ))
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
  for e in std::fs::read_dir(dir).ok()?.flatten() {
    let p = e.path();
    if p.is_dir() {
      if let Some(f) = find_file(&p, name) {
        return Some(f);
      }
    } else if p.file_name().is_some_and(|n| n == name) {
      return Some(p);
    }
  }
  None
}

#[cfg(unix)]
fn make_executable(p: &Path) -> Result<()> {
  use std::os::unix::fs::PermissionsExt;
  let mut perm = std::fs::metadata(p).map_err(|e| io_err("permessi", e))?.permissions();
  perm.set_mode(perm.mode() | 0o755);
  std::fs::set_permissions(p, perm).map_err(|e| io_err("permessi", e))
}

#[cfg(not(unix))]
fn make_executable(_p: &Path) -> Result<()> {
  Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
  use super::*;

  fn tar_gz(path: &str, data: &[u8]) -> Vec<u8> {
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
    let mut h = tar::Header::new_gnu();
    h.set_size(data.len() as u64);
    h.set_mode(0o755);
    h.set_cksum();
    tar.append_data(&mut h, path, data).unwrap();
    tar.into_inner().unwrap().finish().unwrap()
  }

  fn temp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("launcher_self_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
  }

  #[test]
  fn replaces_exe_and_keeps_old_copy() {
    let dir = temp("ok");
    let exe = dir.join(exe_name());
    std::fs::write(&exe, b"vecchio").unwrap();
    let archive = tar_gz(&format!("cnc-launcher-9.9.9-x/{}", exe_name()), b"nuovo");
    replace_exe(&archive, &sha256_hex(&archive), "a.tar.gz", &exe).unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), b"nuovo");
    assert_eq!(std::fs::read(dir.join(old_name())).unwrap(), b"vecchio");
    assert!(!dir.join(".aggiornamento").exists());
  }

  #[test]
  fn wrong_hash_leaves_exe_untouched() {
    let dir = temp("hash");
    let exe = dir.join(exe_name());
    std::fs::write(&exe, b"vecchio").unwrap();
    let archive = tar_gz(&format!("x/{}", exe_name()), b"nuovo");
    let err = replace_exe(&archive, "deadbeef", "a.tar.gz", &exe).unwrap_err();
    assert!(err.message.contains("SHA-256"));
    assert_eq!(std::fs::read(&exe).unwrap(), b"vecchio");
  }

  #[test]
  fn missing_exe_in_archive_leaves_exe_untouched() {
    let dir = temp("missing");
    let exe = dir.join(exe_name());
    std::fs::write(&exe, b"vecchio").unwrap();
    let archive = tar_gz("x/altro", b"nuovo");
    assert!(replace_exe(&archive, &sha256_hex(&archive), "a.tar.gz", &exe).is_err());
    assert_eq!(std::fs::read(&exe).unwrap(), b"vecchio");
  }

  #[test]
  fn version_is_set() {
    assert!(Version::parse(current_version()).is_some());
  }
}
