//! Sorgenti dei rilasci. Il launcher usa solo il trait [`ReleaseSource`]: oggi GitHub privato,
//! domani anche una cartella di rete, senza cambiare il resto.

use std::io::Read;

use serde::Deserialize;

use crate::catalog::{AppSpec, SourceSpec};
use crate::manifest::Manifest;
use crate::secret::Secret;
use crate::version::Version;
use crate::{LauncherError, Result};

/// Dimensione massima di un archivio scaricato.
const MAX_DOWNLOAD: u64 = 1024 * 1024 * 1024;
const MANIFEST_NAME: &str = "manifest.json";

/// Un file di un rilascio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteAsset {
  pub name: String,
  pub size: u64,
  /// Dove scaricarlo (URL dell'API per GitHub, percorso per una cartella).
  pub locator: String,
}

/// L'ultimo rilascio di un'app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteRelease {
  pub version: Version,
  pub tag: String,
  pub manifest: Manifest,
  pub assets: Vec<RemoteAsset>,
}

impl RemoteRelease {
  pub fn asset(&self, name: &str) -> Option<&RemoteAsset> {
    self.assets.iter().find(|a| a.name == name)
  }
}

/// Avanzamento di uno scaricamento: (byte ricevuti, byte totali se noti).
pub type Progress<'a> = &'a mut dyn FnMut(u64, Option<u64>);

/// Una sorgente di rilasci.
pub trait ReleaseSource: Send + Sync {
  /// Ultimo rilascio pubblicato (None se non ce ne sono).
  fn latest(&self, app: &AppSpec) -> Result<Option<RemoteRelease>>;
  /// Scarica un file del rilascio.
  fn download(&self, asset: &RemoteAsset, progress: Progress<'_>) -> Result<Vec<u8>>;
}

/// Sorgente per un'app del catalogo.
pub fn source_for(app: &AppSpec, token: &Secret) -> Result<Box<dyn ReleaseSource>> {
  match &app.source {
    SourceSpec::GitHub { .. } => Ok(Box::new(GitHubSource::new(token.clone()))),
    // TODO: sorgente cartella di rete (\\server\cnc\<app>\<versione>\manifest.json): implementare ReleaseSource
    SourceSpec::NetworkShare { root } => {
      Err(LauncherError::new(format!("sorgente cartella di rete non ancora disponibile: {root}")))
    }
  }
}

// --------------------------------------------------------------------------
// GitHub (Release di un repository privato)
// --------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GhAsset {
  name: String,
  size: u64,
  url: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
  tag_name: String,
  draft: bool,
  prerelease: bool,
  assets: Vec<GhAsset>,
}

/// Release di GitHub lette con un token di sola lettura.
///
/// TODO: accesso con "Sign in with GitHub" (device flow) al posto del token incollato a mano.
pub struct GitHubSource {
  token: Secret,
  api: String,
  agent: ureq::Agent,
}

impl std::fmt::Debug for GitHubSource {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("GitHubSource").field("api", &self.api).field("token", &self.token).finish_non_exhaustive()
  }
}

/// Messaggio per l'utente dagli errori HTTP (mai con il token).
fn http_error(e: &ureq::Error) -> LauncherError {
  match e {
    ureq::Error::StatusCode(401) => LauncherError::new("token non valido o scaduto (401): creane uno nuovo"),
    ureq::Error::StatusCode(403) => LauncherError::new(
      "accesso negato (403): il token non ha il permesso Contents: read, oppure limite di richieste raggiunto",
    ),
    ureq::Error::StatusCode(404) => {
      LauncherError::new("repository non trovato (404): il token non ha accesso al repository privato")
    }
    ureq::Error::StatusCode(n) => LauncherError::new(format!("risposta inattesa di GitHub ({n})")),
    other => LauncherError::new(format!("rete non raggiungibile: {other}")),
  }
}

impl GitHubSource {
  pub fn new(token: Secret) -> Self {
    Self::with_api(token, "https://api.github.com")
  }

  /// Con un indirizzo dell'API diverso (GitHub Enterprise, test).
  pub fn with_api(token: Secret, api: &str) -> Self {
    Self { token, api: api.trim_end_matches('/').to_owned(), agent: ureq::Agent::new_with_defaults() }
  }

