//! Interfaccia del launcher privato.
//!
//! Prima apertura: richiesta del token GitHub di sola lettura (salvato nel portachiavi del
//! sistema). Poi la libreria delle app: versione installata e ultima disponibile, Installa /
//! Aggiorna / Apri con avanzamento. La rete gira in un thread a parte (eventi su un canale).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, RichText, Sense, Stroke, Ui, vec2};
use launcher_core::catalog::AppSpec;
use launcher_core::install::{Installed, Installer};
use launcher_core::secret::{Secret, TokenStore};
use launcher_core::source::{ReleaseSource, RemoteRelease};
use launcher_core::{LauncherError, current_platform};
pub use ui_theme as theme;

pub const SIDEBAR_WIDTH: f32 = 200.0;
pub const TOPBAR_HEIGHT: f32 = 44.0;
/// Guida alla creazione del token (nel repository).
pub const GUIDE: &str = "https://github.com/giacomogaleotti2000/launcher-galbiati/blob/main/docs/launcher.md";

/// Crea la sorgente dei rilasci di un'app con il token.
pub type SourceFactory = Arc<dyn Fn(&AppSpec, &Secret) -> launcher_core::Result<Box<dyn ReleaseSource>> + Send + Sync>;

/// Legge il testo negli appunti.
pub type ClipboardReader = Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Appunti del sistema (None se vuoti o non disponibili).
pub fn system_clipboard() -> Option<String> {
  arboard::Clipboard::new().ok()?.get_text().ok().filter(|t| !t.trim().is_empty())
}

/// Pagine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
  #[default]
  Library,
  Settings,
}

/// Stato dell'ultima versione di un'app.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Latest {
  #[default]
  Unknown,
  Checking,
  Found(Box<RemoteRelease>),
  NoRelease,
  Error(String),
}

/// Operazione in corso su un'app.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Op {
  #[default]
  Idle,
  Downloading {
    received: u64,
    maybe_total: Option<u64>,
  },
  Installing,
  Error(String),
}

/// Una riga della libreria.
#[derive(Clone, Debug)]
pub struct AppRow {
  pub spec: AppSpec,
  pub maybe_installed: Option<Installed>,
  pub latest: Latest,
  pub op: Op,
}

impl AppRow {
  pub fn update_available(&self) -> bool {
    match (&self.latest, &self.maybe_installed) {
      (Latest::Found(r), Some(i)) => launcher_core::version::Version::parse(&i.version).is_none_or(|v| r.version > v),
      (Latest::Found(_), None) => true,
      _ => false,
    }
  }
}

/// Eventi dal thread di lavoro.
#[derive(Debug)]
enum Event {
  /// Verifica del token appena inserito: esito del controllo della prima app.
  Latest {
    index: usize,
    result: launcher_core::Result<Option<RemoteRelease>>,
    verifying: bool,
  },
  Progress {
    index: usize,
    received: u64,
    maybe_total: Option<u64>,
  },
  Installing {
    index: usize,
  },
  Installed {
    index: usize,
    result: launcher_core::Result<Installed>,
  },
}

/// Stato del launcher.
pub struct LauncherUi {
  pub page: Page,
  pub rows: Vec<AppRow>,
  pub maybe_token: Option<Secret>,
  /// Campo del token nella schermata iniziale (mai mostrato in chiaro).
  pub token_input: String,
  pub maybe_message: Option<(bool, String)>,
  /// Esegue i lavori nello stesso thread (test).
  pub inline_jobs: bool,
  /// Lettura degli appunti per il pulsante "Incolla" (sostituibile nei test).
  pub clipboard: ClipboardReader,
  /// Token in prova: salvato nel portachiavi solo se GitHub lo accetta.
  maybe_pending: Option<Secret>,
  store: Arc<dyn TokenStore>,
  factory: SourceFactory,
  installer: Installer,
  tx: Sender<Event>,
  rx: Receiver<Event>,
  maybe_ctx: Option<egui::Context>,
}

