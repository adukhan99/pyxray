//! pyx — see what a Python snippet will do before you run it.

mod args;
mod sheet;
mod tui;

use std::io::{IsTerminal, Read, Write};
use std::process::ExitCode;

use pyxray_core::{extract_python, xray, Report};
use pyxray_render::export::Format;
use pyxray_render::layout;
use pyxray_render::theme::THEMES;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("pyx: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let a = match args::parse(std::env::args().skip(1))? {
        args::Parsed::Help => {
            print!("{}", args::HELP);
            return Ok(ExitCode::SUCCESS);
        }
        args::Parsed::Version => {
            println!("pyx {VERSION}");
            return Ok(ExitCode::SUCCESS);
        }
        args::Parsed::Run(a) => *a,
    };

    if a.list {
        list();
        return Ok(ExitCode::SUCCESS);
    }

    let (source, name) = read_input(&a)?;
    let report = xray(&source, &name);

    if a.json {
        let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        return write_out(a.out.as_deref(), &text).map(|_| ExitCode::SUCCESS);
    }

    if let Some(path) = &a.contact_sheet {
        let width = a.width.unwrap_or(112);
        let html = sheet::build(&report, width, &a.opts);
        write_out(Some(path), &html)?;
        eprintln!("pyx: wrote {path} ({} bytes)", html.len());
        return Ok(ExitCode::SUCCESS);
    }

    if a.tui {
        tui::run(&report, &a).map_err(|e| e.to_string())?;
        return Ok(ExitCode::SUCCESS);
    }

    let width = a.width.unwrap_or_else(|| term_width().unwrap_or(100));
    let style = pyxray_render::Style {
        theme: a.theme,
        layout: a.layout,
        opts: a.opts,
    };
    // Piping to a file or another program should not carry escape codes
    // unless they were asked for explicitly.
    let piped = a.out.is_some() || !std::io::stdout().is_terminal();
    let format = if piped && a.format == Format::Ansi {
        Format::Text
    } else {
        a.format
    };
    let text = if a.fragment && format == Format::Html {
        let buf = pyxray_render::layout::render(
            &report, &style.theme, &style.opts, style.layout, width, a.height,
        );
        pyxray_render::export::to_html_fragment(&buf, &style.theme)
    } else {
        pyxray_render::render_to_string(&report, &style, format, width, a.height)
    };
    write_out(a.out.as_deref(), &text)?;

    if a.exec {
        return execute(&report, &source, &a);
    }
    Ok(ExitCode::SUCCESS)
}

fn read_input(a: &args::Args) -> Result<(String, String), String> {
    match &a.file {
        Some(path) if path != "-" => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            Ok((text, short_name(path)))
        }
        _ => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("stdin: {e}"))?;
            if a.stdin_is_command {
                let (code, label) = extract_python(&buf);
                Ok((code, label))
            } else {
                Ok((buf, "<stdin>".to_string()))
            }
        }
    }
}

fn short_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn write_out(path: Option<&str>, text: &str) -> Result<(), String> {
    match path {
        Some(p) => std::fs::write(p, text).map_err(|e| format!("{p}: {e}")),
        None => {
            let mut out = std::io::stdout().lock();
            out.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())
        }
    }
}

fn term_width() -> Option<u16> {
    crossterm::terminal::size().ok().map(|(w, _)| w.max(40))
}

fn list() {
    println!("themes:");
    for t in THEMES {
        println!("  {:<10} {}", t.id, t.blurb);
    }
    println!("\nlayouts:");
    for l in layout::Layout::all() {
        println!("  {:<10} {}", l.id(), l.blurb());
    }
    println!("\nformats:\n  ansi  text  html  svg  json");
}

/// Run the snippet, subject to whatever gate the caller asked for. The X-ray
/// has already been printed by this point, which is the whole idea: the
/// decision is made with the analysis on screen.
fn execute(report: &Report, source: &str, a: &args::Args) -> Result<ExitCode, String> {
    if let Some(limit) = a.gate {
        if report.metrics.risk > limit {
            eprintln!(
                "pyx: refusing to run — risk {} exceeds the --gate limit of {limit}",
                report.metrics.risk
            );
            return Ok(ExitCode::from(3));
        }
    }
    if a.confirm {
        if !std::io::stdin().is_terminal() {
            eprintln!("pyx: --confirm needs a terminal; not running");
            return Ok(ExitCode::from(3));
        }
        eprint!("pyx: run this? [y/N] ");
        std::io::stderr().flush().ok();
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            eprintln!("pyx: not running");
            return Ok(ExitCode::from(3));
        }
    }

    use std::process::{Command, Stdio};
    let mut child = Command::new("python3")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("python3: {e}"))?;
    child
        .stdin
        .as_mut()
        .ok_or("python3: no stdin")?
        .write_all(source.as_bytes())
        .map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    Ok(ExitCode::from(
        status.code().unwrap_or(1).clamp(0, 255) as u8
    ))
}
