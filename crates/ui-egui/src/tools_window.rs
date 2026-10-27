//! Two screens: the application bar, the ribbon, History and the Browser live in a window of
//! their own that can be dragged to another screen, while the main window keeps the model, its
//! dialogs and the feature sheets beside it. Turned on in Preferences › Interface.
//!
//! While the window is open the main window is laid out docked and uses the system's title bar
//! (the application bar, which was the custom title bar, is in the tools window). Both are the
//! frame's, not the user's: `SolveApp::ui` puts the saved layout and title bar back afterwards.

use crate::SolveApp;
use crate::theme::Tokens;
use crate::ui_kit::{Container, Surface};

const MIN_SIZE: [f32; 2] = [640.0, 360.0];
const DEFAULT_SIZE: [f32; 2] = [1100.0, 720.0];

/// What the tools window remembers between frames.
#[derive(Default)]
pub struct State {
    /// The window is shown this frame (the main window leaves those panels out).
    pub open: bool,
    /// The main window currently has the system's title bar because of this mode.
    system_bar: bool,
    /// The window was given its saved place; it is only given it when it opens.
    placed: bool,
    /// Parameters, kept open in the middle of the window.
    params: Option<crate::params_dialog::ParamsDialog>,
}

/// Whether the tools go to a window of their own this frame. Not on the web, where there are no
/// further windows, nor on the start page, nor in an embedded app.
pub fn active(app: &SolveApp, ctx: &egui::Context) -> bool {
    cfg!(not(target_arch = "wasm32")) && app.preferences.tools_window && !app.embedded && !app.home.open && !ctx.embed_viewports()
}

/// Sets up the frame: the main window's layout and title bar for this mode.
pub fn enter(app: &mut SolveApp, ctx: &egui::Context, tools: bool) {
    if !tools && app.tools_window.open {
        // Closed: the next frame opens it at its saved place again.
        app.tools_window.placed = false;
    }
    app.tools_window.open = tools;
    if tools {
        app.ui.layout = crate::shell::LayoutMode::Docked;
        // `custom_titlebar` is still the real one here: remember to give the main window its
        // system title bar, then lay it out as having none of its own.
        if app.custom_titlebar && !app.tools_window.system_bar {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Decorations(true));
            app.tools_window.system_bar = true;
        }
        app.custom_titlebar = false;
    } else if app.tools_window.system_bar {
        if app.custom_titlebar {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Decorations(false));
        }
        app.tools_window.system_bar = false;
    }
}

fn place(saved: Option<[f32; 4]>) -> Option<([f32; 2], [f32; 2])> {
    let [x, y, w, h] = saved?;
    let usable = [x, y, w, h].iter().all(|v| v.is_finite()) && (200.0..=10_000.0).contains(&w) && (150.0..=10_000.0).contains(&h) && x.abs() <= 100_000.0 && y.abs() <= 100_000.0;
    usable.then_some(([x, y], [w, h]))
}

fn builder(app: &SolveApp) -> egui::ViewportBuilder {
    let title = format!("{} — {}", app.session.doc.name, crate::i18n::tr("ui.tools_window"));
    let builder = egui::ViewportBuilder::default().with_title(title).with_min_inner_size(MIN_SIZE);
    if app.tools_window.placed {
        return builder;
    }
    match place(app.preferences.tools_window_place) {
        Some((pos, size)) => builder.with_position(pos).with_inner_size(size),
        None => builder.with_inner_size(DEFAULT_SIZE),
    }
}

/// Draws the tools window. Closing it turns the mode off (Preferences turns it on again).
pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let builder = builder(app);
    app.tools_window.placed = true;
    ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("sc_tools_window"), builder, |ui, _class| {
        // The keys pressed in this window are this window's own input: the main window never
        // sees them, so Ctrl+Z, Ctrl+S and the other shortcuts are handled here as well.
        // TEMPORARY diagnosis: why does Ctrl+Z do nothing here?
        {
            let ctx = ui.ctx();
            let z = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Z, pressed: true, .. })));
            let any = ctx.input(|i| i.events.len());
            if z || any > 0 && ctx.input(|i| i.modifiers.command) {
                eprintln!(
                    "[tools] events={any} z={z} command={} focused={:?} text_edit={} wants_kb={} popup={} file_load_busy={} file_picker={} report_draft={} shown={}",
                    ctx.input(|i| i.modifiers.command),
                    ctx.memory(|m| m.focused()),
                    ctx.text_edit_focused(),
                    ctx.egui_wants_keyboard_input(),
                    egui::Popup::is_any_open(ctx),
                    app.file_load.busy(),
                    app.file_picker.is_some(),
                    app.issue_report.draft.is_some(),
                    crate::toolbar::design_shown(app),
                );
            }
        }
        // A button or a History step that was clicked keeps egui's keyboard focus, and the
        // shortcuts stay quiet while any widget has it: only a text box needs the keys.
        let ctx = ui.ctx().clone();
        if let Some(id) = ctx.memory(|m| m.focused())
            && !ctx.text_edit_focused()
        {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        if app.issue_report.draft.is_none() && app.file_picker.is_none() {
            if !crate::keymap::capturing(app) {
                crate::expression_complete::begin_frame(ui.ctx());
            }
            crate::toolbar::shortcuts(app, ui.ctx());
        }
        let show_timeline = app.ui.show_timeline && !crate::render_workspace::active(app);
        let left_timeline = show_timeline && app.preferences.timeline_left;
        crate::toolbar::app_bar(app, ui);
        crate::toolbar::toolbar(app, ui);
        crate::theme::TabAccent::publish(app, ui.ctx());
        if show_timeline && !left_timeline {
            crate::timeline::timeline(app, ui);
        }
        if left_timeline {
            crate::timeline_left::left_panel(app, ui);
        }
        if app.ui.show_browser {
            crate::browser::browser(app, ui);
        }
        let fill = Tokens::get().toolbar;
        let frame = Container::new(ui.ctx(), Surface::Viewport).edge_frame(fill);
        Container::new(ui.ctx(), Surface::Viewport).central().frame(frame).show(ui, |ui| {
            let mut params = app.tools_window.params.take();
            // The Browser hugs its contents over the left of the middle (and History sits below
            // it): Parameters take what they leave free.
            let content = crate::shell::workspace_content(app, ui.max_rect());
            let mut free = ui.new_child(egui::UiBuilder::new().max_rect(content));
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut free, |ui| {
                crate::params_dialog::docked(app, ui, &mut params);
            });
            app.tools_window.params = params;
        });
        let ctx = ui.ctx().clone();
        let (outer, inner, close) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().inner_rect, i.viewport().close_requested()));
        if let (Some(o), Some(n)) = (outer, inner) {
            let place = [o.min.x, o.min.y, n.width(), n.height()];
            if app.preferences.tools_window_place != Some(place) {
                app.preferences.tools_window_place = Some(place);
            }
        }
        if close {
            app.preferences.tools_window = false;
        }
    });
    // These panels were measured in the tools window's own coordinates: the main window's input
    // blocking and layout must not see them.
    app.shell.browser = None;
    app.shell.history = None;
    app.shell.cards.clear();
    app.shell.panels.clear();
}