impl std::fmt::Debug for LauncherUi {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LauncherUi").field("page", &self.page).field("rows", &self.rows.len()).finish_non_exhaustive()
  }
}

impl LauncherUi {
  pub fn new(apps: Vec<AppSpec>, store: Arc<dyn TokenStore>, factory: SourceFactory, installer: Installer) -> Self {
    let (tx, rx) = channel();
    let rows = apps
      .into_iter()
      .map(|spec| AppRow {
        maybe_installed: installer.installed(&spec.id),
        spec,
        latest: Latest::Unknown,
        op: Op::Idle,
      })
      .collect();
    let mut ui = Self {
      page: Page::Library,
      rows,
      maybe_token: None,
      token_input: String::new(),
      maybe_message: None,
      inline_jobs: false,
      clipboard: Arc::new(system_clipboard),
      maybe_pending: None,
      store,
      factory,
      installer,
      tx,
      rx,
      maybe_ctx: None,
    };
    match ui.store.load() {
      Ok(t) => ui.maybe_token = t.filter(|t| !t.is_empty()),
      Err(e) => ui.maybe_message = Some((false, e.message)),
    }
    ui
  }

  pub fn install_theme(ctx: &egui::Context) {
    theme::apply(ctx);
  }

  /// Contesto per ridisegnare quando arrivano risultati dal thread di lavoro.
  pub fn set_context(&mut self, ctx: egui::Context) {
    self.maybe_ctx = Some(ctx);
  }

  pub fn installer(&self) -> &Installer {
    &self.installer
  }

  fn spawn(&self, job: impl FnOnce(&Sender<Event>) + Send + 'static) {
    let tx = self.tx.clone();
    if self.inline_jobs {
      job(&tx);
      return;
    }
    let ctx = self.maybe_ctx.clone();
    std::thread::spawn(move || {
      job(&tx);
      if let Some(c) = ctx {
        c.request_repaint();
      }
    });
  }

  /// Controlla l'ultima versione di tutte le app.
  pub fn check_all(&mut self) {
    let Some(token) = self.maybe_token.clone() else { return };
    for index in 0..self.rows.len() {
      self.check(index, token.clone(), false);
    }
  }

  fn check(&mut self, index: usize, token: Secret, verifying: bool) {
    let Some(row) = self.rows.get_mut(index) else { return };
    row.latest = Latest::Checking;
    let (spec, factory) = (row.spec.clone(), self.factory.clone());
    self.spawn(move |tx| {
      let result = factory(&spec, &token).and_then(|s| s.latest(&spec));
      let _ = tx.send(Event::Latest { index, result, verifying });
    });
  }

  /// Prova il token inserito: lo salva solo se GitHub risponde.
  pub fn submit_token(&mut self) {
    let token = Secret::new(std::mem::take(&mut self.token_input));
    if token.is_empty() {
      self.maybe_message = Some((false, "Incolla il token prima di continuare.".to_owned()));
      return;
    }
    self.maybe_message = Some((true, "Verifica del token in corso...".to_owned()));
    self.maybe_pending = Some(token.clone());
    if self.rows.is_empty() {
      return;
    }
    self.check(0, token, true);
  }

  /// Dimentica il token (portachiavi compreso).
  pub fn forget_token(&mut self) {
    self.maybe_token = None;
    self.maybe_pending = None;
    for r in &mut self.rows {
      r.latest = Latest::Unknown;
    }
    self.maybe_message = match self.store.delete() {
      Ok(()) => Some((true, "Token rimosso.".to_owned())),
      Err(e) => Some((false, e.message)),
    };
  }