  fn get(&self, url: &str, accept: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    self
      .agent
      .get(url)
      .header("Authorization", &format!("Bearer {}", self.token.expose()))
      .header("Accept", accept)
      .header("X-GitHub-Api-Version", "2022-11-28")
      .header("User-Agent", "cnc-launcher")
  }

  fn fetch(&self, url: &str, accept: &str, progress: Progress<'_>) -> Result<Vec<u8>> {
    let resp = self.get(url, accept).call().map_err(|e| http_error(&e))?;
    let total = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    let mut reader = resp.into_body().into_reader().take(MAX_DOWNLOAD);
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
      let n = reader.read(&mut buf).map_err(|e| LauncherError::new(format!("scaricamento interrotto: {e}")))?;
      if n == 0 {
        break;
      }
      out.extend_from_slice(buf.get(..n).unwrap_or(&[]));
      progress(out.len() as u64, total);
    }
    Ok(out)
  }
}

impl ReleaseSource for GitHubSource {
  fn latest(&self, app: &AppSpec) -> Result<Option<RemoteRelease>> {
    let SourceSpec::GitHub { owner, repo, tag_prefix } = &app.source else {
      return Err(LauncherError::new("sorgente non GitHub"));
    };
    let url = format!("{}/repos/{owner}/{repo}/releases?per_page=50", self.api);
    let body = self.fetch(&url, "application/vnd.github+json", &mut |_, _| {})?;
    let releases: Vec<GhRelease> =
      serde_json::from_slice(&body).map_err(|e| LauncherError::new(format!("risposta di GitHub non valida: {e}")))?;
    let best = releases
      .into_iter()
      .filter(|r| !r.draft && !r.prerelease)
      .filter_map(|r| {
        let v = Version::parse(r.tag_name.strip_prefix(tag_prefix.as_str())?)?;
        Some((v, r))
      })
      .max_by(|a, b| a.0.cmp(&b.0));
    let Some((version, rel)) = best else { return Ok(None) };
    let assets: Vec<RemoteAsset> =
      rel.assets.into_iter().map(|a| RemoteAsset { name: a.name, size: a.size, locator: a.url }).collect();
    let manifest_asset = assets
      .iter()
      .find(|a| a.name == MANIFEST_NAME)
      .ok_or_else(|| LauncherError::new(format!("il rilascio {} non contiene {MANIFEST_NAME}", rel.tag_name)))?;
    let manifest = Manifest::parse(&self.download(manifest_asset, &mut |_, _| {})?)?;
    if manifest.app_id != app.id {
      return Err(LauncherError::new(format!("il manifest e' di un'altra app ({})", manifest.app_id)));
    }
    Ok(Some(RemoteRelease { version, tag: rel.tag_name, manifest, assets }))
  }

  fn download(&self, asset: &RemoteAsset, progress: Progress<'_>) -> Result<Vec<u8>> {
    // l'API risponde con un rinvio a un indirizzo temporaneo: ureq non vi inoltra il token
    self.fetch(&asset.locator, "application/octet-stream", progress)
  }
}

// --------------------------------------------------------------------------
// Sorgente in memoria (test e prove)
// --------------------------------------------------------------------------

/// Rilasci tenuti in memoria: per i test senza rete.
#[derive(Debug, Default)]
pub struct MemorySource {
  pub maybe_release: Option<RemoteRelease>,
  /// Contenuto dei file per `locator`.
  pub files: Vec<(String, Vec<u8>)>,
  pub maybe_error: Option<String>,
}

impl ReleaseSource for MemorySource {
  fn latest(&self, _app: &AppSpec) -> Result<Option<RemoteRelease>> {
    if let Some(e) = &self.maybe_error {
      return Err(LauncherError::new(e.clone()));
    }
    Ok(self.maybe_release.clone())
  }

  fn download(&self, asset: &RemoteAsset, progress: Progress<'_>) -> Result<Vec<u8>> {
    let data = self
      .files
      .iter()
      .find(|(k, _)| *k == asset.locator)
      .map(|(_, v)| v.clone())
      .ok_or_else(|| LauncherError::new(format!("file non trovato: {}", asset.name)))?;
    progress(data.len() as u64, Some(data.len() as u64));
    Ok(data)
  }
}
