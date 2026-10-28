//! Tema condiviso delle app. Token di design (colori, tipi, forme) e installazione di font e stile.
//!
//! Superfici quasi nere, linee bianche al 15%, angoli di 3 px solo sui controlli, un solo colore
//! di accento. Inter per il testo, Geist Mono per codice ed etichette (maiuscolo, spaziato).
//! Solo tema scuro.

pub mod brand;

use std::sync::Arc;

use egui::epaint::text::VariationCoords;
use egui::{
  Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Stroke, TextStyle, Visuals,
};

const fn hex(rgb: u32) -> Color32 {
  let [_, r, g, b] = rgb.to_be_bytes();
  Color32::from_rgb(r, g, b)
}

const fn alpha(rgb: u32, a: u8) -> Color32 {
  let [_, r, g, b] = rgb.to_be_bytes();
  Color32::from_rgba_unmultiplied_const(r, g, b, a)
}

/// Sfondo del pannello dei contenuti.
pub const BG: Color32 = hex(0x121316);
/// Pannelli, finestre di dialogo.
pub const PANEL: Color32 = hex(0x101014);
/// Controlli: pulsanti, schede.
pub const CONTROLS: Color32 = hex(0x1e1f22);
/// Cornice della finestra: barra in alto e sidebar.
pub const CHROME: Color32 = hex(0x0a0a0c);
/// Pozzetti e campi di testo, area del disegno.
pub const SUNKEN: Color32 = hex(0x060607);

/// Testo.
pub const INK: Color32 = hex(0xf2f1ee);
pub const INK_STRONG: Color32 = Color32::WHITE;
/// Testo secondario.
pub const MUTED: Color32 = alpha(0xf2f1ee, 153);
/// Icone a riposo, numeri di riga.
pub const DIM: Color32 = alpha(0xf2f1ee, 128);
/// Didascalie e contatori.
pub const FAINT: Color32 = alpha(0xf2f1ee, 102);

/// Ogni linea sottile (bianco al 15%).
pub const LINE: Color32 = alpha(0xffffff, 38);
pub const LINE_STRONG: Color32 = alpha(0xf2f1ee, 102);
/// Riempimenti tenui.
pub const WASH: Color32 = alpha(0xffffff, 13);
pub const WASH_SELECTED: Color32 = alpha(0xffffff, 20);
pub const WASH_HOVER: Color32 = alpha(0xffffff, 26);

/// L'unico colore di accento.
pub const ACCENT: Color32 = hex(0x2d81ff);
pub const ACCENT_400: Color32 = hex(0x59a7ff);
pub const ACCENT_INK: Color32 = hex(0x74aaff);
pub const ACCENT_WASH: Color32 = alpha(0x2d81ff, 56);

/// Esiti.
pub const OK: Color32 = hex(0x3ecf8e);
pub const BAD: Color32 = hex(0xf87171);
pub const WARN: Color32 = hex(0xffc14d);

/// 3 px: pulsanti, campi, menu.
pub const RADIUS: CornerRadius = CornerRadius::same(3);
pub const CONTROL_H: f32 = 30.0;
/// Etichette in Geist Mono, maiuscolo.
pub const HUD_SIZE: f32 = 11.0;

pub fn hairline() -> Stroke {
  Stroke::new(1.0, LINE)
}

/// Inter 500.
pub fn medium() -> FontFamily {
  FontFamily::Name("medium".into())
}

/// Inter 600.
pub fn semibold() -> FontFamily {
  FontFamily::Name("semibold".into())
}

/// Geist Mono 500: etichette.
pub fn mono() -> FontFamily {
  FontFamily::Name("mono".into())
}

/// Etichetta maiuscola in Geist Mono.
pub fn hud(text: &str) -> egui::RichText {
  egui::RichText::new(text.to_uppercase()).font(FontId::new(HUD_SIZE, mono())).color(DIM)
}

/// Titolo di pagina.
pub fn title(text: &str) -> egui::RichText {
  egui::RichText::new(text).font(FontId::new(20.0, semibold())).color(INK_STRONG)
}

/// Codice CNC.
pub fn code_font() -> FontId {
  FontId::new(12.5, FontFamily::Monospace)
}

/// Colore di una quota Z nella scala da `lo` (blu) a `hi` (giallo), passando per il verde acqua.
/// Scala delle quote Z, dalla piu bassa alla piu alta.
pub const Z_STOPS: [Color32; 4] = [hex(0x2d81ff), hex(0x22b8cf), hex(0x3ecf8e), hex(0xffc14d)];

