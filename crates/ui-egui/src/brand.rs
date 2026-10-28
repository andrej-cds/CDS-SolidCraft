//! The CDS brand: the gold of the product name and the round logo drawn in windows.
use egui::{Color32, Rect, pos2};

/// The CDS brand gold, used for the product name.
pub const CDS_GOLD: Color32 = Color32::from_rgb(230, 185, 60);

/// The round CDS logo as a texture, created once per egui context.
pub fn cds_logo(ctx: &egui::Context) -> egui::TextureId {
    let id = egui::Id::new("cds_logo_texture");
    if let Some(h) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return h.id();
    }
    static RAW: &[u8] = include_bytes!("../../../assets/branding/cds/cds-logo-128.rgba");
    let image = egui::ColorImage::from_rgba_unmultiplied([128, 128], RAW);
    let h = ctx.load_texture("cds_logo", image, egui::TextureOptions::LINEAR);
    let tid = h.id();
    ctx.data_mut(|d| d.insert_temp(id, h));
    tid
}

/// Paints the logo into `rect`.
pub fn paint_logo(ui: &egui::Ui, rect: Rect) {
    ui.painter().image(cds_logo(ui.ctx()), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
}
