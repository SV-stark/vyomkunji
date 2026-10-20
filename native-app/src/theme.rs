// Design system for the Signer app: palette, spacing, rounded shapes, shadows.
use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Visuals};

pub fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// App background
pub fn bg() -> Color32 {
    rgb(0x0A, 0x0D, 0x13)
}
/// Top bar / rail surface
pub fn surface() -> Color32 {
    rgb(0x11, 0x16, 0x20)
}
/// Card surface
pub fn card() -> Color32 {
    rgb(0x16, 0x1C, 0x28)
}
/// Card surface (hover / elevated)
pub fn card_hi() -> Color32 {
    rgb(0x1C, 0x24, 0x33)
}
/// Hairline border
pub fn border() -> Color32 {
    rgb(0x24, 0x2D, 0x3E)
}
/// Stronger border / focus
pub fn border_hi() -> Color32 {
    rgb(0x33, 0x3F, 0x55)
}
/// Primary text
pub fn text() -> Color32 {
    rgb(0xE9, 0xED, 0xF5)
}
/// Secondary text
pub fn text_dim() -> Color32 {
    rgb(0x8E, 0x99, 0xAD)
}
/// Faint text
pub fn text_faint() -> Color32 {
    rgb(0x5E, 0x69, 0x7D)
}
/// Brand accent
pub fn accent() -> Color32 {
    rgb(0x4C, 0x8D, 0xFF)
}
/// Secondary accent
pub fn accent2() -> Color32 {
    rgb(0x8B, 0x5C, 0xF6)
}
pub fn ok() -> Color32 {
    rgb(0x2F, 0xD0, 0x7A)
}
pub fn warn() -> Color32 {
    rgb(0xFF, 0xB0, 0x20)
}
pub fn err() -> Color32 {
    rgb(0xFF, 0x5F, 0x5F)
}

/// Tinted background for a colour (used for pills / chips).
pub fn tint(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn radius() -> CornerRadius {
    CornerRadius::same(12)
}
pub fn radius_sm() -> CornerRadius {
    CornerRadius::same(8)
}

pub fn shadow() -> Shadow {
    Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 2,
        color: Color32::from_black_alpha(90),
    }
}

/// Apply the Signer theme to a context.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals = Visuals {
        dark_mode: true,
        panel_fill: surface(),
        window_fill: bg(),
        window_corner_radius: CornerRadius::same(14),
        window_shadow: shadow(),
        window_stroke: Stroke::new(1.0_f32, border()),
        menu_corner_radius: CornerRadius::same(10),
        popup_shadow: shadow(),
        extreme_bg_color: rgb(0x08, 0x0B, 0x10),
        faint_bg_color: surface(),
        code_bg_color: rgb(0x0D, 0x12, 0x1A),
        hyperlink_color: accent(),
        warn_fg_color: warn(),
        error_fg_color: err(),
        selection: egui::style::Selection {
            bg_fill: tint(accent(), 70),
            stroke: Stroke::new(1.0_f32, accent()),
        },
        widgets: egui::style::Widgets {
            noninteractive: egui::style::WidgetVisuals {
                bg_fill: card(),
                weak_bg_fill: card(),
                bg_stroke: Stroke::new(1.0_f32, border()),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, text()),
                expansion: 0.0,
            },
            inactive: egui::style::WidgetVisuals {
                bg_fill: card_hi(),
                weak_bg_fill: card_hi(),
                bg_stroke: Stroke::new(1.0_f32, border_hi()),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, text()),
                expansion: 0.0,
            },
            hovered: egui::style::WidgetVisuals {
                bg_fill: rgb(0x24, 0x2E, 0x40),
                weak_bg_fill: rgb(0x21, 0x2A, 0x3A),
                bg_stroke: Stroke::new(1.0_f32, accent()),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, text()),
                expansion: 0.0,
            },
            active: egui::style::WidgetVisuals {
                bg_fill: rgb(0x2A, 0x35, 0x49),
                weak_bg_fill: rgb(0x26, 0x30, 0x43),
                bg_stroke: Stroke::new(1.0_f32, accent()),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, text()),
                expansion: 0.0,
            },
            open: egui::style::WidgetVisuals {
                bg_fill: rgb(0x24, 0x2E, 0x40),
                weak_bg_fill: rgb(0x21, 0x2A, 0x3A),
                bg_stroke: Stroke::new(1.0_f32, accent()),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, text()),
                expansion: 0.0,
            },
        },
        ..style.visuals
    };

    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.window_margin = Margin::same(18);
    style.spacing.menu_margin = Margin::same(8);
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 8.0,
        floating_allocated_width: 8.0,
        ..style.spacing.scroll
    };

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(24.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.0, FontFamily::Monospace),
    );

    ctx.set_style(style);
}

/// A card container.
pub fn card_frame(fill: Option<Color32>) -> egui::Frame {
    egui::Frame::new()
        .fill(fill.unwrap_or_else(card))
        .stroke(Stroke::new(1.0_f32, border()))
        .corner_radius(radius())
        .inner_margin(Margin::same(16))
        .shadow(shadow())
}
