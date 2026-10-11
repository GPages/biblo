//! Look and feel: a light, warm theme for a primary school, with the Andika
//! font (made by SIL for children learning to read; licence in
//! `assets/fonts/OFL.txt`). Colours are constants so they are easy to tweak.

use std::sync::Arc;

use biblo::search::fold;
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText,
    Shadow, Stroke, TextStyle, Theme,
};

// ----------------------------------------------------------------- palette

/// Window background: warm paper.
pub const PAPER: Color32 = Color32::from_rgb(0xFF, 0xF8, 0xE7);
pub const CARD: Color32 = Color32::WHITE;
/// Text: soft navy, easier on the eyes than pure black.
pub const INK: Color32 = Color32::from_rgb(0x2D, 0x30, 0x47);
pub const LINE: Color32 = Color32::from_rgb(0xEC, 0xDF, 0xC4);
pub const BLUE: Color32 = Color32::from_rgb(0x2F, 0x6F, 0xED);
pub const GREEN: Color32 = Color32::from_rgb(0x1F, 0x9D, 0x6B);
pub const RED: Color32 = Color32::from_rgb(0xD6, 0x28, 0x28);
/// Highlight of the selected choice (sunflower).
pub const SUN: Color32 = Color32::from_rgb(0xFF, 0xD1, 0x66);

/// Badge colours: (background, text).
pub type Badge = (Color32, Color32);
pub const AVAILABLE: Badge = (
    Color32::from_rgb(0xDF, 0xF5, 0xE8),
    Color32::from_rgb(0x1D, 0x7A, 0x46),
);
pub const ON_LOAN: Badge = (
    Color32::from_rgb(0xFF, 0xE6, 0xCC),
    Color32::from_rgb(0xA4, 0x51, 0x0B),
);
pub const INFO: Badge = (
    Color32::from_rgb(0xDC, 0xEB, 0xFF),
    Color32::from_rgb(0x1E, 0x4F, 0xA3),
);
pub const ERROR: Badge = (
    Color32::from_rgb(0xFF, 0xE0, 0xE6),
    Color32::from_rgb(0xA8, 0x23, 0x4A),
);

/// Loan desk: everything about lending is blue and everything about
/// returning is orange, so the two can't be mistaken for each other.
pub const LEND: Color32 = BLUE;
pub const LEND_SOFT: Badge = INFO;
pub const RETURN: Color32 = Color32::from_rgb(0xC2, 0x56, 0x00);
pub const RETURN_SOFT: Badge = ON_LOAN;

const BADGES: [Badge; 8] = [
    (
        Color32::from_rgb(0xDC, 0xEB, 0xFF),
        Color32::from_rgb(0x1E, 0x4F, 0xA3),
    ), // blue
    (
        Color32::from_rgb(0xF1, 0xE3, 0xFF),
        Color32::from_rgb(0x6B, 0x2F, 0xA8),
    ), // purple
    (
        Color32::from_rgb(0xFF, 0xE6, 0xCC),
        Color32::from_rgb(0xA4, 0x51, 0x0B),
    ), // orange
    (
        Color32::from_rgb(0xDF, 0xF5, 0xE8),
        Color32::from_rgb(0x1D, 0x7A, 0x46),
    ), // green
    (
        Color32::from_rgb(0xFF, 0xE0, 0xE6),
        Color32::from_rgb(0xA8, 0x23, 0x4A),
    ), // pink
    (
        Color32::from_rgb(0xFF, 0xF3, 0xBF),
        Color32::from_rgb(0x8A, 0x61, 0x00),
    ), // yellow
    (
        Color32::from_rgb(0xD9, 0xF4, 0xF4),
        Color32::from_rgb(0x12, 0x70, 0x6F),
    ), // teal
    (
        Color32::from_rgb(0xED, 0xE7, 0xDE),
        Color32::from_rgb(0x5B, 0x4A, 0x36),
    ), // brown
];

/// Each category always gets the same colour: fixed ones for the usual
/// categories, a colour derived from the name for the others.
pub fn category_badge(category: &str) -> Badge {
    let key = fold(category);
    let index = match key.as_str() {
        // Green and orange are left out: they already mean "available" / "on loan".
        "roman" | "romans" => 0,
        "bd" | "bande dessinee" | "bandes dessinees" => 1,
        "album" | "albums" => 4,
        "documentaire" | "documentaires" => 6,
        "conte" | "contes" => 5,
        "poesie" => 7,
        _ => key
            .bytes()
            .fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)),
    };
    BADGES[index % BADGES.len()]
}

// ------------------------------------------------------------------ fonts

const BOLD: &str = "andika-bold";

