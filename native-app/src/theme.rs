// Signer's colour system: a fastframe `Palette` implementation (JSON-editable,
// light + dark) mapped onto egui's visuals.
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Theme, Visuals,
};
use std::collections::BTreeSet;
use std::sync::RwLock;

/// The colours a Signer screen is made of. The names match
/// `fastframe_theme::BASE_COLORS`, so a JSON palette file can set any of them.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub base: fastframe_theme::Base,
    pub window: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub surface_active: Color32,
    pub outline: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub overlay: Color32,
    pub shadow: Color32,
    /// App-only colours, not part of the shared base set.
    pub accent2: Color32,
    pub ok: Color32,
}

impl fastframe_theme::Palette for Palette {
    fn base(base: fastframe_theme::Base) -> Self {
        match base {
            fastframe_theme::Base::Dark => Palette::dark(),
            fastframe_theme::Base::Light => Palette::light(),
        }
    }

    fn set(&mut self, name: &str, color: Color32) -> bool {
        match name {
            "window" => self.window = color,
            "panel" => self.panel = color,
            "surface" => self.surface = color,
            "surface_hover" => self.surface_hover = color,
            "surface_active" => self.surface_active = color,
            "outline" => self.outline = color,
            "text" => self.text = color,
            "secondary" => self.secondary = color,
            "dim" => self.dim = color,
            "accent" => self.accent = color,
            "accent_hover" => self.accent_hover = color,
            "on_accent" => self.on_accent = color,
            "danger" => self.danger = color,
            "warning" => self.warning = color,
            "overlay" => self.overlay = color,
            "shadow" => self.shadow = color,
            _ => return false,
        }
        true
    }

    fn derive(&mut self, given: &BTreeSet<&str>) {
        if given.contains("accent") && !given.contains("accent_hover") {
            self.accent_hover = lighten(self.accent, 0.12);
        }
        if given.contains("surface") && !given.contains("surface_hover") {
            self.surface_hover = lighten(self.surface, 0.06);
        }
    }
}

impl fastframe_theme::PaletteBase for Palette {
    fn palette_base(&self) -> fastframe_theme::Base {
        self.base
    }
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            base: fastframe_theme::Base::Dark,
            window: rgb(0x0A, 0x0D, 0x13),
            panel: rgb(0x11, 0x16, 0x20),
            surface: rgb(0x16, 0x1C, 0x28),
            surface_hover: rgb(0x1C, 0x24, 0x33),
            surface_active: rgb(0x24, 0x2E, 0x40),
            outline: rgb(0x24, 0x2D, 0x3E),
            text: rgb(0xE9, 0xED, 0xF5),
            secondary: rgb(0x8E, 0x99, 0xAD),
            dim: rgb(0x5E, 0x69, 0x7D),
            accent: rgb(0x4C, 0x8D, 0xFF),
            accent_hover: rgb(0x6C, 0xA3, 0xFF),
            on_accent: Color32::WHITE,
            danger: rgb(0xFF, 0x5F, 0x5F),
            warning: rgb(0xFF, 0xB0, 0x20),
            overlay: rgb(0x08, 0x0B, 0x10),
            shadow: Color32::from_black_alpha(90),
            accent2: rgb(0x8B, 0x5C, 0xF6),
            ok: rgb(0x2F, 0xD0, 0x7A),
        }
    }

    pub fn light() -> Self {
        Self {
            base: fastframe_theme::Base::Light,
            window: rgb(0xF7, 0xF8, 0xFA),
            panel: rgb(0xFF, 0xFF, 0xFF),
            surface: rgb(0xFF, 0xFF, 0xFF),
            surface_hover: rgb(0xF1, 0xF3, 0xF7),
            surface_active: rgb(0xE8, 0xEC, 0xF2),
            outline: rgb(0xDF, 0xE4, 0xEC),
            text: rgb(0x12, 0x18, 0x22),
            secondary: rgb(0x5B, 0x66, 0x78),
            dim: rgb(0x8B, 0x95, 0xA6),
            accent: rgb(0x25, 0x63, 0xEB),
            accent_hover: rgb(0x1D, 0x54, 0xC7),
            on_accent: Color32::WHITE,
            danger: rgb(0xD9, 0x3F, 0x3F),
            warning: rgb(0xC1, 0x7D, 0x0A),
            overlay: rgb(0xE9, 0xED, 0xF3),
            shadow: Color32::from_black_alpha(28),
            accent2: rgb(0x7C, 0x3A, 0xED),
            ok: rgb(0x18, 0x8A, 0x4B),
        }
    }
}

/// The installed dark and light palettes, or the built-in ones.
pub fn palettes() -> (Palette, Palette) {
    let guard = PALETTES.read().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        Some(p) => (p.dark.clone(), p.light.clone()),
        None => (Palette::dark(), Palette::light()),
    }
}

/// Read a user palette from `%APPDATA%\Signer\theme.json`, if it is there.
pub fn load_user_palette() -> Option<Palette> {
    let path = std::env::var("APPDATA").ok().map(|a| {
        std::path::PathBuf::from(a)
            .join("Signer")
            .join("theme.json")
    })?;
    let text = std::fs::read_to_string(path).ok()?;
    fastframe_theme::parse_palette::<Palette>(&text).ok()
}

