//! Installazione per utente: verifica SHA-256, estrazione sicura, stato delle app installate, avvio.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::catalog::AppSpec;
use crate::manifest::ManifestAsset;
use crate::{LauncherError, Result};

const STATE_FILE: &str = "installate.json";

/// Un'app installata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
  pub version: String,
  pub executable: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
  apps: BTreeMap<String, Installed>,
}

/// SHA-256 esadecimale minuscolo.
pub fn sha256_hex(bytes: &[u8]) -> String {
  Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn io_err(what: &str, e: impl std::fmt::Display) -> LauncherError {
  LauncherError::new(format!("{what}: {e}"))
}

/// Percorso relativo senza `..`, radici o unita' (protezione dagli archivi malevoli).
fn safe_relative(p: &Path) -> Option<PathBuf> {
  let mut out = PathBuf::new();
  for c in p.components() {
    match c {
      Component::Normal(x) => out.push(x),
      Component::CurDir => {}
      _ => return None,
    }
  }
  (!out.as_os_str().is_empty()).then_some(out)
}

/// Installatore in una cartella dell'utente.
#[derive(Clone, Debug)]
pub struct Installer {
  root: PathBuf,
}

impl Installer {
  /// Cartella dati dell'utente (es. `%LOCALAPPDATA%\cnc-launcher`, `~/Library/Application Support/cnc-launcher`).
  pub fn default_root() -> Result<PathBuf> {
    directories::ProjectDirs::from("", "", "cnc-launcher")
      .map(|d| d.data_local_dir().to_path_buf())
      .ok_or_else(|| LauncherError::new("cartella dell'utente non trovata"))
  }

  pub fn new(root: PathBuf) -> Self {
    Self { root }
  }

  pub fn root(&self) -> &Path {
    &self.root
  }

  fn state(&self) -> State {
    std::fs::read(self.root.join(STATE_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
  }

  fn save_state(&self, state: &State) -> Result<()> {
    std::fs::create_dir_all(&self.root).map_err(|e| io_err("cartella del launcher", e))?;
    let json = serde_json::to_vec_pretty(state).map_err(|e| io_err("stato", e))?;
    std::fs::write(self.root.join(STATE_FILE), json).map_err(|e| io_err("stato", e))
  }

  /// Versione installata di un'app (se l'eseguibile esiste ancora).
  pub fn installed(&self, app_id: &str) -> Option<Installed> {
    self.state().apps.get(app_id).filter(|i| i.executable.is_file()).cloned()
  }

  /// Verifica l'impronta, estrae e registra l'installazione.
  pub fn install(&self, app: &AppSpec, version: &str, asset: &ManifestAsset, archive: &[u8]) -> Result<Installed> {
    let got = sha256_hex(archive);
    if !got.eq_ignore_ascii_case(asset.sha256.trim()) {
      return Err(LauncherError::new(format!(
        "impronta SHA-256 diversa da quella del manifest: file alterato o scaricato male (atteso {}, ottenuto {got})",
        asset.sha256
      )));
    }
    let app_dir = self.root.join("app").join(&app.id);
    let dest = app_dir.join(version);
    if dest.exists() {
      std::fs::remove_dir_all(&dest).map_err(|e| io_err("pulizia della versione precedente", e))?;
    }
    std::fs::create_dir_all(&dest).map_err(|e| io_err("cartella di installazione", e))?;
    if asset.file.ends_with(".zip") {
      extract_zip(archive, &dest)?;
    } else if asset.file.ends_with(".tar.gz") || asset.file.ends_with(".tgz") {
      extract_tar_gz(archive, &dest)?;
    } else {
      return Err(LauncherError::new(format!("formato di archivio non gestito: {}", asset.file)));
    }
    let rel = safe_relative(Path::new(&asset.executable))
      .ok_or_else(|| LauncherError::new("percorso dell'eseguibile non valido nel manifest"))?;
    let executable = dest.join(rel);
    if !executable.is_file() {
      return Err(LauncherError::new(format!("eseguibile non trovato nell'archivio: {}", asset.executable)));
    }
    make_executable(&executable)?;
    let installed = Installed { version: version.to_owned(), executable };
    let mut state = self.state();
    state.apps.insert(app.id.clone(), installed.clone());
    self.save_state(&state)?;
    // le versioni precedenti non servono piu'
    if let Ok(entries) = std::fs::read_dir(&app_dir) {
      for e in entries.flatten() {
        if e.path() != dest {
          let _ = std::fs::remove_dir_all(e.path());
        }
      }
    }
    Ok(installed)
  }

  /// Avvia l'app installata.
  pub fn launch(&self, installed: &Installed) -> Result<()> {
    let mut cmd = std::process::Command::new(&installed.executable);
    if let Some(dir) = installed.executable.parent() {
      cmd.current_dir(dir);
    }
    cmd.spawn().map(|_| ()).map_err(|e| io_err("avvio dell'app", e))
  }
}

fn extract_zip(archive: &[u8], dest: &Path) -> Result<()> {
  let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(|e| io_err("archivio zip", e))?;
  for i in 0..zip.len() {
    let mut file = zip.by_index(i).map_err(|e| io_err("archivio zip", e))?;
    let Some(rel) = file.enclosed_name().and_then(|p| safe_relative(&p)) else {
      let name = file.name().map(|n| n.to_string()).unwrap_or_default();
      return Err(LauncherError::new(format!("percorso non sicuro nell'archivio: {name}")));
    };
    let out = dest.join(rel);
    if file.is_dir() {
      std::fs::create_dir_all(&out).map_err(|e| io_err("estrazione", e))?;
      continue;
    }
    if let Some(parent) = out.parent() {
      std::fs::create_dir_all(parent).map_err(|e| io_err("estrazione", e))?;
    }
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| io_err("estrazione", e))?;
    std::fs::write(&out, data).map_err(|e| io_err("estrazione", e))?;
  }
  Ok(())
}

fn extract_tar_gz(archive: &[u8], dest: &Path) -> Result<()> {
  let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(Cursor::new(archive)));
  for entry in tar.entries().map_err(|e| io_err("archivio tar.gz", e))? {
    let mut entry = entry.map_err(|e| io_err("archivio tar.gz", e))?;
    let path = entry.path().map_err(|e| io_err("archivio tar.gz", e))?.into_owned();
    if safe_relative(&path).is_none() {
      return Err(LauncherError::new(format!("percorso non sicuro nell'archivio: {}", path.display())));
    }
    let kind = entry.header().entry_type();
    if kind.is_symlink() || kind.is_hard_link() {
      return Err(LauncherError::new("collegamenti nell'archivio non ammessi"));
    }
    entry.unpack_in(dest).map_err(|e| io_err("estrazione", e))?;
  }
  Ok(())
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
mod tests {
  use super::*;

  #[test]
  fn rejects_unsafe_paths() {
    assert!(safe_relative(Path::new("../x")).is_none());
    assert!(safe_relative(Path::new("/etc/passwd")).is_none());
    assert_eq!(safe_relative(Path::new("a/./b")), Some(PathBuf::from("a/b")));
  }

  #[test]
  fn sha256_known_value() {
    assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
  }
}