pub fn z_color(z: f64, lo: f64, hi: f64) -> Color32 {
  const STOPS: [Color32; 4] = Z_STOPS;
  let t = if hi - lo > 1e-9 { ((z - lo) / (hi - lo)).clamp(0.0, 1.0) as f32 } else { 1.0 };
  let pos = t * (STOPS.len() - 1) as f32;
  let i = (pos.floor() as usize).min(STOPS.len() - 2);
  let f = pos - i as f32;
  match (STOPS.get(i), STOPS.get(i + 1)) {
    (Some(a), Some(b)) => a.lerp_to_gamma(*b, f),
    _ => ACCENT,
  }
}

fn visuals() -> Visuals {
  let mut v = Visuals::dark();
  v.panel_fill = BG;
  v.window_fill = PANEL;
  v.extreme_bg_color = SUNKEN;
  v.faint_bg_color = CONTROLS;
  v.code_bg_color = SUNKEN;
  v.window_stroke = hairline();
  v.window_corner_radius = RADIUS;
  v.menu_corner_radius = RADIUS;
  v.override_text_color = Some(INK);
  v.hyperlink_color = ACCENT_INK;
  v.selection.bg_fill = ACCENT;
  v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
  v.warn_fg_color = WARN;
  v.error_fg_color = BAD;
  v.striped = false;
  for (w, fill, stroke, fg) in [
    (&mut v.widgets.noninteractive, CONTROLS, LINE, MUTED),
    (&mut v.widgets.inactive, CONTROLS, LINE, INK),
    (&mut v.widgets.hovered, WASH_HOVER, LINE_STRONG, INK_STRONG),
    (&mut v.widgets.active, WASH_HOVER, LINE_STRONG, INK_STRONG),
    (&mut v.widgets.open, WASH_HOVER, LINE_STRONG, INK_STRONG),
  ] {
    w.bg_fill = fill;
    w.weak_bg_fill = fill;
    w.bg_stroke = Stroke::new(1.0, stroke);
    w.corner_radius = RADIUS;
    w.fg_stroke = Stroke::new(1.0, fg);
    w.expansion = 0.0;
  }
  v
}

/// Installa font, colori e spaziature. Solo tema scuro.
pub fn apply(ctx: &egui::Context) {
  install_fonts(ctx);
  ctx.set_visuals_of(egui::Theme::Dark, visuals());
  ctx.set_theme(egui::ThemePreference::Dark);
  ctx.all_styles_mut(|style| {
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = CONTROL_H;
    style.spacing.menu_margin = egui::Margin::same(4);
    style.spacing.window_margin = egui::Margin::same(16);
    style.text_styles = [
      (TextStyle::Heading, FontId::new(20.0, semibold())),
      (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
      (TextStyle::Button, FontId::new(13.0, medium())),
      (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
      (TextStyle::Monospace, code_font()),
    ]
    .into();
  });
}

/// Un font registrato: nome, dati, coordinate delle variazioni, famiglia.
type Face<'a> = (&'a str, &'static [u8], &'a [([u8; 4], f32)], FontFamily);

fn install_fonts(ctx: &egui::Context) {
  const INTER: &[u8] = include_bytes!("../assets/fonts/Inter-Variable.ttf");
  const GEIST_MONO: &[u8] = include_bytes!("../assets/fonts/GeistMono-Variable.ttf");
  let mut fonts = FontDefinitions::default();
  // i font di egui restano dietro ai nostri, per i simboli che i nostri non hanno
  let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
  let faces: [Face<'_>; 5] = [
    ("Inter", INTER, &[(*b"wght", 400.0), (*b"opsz", 14.0)], FontFamily::Proportional),
    ("Inter-Medium", INTER, &[(*b"wght", 500.0), (*b"opsz", 14.0)], medium()),
    ("Inter-SemiBold", INTER, &[(*b"wght", 600.0), (*b"opsz", 14.0)], semibold()),
    ("GeistMono", GEIST_MONO, &[(*b"wght", 400.0)], FontFamily::Monospace),
    ("GeistMono-Medium", GEIST_MONO, &[(*b"wght", 550.0)], mono()),
  ];
  for (name, bytes, coords, family) in faces {
    let tweak = FontTweak { coords: VariationCoords::new(coords.iter().copied()), ..Default::default() };
    fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes).tweak(tweak)));
    let mut stack = vec![name.to_owned()];
    stack.extend(fallback.iter().cloned());
    fonts.families.insert(family, stack);
  }
  ctx.set_fonts(fonts);
}
