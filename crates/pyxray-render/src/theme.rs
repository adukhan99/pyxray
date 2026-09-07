//! Visual identity. A theme is a palette, a glyph set and a framing style;
//! everything the layouts draw goes through one of those three, so a new look
//! is a data change rather than a code change.

use pyxray_core::model::{Effect, NodeKind, Severity, ValueKind};
use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// No box at all — panels separated by whitespace only.
    None,
    /// A single thin rule under the title, nothing else.
    Underline,
    /// Full hairline box.
    Hairline,
    /// Full box with rounded corners.
    Rounded,
    /// Heavy box, for themes that want weight.
    Heavy,
    /// Double-ruled box.
    Double,
    /// Corner brackets only — the frame implied rather than drawn.
    Bracket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    Plain,
    Upper,
    /// Upper case with a space between every letter.
    Spaced,
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color,
    /// Background for inset panels; equal to `bg` when the theme is flat.
    pub surface: Color,
    pub fg: Color,
    pub dim: Color,
    pub faint: Color,
    pub rule: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub ok: Color,
    pub warn: Color,
    pub danger: Color,
    /// One colour per [`Effect`], indexed by `Effect as usize`.
    pub effects: [Color; 13],
}

#[derive(Clone, Copy, Debug)]
pub struct Glyphs {
    pub h: char,
    pub v: char,
    pub tl: char,
    pub tr: char,
    pub bl: char,
    pub br: char,
    /// Tree drawing: a branch, the last branch, the continuation pipe, and the
    /// blank that sits under a finished branch.
    pub branch: &'static str,
    pub branch_last: &'static str,
    pub pipe: &'static str,
    pub gap: &'static str,
    pub bullet: char,
    pub arrow: &'static str,
    pub ellipsis: char,
    /// Eight-step ramp for bars and the minimap, lightest first.
    pub ramp: [char; 8],
    /// One character per [`Effect`].
    pub effect_icons: [char; 13],
    /// Severity markers: info, notable, caution.
    pub severity: [char; 3],
    pub check: char,
    pub cross: char,
}

pub const UNICODE: Glyphs = Glyphs {
    h: '\u{2500}',
    v: '\u{2502}',
    tl: '\u{250c}',
    tr: '\u{2510}',
    bl: '\u{2514}',
    br: '\u{2518}',
    branch: "\u{251c}\u{2500} ",
    branch_last: "\u{2514}\u{2500} ",
    pipe: "\u{2502}  ",
    gap: "   ",
    bullet: '\u{2022}',
    arrow: "\u{2192}",
    ellipsis: '\u{2026}',
    ramp: [
        '\u{00b7}', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}',
        '\u{2588}',
    ],
    // Every glyph here is one cell wide in the fonts that matter and is
    // carried by DejaVu, JetBrains Mono, Fira Code and Menlo alike. Prettier
    // candidates from Dingbats and the Arrows block fall back to a different
    // width in a browser, which knocks a whole row of the grid out of line.
    effect_icons: [
        '\u{25c2}', '\u{25b8}', '\u{00d7}', '\u{2248}', '\u{00bb}', '\u{00a7}', '\u{2021}',
        '\u{00b7}', '?', '\u{25cb}', '\u{2261}', '\u{2211}', '\u{25a0}',
    ],
    severity: [' ', '\u{00b7}', '!'],
    check: '\u{221a}',
    cross: '\u{00d7}',
};

pub const ASCII: Glyphs = Glyphs {
    h: '-',
    v: '|',
    tl: '+',
    tr: '+',
    bl: '+',
    br: '+',
    branch: "|- ",
    branch_last: "`- ",
    pipe: "|  ",
    gap: "   ",
    bullet: '*',
    arrow: "->",
    ellipsis: '~',
    ramp: ['.', '_', '_', '-', '-', '=', '=', '#'],
    effect_icons: [
        '<', '>', 'X', '~', '$', 'e', '!', '.', '?', 't', '=', '+', 'x',
    ],
    severity: [' ', '-', '!'],
    check: 'y',
    cross: 'x',
};

const HEAVY: Glyphs = Glyphs {
    h: '\u{2501}',
    v: '\u{2503}',
    tl: '\u{250f}',
    tr: '\u{2513}',
    bl: '\u{2517}',
    br: '\u{251b}',
    branch: "\u{2523}\u{2501} ",
    branch_last: "\u{2517}\u{2501} ",
    pipe: "\u{2503}  ",
    ..UNICODE
};

const ROUND: Glyphs = Glyphs {
    tl: '\u{256d}',
    tr: '\u{256e}',
    bl: '\u{2570}',
    br: '\u{256f}',
    ..UNICODE
};

