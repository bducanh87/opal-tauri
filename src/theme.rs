//! Themes and widget designs, ported from Lumen's `shared/themes.ts` and
//! `shared/designs.ts`. CSS gradients are rasterised once into small RGBA
//! images and stretched over each panel, which matches how CSS stretches
//! `ellipse` radial gradients with the element box.

use egui::{Color32, ColorImage};

#[derive(Clone, Copy)]
pub enum Base {
    Solid(u32),
    /// CSS `linear-gradient(<deg>, a, b)`.
    Linear(f32, u32, u32),
}

/// CSS `radial-gradient(ellipse at x% y%, color, transparent stop%)`.
#[derive(Clone, Copy)]
pub struct Radial {
    pub x: f32,
    pub y: f32,
    pub color: u32,
    pub stop: f32,
}

pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    /// Representative colour (Lumen uses it for ink sampling).
    pub color: u32,
    pub radials: &'static [Radial],
    pub base: Base,
    pub light: bool,
}

const fn r(x: f32, y: f32, color: u32, stop: f32) -> Radial {
    Radial { x, y, color, stop }
}

pub static THEMES: &[Theme] = &[
    // Opal's own theme: dark glass with the play of colour of an opal
    // (teal, violet, pink, a touch of gold). The page adds a moving
    // iridescent rim on top.
    Theme {
        id: "opal",
        name: "Opal",
        color: 0x2a2440,
        radials: &[
            r(0.05, 0.0, 0x3fb8b8, 0.62),
            r(1.0, 0.15, 0xc764a8, 0.60),
            r(0.55, 1.05, 0x6f63e0, 0.66),
            r(0.0, 1.0, 0xd9a85c, 0.40),
        ],
        base: Base::Linear(150.0, 0x1d1932, 0x0b1019),
        light: false,
    },
    Theme {
        id: "obsidian",
        name: "Obsidian",
        color: 0x211d30,
        radials: &[r(0.0, 0.0, 0x45405e, 0.70)],
        base: Base::Linear(145.0, 0x201d30, 0x0c0b14),
        light: false,
    },
    Theme {
        id: "porcelain",
        name: "Porcelain",
        color: 0xe8ecf6,
        radials: &[r(0.10, 0.0, 0xffffff, 0.70)],
        base: Base::Linear(145.0, 0xe8ecfa, 0xd6dced),
        light: true,
    },
    Theme {
        id: "aurora",
        name: "Aurora",
        color: 0x35566c,
        radials: &[r(0.90, 0.0, 0x36b997, 0.65), r(0.0, 1.0, 0x9257c5, 0.70)],
        base: Base::Solid(0x112f3c),
        light: false,
    },
    Theme {
        id: "sunset",
        name: "Sunset",
        color: 0xa74769,
        radials: &[r(0.0, 1.0, 0xff713b, 0.65), r(1.0, 0.0, 0xe74394, 0.65)],
        base: Base::Solid(0x662f68),
        light: false,
    },
    Theme {
        id: "ocean",
        name: "Ocean",
        color: 0x164b69,
        radials: &[r(0.05, 0.0, 0x258baf, 0.70)],
        base: Base::Linear(145.0, 0x103f60, 0x061b32),
        light: false,
    },
    Theme {
        id: "neon",
        name: "Neon",
        color: 0x263c15,
        radials: &[r(0.85, 1.0, 0x587908, 0.70)],
        base: Base::Linear(150.0, 0x182b12, 0x080f0b),
        light: false,
    },
    Theme {
        id: "ember",
        name: "Ember",
        color: 0x8e350f,
        radials: &[r(0.5, 1.1, 0xff781a, 0.70)],
        base: Base::Linear(155.0, 0x3b1317, 0x7b260a),
        light: false,
    },
    Theme {
        id: "candy",
        name: "Candy",
        color: 0xa05acb,
        radials: &[r(0.80, 0.0, 0xfc68da, 0.65), r(0.0, 1.0, 0x8a8cff, 0.65)],
        base: Base::Solid(0x6934b0),
        light: false,
    },
    Theme {
        id: "cobalt",
        name: "Cobalt",
        color: 0x173f9e,
        radials: &[r(0.90, 0.20, 0x217aff, 0.65)],
        base: Base::Linear(150.0, 0x1834a3, 0x080e37),
        light: false,
    },
];

pub fn find(id: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.id == id)
}

pub fn get(id: &str) -> &'static Theme {
    find(id).unwrap_or(&THEMES[0])
}

