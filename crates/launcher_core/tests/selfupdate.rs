//! Aggiornamento del launcher senza rete: un server locale imita la Release pubblica.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use launcher_core::current_platform;
use launcher_core::install::sha256_hex;
use launcher_core::selfupdate::{LauncherUpdater, exe_name};

fn archive(version: &str) -> Vec<u8> {
  let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
  let data = format!("launcher {version}");
  let mut h = tar::Header::new_gnu();
  h.set_size(data.len() as u64);
  h.set_mode(0o755);
  h.set_cksum();
  let path = format!("cnc-launcher-{version}-{}/{}", current_platform(), exe_name());
  tar.append_data(&mut h, path, data.as_bytes()).unwrap();
  tar.into_inner().unwrap().finish().unwrap()
}

/// Server con la Release `launcher-v<version>`; l'archivio e' sempre .tar.gz (anche col nome .zip
/// su Windows l'estrazione sceglie dal nome: qui il test gira su macOS/Linux).
fn server(version: &'static str, bad_hash: bool) -> String {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());
  let arch = archive(version);
  let sha = if bad_hash { "0".repeat(64) } else { sha256_hex(&arch) };
  let name = format!("cnc-launcher-{version}-{}.tar.gz", current_platform());
  let release = format!(
    r#"{{"tag_name":"launcher-v{version}","assets":[
      {{"name":"{name}","browser_download_url":"{base}/a"}},
      {{"name":"{name}.sha256","browser_download_url":"{base}/s"}}]}}"#
  );
  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let mut reader = BufReader::new(stream.try_clone().unwrap());
      let mut line = String::new();
      reader.read_line(&mut line).unwrap();
      loop {
        let mut l = String::new();
        if reader.read_line(&mut l).unwrap() == 0 || l == "\r\n" {
          break;
        }
      }
      let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
      let body: Vec<u8> = match path.as_str() {
        "/repos/o/r/releases/latest" => release.clone().into_bytes(),
        "/a" => arch.clone(),
        "/s" => format!("{sha}  {name}\n").into_bytes(),
        _ => Vec::new(),
      };
      let mut s = stream;
      let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
      let _ = s.write_all(&body);
    }
  });
  base
}

#[test]
fn newer_release_is_found_verified_and_replaces_the_exe() {
  if !current_platform().starts_with("macos") && !current_platform().starts_with("linux") {
    return;
  }
  let up = LauncherUpdater::with_api(&server("9.1.0", false), "o/r");
  assert!(up.check("9.1.0").unwrap().is_none(), "stessa versione: niente aggiornamento");
  let rel = up.check("0.4.0").unwrap().expect("aggiornamento");
  assert_eq!(rel.version.text, "9.1.0");
  let dir = std::env::temp_dir().join(format!("launcher_selfupdate_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  let exe = dir.join(exe_name());
  std::fs::write(&exe, b"launcher 0.4.0").unwrap();
  up.apply(&rel, &exe).unwrap();
  assert_eq!(std::fs::read_to_string(&exe).unwrap(), "launcher 9.1.0");
}

#[test]
fn tampered_release_is_refused() {
  if !current_platform().starts_with("macos") && !current_platform().starts_with("linux") {
    return;
  }
  let up = LauncherUpdater::with_api(&server("9.2.0", true), "o/r");
  let rel = up.check("0.4.0").unwrap().unwrap();
  let dir = std::env::temp_dir().join(format!("launcher_selfupdate_bad_{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  let exe = dir.join(exe_name());
  std::fs::write(&exe, b"launcher 0.4.0").unwrap();
  assert!(up.apply(&rel, &exe).unwrap_err().message.contains("SHA-256"));
  assert_eq!(std::fs::read_to_string(&exe).unwrap(), "launcher 0.4.0");
}