/// Font family for titles, tabs and main buttons.
pub fn bold() -> FontFamily {
    FontFamily::Name(BOLD.into())
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "andika".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Andika-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        BOLD.into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Andika-Bold.ttf"
        ))),
    );
    // Andika first; egui's own fonts stay as fallback for symbols and emoji.
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "andika".into());
    let mut bold_family = vec![BOLD.to_string()];
    bold_family.extend(proportional.iter().cloned());
    fonts.families.insert(bold(), bold_family);
    fonts
}

// ------------------------------------------------------------------ style

pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_theme(Theme::Light);
    ctx.style_mut_of(Theme::Light, |style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(13.0)),
            (TextStyle::Body, FontId::proportional(17.0)),
            (TextStyle::Button, FontId::proportional(17.0)),
            (TextStyle::Heading, FontId::new(24.0, bold())),
            (TextStyle::Monospace, FontId::monospace(15.0)),
        ]
        .into();

        let spacing = &mut style.spacing;
        spacing.item_spacing = egui::vec2(10.0, 8.0);
        spacing.button_padding = egui::vec2(14.0, 5.0);
        spacing.interact_size.y = 30.0;
        // Bigger tick boxes and radio buttons, in proportion with the text.
        spacing.icon_width = 20.0;
        spacing.icon_width_inner = 12.0;

        let v = &mut style.visuals;
        v.panel_fill = PAPER;
        v.window_fill = CARD;
        v.extreme_bg_color = Color32::WHITE;
        v.text_edit_bg_color = Some(Color32::WHITE);
        v.faint_bg_color = Color32::from_rgb(0xFF, 0xF6, 0xE0); // striped rows
        v.hyperlink_color = BLUE;
        v.warn_fg_color = ON_LOAN.1;
        v.error_fg_color = RED;
        v.selection.bg_fill = SUN;
        v.selection.stroke = Stroke::new(1.5, INK);
        v.window_corner_radius = CornerRadius::same(14);
        v.menu_corner_radius = CornerRadius::same(12);
        let soft_shadow = Shadow {
            offset: [0, 4],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(30),
        };
        v.window_shadow = soft_shadow;
        v.popup_shadow = soft_shadow;

        let w = &mut v.widgets;
        for state in [
            &mut w.noninteractive,
            &mut w.inactive,
            &mut w.hovered,
            &mut w.active,
            &mut w.open,
        ] {
            // 6, not more: a rounder tick box would look like a radio button.
            state.corner_radius = CornerRadius::same(6);
            state.fg_stroke.color = INK;
        }
        w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        w.inactive.bg_fill = Color32::WHITE;
        w.inactive.weak_bg_fill = Color32::from_rgb(0xE8, 0xF0, 0xFF);
        w.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0xD5, 0xC6, 0xA3));
        w.hovered.bg_fill = Color32::WHITE;
        w.hovered.weak_bg_fill = Color32::from_rgb(0xD6, 0xE5, 0xFF);
        w.hovered.bg_stroke = Stroke::new(1.5, BLUE);
        w.active.bg_fill = Color32::WHITE;
        w.active.weak_bg_fill = Color32::from_rgb(0xBF, 0xD6, 0xFF);
        w.active.bg_stroke = Stroke::new(1.5, BLUE);
        w.open.weak_bg_fill = Color32::from_rgb(0xD6, 0xE5, 0xFF);
    });
}

// ---------------------------------------------------------------- helpers

/// A white rounded card with a soft shadow, as wide as the space available.
pub fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_with_border(ui, Stroke::new(1.0, LINE), add_contents)
}

/// A card with a thick coloured border, to show which mode a screen is in.
pub fn outlined_card<R>(
    ui: &mut egui::Ui,
    color: Color32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    card_with_border(ui, Stroke::new(3.0, color), add_contents)
}

fn card_with_border<R>(
    ui: &mut egui::Ui,
    border: Stroke,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(border)
        .corner_radius(16)
        .inner_margin(Margin::same(16))
        .shadow(Shadow {
            offset: [0, 2],
            blur: 10,
            spread: 0,
            color: Color32::from_black_alpha(18),
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

/// Small rounded coloured label (status, category).
pub fn badge(ui: &mut egui::Ui, text: &str, (fill, color): Badge) {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(10)
        .inner_margin(Margin::symmetric(8, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).size(15.0));
        });
}

/// Coloured button for the main action of a card.
pub fn main_button<'a>(text: &str, color: Color32) -> egui::Button<'a> {
    egui::Button::new(RichText::new(text).family(bold()).color(Color32::WHITE))
        .fill(color)
        .stroke(Stroke::NONE)
        .corner_radius(12)
}
