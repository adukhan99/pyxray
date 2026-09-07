//! Argument parsing. Hand-rolled: the surface is small, the help text is the
//! part worth spending effort on, and it keeps the binary's dependency list
//! down to what actually draws pixels.

use pyxray_render::export::{self, Format};
use pyxray_render::layout::{self, Layout};
use pyxray_render::panels::{Icons, Opts};
use pyxray_render::theme::{self, Theme};

pub const HELP: &str = "\
pyx — see what a Python snippet will do before you run it

USAGE
    pyx [OPTIONS] [FILE]
    cat script.py | pyx
    pyx --tui script.py
    pyx watch                 live feed of everything analysed elsewhere

INPUT
    FILE                Python file to read. Omit to read stdin.
    --stdin-is-command  Treat stdin as a shell command and pull the Python out
                        of it (python3 <<'EOF' … EOF, python -c '…').

LOOK
    -t, --theme ID      blueprint | neon | paper | carbon | amber | ansi | mono
    -l, --layout ID     line | auto | card | dashboard | stack | flow
                        auto is one row when it is dull, the card when it is
                        not — the right default when a harness is firing
                        several a second
    -i, --icons MODE    glyph | tag | both            (default: both)
        --depth N       fold the outline below depth N (default: 6)
        --code          show the source alongside the analysis
        --no-gutter     drop line numbers from the outline

OUTPUT
    -f, --format FMT    ansi | text | html | svg | json   (default: ansi)
        --fragment      with --format html, emit the bare <pre> for embedding
    -w, --width N       columns (default: terminal width, else 100)
        --height N      rows; only meaningful for a fixed-size render
    -o, --out PATH      write to a file instead of stdout
        --tui           interactive viewer; cycle themes and layouts live
        --contact-sheet PATH
                        render every theme and layout to one HTML page

FEED
        --emit [PATH]   append this analysis to the feed log as one JSON line
                        (default: $PYXRAY_LOG, else $XDG_RUNTIME_DIR/pyxray)
        --source NAME   label the event with where it came from
        --quiet         emit only; draw nothing

    pyx watch [PATH]    follow the log. Keys: q quit, p pause, f filter,
                        n notes, t theme, c clear.
        --dump          render the log once to stdout and exit
        --replay        start from the whole log, not just what arrives next
        --floor BAND    hide below inert | routine | check | read

RUN
        --exec          run the snippet with python3 after showing the X-ray
        --confirm       with --exec, ask before running
        --gate N        with --exec, refuse to run above this risk score.
                        Off unless you set it: pyxray describes by default and
                        blocks only on request. Note that --gate 0 blocks
                        everything with any effect at all; --gate off is the
                        default and is accepted explicitly.

OTHER
        --list          list the available themes and layouts
    -h, --help          this text
    -V, --version       version
";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Xray,
    Watch,
}

pub struct Args {
    pub mode: Mode,
    pub file: Option<String>,
    pub stdin_is_command: bool,
    pub theme: Theme,
    pub layout: Layout,
    pub format: Format,
    pub json: bool,
    pub fragment: bool,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub out: Option<String>,
    pub tui: bool,
    pub contact_sheet: Option<String>,
    pub list: bool,
    pub opts: Opts,
    pub exec: bool,
    pub confirm: bool,
    pub gate: Option<u8>,
    pub emit: Option<Option<String>>,
    pub source: String,
    pub quiet: bool,
    pub replay: bool,
    pub dump: bool,
    pub floor: pyxray_core::model::Band,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            mode: Mode::Xray,
            file: None,
            stdin_is_command: false,
            theme: theme::default_theme(),
            layout: Layout::Dashboard,
            format: Format::Ansi,
            json: false,
            fragment: false,
            width: None,
            height: None,
            out: None,
            tui: false,
            contact_sheet: None,
            list: false,
            opts: Opts::default(),
            exec: false,
            confirm: false,
            gate: None,
            emit: None,
            source: "cli".to_string(),
            quiet: false,
            replay: false,
            dump: false,
            floor: pyxray_core::model::Band::Inert,
        }
    }
}