#[allow(dead_code)]
const DOUBLE: Glyphs = Glyphs {
    h: '\u{2550}',
    v: '\u{2551}',
    tl: '\u{2554}',
    tr: '\u{2557}',
    bl: '\u{255a}',
    br: '\u{255d}',
    ..UNICODE
};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    pub pal: Palette,
    pub gl: Glyphs,
    pub frame: Frame,
    pub heading: Heading,
    /// Blank line between panel rows. Compact themes set this to 0.
    pub breathe: u16,
    /// True when the palette is 24-bit; the 16-colour themes clear it so
    /// exporters know to resolve names rather than read RGB straight off.
    pub truecolor: bool,
    /// Light-ground themes need inverted contrast decisions in places.
    pub light: bool,
}

const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// Technical drawing. Navy ground, cyan hairlines, letterspaced headings.
pub const BLUEPRINT: Theme = Theme {
    id: "blueprint",
    name: "Blueprint",
    blurb: "Technical drawing. Navy ground, cyan hairlines, letterspaced headings.",
    pal: Palette {
        bg: rgb(0x0a1420),
        surface: rgb(0x0f1e2e),
        fg: rgb(0xdce9f5),
        dim: rgb(0x8aa6bf),
        faint: rgb(0x4c6a86),
        rule: rgb(0x1d3448),
        accent: rgb(0x59d2ff),
        accent_alt: rgb(0xffd479),
        ok: rgb(0x6fd08c),
        warn: rgb(0xffc75a),
        danger: rgb(0xff6b6b),
        effects: [
            rgb(0x59d2ff),
            rgb(0xffb454),
            rgb(0xff6b6b),
            rgb(0xb98cff),
            rgb(0xff8f5a),
            rgb(0xffd479),
            rgb(0xff79c6),
            rgb(0x7fbfa5),
            rgb(0x5fd8c4),
            rgb(0x7f9cb5),
            rgb(0x8fa8ff),
            rgb(0x9ede6a),
            rgb(0xff5370),
        ],
    },
    gl: UNICODE,
    frame: Frame::Hairline,
    heading: Heading::Spaced,
    breathe: 1,
    truecolor: true,
    light: false,
};
/// After dark. Near-black ground, saturated magenta and lime, heavy rules.
pub const NEON: Theme = Theme {
    id: "neon",
    name: "Neon",
    blurb: "After dark. Near-black ground, saturated magenta and lime, heavy rules.",
    pal: Palette {
        bg: rgb(0x08070d),
        surface: rgb(0x12101c),
        fg: rgb(0xf2eeff),
        dim: rgb(0x8f86a8),
        faint: rgb(0x675e80),
        rule: rgb(0x2c2740),
        accent: rgb(0xff4fd8),
        accent_alt: rgb(0x4ff0d0),
        ok: rgb(0x7dff9b),
        warn: rgb(0xffd93d),
        danger: rgb(0xff3864),
        effects: [
            rgb(0x36d6ff),
            rgb(0xffa62b),
            rgb(0xff3864),
            rgb(0xc77dff),
            rgb(0xff6b35),
            rgb(0xffe14d),
            rgb(0xff4fd8),
            rgb(0x9be36b),
            rgb(0x4ff0d0),
            rgb(0x6c7a9c),
            rgb(0x6f8dff),
            rgb(0xb4ff39),
            rgb(0xff1f5a),
        ],
    },
    gl: HEAVY,
    frame: Frame::Heavy,
    heading: Heading::Upper,
    breathe: 0,
    truecolor: true,
    light: false,
};
/// Daylight. Warm paper ground, ink type, rules instead of boxes.
pub const PAPER: Theme = Theme {
    id: "paper",
    name: "Paper",
    blurb: "Daylight. Warm paper ground, ink type, rules instead of boxes.",
    pal: Palette {
        bg: rgb(0xf6f2e9),
        surface: rgb(0xefe9dc),
        fg: rgb(0x221f1a),
        dim: rgb(0x6b6455),
        faint: rgb(0x8d8470),
        rule: rgb(0xd3cbb8),
        accent: rgb(0x1f6f6b),
        accent_alt: rgb(0x9a5b1f),
        ok: rgb(0x2f7a3f),
        warn: rgb(0x94660f),
        danger: rgb(0xa52a2a),
        effects: [
            rgb(0x1f6f6b),
            rgb(0x9a5b1f),
            rgb(0xa52a2a),
            rgb(0x5b4b9a),
            rgb(0xa04a1e),
            rgb(0x8a6a10),
            rgb(0x8e2f7a),
            rgb(0x4a5a4a),
            rgb(0x2f7a6a),
            rgb(0x6b6455),
            rgb(0x3a4a8a),
            rgb(0x3f7a2f),
            rgb(0x8a1f3a),
        ],
    },
    gl: UNICODE,
    frame: Frame::Underline,
    heading: Heading::Upper,
    breathe: 1,
    truecolor: true,
    light: true,
};
/// Product UI. Neutral greys, one blue accent, rounded frames, tidy.
pub const CARBON: Theme = Theme {
    id: "carbon",
    name: "Carbon",
    blurb: "Product UI. Neutral greys, one blue accent, rounded frames, tidy.",
    pal: Palette {
        bg: rgb(0x161616),
        surface: rgb(0x1f1f1f),
        fg: rgb(0xf4f4f4),
        dim: rgb(0xa8a8a8),
        faint: rgb(0x6f6f6f),
        rule: rgb(0x393939),
        accent: rgb(0x78a9ff),
        accent_alt: rgb(0x3ddbd9),
        ok: rgb(0x42be65),
        warn: rgb(0xf1c21b),
        danger: rgb(0xfa4d56),
        effects: [
            rgb(0x78a9ff),
            rgb(0xff832b),
            rgb(0xfa4d56),
            rgb(0xbe95ff),
            rgb(0xff7eb6),
            rgb(0xf1c21b),
            rgb(0xee5396),
            rgb(0x8d8d8d),
            rgb(0x3ddbd9),
            rgb(0x6f6f6f),
            rgb(0x4589ff),
            rgb(0x42be65),
            rgb(0xda1e28),
        ],
    },
    gl: ROUND,
    frame: Frame::Rounded,
    heading: Heading::Plain,
    breathe: 1,
    truecolor: true,
    light: false,
};
/// Phosphor CRT. One hue, brightness does the work, bracketed panels.
pub const AMBER: Theme = Theme {
    id: "amber",
    name: "Amber",
    blurb: "Phosphor CRT. One hue, brightness does the work, bracketed panels.",
    pal: Palette {
        bg: rgb(0x140d03),
        surface: rgb(0x1c1305),
        fg: rgb(0xffb642),
        dim: rgb(0xb87d24),
        faint: rgb(0x895c18),
        rule: rgb(0x4a3210),
        accent: rgb(0xffd48a),
        accent_alt: rgb(0xff8c1a),
        ok: rgb(0xffc75a),
        warn: rgb(0xff9a1f),
        danger: rgb(0xff5722),
        effects: [
            rgb(0xffb642),
            rgb(0xff9a1f),
            rgb(0xff5722),
            rgb(0xffd48a),
            rgb(0xff7a00),
            rgb(0xffc75a),
            rgb(0xff6f3c),
            rgb(0xb87d24),
            rgb(0xe0a040),
            rgb(0x895c18),
            rgb(0xd99a30),
            rgb(0xffe0a0),
            rgb(0xff3d00),
        ],
    },
    gl: UNICODE,
    frame: Frame::Bracket,
    heading: Heading::Spaced,
    breathe: 0,
    truecolor: true,
    light: false,
};
/// Sixteen colours only. Survives any terminal, tmux and SSH session.
pub const ANSI: Theme = Theme {
    id: "ansi",
    name: "ANSI",
    blurb: "Sixteen colours only. Survives any terminal, tmux and SSH session.",
    pal: Palette {
        bg: Color::Reset,
        surface: Color::Reset,
        fg: Color::Gray,
        dim: Color::DarkGray,
        faint: Color::DarkGray,
        rule: Color::DarkGray,
        accent: Color::Cyan,
        accent_alt: Color::Yellow,
        ok: Color::Green,
        warn: Color::Yellow,
        danger: Color::Red,
        effects: [
            Color::Cyan,
            Color::Yellow,
            Color::Red,
            Color::Magenta,
            Color::LightRed,
            Color::Yellow,
            Color::LightMagenta,
            Color::Gray,
            Color::LightCyan,
            Color::DarkGray,
            Color::Blue,
            Color::Green,
            Color::LightRed,
        ],
    },
    gl: UNICODE,
    frame: Frame::Hairline,
    heading: Heading::Upper,
    breathe: 0,
    truecolor: false,
    light: false,
};
/// No colour at all. Structure carried entirely by glyphs and weight.
pub const MONO: Theme = Theme {
    id: "mono",
    name: "Mono",
    blurb: "No colour at all. Structure carried entirely by glyphs and weight.",
    pal: Palette {
        bg: Color::Reset,
        surface: Color::Reset,
        fg: Color::Reset,
        dim: Color::Reset,
        faint: Color::Reset,
        rule: Color::Reset,
        accent: Color::Reset,
        accent_alt: Color::Reset,
        ok: Color::Reset,
        warn: Color::Reset,
        danger: Color::Reset,
        effects: [
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
        ],
    },
    gl: ASCII,
    frame: Frame::Hairline,
    heading: Heading::Upper,
    breathe: 0,
    truecolor: false,
    light: false,
};