  /// Scarica, verifica e installa l'ultima versione.
  pub fn install(&mut self, index: usize) {
    let Some(token) = self.maybe_token.clone() else { return };
    let Some(row) = self.rows.get_mut(index) else { return };
    let Latest::Found(release) = &row.latest else { return };
    let platform = current_platform();
    let Some(asset) = release.manifest.asset_for(platform).cloned() else {
      row.op = Op::Error(format!("nessun pacchetto per questa piattaforma ({platform})"));
      return;
    };
    let Some(remote) = release.asset(&asset.file).cloned() else {
      row.op = Op::Error(format!("file {} mancante nel rilascio", asset.file));
      return;
    };
    row.op = Op::Downloading { received: 0, maybe_total: Some(asset.size) };
    let (spec, version, factory, installer) =
      (row.spec.clone(), release.manifest.version.clone(), self.factory.clone(), self.installer.clone());
    self.spawn(move |tx| {
      let result = factory(&spec, &token).and_then(|s| {
        let mut last = 0u64;
        let bytes = s.download(&remote, &mut |received, maybe_total| {
          if received.saturating_sub(last) >= 256 * 1024 || maybe_total == Some(received) {
            last = received;
            let _ = tx.send(Event::Progress { index, received, maybe_total });
          }
        })?;
        let _ = tx.send(Event::Installing { index });
        installer.install(&spec, &version, &asset, &bytes)
      });
      let _ = tx.send(Event::Installed { index, result });
    });
  }

  /// Avvia l'app installata.
  pub fn open(&mut self, index: usize) {
    let Some(row) = self.rows.get(index) else { return };
    let Some(inst) = row.maybe_installed.clone() else { return };
    if let Err(e) = self.installer.launch(&inst) {
      self.maybe_message = Some((false, e.message));
    }
  }

  /// Applica gli eventi arrivati dal thread di lavoro.
  pub fn poll(&mut self) {
    while let Ok(ev) = self.rx.try_recv() {
      match ev {
        Event::Latest { index, result, verifying } => {
          if verifying {
            match &result {
              Ok(_) => {
                let token = self.maybe_pending.take();
                if let Some(t) = &token {
                  self.maybe_message = match self.store.save(t) {
                    Ok(()) => Some((true, format!("Token salvato in: {}.", self.store.describe()))),
                    Err(e) => Some((false, format!("{}. Il token vale solo per questa sessione.", e.message))),
                  };
                }
                self.maybe_token = token;
                // controllo delle altre app
                if let Some(t) = self.maybe_token.clone() {
                  for i in 1..self.rows.len() {
                    self.check(i, t.clone(), false);
                  }
                }
              }
              Err(e) => {
                self.maybe_pending = None;
                self.maybe_message = Some((false, format!("Token non accettato: {}", e.message)));
              }
            }
          }
          if let Some(row) = self.rows.get_mut(index) {
            row.latest = match result {
              Ok(Some(r)) => Latest::Found(Box::new(r)),
              Ok(None) => Latest::NoRelease,
              Err(e) => Latest::Error(e.message),
            };
          }
        }
        Event::Progress { index, received, maybe_total } => {
          if let Some(row) = self.rows.get_mut(index) {
            row.op = Op::Downloading { received, maybe_total };
          }
        }
        Event::Installing { index } => {
          if let Some(row) = self.rows.get_mut(index) {
            row.op = Op::Installing;
          }
        }
        Event::Installed { index, result } => {
          if let Some(row) = self.rows.get_mut(index) {
            match result {
              Ok(i) => {
                row.maybe_installed = Some(i);
                row.op = Op::Idle;
              }
              Err(e) => row.op = Op::Error(e.message),
            }
          }
        }
      }
    }
  }

