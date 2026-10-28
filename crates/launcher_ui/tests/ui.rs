//! Test del launcher senza finestra (egui_kittest) con rilasci e token in memoria.
//!
//! `CNC_SCREENSHOT_DIR=cartella cargo test -p launcher_ui` salva anche le immagini renderizzate.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use launcher_core::catalog::catalog;
use launcher_core::current_platform;
use launcher_core::install::{Installer, sha256_hex};
use launcher_core::manifest::Manifest;
use launcher_core::secret::{MemoryStore, Secret, TokenStore};
use launcher_core::source::{MemorySource, RemoteAsset, RemoteRelease};
use launcher_core::version::Version;
use launcher_ui::{Latest, LauncherUi, Op, Page, SourceFactory};

const TOKEN: &str = "github_pat_PROVA";
const APP: &str = "heidenhain-siemens-converter";
const LOCATOR: &str = "mem://archivio";

fn archive(version: &str) -> (String, Vec<u8>) {
  let dir = format!("{APP}-{version}-{}", current_platform());
  let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
  let data = b"#!/bin/sh\necho ciao\n";
  let mut h = tar::Header::new_gnu();
  h.set_size(data.len() as u64);
  h.set_mode(0o755);
  h.set_cksum();
  tar.append_data(&mut h, format!("{dir}/{APP}"), &data[..]).unwrap();
  (format!("{dir}/{APP}"), tar.into_inner().unwrap().finish().unwrap())
}

/// Rilascio `version` per la piattaforma corrente.
fn release(version: &str) -> (RemoteRelease, Vec<u8>) {
  let (exe, bytes) = archive(version);
  let file = format!("{APP}-{version}-{}.tar.gz", current_platform());
  let json = format!(
    r#"{{"schema":1,"app_id":"{APP}","name":"HEIDENHAIN-SIEMENS CONVERTER","version":"{version}",
      "assets":[{{"platform":"{}","file":"{file}","sha256":"{}","size":{},"executable":"{exe}"}}]}}"#,
    current_platform(),
    sha256_hex(&bytes),
    bytes.len()
  );
  let rel = RemoteRelease {
    version: Version::parse(version).unwrap(),
    tag: format!("converter-v{version}"),
    manifest: Manifest::parse(json.as_bytes()).unwrap(),
    assets: vec![RemoteAsset { name: file, size: bytes.len() as u64, locator: LOCATOR.to_owned() }],
  };
  (rel, bytes)
}

fn source(rel: RemoteRelease, bytes: Vec<u8>) -> Box<MemorySource> {
  Box::new(MemorySource { maybe_release: Some(rel), files: vec![(LOCATOR.to_owned(), bytes)], maybe_error: None })
}

/// Sorgente finta: accetta solo TOKEN (come l'API privata di GitHub).
fn factory(version: &'static str) -> SourceFactory {
  Arc::new(move |_spec, token: &Secret| {
    if token.expose() != TOKEN {
      return Err(launcher_core::LauncherError::new("token non valido o scaduto (401): creane uno nuovo"));
    }
    let (rel, bytes) = release(version);
    Ok(source(rel, bytes))
  })
}

