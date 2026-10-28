//! Launcher privato delle app (eframe + wgpu).
//!
//! Il token GitHub di sola lettura sta nel portachiavi del sistema; le app si installano nella
//! cartella dati dell'utente.
//!
//! Diagnostica: `CNC_SCREENSHOT=file.png` salva uno screenshot quando la finestra e' pronta,
//! `CNC_SCREENSHOT_EXIT=1` chiude il launcher subito dopo.

#![allow(clippy::print_stderr)]
// in release su Windows niente finestra di console accanto al launcher
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use launcher_core::catalog::catalog;
use launcher_core::install::Installer;
use launcher_core::secret::{KeyringStore, MemoryStore, TokenStore};
use launcher_ui::{LauncherUi, default_factory, default_root};

/// Frame da attendere prima dello screenshot (font e layout stabili).
const SCREENSHOT_AFTER_FRAMES: u32 = 30;

struct App {
  ui: LauncherUi,
  frames: u32,
  maybe_screenshot: Option<String>,
  screenshot_requested: bool,
  saved: Arc<AtomicBool>,
  exit_after_screenshot: bool,
}

impl eframe::App for App {
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    self.ui.ui(ui);
    self.frames = self.frames.saturating_add(1);
    if let Some(path) = &self.maybe_screenshot {
      if !self.screenshot_requested && self.frames >= SCREENSHOT_AFTER_FRAMES {
        self.screenshot_requested = true;
        let (path, saved, ctx) = (path.clone(), self.saved.clone(), ui.ctx().clone());
        ui.ctx().request_screenshot(move |img| {
          let [w, h] = img.size;
          let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
          let result = image::RgbaImage::from_raw(u32::try_from(w).unwrap_or(0), u32::try_from(h).unwrap_or(0), bytes)
            .ok_or_else(|| "immagine non valida".to_owned())
            .and_then(|i| i.save(&path).map_err(|e| e.to_string()));
          match result {
            Ok(()) => eprintln!("screenshot salvato in {path}"),
            Err(e) => eprintln!("screenshot non salvato: {e}"),
          }
          saved.store(true, Ordering::SeqCst);
          ctx.request_repaint();
        });
      }
      if self.exit_after_screenshot && self.saved.load(Ordering::SeqCst) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
      }
      ui.ctx().request_repaint();
    }
  }
}

/// Portachiavi del sistema; se non risponde, token solo in memoria per questa sessione.
fn token_store() -> Arc<dyn TokenStore> {
  let keyring = KeyringStore;
  match keyring.load() {
    Ok(_) => Arc::new(keyring),
    Err(e) => {
      eprintln!("{}: uso un archivio in memoria", e.message);
      Arc::new(MemoryStore::default())
    }
  }
}

fn main() -> eframe::Result {
  let root = match default_root() {
    Ok(r) => r,
    Err(e) => {
      eprintln!("{}", e.message);
      std::env::temp_dir().join("cnc-launcher")
    }
  };
  let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
      .with_title("CNC LAUNCHER")
      .with_app_id("cnc-launcher")
      .with_inner_size([1100.0, 680.0])
      .with_min_inner_size([760.0, 460.0])
      .with_icon(window_icon(ui_theme::brand::LAUNCHER_ICON)),
    ..Default::default()
  };
  eframe::run_native(
    "cnc-launcher",
    options,
    Box::new(move |cc| {
      LauncherUi::install_theme(&cc.egui_ctx);
      let mut ui = LauncherUi::new(catalog(), token_store(), default_factory(), Installer::new(root));
      ui.set_context(cc.egui_ctx.clone());
      ui.check_all();
      Ok(Box::new(App {
        ui,
        frames: 0,
        maybe_screenshot: std::env::var("CNC_SCREENSHOT").ok().filter(|s| !s.is_empty()),
        screenshot_requested: false,
        saved: Arc::new(AtomicBool::new(false)),
        exit_after_screenshot: std::env::var("CNC_SCREENSHOT_EXIT").is_ok_and(|v| v == "1"),
      }))
    }),
  )
}

/// Icona della finestra (barra delle applicazioni / Dock).
fn window_icon(png: &[u8]) -> egui::IconData {
  ui_theme::brand::icon_rgba(png).map_or_else(egui::IconData::default, |(rgba, width, height)| egui::IconData {
    rgba,
    width,
    height,
  })
}