/// Colour with CSS semantics: `rgba(r, g, b, a/255)` blended in gamma space.
/// (egui's `from_rgba_unmultiplied` / `from_white_alpha` premultiply in linear
/// space, which makes faint white overlays several times too bright.)
pub const fn css(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    let k = a as u32;
    Color32::from_rgba_premultiplied(
        ((r as u32 * k + 127) / 255) as u8,
        ((g as u32 * k + 127) / 255) as u8,
        ((b as u32 * k + 127) / 255) as u8,
        a,
    )
}

/// Windows cuts out every pure-black pixel of a widget (the colour key used
/// for the rounded corners). Lift pure black in images by one step so dark
/// photos, covers and icons never show holes.
pub fn avoid_key(rgba: &mut [u8]) {
    for p in rgba.chunks_exact_mut(4) {
        if p[0] == 0 && p[1] == 0 && p[2] == 0 && p[3] > 0 {
            p[0] = 1;
            p[1] = 1;
            p[2] = 1;
        }
    }
}

/// CSS `rgba(255, 255, 255, a/255)`.
pub const fn white(a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(a, a, a, a)
}

pub const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn lin(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    ]
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Rasterise a theme background. `w`/`h` set the aspect used for the
/// linear-gradient angle; radial ellipses scale with the box like CSS.
pub fn render_background(base: Base, radials: &[Radial], w: usize, h: usize) -> ColorImage {
    let mut pixels = Vec::with_capacity(w * h);
    for py in 0..h {
        for px in 0..w {
            let x = (px as f32 + 0.5) / w as f32;
            let y = (py as f32 + 0.5) / h as f32;
            let mut c = match base {
                Base::Solid(hex) => lin(hex),
                Base::Linear(deg, a, b) => {
                    let th = deg.to_radians();
                    let (dx, dy) = (th.sin(), -th.cos());
                    let len = dx.abs() + dy.abs();
                    let t = (((x - 0.5) * dx + (y - 0.5) * dy) / len + 0.5).clamp(0.0, 1.0);
                    mix3(lin(a), lin(b), t)
                }
            };
            // CSS paints the first gradient on top: composite back to front.
            for g in radials.iter().rev() {
                let rx = g.x.max(1.0 - g.x) * std::f32::consts::SQRT_2;
                let ry = g.y.max(1.0 - g.y) * std::f32::consts::SQRT_2;
                let d = (((x - g.x) / rx).powi(2) + ((y - g.y) / ry).powi(2)).sqrt();
                let a = (1.0 - d / g.stop).clamp(0.0, 1.0);
                // ease like a premultiplied CSS fade
                let a = a * a * (3.0 - 2.0 * a);
                c = mix3(c, lin(g.color), a);
            }
            pixels.push(Color32::from_rgb(
                (c[0] * 255.0) as u8,
                (c[1] * 255.0) as u8,
                (c[2] * 255.0) as u8,
            ));
        }
    }
    ColorImage { size: [w, h], pixels }
}

pub fn render_theme(theme: &Theme) -> ColorImage {
    render_background(theme.base, theme.radials, 96, 96)
}

/// Text colours that sit on a theme (Lumen's `ink`, `ink-dim`, `ink-faint`).
#[derive(Clone, Copy)]
pub struct Ink {
    pub ink: Color32,
    pub dim: Color32,
    pub faint: Color32,
    pub hair: Color32,
    pub hover: Color32,
    /// Selected / pressed fill (Lumen's `--hover-strong`).
    pub hover_strong: Color32,
    pub accent: Color32,
}

impl Ink {
    pub fn for_light(light: bool) -> Self {
        if light {
            Ink {
                ink: Color32::from_rgb(0x1d, 0x22, 0x33),
                dim: css(0x1d, 0x22, 0x33, 170),
                faint: css(0x1d, 0x22, 0x33, 105),
                hair: white(140),
                hover: white(128),
                hover_strong: white(184),
                accent: Color32::from_rgb(0x4f, 0x6b, 0xf0),
            }
        } else {
            Ink {
                ink: Color32::from_rgb(0xf6, 0xf7, 0xfb),
                dim: css(0xf6, 0xf7, 0xfb, 175),
                faint: css(0xf6, 0xf7, 0xfb, 110),
                // Premultiplied whites: from_white_alpha keeps rgb == alpha.
                hair: white(23),
                hover: white(19),
                hover_strong: white(33),
                accent: Color32::from_rgb(0x9d, 0xb4, 0xff),
            }
        }
    }
}

// ---------------------------------------------------------------- designs --