fn temp_root(name: &str) -> PathBuf {
  let p = std::env::temp_dir().join(format!("launcher_ui_{name}_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&p);
  p
}

fn launcher(store: Arc<dyn TokenStore>, factory: SourceFactory, root: PathBuf) -> LauncherUi {
  let mut ui = LauncherUi::new(catalog(), store, factory, Installer::new(root));
  ui.inline_jobs = true;
  ui
}

fn harness(state: LauncherUi) -> Harness<'static, LauncherUi> {
  Harness::builder()
    .with_size([1100.0, 680.0])
    .with_context_setup(LauncherUi::install_theme)
    .build_ui_state(|ui, app: &mut LauncherUi| app.ui(ui), state)
}

fn save(harness: &mut Harness<'_, LauncherUi>, name: &str) {
  let Ok(dir) = std::env::var("CNC_SCREENSHOT_DIR") else { return };
  let image = harness.render().expect("render wgpu");
  image.save(PathBuf::from(dir).join(format!("{name}.png"))).expect("salvataggio screenshot");
}

fn type_token(h: &mut Harness<'_, LauncherUi>, text: &str) {
  h.get_by_label("TOKEN GITHUB").click();
  h.run();
  h.get_by_label("TOKEN GITHUB").type_text(text);
  h.run();
  h.get_by_label("Salva e continua").click();
  h.run();
}

#[test]
fn first_run_asks_token_rejects_wrong_one_then_installs_and_updates() {
  let store = Arc::new(MemoryStore::default());
  let root = temp_root("flow");
  let mut h = harness(launcher(store.clone(), factory("0.1.0"), root.clone()));
  h.run();
  save(&mut h, "launcher_token");

  // token sbagliato: non salvato, messaggio chiaro senza il token
  type_token(&mut h, "sbagliato");
  assert!(h.state().maybe_token.is_none());
  assert!(store.load().unwrap().is_none());
  let (ok, msg) = h.state().maybe_message.clone().unwrap();
  assert!(!ok && msg.contains("401") && !msg.contains("sbagliato"), "{msg}");
  assert!(h.state().token_input.is_empty(), "il campo va svuotato");

  // token giusto: verificato, salvato, libreria con la 0.1.0 da installare
  type_token(&mut h, TOKEN);
  assert_eq!(store.load().unwrap().map(|t| t.expose().to_owned()).as_deref(), Some(TOKEN));
  assert!(matches!(&h.state().rows[0].latest, Latest::Found(r) if r.manifest.version == "0.1.0"));
  h.get_by_label("NON INSTALLATA");
  save(&mut h, "launcher_library");

  h.get_by_label("Installa").click();
  h.run();
  assert_eq!(h.state().rows[0].op, Op::Idle);
  let inst = h.state().rows[0].maybe_installed.clone().expect("installata");
  assert_eq!(inst.version, "0.1.0");
  assert!(inst.executable.is_file());
  h.get_by_label("AGGIORNATA");
  // "Apri" compare a destra al passaggio del mouse sull'app
  h.get_by_label("AGGIORNATA").hover();
  h.run();
  h.get_by_label("Apri");
  save(&mut h, "launcher_installed");
  drop(h);

  // nuovo avvio con la 0.2.0 pubblicata: token gia' salvato, si vede l'aggiornamento
  let mut ui = launcher(store.clone(), factory("0.2.0"), root.clone());
  assert!(ui.maybe_token.is_some());
  ui.check_all();
  let mut h = harness(ui);
  h.run();
  h.get_by_label("AGGIORNAMENTO");
  save(&mut h, "launcher_update");
  h.get_by_label("Aggiorna").click();
  h.run();
  assert_eq!(h.state().rows[0].maybe_installed.as_ref().map(|i| i.version.as_str()), Some("0.2.0"));
  // la versione vecchia e' stata rimossa
  assert!(!root.join("app").join(APP).join("0.1.0").exists());
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn settings_can_remove_token() {
  let store = Arc::new(MemoryStore::default());
  store.save(&Secret::new(TOKEN)).unwrap();
  let root = temp_root("settings");
  let mut ui = launcher(store.clone(), factory("0.1.0"), root.clone());
  ui.page = Page::Settings;
  let mut h = harness(ui);
  h.run();
  save(&mut h, "launcher_settings");
  // il token non compare mai nell'interfaccia
  assert!(h.query_by_label_contains(TOKEN).is_none());
  h.get_by_label("Rimuovi token").click();
  h.run();
  assert!(store.load().unwrap().is_none());
  assert!(h.state().maybe_token.is_none());
  h.get_by_label("TOKEN GITHUB");
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tampered_archive_is_rejected() {
  let store = Arc::new(MemoryStore::default());
  store.save(&Secret::new(TOKEN)).unwrap();
  let root = temp_root("tamper");
  let bad: SourceFactory = Arc::new(|_spec, _token| {
    let (rel, mut bytes) = release("0.1.0");
    let n = bytes.len() - 1;
    bytes[n] ^= 0xff;
    Ok(source(rel, bytes))
  });
  let mut ui = launcher(store, bad, root.clone());
  ui.check_all();
  let mut h = harness(ui);
  h.run();
  h.get_by_label("Installa").click();
  h.run();
  assert!(matches!(&h.state().rows[0].op, Op::Error(e) if e.contains("SHA-256")));
  assert!(h.state().rows[0].maybe_installed.is_none());
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn paste_with_ctrl_v_fills_password_field() {
  let store = Arc::new(MemoryStore::default());
  let root = temp_root("ctrlv");
  let mut h = harness(launcher(store.clone(), factory("0.1.0"), root.clone()));
  h.run();
  h.get_by_label("TOKEN GITHUB").click();
  h.run();
  // quello che eframe invia con Ctrl+V (feature "clipboard")
  h.event(egui::Event::Paste(format!("  {TOKEN}\n")));
  h.run();
  assert_eq!(h.state().token_input.trim(), TOKEN);
  h.get_by_label("Salva e continua").click();
  h.run();
  assert_eq!(store.load().unwrap().map(|t| t.expose().to_owned()).as_deref(), Some(TOKEN));
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn paste_button_reads_clipboard() {
  let store = Arc::new(MemoryStore::default());
  let root = temp_root("incolla");
  let mut ui = launcher(store.clone(), factory("0.1.0"), root.clone());
  ui.clipboard = Arc::new(|| Some(format!("{TOKEN}\r\n")));
  let mut h = harness(ui);
  h.run();
  h.get_by_label("Incolla").click();
  h.run();
  assert_eq!(h.state().token_input, TOKEN);
  h.get_by_label("Salva e continua").click();
  h.run();
  assert!(h.state().maybe_token.is_some());

  // appunti vuoti: messaggio chiaro
  let mut ui = launcher(Arc::new(MemoryStore::default()), factory("0.1.0"), root.clone());
  ui.clipboard = Arc::new(|| None);
  let mut h = harness(ui);
  h.run();
  h.get_by_label("Incolla").click();
  h.run();
  assert!(h.state().maybe_message.as_ref().is_some_and(|(ok, m)| !ok && m.contains("Appunti vuoti")));
  let _ = std::fs::remove_dir_all(root);
}