pub const THEMES: &[Theme] = &[BLUEPRINT, NEON, PAPER, CARBON, AMBER, ANSI, MONO];

pub fn theme(id: &str) -> Option<Theme> {
    THEMES.iter().copied().find(|t| t.id == id)
}

pub fn default_theme() -> Theme {
    BLUEPRINT
}

impl Theme {
    pub fn base(&self) -> Style {
        Style::default().fg(self.pal.fg).bg(self.pal.bg)
    }
    pub fn fg(&self, c: Color) -> Style {
        Style::default().fg(c)
    }
    pub fn dim(&self) -> Style {
        Style::default().fg(self.pal.dim)
    }
    pub fn faint(&self) -> Style {
        Style::default().fg(self.pal.faint)
    }
    pub fn rule_style(&self) -> Style {
        Style::default().fg(self.pal.rule)
    }
    pub fn accent(&self) -> Style {
        Style::default().fg(self.pal.accent)
    }
    pub fn strong(&self) -> Style {
        Style::default()
            .fg(self.pal.fg)
            .add_modifier(Modifier::BOLD)
    }
    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.pal.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn effect_color(&self, e: Effect) -> Color {
        self.pal.effects[e as usize]
    }

    pub fn effect_icon(&self, e: Effect) -> char {
        self.gl.effect_icons[e as usize]
    }