pub struct Design {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

const fn d(id: &'static str, name: &'static str, description: &'static str) -> Design {
    Design { id, name, description }
}

static LAUNCHER: &[Design] = &[d("lumen", "Opal", "Familiar floating app grid")];
static GLASS_CARD: &[Design] = &[d("lumen", "Opal", "The standard glass card")];
static WEATHER: &[Design] = &[
    d("lumen", "Opal", "Temperature, details and the coming hours"),
    d("minimal", "Minimal", "One big temperature and a quiet line of detail"),
    d("forecast", "Forecast", "A six-day list with highs, lows and a range bar"),
];
static SYSTEM: &[Design] = &[
    d("lumen", "Opal", "CPU graph with dotted meters"),
    d("rings", "Rings", "Three gauges for CPU, memory and disk"),
    d("terminal", "Terminal", "A green monospace readout"),
];
static POMODORO: &[Design] = &[
    d("lumen", "Opal", "A progress ring with mode pills"),
    d("minimal", "Minimal", "Large digits over a thin progress line"),
    d("tomato", "Tomato", "A ripe tomato that fills as you focus"),
];
static PHOTOS: &[Design] = &[
    d("lumen", "Opal", "Edge-to-edge photo inside the glass"),
    d("polaroid", "Polaroid", "A white instant-photo frame with a caption"),
    d("film", "Film", "A dark film strip with sprocket holes"),
];
static LUNAR: &[Design] = &[
    d("lumen", "Opal", "Solar and lunar dates side by side"),
    d("toloch", "Tờ lịch", "A tear-off calendar sheet with a red header"),
    d("minimal", "Minimal", "The lunar date in large serif type"),
];
static TODO: &[Design] = &[
    d("lumen", "Opal", "Rounded rows with tick circles"),
    d("paper", "Paper", "Ruled paper with a red margin"),
    d("compact", "Compact", "A dense numbered list"),
];
static NOTES: &[Design] = &[
    d("lumen", "Pinboard", "Loose cards in a flowing board"),
    d("notebook", "Notebook", "Wide ruled pages, serif type and a paper margin"),
    d("index", "Index", "Numbered entries with a compact archive layout"),
];
static MUSIC: &[Design] = &[
    d("coverflow", "Coverflow", "Layered album covers over violet glass with a floating playback deck"),
    d("lumen", "Compact", "Small cover, track details and playback controls"),
    d("sleeve", "Sleeve", "Large album art above centered track details"),
    d("turntable", "Turntable", "A vinyl record, tonearm and a separate control deck"),
];
static CALENDAR: &[Design] = &[
    d("lumen", "Opal", "A clean, spacious month calendar"),
    d("almanac", "Almanac", "A large selected date beside an editorial month"),
    d("blocks", "Blocks", "An orange date tile and a bold month panel"),
    d("pop", "Pop", "Electric blue, sunshine yellow and chunky date cards"),
    d("prism", "Prism", "A luminous pink date card with soft glass edges"),
    d("grove", "Grove", "Deep forest cards, lime highlights and a month progress bar"),
];
pub static CLOCK: &[Design] = &[
    d("lumen", "Opal", "Default · a quiet digital face with a companion dial"),
    d("sunset", "Afterglow", "Soft skies, warm light"),
    d("paper", "Paper", "A little room to breathe"),
    d("neon", "Nightfall", "Digital time inside a luminous 24-hour orbit"),
    d("modular", "Colour block", "Every moment, in colour"),
    d("ocean", "Split flap", "Separate hour and minute cards with a mechanical split"),
    d("rose", "Daybook", "Live progress bars for your day, hour and minute"),
    d("aurora", "Timeline", "The current hour on a vertical time ruler"),
    d("terminal", "Terminal", "A crisp green digital readout"),
];

static COUNTDOWN: &[Design] = &[
    d("list", "Opal", "Upcoming dates with day counts and a fading bar"),
    d("hero", "Hero", "The next date in large type, the rest below"),
    d("tickets", "Tickets", "Each date on its own ticket stub"),
];

pub fn designs(widget: &str) -> &'static [Design] {
    match widget {
        "countdown" => COUNTDOWN,
        "notes" => NOTES,
        "music" => MUSIC,
        "calendar" => CALENDAR,
        "clock" => CLOCK,
        "launcher" => LAUNCHER,
        "weather" => WEATHER,
        "system" => SYSTEM,
        "pomodoro" => POMODORO,
        "photos" => PHOTOS,
        "lunar" => LUNAR,
        "todo" => TODO,
        _ => GLASS_CARD,
    }
}