  // ------------------------------------------------------------------ interfaccia
  pub fn ui(&mut self, ui: &mut Ui) {
    if self.maybe_ctx.is_none() {
      self.maybe_ctx = Some(ui.ctx().clone());
    }
    self.poll();
    egui::Panel::top("topbar")
      .exact_size(TOPBAR_HEIGHT)
      .resizable(false)
      .show_separator_line(false)
      .frame(Frame::new().fill(theme::CHROME).inner_margin(Margin::symmetric(14, 8)))
      .show(ui, |ui| {
        ui.horizontal_centered(|ui| {
          ui_theme::brand::image(ui, "launcher-icon", ui_theme::brand::LAUNCHER_ICON, 22.0, "icona");
          ui.label(RichText::new("CNC").font(FontId::new(14.0, theme::mono())).color(theme::ACCENT));
          ui.label(RichText::new("LAUNCHER").font(FontId::new(14.0, theme::mono())).color(theme::INK_STRONG));
          ui.add_space(24.0);
          ui.label(RichText::new("App private").color(theme::MUTED));
          ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(current_platform()).font(FontId::new(12.0, theme::mono())).color(theme::FAINT));
          });
        });
      });
    egui::Panel::left("sidebar")
      .exact_size(SIDEBAR_WIDTH)
      .resizable(false)
      .show_separator_line(false)
      .frame(Frame::new().fill(theme::CHROME).inner_margin(Margin::symmetric(10, 10)))
      .show(ui, |ui| self.sidebar(ui));
    egui::CentralPanel::default()
      .frame(Frame::new().fill(theme::CHROME).inner_margin(Margin { left: 0, right: 8, top: 0, bottom: 8 }))
      .show(ui, |ui| {
        Frame::new()
          .fill(theme::BG)
          .stroke(theme::hairline())
          .corner_radius(CornerRadius::same(8))
          .inner_margin(Margin::same(20))
          .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            if self.maybe_token.is_none() {
              self.token_page(ui);
            } else {
              match self.page {
                Page::Library => self.library(ui),
                Page::Settings => self.settings(ui),
              }
            }
          });
      });
  }

  fn sidebar(&mut self, ui: &mut Ui) {
    for (section, pages) in [("App", [Page::Library].as_slice()), ("Sistema", [Page::Settings].as_slice())] {
      ui.add_space(10.0);
      ui.label(theme::hud(section));
      ui.add_space(2.0);
      for page in pages {
        let label = match page {
          Page::Library => "Libreria",
          Page::Settings => "Impostazioni",
        };
        let selected = self.page == *page && self.maybe_token.is_some();
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        if selected {
          ui.painter().rect_filled(rect, theme::RADIUS, theme::WASH_SELECTED);
        } else if resp.hovered() {
          ui.painter().rect_filled(rect, theme::RADIUS, theme::WASH);
        }
        ui.painter().text(
          egui::pos2(rect.left() + 10.0, rect.center().y),
          egui::Align2::LEFT_CENTER,
          label,
          FontId::new(13.5, theme::medium()),
          if selected { theme::INK_STRONG } else { theme::MUTED },
        );
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        if resp.clicked() {
          self.page = *page;
        }
      }
    }
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
      ui_theme::brand::logo(ui, 40.0);
    });
  }

  fn message(&self, ui: &mut Ui) {
    if let Some((ok, text)) = &self.maybe_message {
      ui.label(RichText::new(text).color(if *ok { theme::MUTED } else { theme::BAD }));
    }
  }

  fn token_page(&mut self, ui: &mut Ui) {
    ui.label(theme::title("Accesso alle app private"));
    ui.add_space(6.0);
    ui.label(
      RichText::new(
        "Le app si scaricano dal repository privato su GitHub. Serve un token personale di SOLA LETTURA \
         (fine-grained, permesso Contents: read solo su questo repository).",
      )
      .color(theme::MUTED),
    );
    ui.label(
      RichText::new(format!(
        "Il token viene verificato e poi conservato in: {}. Mai in un file di testo. Guida: {GUIDE}",
        self.store.describe()
      ))
      .color(theme::FAINT),
    );
    ui.add_space(14.0);
    let mut submit = false;
    ui.horizontal(|ui| {
      let label = ui.label(theme::hud("Token GitHub"));
      let resp = ui
        .add(
          egui::TextEdit::singleline(&mut self.token_input)
            .password(true)
            .hint_text("github_pat_...")
            .font(FontId::new(12.5, egui::FontFamily::Monospace))
            .desired_width(420.0),
        )
        .labelled_by(label.id);
      submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
      let checking = self.maybe_pending.is_some();
      if ui.add_enabled(!checking, egui::Button::new("Incolla")).clicked() {
        match (self.clipboard)() {
          Some(text) => {
            self.token_input = text.trim().to_owned();
            self.maybe_message = None;
          }
          None => self.maybe_message = Some((false, "Appunti vuoti: copia prima il token da GitHub.".to_owned())),
        }
      }
      let button = egui::Button::new(RichText::new("Salva e continua").color(Color32::WHITE)).fill(theme::ACCENT);
      if ui.add_enabled(!checking, button).clicked() {
        submit = true;
      }
      if checking {
        ui.spinner();
      }
    });
    if submit && self.maybe_pending.is_none() {
      self.submit_token();
    }
    ui.add_space(8.0);
    self.message(ui);
  }

  fn library(&mut self, ui: &mut Ui) {
    ui.horizontal(|ui| {
      ui.label(theme::title("Libreria"));
      ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui.button("Controlla aggiornamenti").clicked() {
          self.check_all();
        }
      });
    });
    self.message(ui);
    ui.add_space(10.0);
    let mut actions: Vec<(usize, &'static str)> = Vec::new();
    for (i, row) in self.rows.iter().enumerate() {
      // bordo blu al passaggio del mouse (stato del fotogramma precedente)
      let hover_id = egui::Id::unique(("card_hover", i));
      let hovered = ui.ctx().data(|d| d.get_temp::<bool>(hover_id)).unwrap_or(false);
      let card = Frame::new()
        .fill(theme::CONTROLS)
        .stroke(Stroke::new(if hovered { 1.5 } else { 1.0 }, if hovered { theme::ACCENT } else { theme::LINE }))
        .corner_radius(theme::RADIUS)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
          ui.set_width(ui.available_width());
          let busy = !matches!(row.op, Op::Idle | Op::Error(_));
          ui.horizontal_top(|ui| {
            ui_theme::brand::image(ui, "app-icon", ui_theme::brand::APP_ICON, 64.0, &row.spec.name);
            ui.add_space(12.0);
            // azioni a destra: Installa / Aggiorna sempre, Apri al passaggio del mouse
            let right_w = 190.0;
            let text_w = (ui.available_width() - right_w).max(200.0);
            ui.vertical(|ui| {
              ui.set_width(text_w);
              ui.horizontal(|ui| {
                ui.label(RichText::new(&row.spec.name).font(FontId::new(16.0, theme::mono())).color(theme::INK_STRONG));
                ui.add_space(8.0);
                if row.update_available() {
                  let text = if row.maybe_installed.is_some() { "AGGIORNAMENTO" } else { "NON INSTALLATA" };
                  badge(ui, text, theme::WARN);
                } else if row.maybe_installed.is_some() && matches!(row.latest, Latest::Found(_)) {
                  badge(ui, "AGGIORNATA", theme::OK);
                }
              });
              ui.label(RichText::new(&row.spec.description).color(theme::MUTED));
              ui.add_space(6.0);
              ui.horizontal(|ui| {
                ui.label(theme::hud("Installata"));
                ui.label(mono(row.maybe_installed.as_ref().map_or("—".to_owned(), |i| i.version.clone())));
                ui.add_space(16.0);
                ui.label(theme::hud("Ultima"));
                let latest = match &row.latest {
                  Latest::Unknown => "—".to_owned(),
                  Latest::Checking => "controllo...".to_owned(),
                  Latest::Found(r) => r.manifest.version.clone(),
                  Latest::NoRelease => "nessun rilascio".to_owned(),
                  Latest::Error(_) => "errore".to_owned(),
                };
                ui.label(mono(latest));
              });
              if let Latest::Error(e) = &row.latest {
                ui.label(RichText::new(e).color(theme::BAD));
              }
              match &row.op {
                Op::Downloading { received, maybe_total } => {
                  let frac = maybe_total.filter(|t| *t > 0).map_or(0.0, |t| *received as f32 / t as f32);
                  ui.add(
                    egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                      .desired_width(360.0)
                      .text(format!("Scaricamento {:.1} MB", *received as f64 / 1_048_576.0)),
                  );
                }
                Op::Installing => {
                  ui.label(RichText::new("Verifica SHA-256 e installazione...").color(theme::MUTED));
                }
                Op::Error(e) => {
                  ui.label(RichText::new(e).color(theme::BAD));
                }
                Op::Idle => {}
              }
            });
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
              if row.update_available() {
                let label = if row.maybe_installed.is_some() { "Aggiorna" } else { "Installa" };
                let b = egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(theme::ACCENT);
                if ui.add_enabled(!busy, b).clicked() {
                  actions.push((i, "install"));
                }
              }
              if hovered && row.maybe_installed.is_some() {
                let b = if row.update_available() {
                  egui::Button::new("Apri")
                } else {
                  egui::Button::new(RichText::new("Apri").color(Color32::WHITE)).fill(theme::ACCENT)
                };
                if ui.add_enabled(!busy, b).clicked() {
                  actions.push((i, "open"));
                }
              }
            });
          });
        });
      let now = ui.rect_contains_pointer(card.response.rect);
      if now != hovered {
        ui.ctx().data_mut(|d| d.insert_temp(hover_id, now));
        ui.ctx().request_repaint();
      }
      ui.add_space(10.0);
    }
    for (i, what) in actions {
      match what {
        "open" => self.open(i),
        _ => self.install(i),
      }
    }
  }

  fn settings(&mut self, ui: &mut Ui) {
    ui.label(theme::title("Impostazioni"));
    ui.add_space(10.0);
    egui::Grid::new("settings").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
      ui.label(theme::hud("Token"));
      ui.label(format!("salvato in: {}", self.store.describe()));
      ui.end_row();
      ui.label(theme::hud("Cartella app"));
      ui.label(mono(self.installer.root().display().to_string()));
      ui.end_row();
      ui.label(theme::hud("Piattaforma"));
      ui.label(mono(current_platform().to_owned()));
      ui.end_row();
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
      if ui.button("Sostituisci token").clicked() {
        self.maybe_token = None;
        self.maybe_message = None;
      }
      if ui.button("Rimuovi token").clicked() {
        self.forget_token();
      }
    });
    ui.add_space(8.0);
    self.message(ui);
  }
}

fn mono(t: String) -> RichText {
  RichText::new(t).font(FontId::new(13.0, egui::FontFamily::Monospace)).color(theme::INK)
}

fn badge(ui: &mut Ui, text: &str, color: Color32) {
  Frame::new()
    .fill(color.gamma_multiply(0.18))
    .stroke(Stroke::new(1.0, color.gamma_multiply(0.7)))
    .corner_radius(theme::RADIUS)
    .inner_margin(Margin::symmetric(6, 1))
    .show(ui, |ui| {
      ui.label(RichText::new(text).font(FontId::new(theme::HUD_SIZE, theme::mono())).color(color));
    });
}

/// Sorgente reale: GitHub (o altre, secondo il catalogo).
pub fn default_factory() -> SourceFactory {
  Arc::new(|spec: &AppSpec, token: &Secret| launcher_core::source::source_for(spec, token))
}

/// Cartella di installazione predefinita.
pub fn default_root() -> Result<PathBuf, LauncherError> {
  Installer::default_root()
}
