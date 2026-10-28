//! Flusso completo senza rete: un piccolo server HTTP locale imita l'API di GitHub.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use launcher_core::catalog::catalog;
use launcher_core::install::{Installer, sha256_hex};
use launcher_core::secret::Secret;
use launcher_core::source::{GitHubSource, ReleaseSource};

const TOKEN: &str = "github_pat_TEST";

/// Archivio tar.gz con un "eseguibile".
fn archive(version: &str) -> Vec<u8> {
  let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
  let data = b"#!/bin/sh\necho ciao\n";
  let mut h = tar::Header::new_gnu();
  h.set_size(data.len() as u64);
  h.set_mode(0o644);
  h.set_cksum();
  let path = format!("heidenhain-siemens-converter-{version}-linux-x64/heidenhain-siemens-converter");
  tar.append_data(&mut h, path, &data[..]).unwrap();
  tar.into_inner().unwrap().finish().unwrap()
}

/// Server che risponde a: lista delle release, manifest, archivio. Registra gli header ricevuti.
fn server(platform: &str) -> (String, Arc<Mutex<Vec<String>>>) {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen2 = seen.clone();
  let arch = archive("0.2.0");
  let file = format!("heidenhain-siemens-converter-0.2.0-{platform}.tar.gz");
  let manifest = format!(
    r#"{{"schema":1,"app_id":"heidenhain-siemens-converter","name":"HEIDENHAIN-SIEMENS CONVERTER","version":"0.2.0",
      "assets":[{{"platform":"{platform}","file":"{file}","sha256":"{}","size":{},
      "executable":"heidenhain-siemens-converter-0.2.0-linux-x64/heidenhain-siemens-converter"}}]}}"#,
    sha256_hex(&arch),
    arch.len()
  );
  let releases = format!(
    r#"[{{"tag_name":"converter-v0.1.0","draft":false,"prerelease":false,"assets":[]}},
        {{"tag_name":"converter-v0.3.0","draft":true,"prerelease":false,"assets":[]}},
        {{"tag_name":"converter-v0.2.0","draft":false,"prerelease":false,"assets":[
          {{"name":"manifest.json","size":1,"url":"{base}/assets/1"}},
          {{"name":"{file}","size":{},"url":"{base}/assets/2"}}]}}]"#,
    arch.len()
  );
  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let mut reader = BufReader::new(stream.try_clone().unwrap());
      let mut request_line = String::new();
      reader.read_line(&mut request_line).unwrap();
      let mut auth = String::new();
      loop {
        let mut l = String::new();
        if reader.read_line(&mut l).unwrap() == 0 || l == "\r\n" {
          break;
        }
        if l.to_ascii_lowercase().starts_with("authorization:") {
          auth = l.split_once(':').map(|(_, v)| v.trim().to_owned()).unwrap_or_default();
        }
      }
      seen2.lock().unwrap().push(format!("{} | {auth}", request_line.trim()));
      let path = request_line.split_whitespace().nth(1).unwrap_or("").to_owned();
      let (status, body): (&str, Vec<u8>) = if auth != format!("Bearer {TOKEN}") {
        ("401 Unauthorized", b"{}".to_vec())
      } else if path.starts_with("/repos/giacomogaleotti2000/HEIDEN-SIEMENS/releases") {
        ("200 OK", releases.clone().into_bytes())
      } else if path == "/assets/1" {
        ("200 OK", manifest.clone().into_bytes())
      } else if path == "/assets/2" {
        ("200 OK", arch.clone())
      } else {
        ("404 Not Found", b"{}".to_vec())
      };
      let mut s = stream;
      let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
      let _ = s.write_all(&body);
    }
  });
  (base, seen)
}