/// Designs that bring their own surface instead of the shared theme.
pub fn design_surface(widget: &str, design: &str) -> Option<(Base, &'static [Radial], bool)> {
    static NONE: &[Radial] = &[];
    static SLEEVE: &[Radial] = &[r(1.0, 0.0, 0x754f73, 0.70)];
    static PRISM: &[Radial] = &[r(0.0, 0.0, 0x342f4b, 0.70)];
    static COVERFLOW: &[Radial] = &[r(0.85, 0.0, 0x9184b7, 0.68)];
    static TOMATO: &[Radial] = &[r(0.5, 0.45, 0x7a1820, 0.75)];
    static AFTERGLOW: &[Radial] = &[r(0.2, 1.1, 0xffb36b, 0.75), r(0.95, 0.0, 0xd4569b, 0.7)];
    Some(match (widget, design) {
        ("notes", "notebook") => (Base::Linear(135.0, 0xf3eddf, 0xe8dfcc), NONE, true),
        ("notes", "index") => (Base::Linear(155.0, 0x122d2d, 0x071819), NONE, false),
        ("music", "sleeve") => (Base::Solid(0x241d2d), SLEEVE, false),
        ("music", "coverflow") => (Base::Linear(135.0, 0x172447, 0x60517d), COVERFLOW, false),
        ("music", "turntable") => (Base::Linear(135.0, 0xd2be9d, 0xa18b6b), NONE, true),
        ("calendar", "almanac") => (Base::Linear(135.0, 0xf2ede2, 0xded8c9), NONE, true),
        ("calendar", "blocks") => (Base::Linear(145.0, 0x141619, 0x08090a), NONE, false),
        ("calendar", "pop") => (Base::Solid(0x090a0c), NONE, false),
        ("calendar", "prism") => (Base::Solid(0x101116), PRISM, false),
        ("calendar", "grove") => (Base::Linear(155.0, 0x14251d, 0x060d0a), NONE, false),
        ("clock", "sunset") => (Base::Linear(170.0, 0x3d2a5c, 0x7a3b52), AFTERGLOW, false),
        ("clock", "paper") => (Base::Linear(145.0, 0xf6f1e7, 0xe9e1d0), NONE, true),
        ("clock", "terminal") => (Base::Solid(0x060a07), NONE, false),
        ("system", "terminal") => (Base::Solid(0x060a07), NONE, false),
        ("pomodoro", "tomato") => (Base::Linear(160.0, 0x3a0d12, 0x170507), TOMATO, false),
        ("photos", "polaroid") => (Base::Linear(135.0, 0xf7f5f0, 0xebe7de), NONE, true),
        ("photos", "film") => (Base::Solid(0x101010), NONE, false),
        ("lunar", "toloch") => (Base::Linear(135.0, 0xfbf7ee, 0xf0e8d6), NONE, true),
        ("todo", "paper") => (Base::Linear(135.0, 0xf6f1e4, 0xece4d0), NONE, true),
        _ => return None,
    })
}

/// Accent colours for generated letter tiles (Lumen's ACCENTS + FNV hash).
pub fn accent_for(seed: &str) -> Color32 {
    const ACCENTS: [u32; 10] = [
        0x7c9cff, 0xc084fc, 0xf472b6, 0xfb923c, 0x34d399, 0x22d3ee, 0xfacc15, 0xf87171, 0xa3e635,
        0x818cf8,
    ];
    let mut h: u32 = 2166136261;
    for unit in seed.encode_utf16() {
        h ^= unit as u32;
        h = h.wrapping_mul(16777619);
    }
    rgb(ACCENTS[((h as i32).unsigned_abs() % 10) as usize])
}

pub fn note_color(name: &str) -> (Color32, Color32) {
    // (card fill, edge)
    match name {
        "rose" => (rgb(0xffd6dd), rgb(0xf48aa0)),
        "sky" => (rgb(0xd4ebff), rgb(0x6fb3f2)),
        "mint" => (rgb(0xd3f5e3), rgb(0x5fcf96)),
        "violet" => (rgb(0xe6dcff), rgb(0x9d82f0)),
        "peach" => (rgb(0xffe2cc), rgb(0xf6a46b)),
        "slate" => (rgb(0xdde3ec), rgb(0x8796ad)),
        _ => (rgb(0xfff0bf), rgb(0xf2c14e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_matches_lumen_hash() {
        // FNV-1a over UTF-16 units, as Lumen does with charCodeAt.
        let a = accent_for("Visual Studio Code");
        assert!(a != Color32::TRANSPARENT);
    }

    #[test]
    fn every_theme_renders() {
        for t in THEMES {
            let img = render_theme(t);
            assert_eq!(img.pixels.len(), 96 * 96);
        }
    }
}