pub enum Parsed {
    Run(Box<Args>),
    Help,
    Version,
}

pub fn parse<I: Iterator<Item = String>>(mut it: I) -> Result<Parsed, String> {
    let mut a = Args::default();
    let mut explicit_layout = false;
    let mut first = true;
    while let Some(arg) = it.next() {
        if first {
            first = false;
            if arg == "watch" {
                a.mode = Mode::Watch;
                continue;
            }
        }
        let mut value = |name: &str| -> Result<String, String> {
            it.next().ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            "-V" | "--version" => return Ok(Parsed::Version),
            "--list" => a.list = true,
            "--tui" => a.tui = true,
            "--code" => a.opts.code = true,
            "--no-gutter" => a.opts.gutter = false,
            "--stdin-is-command" => a.stdin_is_command = true,
            "--quiet" => a.quiet = true,
            "--replay" => a.replay = true,
            "--dump" => a.dump = true,
            "--emit" => a.emit = Some(None),
            _ if arg.starts_with("--emit=") => {
                a.emit = Some(Some(arg["--emit=".len()..].to_string()))
            }
            "--source" => a.source = value("--source")?,
            "--floor" => {
                a.floor = match value("--floor")?.as_str() {
                    "inert" => pyxray_core::model::Band::Inert,
                    "routine" => pyxray_core::model::Band::Routine,
                    "check" => pyxray_core::model::Band::Check,
                    "read" => pyxray_core::model::Band::Read,
                    other => return Err(format!("unknown band {other:?}")),
                }
            }
            "--fragment" => a.fragment = true,
            "--exec" => a.exec = true,
            "--confirm" => a.confirm = true,
            "--json" => {
                a.json = true;
            }
            "-t" | "--theme" => {
                let v = value("--theme")?;
                a.theme =
                    theme::theme(&v).ok_or_else(|| format!("unknown theme {v:?}; try --list"))?;
            }
            "-l" | "--layout" => {
                let v = value("--layout")?;
                a.layout = layout::layout(&v)
                    .ok_or_else(|| format!("unknown layout {v:?}; try --list"))?;
                explicit_layout = true;
            }
            "-i" | "--icons" => {
                a.opts.icons = match value("--icons")?.as_str() {
                    "glyph" => Icons::Glyph,
                    "tag" => Icons::Tag,
                    "both" => Icons::Both,
                    other => return Err(format!("unknown icon mode {other:?}")),
                };
            }
            "-f" | "--format" => {
                let v = value("--format")?;
                if v == "json" {
                    a.json = true;
                } else {
                    a.format = export::format(&v).ok_or_else(|| format!("unknown format {v:?}"))?;
                }
            }
            "-w" | "--width" => {
                a.width = Some(parse_num(&value("--width")?, "--width")?);
            }
            "--height" => {
                a.height = Some(parse_num(&value("--height")?, "--height")?);
            }
            "--depth" => {
                a.opts.max_depth = parse_num(&value("--depth")?, "--depth")?;
            }
            "--gate" => {
                // `off` is spelled out because `0` reads like "no gate" and
                // means the exact opposite: block anything with any effect.
                let raw = value("--gate")?;
                a.gate = match raw.as_str() {
                    "off" | "none" | "" => None,
                    other => Some(parse_num::<u8>(other, "--gate")?),
                };
            }
            "-o" | "--out" => a.out = Some(value("--out")?),
            "--contact-sheet" => a.contact_sheet = Some(value("--contact-sheet")?),
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("unknown option {other:?}; try --help"));
            }
            other => a.file = Some(other.to_string()),
        }
    }
    // Defaults follow the situation. Somebody who ran `pyx file.py` is reading
    // a report; somebody about to execute wants a glance that gets out of the
    // way unless it shouldn't.
    if !explicit_layout && a.exec {
        a.layout = Layout::Auto;
    }
    Ok(Parsed::Run(Box::new(a)))
}

fn parse_num<T: std::str::FromStr>(s: &str, name: &str) -> Result<T, String> {
    s.parse::<T>()
        .map_err(|_| format!("{name} needs a number, got {s:?}"))
}