#[test]
fn latest_release_download_verify_install() {
  let platform = "linux-x64";
  let (base, seen) = server(platform);
  let app = catalog().remove(0);
  let src = GitHubSource::with_api(Secret::new(TOKEN), &base);
  let rel = src.latest(&app).unwrap().unwrap();
  // la bozza 0.3.0 e' ignorata, la 0.1.0 e' piu' vecchia
  assert_eq!(rel.version.text, "0.2.0");
  let asset = rel.manifest.asset_for(platform).unwrap().clone();
  let remote = rel.asset(&asset.file).unwrap().clone();
  let mut last = 0;
  let bytes = src.download(&remote, &mut |n, _| last = n).unwrap();
  assert_eq!(last, bytes.len() as u64);

  let root = std::env::temp_dir().join(format!("launcher_test_{}", std::process::id()));
  let inst = Installer::new(root.clone());
  assert!(inst.installed(&app.id).is_none());
  let installed = inst.install(&app, "0.2.0", &asset, &bytes).unwrap();
  assert!(installed.executable.is_file());
  assert_eq!(inst.installed(&app.id).unwrap().version, "0.2.0");

  // archivio alterato: rifiutato
  let mut bad = bytes.clone();
  let last_byte = bad.len() - 1;
  bad[last_byte] ^= 0xff;
  let err = inst.install(&app, "0.2.0", &asset, &bad).unwrap_err();
  assert!(err.message.contains("SHA-256"));

  // il token arriva all'API e non compare in nessun messaggio d'errore
  assert!(seen.lock().unwrap().iter().all(|l| l.contains(TOKEN)));
  let _ = std::fs::remove_dir_all(root);
}

#[test]
fn wrong_token_gives_clear_message_without_leaking_it() {
  let (base, _) = server("linux-x64");
  let src = GitHubSource::with_api(Secret::new("sbagliato"), &base);
  let err = src.latest(&catalog().remove(0)).unwrap_err();
  assert!(err.message.contains("401"), "{}", err.message);
  assert!(!err.message.contains("sbagliato"));
  assert!(!format!("{src:?}").contains("sbagliato"));
}

#[test]
fn update_replaces_the_installed_version() {
  use launcher_core::manifest::ManifestAsset;
  let app = catalog().remove(0);
  let root = std::env::temp_dir().join(format!("launcher_update_{}", std::process::id()));
  let inst = Installer::new(root.clone());
  let asset = |v: &str, bytes: &[u8]| ManifestAsset {
    platform: "linux-x64".to_owned(),
    file: format!("heidenhain-siemens-converter-{v}-linux-x64.tar.gz"),
    sha256: sha256_hex(bytes),
    size: bytes.len() as u64,
    executable: format!("heidenhain-siemens-converter-{v}-linux-x64/heidenhain-siemens-converter"),
  };
  let mk = |v: &str| {
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
    let data = b"#!/bin/sh\necho ciao\n";
    let mut h = tar::Header::new_gnu();
    h.set_size(data.len() as u64);
    h.set_mode(0o644);
    h.set_cksum();
    let path = format!("heidenhain-siemens-converter-{v}-linux-x64/heidenhain-siemens-converter");
    tar.append_data(&mut h, path, &data[..]).unwrap();
    tar.into_inner().unwrap().finish().unwrap()
  };
  let (a1, a2) = (mk("0.1.0"), mk("0.2.0"));
  let old = inst.install(&app, "0.1.0", &asset("0.1.0", &a1), &a1).unwrap();
  assert_eq!(inst.installed(&app.id).unwrap().version, "0.1.0");
  // aggiornamento: nuova versione registrata, cartella della vecchia rimossa
  let new = inst.install(&app, "0.2.0", &asset("0.2.0", &a2), &a2).unwrap();
  assert_eq!(inst.installed(&app.id).unwrap().version, "0.2.0");
  assert!(new.executable.is_file());
  assert!(!old.executable.exists());
  // lo stato sopravvive a un nuovo avvio del launcher
  assert_eq!(Installer::new(root.clone()).installed(&app.id).unwrap().version, "0.2.0");
  let _ = std::fs::remove_dir_all(root);
}