    pub fn severity_color(&self, s: Severity) -> Color {
        match s {
            Severity::Info => self.pal.dim,
            Severity::Notable => self.pal.warn,
            Severity::Caution => self.pal.danger,
        }
    }

    pub fn severity_mark(&self, s: Severity) -> char {
        self.gl.severity[s as usize]
    }

    /// Colour for a structural node: control flow reads warm, definitions read
    /// as the accent, and plain data movement stays neutral.
    pub fn node_color(&self, kind: NodeKind) -> Color {
        match kind {
            NodeKind::Module => self.pal.accent,
            NodeKind::Def | NodeKind::Class => self.pal.accent,
            NodeKind::Loop
            | NodeKind::Branch
            | NodeKind::BranchArm
            | NodeKind::Try
            | NodeKind::Handler => self.pal.accent_alt,
            NodeKind::Import => self.pal.faint,
            NodeKind::Return | NodeKind::Raise | NodeKind::Jump => self.pal.warn,
            NodeKind::Assign
            | NodeKind::Call
            | NodeKind::Expr
            | NodeKind::With
            | NodeKind::Assert
            | NodeKind::Delete
            | NodeKind::Global
            | NodeKind::Pass => self.pal.fg,
        }
    }

    pub fn value_color(&self, v: ValueKind) -> Color {
        match v {
            ValueKind::File | ValueKind::Path => self.effect_color(Effect::FsRead),
            ValueKind::Response | ValueKind::Session | ValueKind::Socket => {
                self.effect_color(Effect::Net)
            }
            ValueKind::Process => self.effect_color(Effect::Process),
            ValueKind::Frame | ValueKind::Array | ValueKind::Tensor => {
                self.effect_color(Effect::Compute)
            }
            ValueKind::Callable | ValueKind::Class => self.pal.accent,
            ValueKind::Unknown => self.pal.faint,
            _ => self.pal.dim,
        }
    }

    /// A risk score, coloured by band rather than by continuous gradient — the
    /// bands are the thing a reader acts on.
    pub fn risk_color(&self, risk: u8) -> Color {
        match risk {
            0..=9 => self.pal.dim,
            10..=29 => self.pal.ok,
            30..=59 => self.pal.warn,
            _ => self.pal.danger,
        }
    }

    pub fn risk_word(&self, risk: u8) -> &'static str {
        match risk {
            0..=9 => "inert",
            10..=29 => "routine",
            30..=59 => "check it",
            _ => "read it first",
        }
    }

    /// Apply the theme's heading transform.
    pub fn head(&self, s: &str) -> String {
        match self.heading {
            Heading::Plain => s.to_string(),
            Heading::Upper => s.to_uppercase(),
            Heading::Spaced => s
                .to_uppercase()
                .chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    /// Pick a ramp character for a 0..=1 fraction.
    pub fn ramp(&self, frac: f32) -> char {
        let n = self.gl.ramp.len();
        let idx = ((frac.clamp(0.0, 1.0) * (n - 1) as f32).round()) as usize;
        self.gl.ramp[idx.min(n - 1)]
    }
}

/// Short word for an effect, used where a glyph would be too terse.
pub fn effect_tag(e: Effect) -> &'static str {
    match e {
        Effect::FsRead => "read",
        Effect::FsWrite => "write",
        Effect::FsDelete => "delete",
        Effect::Net => "net",
        Effect::Process => "shell",
        Effect::Env => "env",
        Effect::Dynamic => "eval",
        Effect::Stdout => "print",
        Effect::Random => "rand",
        Effect::Clock => "time",
        Effect::Concurrency => "par",
        Effect::Compute => "calc",
        Effect::Exit => "exit",
    }
}
