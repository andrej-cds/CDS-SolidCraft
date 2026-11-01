//! 1:1 view (CDS fork): the model drawn at its real size on the screen, so a part can be held
//! against the monitor or measured with a ruler. The navigation bar's carrot button turns it on
//! and back off.
//!
//! The screen's millimetres per pixel come from the monitor itself (its EDID, which Windows keeps
//! in the registry), or from Preferences › Navigation › Screen width when that is set (0 means
//! automatic). Elsewhere the preference is the only source.
use crate::SolveApp;
use solidcraft_engine::render::Camera;
use std::cell::Cell;

thread_local! {
    /// The view before 1:1 was turned on: the camera and the perspective setting.
    static PREVIOUS: Cell<Option<(Camera, bool)>> = const { Cell::new(None) };
    /// The window's points per pixel and the monitor's width in physical pixels, from the last frame.
    static SCREEN: Cell<(f32, f32)> = const { Cell::new((1.0, 0.0)) };
}

/// Called every frame with the egui context: what 1:1 needs to know about the screen.
pub(crate) fn note_screen(ctx: &egui::Context) {
    let ppp = ctx.pixels_per_point();
    let monitor = ctx.input(|i| i.viewport().monitor_size).map_or(0.0, |s| s.x * ppp);
    SCREEN.set((ppp, monitor));
}

/// Is the 1:1 view on?
pub(crate) fn active() -> bool {
    PREVIOUS.get().is_some()
}

/// Millimetres per egui point on this screen, if known.
fn mm_per_point(app: &SolveApp) -> Option<f64> {
    let (ppp, monitor_px) = SCREEN.get();
    let width_mm = f64::from(app.preferences.screen_width_mm);
    let mm_per_px = if width_mm > 0.0 && monitor_px > 0.0 {
        width_mm / f64::from(monitor_px)
    } else {
        detected_mm_per_px(monitor_px)?
    };
    (mm_per_px.is_finite() && mm_per_px > 0.0).then(|| mm_per_px * f64::from(ppp))
}

/// Turns the 1:1 view on, or back to the view from before.
pub(crate) fn toggle(app: &mut SolveApp) {
    if let Some((cam, perspective)) = PREVIOUS.take() {
        app.cam = cam;
        app.ui.perspective = perspective;
        return;
    }
    let Some(mm_per_point) = mm_per_point(app) else {
        app.set_status(crate::i18n::tr("ui.true_scale_unknown"), true);
        return;
    };
    let Some(rect) = app.viewport.rect else { return };
    PREVIOUS.set(Some((app.cam, app.ui.perspective)));
    app.ui.perspective = false;
    // Orthographic: the view's height shows 0.8 × distance model millimetres (`Camera::half_height`).
    app.cam.distance = f64::from(rect.height()) * mm_per_point / 0.8;
}

#[cfg(target_os = "windows")]
fn detected_mm_per_px(monitor_px: f32) -> Option<f64> {
    use std::sync::OnceLock;
    static EDIDS: OnceLock<Vec<(f64, f64)>> = OnceLock::new();
    let monitors = EDIDS.get_or_init(read_edids);
    // Prefer the monitor whose width matches this screen; the registry also lists monitors that
    // were connected once.
    monitors
        .iter()
        .find(|(px, _)| (*px - f64::from(monitor_px)).abs() < 0.5)
        .or_else(|| monitors.first())
        .map(|(px, mm)| mm / px)
}

#[cfg(not(target_os = "windows"))]
fn detected_mm_per_px(_monitor_px: f32) -> Option<f64> {
    None
}

/// Every monitor's (active width in pixels, image width in mm) from the EDIDs in the registry.
#[cfg(target_os = "windows")]
fn read_edids() -> Vec<(f64, f64)> {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(out) = std::process::Command::new("reg")
        .args(["query", r"HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY", "/s", "/v", "EDID"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    parse_reg_edids(&String::from_utf8_lossy(&out.stdout))
}

/// The `EDID    REG_BINARY    00FFFF…` lines of `reg query` output, decoded.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_reg_edids(text: &str) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(hex) = line.split_whitespace().skip_while(|w| *w != "REG_BINARY").nth(1) else { continue };
        let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|i| hex.get(2 * i..2 * i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
        if let Some(m) = edid_width(&bytes) {
            out.push(m);
        }
    }
    out
}

/// (horizontal active pixels, horizontal image size in mm) from the first detailed timing.
fn edid_width(edid: &[u8]) -> Option<(f64, f64)> {
    let d = edid.get(54..72)?;
    let pixels = u16::from(d[2]) | (u16::from(d[4] & 0xF0) << 4);
    let mm = u16::from(d[12]) | (u16::from(d[14] & 0xF0) << 4);
    (pixels > 0 && mm > 0).then(|| (f64::from(pixels), f64::from(mm)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_the_width_from_an_edid() {
        let mut edid = vec![0u8; 128];
        // 2560 px active, 597 mm wide: a 27-inch 1440p monitor.
        edid[56] = 0x00;
        edid[58] = 0xA0;
        edid[66] = 0x55;
        edid[68] = 0x20;
        assert_eq!(super::edid_width(&edid), Some((2560.0, 597.0)));
        let hex: String = edid.iter().map(|b| format!("{b:02X}")).collect();
        let text = format!("HKEY_LOCAL_MACHINE\\...\\Device Parameters\n    EDID    REG_BINARY    {hex}\n");
        assert_eq!(super::parse_reg_edids(&text), vec![(2560.0, 597.0)]);
        assert_eq!(super::edid_width(&[0u8; 20]), None);
    }
}
