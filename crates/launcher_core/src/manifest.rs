//! `manifest.json` di un rilascio (scritto da `app/scripts/manifest.py` nel workflow).

use serde::{Deserialize, Serialize};

use crate::{LauncherError, Result};

/// Un archivio per una piattaforma.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestAsset {
  pub platform: String,
  pub file: String,
  pub sha256: String,
  pub size: u64,
  /// Percorso dell'eseguibile dentro l'archivio.
  pub executable: String,
}

/// Manifest di un rilascio.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
  pub schema: u32,
  pub app_id: String,
  pub name: String,
  pub version: String,
  pub assets: Vec<ManifestAsset>,
}

impl Manifest {
  pub fn parse(bytes: &[u8]) -> Result<Self> {
    let m: Self = serde_json::from_slice(bytes).map_err(|e| LauncherError::new(format!("manifest non valido: {e}")))?;
    if m.schema != 1 {
      return Err(LauncherError::new(format!("versione del manifest non supportata: {}", m.schema)));
    }
    Ok(m)
  }

  /// Archivio per la piattaforma indicata.
  pub fn asset_for(&self, platform: &str) -> Option<&ManifestAsset> {
    self.assets.iter().find(|a| a.platform == platform)
  }
}