fn lighten(c: Color32, amount: f32) -> Color32 {
    let f = |v: u8| -> u8 {
        let v = v as f32 + 255.0 * amount;
        v.min(255.0) as u8
    };
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

// ------------------------------------------------------------- global state

struct Palettes {
    dark: Palette,
    light: Palette,
    active: fastframe_theme::Base,
}

static PALETTES: RwLock<Option<Palettes>> = RwLock::new(None);

/// The palette currently in use.
pub fn current() -> Palette {
    let guard = PALETTES.read().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        Some(p) => match p.active {
            fastframe_theme::Base::Dark => p.dark.clone(),
            fastframe_theme::Base::Light => p.light.clone(),
        },
        None => Palette::dark(),
    }
}

/// Install the dark and light palettes, and pick the active one.
pub fn install(dark: Palette, light: Palette, active: fastframe_theme::Base) {
    let mut guard = PALETTES.write().unwrap_or_else(|e| e.into_inner());
    *guard = Some(Palettes {
        dark,
        light,
        active,
    });
}

/// Switch between the light and dark palettes.
pub fn set_base(base: fastframe_theme::Base) {
    let mut guard = PALETTES.write().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = guard.as_mut() {
        p.active = base;
    }
}

pub fn active_base() -> fastframe_theme::Base {
    let guard = PALETTES.read().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .map(|p| p.active)
        .unwrap_or(fastframe_theme::Base::Dark)
}

// --------------------------------------------------------------- accessors

pub fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
pub fn bg() -> Color32 {
    current().window
}
pub fn surface() -> Color32 {
    current().panel
}
pub fn card() -> Color32 {
    current().surface
}
pub fn card_hi() -> Color32 {
    current().surface_hover
}
pub fn border() -> Color32 {
    current().outline
}
pub fn text() -> Color32 {
    current().text
}
pub fn text_dim() -> Color32 {
    current().secondary
}
pub fn text_faint() -> Color32 {
    current().dim
}
pub fn accent() -> Color32 {
    current().accent
}
pub fn accent2() -> Color32 {
    current().accent2
}
pub fn ok() -> Color32 {
    current().ok
}
pub fn warn() -> Color32 {
    current().warning
}
pub fn err() -> Color32 {
    current().danger
}

/// Tinted background for a colour (pills, chips, soft fills).
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
    let c = current().shadow;
    Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 2,
        color: c,
    }
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

// ------------------------------------------------------------------ styling

fn style_for(p: &Palette) -> egui::Style {
    let mut style = egui::Style::default();
    style.visuals = Visuals {
        dark_mode: p.base == fastframe_theme::Base::Dark,
        panel_fill: p.panel,
        window_fill: p.window,
        window_corner_radius: CornerRadius::same(14),
        window_shadow: Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 2,
            color: p.shadow,
        },
        window_stroke: Stroke::new(1.0_f32, p.outline),
        menu_corner_radius: CornerRadius::same(10),
        popup_shadow: Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 2,
            color: p.shadow,
        },
        extreme_bg_color: p.overlay,
        faint_bg_color: p.panel,
        code_bg_color: p.window,
        hyperlink_color: p.accent,
        warn_fg_color: p.warning,
        error_fg_color: p.danger,
        selection: egui::style::Selection {
            bg_fill: tint(p.accent, 70),
            stroke: Stroke::new(1.0_f32, p.accent),
        },
        widgets: egui::style::Widgets {
            noninteractive: egui::style::WidgetVisuals {
                bg_fill: p.surface,
                weak_bg_fill: p.surface,
                bg_stroke: Stroke::new(1.0_f32, p.outline),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, p.text),
                expansion: 0.0,
            },
            inactive: egui::style::WidgetVisuals {
                bg_fill: p.surface_hover,
                weak_bg_fill: p.surface_hover,
                bg_stroke: Stroke::new(1.0_f32, p.outline),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, p.text),
                expansion: 0.0,
            },
            hovered: egui::style::WidgetVisuals {
                bg_fill: p.surface_active,
                weak_bg_fill: p.surface_active,
                bg_stroke: Stroke::new(1.0_f32, p.accent),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, p.text),
                expansion: 0.0,
            },
            active: egui::style::WidgetVisuals {
                bg_fill: p.surface_active,
                weak_bg_fill: p.surface_active,
                bg_stroke: Stroke::new(1.0_f32, p.accent),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, p.text),
                expansion: 0.0,
            },
            open: egui::style::WidgetVisuals {
                bg_fill: p.surface_active,
                weak_bg_fill: p.surface_active,
                bg_stroke: Stroke::new(1.0_f32, p.accent),
                corner_radius: radius_sm(),
                fg_stroke: Stroke::new(1.0_f32, p.text),
                expansion: 0.0,
            },
        },
        ..style.visuals
    };

    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.window_margin = Margin::same(18);
    style.spacing.menu_margin = Margin::same(8);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating_allocated_width = 8.0;

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

    style
}

/// Apply the Signer theme (both palettes) to a context.
pub fn apply(ctx: &egui::Context) {
    let (dark, light) = palettes();
    ctx.set_style_of(Theme::Dark, style_for(&dark));
    ctx.set_style_of(Theme::Light, style_for(&light));
    let theme = match active_base() {
        fastframe_theme::Base::Dark => Theme::Dark,
        fastframe_theme::Base::Light => Theme::Light,
    };
    ctx.set_theme(theme);
}
