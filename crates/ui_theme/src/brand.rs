//! Logo dell'azienda e icone delle app (PNG incorporati nell'eseguibile).

use egui::{Context, Response, TextureHandle, TextureOptions, Ui, Vec2};

/// Logo Galbiati Costruzioni Meccaniche per il tema scuro (scritte chiare, fondo trasparente).
pub const GALBIATI_LOGO: &[u8] = include_bytes!("../assets/brand/galbiati-logo-dark.png");
/// Icona del convertitore.
pub const APP_ICON: &[u8] = include_bytes!("../assets/brand/app-icon.png");
/// Icona del launcher.
pub const LAUNCHER_ICON: &[u8] = include_bytes!("../assets/brand/launcher-icon.png");

/// Texture di un PNG incorporato, creata una volta e tenuta nella memoria di egui.
pub fn texture(ctx: &Context, name: &'static str, png: &[u8]) -> Option<TextureHandle> {
  let id = egui::Id::unique(("brand", name));
  if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
    return Some(t);
  }
  let rgba = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.to_rgba8();
  let size = [rgba.width() as usize, rgba.height() as usize];
  let img = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
  let opts = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
  let t = ctx.load_texture(name, img, opts);
  ctx.data_mut(|d| d.insert_temp(id, t.clone()));
  Some(t)
}

/// Immagine alta `height` punti, con le proporzioni del PNG.
pub fn image(ui: &mut Ui, name: &'static str, png: &[u8], height: f32, alt: &str) -> Option<Response> {
  let t = texture(ui.ctx(), name, png)?;
  let s = t.size_vec2();
  let size = Vec2::new(s.x * height / s.y.max(1.0), height);
  let resp = ui.add(egui::Image::new((t.id(), size)).alt_text(alt));
  Some(resp)
}

/// Logo dell'azienda.
pub fn logo(ui: &mut Ui, height: f32) -> Option<Response> {
  image(ui, "galbiati-logo", GALBIATI_LOGO, height, "Galbiati Costruzioni Meccaniche")
}

/// Pixel RGBA e dimensioni di un'icona (per l'icona della finestra).
pub fn icon_rgba(png: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
  let rgba = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.to_rgba8();
  let (w, h) = (rgba.width(), rgba.height());
  Some((rgba.into_raw(), w, h))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn embedded_images_decode() {
    for png in [GALBIATI_LOGO, APP_ICON, LAUNCHER_ICON] {
      let (rgba, w, h) = icon_rgba(png).unwrap_or_default();
      assert!(w > 0 && h > 0 && rgba.len() == (w * h * 4) as usize);
    }
  }
}
