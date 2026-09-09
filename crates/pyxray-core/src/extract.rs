//! Finding the Python inside a shell command.
//!
//! An agent does not hand you Python; it hands you a *command* — `python3
//! script.py`, `cd build && python3 -c '…'`, `python3 - <<'EOF' … EOF`,
//! `bash -c "…"`, `cat gen.py | python3`. The analyser cannot look at what it
//! never receives, and a hook that only recognises one spelling is a hook an
//! agent walks straight past without meaning to. So this is a small,
//! deliberate shell reader: it splits the command the way `sh` would (quotes,
//! escapes, operators, redirections, heredocs), strips the wrappers people put
//! in front of an interpreter (`env`, `sudo`, `nohup`, `VAR=1`, `uv run`), and
//! reads the interpreter's own options the way `python` does (`-c`, `-m`,
//! `-`, flags that take a value, clustered flags).
//!
//! It is the single implementation: the Python package calls this through the
//! extension or the `pyx extract` subcommand rather than keeping a twin.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How to go about it.
#[derive(Clone, Debug)]
pub struct ExtractOpts {
    /// Directory relative paths resolve against. A harness usually knows it
    /// (Claude Code sends `cwd` with every tool call); when it is `None`,
    /// paths resolve against the process's own working directory.
    pub cwd: Option<PathBuf>,
    /// Whether to read script files off disk. Off, a script path comes back
    /// with an empty `source` and its resolved `path`.
    pub read_files: bool,
    /// Largest file worth reading. Bigger ones come back empty and labelled.
    pub max_bytes: u64,
    /// How many levels of `sh -c "…"` to look inside.
    pub depth: u8,
}

impl Default for ExtractOpts {
    fn default() -> Self {
        ExtractOpts {
            cwd: None,
            read_files: true,
            max_bytes: 1 << 20,
            depth: 1,
        }
    }
}

/// One piece of Python found in a command.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Extracted {
    /// The Python. Empty when we know *where* it is but could not get it (a
    /// missing or oversized file, a pipe from the network); the label says.
    pub source: String,
    /// Where it came from: `python -c`, `heredoc <<EOF`, `script.py`,
    /// `script.py (unreadable)`, `<pipe> curl`.
    pub label: String,
    /// The file it was read from, when it was one.
    pub path: Option<PathBuf>,
    /// The command segment it was found in, for the feed's name column.
    pub segment: String,
}

/// What an interpreter's own argument list asks it to run.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Invocation {
    /// `-c CODE`.
    pub code: Option<String>,
    /// `-m MODULE` — the source is not in the command, and is not read.
    pub module: Option<String>,
    /// The first non-option argument.
    pub script: Option<String>,
    /// `-`, or nothing at all: the program comes from standard input.
    pub stdin: bool,
}

/// Every piece of Python a command would run, in command order.
pub fn extract_all(command: &str, opts: &ExtractOpts) -> Vec<Extracted> {
    let mut out = Vec::new();
    let segments = segments(&lex(command));
    let mut cwd = opts.cwd.clone();
    for seg in segments.iter() {
        let (words, wrapper_cwd) = strip_wrappers(&seg.words);
        // `cd dir && python3 x.py`: later segments resolve against `dir`.
        if let Some(dir) = wrapper_cwd {
            let dir = expand_home(&dir);
            cwd = Some(match &cwd {
                Some(base) if !Path::new(&dir).is_absolute() => base.join(&dir),
                _ => PathBuf::from(&dir),
            });
            continue;
        }
        if words.is_empty() {
            continue;
        }
        let segment_text = words.join(" ");
        match classify(&words) {
            Command::Python(args) => {
                let inv = classify_argv(&args);
                let piped_from = seg.piped_from.map(|p| &segments[p]);
                for item in sources(&inv, seg, piped_from, cwd.as_deref(), opts) {
                    out.push(Extracted {
                        segment: segment_text.clone(),
                        ..item
                    });
                }
            }
            Command::Shell(inner) if opts.depth > 0 => {
                let nested = ExtractOpts {
                    cwd: cwd.clone(),
                    depth: opts.depth - 1,
                    ..opts.clone()
                };
                out.extend(extract_all(&inner, &nested));
            }
            Command::Shell(_) | Command::Other => {}
        }
    }
    out
}

/// The first piece of Python in a command, or the command itself when it is
/// not recognisably a wrapped snippet — on the assumption it was Python all
/// along. Kept for callers that want one answer.
pub fn extract_python(input: &str) -> (String, String) {
    extract_all(input, &ExtractOpts::default())
        .into_iter()
        .find(|e| !e.source.trim().is_empty())
        .map(|e| (e.source, e.label))
        .unwrap_or_else(|| (input.to_string(), "<stdin>".to_string()))
}

// ------------------------------------------------------------------ lexer

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    Word(String),
    /// `&&`, `||`, `;`, `|`, `&`, newline, `(`, `)`.
    Sep(&'static str),
    /// A heredoc body, tagged with the ordinal of the segment whose command
    /// declared it (the body arrives after the newline that ends that line).
    Heredoc {
        seg: usize,
        body: String,
        delim: String,
        terminated: bool,
    },
    /// `< file`.
    StdinFrom(String),
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    toks: Vec<Tok>,
    /// Segment ordinal, for attaching heredoc bodies to the right command.
    seg: usize,
    /// Heredocs declared on the current line: (delimiter, strip leading tabs, segment).
    pending: Vec<(String, bool, usize)>,
}

fn is_meta(c: char) -> bool {
    matches!(c, '&' | '|' | ';' | '<' | '>' | '(' | ')' | '\n')
}

impl Lexer {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn peek_at(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }
    fn skip_blanks(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\r')) {
            self.pos += 1;
        }
    }
    fn sep(&mut self, s: &'static str) {
        self.toks.push(Tok::Sep(s));
        self.seg += 1;
    }

    /// One shell word, quotes and escapes resolved, stopping at unquoted
    /// whitespace or a metacharacter. Returns `None` at a metacharacter.
    fn word(&mut self) -> Option<String> {
        let mut out = String::new();
        let mut any = false;
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\r' => break,
                c if is_meta(c) => break,
                '\'' => {
                    self.pos += 1;
                    any = true;
                    while let Some(c) = self.bump() {
                        if c == '\'' {
                            break;
                        }
                        out.push(c);
                    }
                }
                '"' => {
                    self.pos += 1;
                    any = true;
                    while let Some(c) = self.bump() {
                        match c {
                            '"' => break,
                            '\\' => match self.peek() {
                                Some(n @ ('"' | '\\' | '$' | '`')) => {
                                    out.push(n);
                                    self.pos += 1;
                                }
                                Some('\n') => {
                                    self.pos += 1;
                                }
                                _ => out.push('\\'),
                            },
                            c => out.push(c),
                        }
                    }
                }
                '\\' => {
                    self.pos += 1;
                    match self.peek() {
                        // Line continuation.
                        Some('\n') => {
                            self.pos += 1;
                        }
                        // Escaping a character that means something to the
                        // shell yields that character. Escaping an ordinary
                        // one keeps the backslash, which is what keeps a
                        // `C:\Users\…` path from being mangled.
                        Some(n)
                            if n.is_whitespace()
                                || matches!(n, '\'' | '"' | '\\' | '$' | '`' | '#')
                                || is_meta(n) =>
                        {
                            out.push(n);
                            self.pos += 1;
                            any = true;
                        }
                        Some(_) => {
                            out.push('\\');
                            any = true;
                        }
                        None => {
                            any = true;
                        }
                    }
                }
                '`' => {
                    // Command substitution is opaque: keep it as text.
                    self.pos += 1;
                    out.push('`');
                    any = true;
                    while let Some(c) = self.bump() {
                        out.push(c);
                        if c == '`' {
                            break;
                        }
                    }
                }
                '$' if self.peek_at(1) == Some('(') => {
                    // `$( … )` — balanced, opaque.
                    let mut depth = 0usize;
                    any = true;
                    while let Some(c) = self.bump() {
                        out.push(c);
                        match c {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                c => {
                    out.push(c);
                    self.pos += 1;
                    any = true;
                }
            }
        }
        any.then_some(out)
    }

    /// After the newline that ends a line with `<<` on it, the bodies follow,
    /// one per declaration, in order.
    fn read_heredoc_bodies(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for (delim, strip_tabs, seg) in pending {
            let mut body = String::new();
            let mut terminated = false;
            while self.pos < self.chars.len() {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == '\n' {
                        break;
                    }
                }
                let raw: String = self.chars[start..self.pos].iter().collect();
                let line = raw.trim_end_matches(['\n', '\r']);
                let candidate = if strip_tabs {
                    line.trim_start_matches('\t')
                } else {
                    line
                };
                if candidate == delim {
                    terminated = true;
                    break;
                }
                if strip_tabs {
                    body.push_str(line.trim_start_matches('\t'));
                    body.push('\n');
                } else {
                    body.push_str(&raw);
                    if !raw.ends_with('\n') {
                        body.push('\n');
                    }
                }
            }
            self.toks.push(Tok::Heredoc {
                seg,
                body,
                delim,
                terminated,
            });
        }
    }

    fn run(mut self) -> Vec<Tok> {
        loop {
            self.skip_blanks();
            let Some(c) = self.peek() else { break };
            match c {
                '\n' => {
                    self.pos += 1;
                    if !self.pending.is_empty() {
                        self.read_heredoc_bodies();
                    }
                    self.sep("\n");
                }
                '#' => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                '&' => {
                    self.pos += 1;
                    match self.peek() {
                        Some('&') => {
                            self.pos += 1;
                            self.sep("&&");
                        }
                        Some('>') => {
                            // `&> file`: a redirection, not a background op.
                            self.pos += 1;
                            if self.peek() == Some('>') {
                                self.pos += 1;
                            }
                            self.skip_blanks();
                            let _ = self.word();
                        }
                        _ => self.sep("&"),
                    }
                }
                '|' => {
                    self.pos += 1;
                    match self.peek() {
                        Some('|') => {
                            self.pos += 1;
                            self.sep("||");
                        }
                        Some('&') => {
                            self.pos += 1;
                            self.sep("|");
                        }
                        _ => self.sep("|"),
                    }
                }
                ';' => {
                    self.pos += 1;
                    if self.peek() == Some(';') {
                        self.pos += 1;
                    }
                    self.sep(";");
                }
                '(' => {
                    self.pos += 1;
                    self.sep("(");
                }
                ')' => {
                    self.pos += 1;
                    self.sep(")");
                }
                '<' => {
                    self.pos += 1;
                    if self.peek() == Some('<') {
                        self.pos += 1;
                        if self.peek() == Some('<') {
                            // Here-string: `<<< 'print(1)'`.
                            self.pos += 1;
                            self.skip_blanks();
                            let text = self.word().unwrap_or_default();
                            let seg = self.seg;
                            self.toks.push(Tok::Heredoc {
                                seg,
                                body: format!("{text}\n"),
                                delim: "<".into(),
                                terminated: true,
                            });
                        } else {
                            let strip_tabs = self.peek() == Some('-');
                            if strip_tabs {
                                self.pos += 1;
                            }
                            self.skip_blanks();
                            if let Some(delim) = self.word() {
                                let seg = self.seg;
                                self.pending.push((delim, strip_tabs, seg));
                            }
                        }
                    } else {
                        // `< file` (also `<& n`, which has no file).
                        self.drop_fd_word();
                        if self.peek() == Some('&') {
                            self.pos += 1;
                            self.skip_blanks();
                            let _ = self.word();
                        } else {
                            self.skip_blanks();
                            if let Some(file) = self.word() {
                                self.toks.push(Tok::StdinFrom(file));
                            }
                        }
                    }
                }
                '>' => {
                    self.pos += 1;
                    self.drop_fd_word();
                    if matches!(self.peek(), Some('>' | '&' | '|')) {
                        self.pos += 1;
                    }
                    self.skip_blanks();
                    let _ = self.word();
                }
                _ => {
                    if let Some(w) = self.word() {
                        self.toks.push(Tok::Word(w));
                    } else {
                        // Defensive: never loop without consuming.
                        self.pos += 1;
                    }
                }
            }
        }
        if !self.pending.is_empty() {
            // `python3 <<EOF` with no newline after it: nothing to read.
            self.read_heredoc_bodies();
        }
        self.toks
    }

    /// `2>&1`, `0<file`: the digit before a redirection is a descriptor, not
    /// an argument.
    fn drop_fd_word(&mut self) {
        if let Some(Tok::Word(w)) = self.toks.last() {
            if !w.is_empty() && w.chars().all(|c| c.is_ascii_digit()) {
                // Only if it was glued to the operator.
                let before = self.chars.get(self.pos.wrapping_sub(2)).copied();
                if before.is_some_and(|c| c.is_ascii_digit()) {
                    self.toks.pop();
                }
            }
        }
    }
}

fn lex(input: &str) -> Vec<Tok> {
    Lexer {
        chars: input.chars().collect(),
        pos: 0,
        toks: Vec::new(),
        seg: 0,
        pending: Vec::new(),
    }
    .run()
}

// --------------------------------------------------------------- segments

#[derive(Clone, Debug, Default)]
struct Heredoc {
    body: String,
    delim: String,
    terminated: bool,
}

#[derive(Clone, Debug, Default)]
struct Segment {
    words: Vec<String>,
    heredocs: Vec<Heredoc>,
    stdin_from: Option<String>,
    /// Index of the segment whose output this one reads, for `a | b`.
    piped_from: Option<usize>,
}

fn segments(toks: &[Tok]) -> Vec<Segment> {
    let mut out: Vec<Segment> = vec![Segment::default()];
    let mut last_sep: Option<&str> = None;
    for tok in toks {
        match tok {
            Tok::Word(w) => out.last_mut().unwrap().words.push(w.clone()),
            Tok::StdinFrom(f) => out.last_mut().unwrap().stdin_from = Some(f.clone()),
            Tok::Heredoc {
                seg,
                body,
                delim,
                terminated,
            } => {
                let idx = (*seg).min(out.len() - 1);
                out[idx].heredocs.push(Heredoc {
                    body: body.clone(),
                    delim: delim.clone(),
                    terminated: *terminated,
                });
            }
            Tok::Sep(s) => {
                last_sep = Some(s);
                let prev = out.len() - 1;
                out.push(Segment {
                    piped_from: (*s == "|").then_some(prev),
                    ..Segment::default()
                });
            }
        }
    }
    let _ = last_sep;
    out
}

// --------------------------------------------------------------- wrappers

fn is_assignment(w: &str) -> bool {
    let Some((name, _)) = w.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
}

/// Peel `VAR=x`, `env`, `sudo`, `nohup`, `time`, `exec` and friends off the
/// front. Returns the remaining words and, for a bare `cd DIR`, the directory.
fn strip_wrappers(words: &[String]) -> (Vec<String>, Option<String>) {
    let mut i = 0;
    while i < words.len() {
        let w = words[i].as_str();
        if is_assignment(w) {
            i += 1;
            continue;
        }
        match w {
            "env" => {
                i += 1;
                while i < words.len() {
                    let a = words[i].as_str();
                    if a == "-i" || a == "--ignore-environment" || a == "-" {
                        i += 1;
                    } else if a == "-u" || a == "--unset" || a == "-C" || a == "--chdir" {
                        i += 2;
                    } else if a == "--" {
                        i += 1;
                        break;
                    } else if is_assignment(a) || a.starts_with('-') {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            "exec" | "nohup" | "command" | "builtin" | "caffeinate" | "unbuffer" => {
                i += 1;
                while i < words.len() && words[i].starts_with('-') && words[i] != "-" {
                    i += 1;
                }
            }
            "time" => {
                i += 1;
                while i < words.len() && (words[i] == "-p" || words[i] == "-v") {
                    i += 1;
                }
            }
            "nice" => {
                i += 1;
                if i < words.len() && words[i] == "-n" {
                    i += 2;
                } else if i < words.len() && words[i].starts_with('-') {
                    i += 1;
                }
            }
            "sudo" | "doas" => {
                i += 1;
                while i < words.len() {
                    let a = words[i].as_str();
                    if a == "-u" || a == "-g" || a == "-p" || a == "-C" || a == "-h" {
                        i += 2;
                    } else if a == "--" {
                        i += 1;
                        break;
                    } else if a.starts_with('-') || is_assignment(a) {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            "timeout" => {
                i += 1;
                while i < words.len() {
                    let a = words[i].as_str();
                    if a == "-k" || a == "-s" || a == "--kill-after" || a == "--signal" {
                        i += 2;
                    } else if a.starts_with('-') {
                        i += 1;
                    } else {
                        break;
                    }
                }
                // The duration.
                i += 1;
            }
            "stdbuf" | "xvfb-run" | "chrt" | "ionice" => {
                i += 1;
                while i < words.len() && words[i].starts_with('-') {
                    // Options with a separate value are the common ones here.
                    if matches!(
                        words[i].as_str(),
                        "-n" | "-c" | "-s" | "-a" | "-e" | "-f" | "-p"
                    ) {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            }
            "cd" if i == 0 => {
                let dir = words.get(1).cloned().unwrap_or_else(|| "~".into());
                return (Vec::new(), Some(dir));
            }
            _ => break,
        }
    }
    (words[i.min(words.len())..].to_vec(), None)
}

// ----------------------------------------------------------- interpreters

enum Command {
    /// A Python interpreter, with the arguments that follow it.
    Python(Vec<String>),
    /// `sh -c "…"`: the inner command.
    Shell(String),
    Other,
}

fn basename(w: &str) -> &str {
    let base = w.rsplit(['/', '\\']).next().unwrap_or(w);
    let lower_ext = base.len() >= 4 && base[base.len() - 4..].eq_ignore_ascii_case(".exe");
    if lower_ext {
        &base[..base.len() - 4]
    } else {
        base
    }
}

/// `python`, `python3`, `python3.12`, `python2.7`, `py`, `pypy3`, `pythonw`,
/// `ipython`.
pub fn is_python_name(w: &str) -> bool {
    let lower = basename(w).to_ascii_lowercase();
    let b = lower.as_str();
    if matches!(
        b,
        "python"
            | "python2"
            | "python3"
            | "pythonw"
            | "py"
            | "pypy"
            | "pypy2"
            | "pypy3"
            | "ipython"
            | "ipython3"
    ) {
        return true;
    }
    for prefix in ["python3.", "python2.", "pypy3."] {
        if let Some(rest) = b.strip_prefix(prefix) {
            return !rest.is_empty() && rest.len() <= 2 && rest.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

fn is_shell_name(w: &str) -> bool {
    matches!(
        basename(w),
        "sh" | "bash" | "zsh" | "dash" | "ksh" | "ash" | "busybox"
    )
}

fn classify(words: &[String]) -> Command {
    let Some(first) = words.first() else {
        return Command::Other;
    };
    if is_python_name(first) {
        return Command::Python(words[1..].to_vec());
    }
    if is_shell_name(first) {
        // `bash -x -c 'cmd'`, `sh -euc 'cmd'`
        let mut i = 1;
        if basename(first) == "busybox" {
            if words.get(1).map(String::as_str) != Some("sh") {
                return Command::Other;
            }
            i = 2;
        }
        while i < words.len() {
            let a = words[i].as_str();
            if a == "-c" {
                return words
                    .get(i + 1)
                    .map(|s| Command::Shell(s.clone()))
                    .unwrap_or(Command::Other);
            }
            if a.starts_with('-') && !a.starts_with("--") && a.contains('c') && a.len() > 1 {
                return words
                    .get(i + 1)
                    .map(|s| Command::Shell(s.clone()))
                    .unwrap_or(Command::Other);
            }
            if a.starts_with('-') {
                i += 1;
                continue;
            }
            return Command::Other;
        }
        return Command::Other;
    }
    match basename(first) {
        "uv" => {
            if words.get(1).map(String::as_str) != Some("run") {
                return Command::Other;
            }
            let mut i = 2;
            while i < words.len() {
                let a = words[i].as_str();
                if matches!(
                    a,
                    "--with"
                        | "-w"
                        | "--python"
                        | "-p"
                        | "--directory"
                        | "--project"
                        | "--index"
                        | "--extra"
                        | "--group"
                        | "--env-file"
                        | "--with-requirements"
                        | "--with-editable"
                        | "--only-group"
                        | "--package"
                        | "--script"
                ) {
                    i += 2;
                } else if a == "--" {
                    i += 1;
                    break;
                } else if a.starts_with('-') {
                    i += 1;
                } else {
                    break;
                }
            }
            match words.get(i) {
                Some(w) if is_python_name(w) => Command::Python(words[i + 1..].to_vec()),
                // `uv run script.py` runs the script with the project python.
                Some(w) if w.ends_with(".py") => Command::Python(words[i..].to_vec()),
                _ => Command::Other,
            }
        }
        "poetry" | "pipenv" | "pdm" | "hatch" | "rye" | "conda" | "micromamba" | "mamba" => {
            if words.get(1).map(String::as_str) != Some("run") {
                return Command::Other;
            }
            let mut i = 2;
            while i < words.len() {
                let a = words[i].as_str();
                if matches!(
                    a,
                    "-n" | "--name" | "-p" | "--prefix" | "--env" | "-e" | "--cwd"
                ) {
                    i += 2;
                } else if a == "--" {
                    i += 1;
                    break;
                } else if a.starts_with('-') {
                    i += 1;
                } else {
                    break;
                }
            }
            match words.get(i) {
                Some(w) if is_python_name(w) => Command::Python(words[i + 1..].to_vec()),
                _ => Command::Other,
            }
        }
        _ => Command::Other,
    }
}

/// Read an interpreter's argument list the way CPython's launcher does.
pub fn classify_argv(args: &[String]) -> Invocation {
    let mut inv = Invocation::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            // Informational: the interpreter prints and exits, and never
            // touches stdin — so neither may we.
            "-V" | "--version" | "-h" | "--help" | "-?" | "-VV" => return inv,
            "--" => {
                inv.script = args.get(i + 1).cloned();
                return inv;
            }
            "-" => {
                inv.stdin = true;
                return inv;
            }
            "-c" => {
                inv.code = args.get(i + 1).cloned().or(Some(String::new()));
                return inv;
            }
            "-m" => {
                inv.module = args.get(i + 1).cloned().or(Some(String::new()));
                return inv;
            }
            // Options that take a separate value.
            "-W" | "-X" | "-Q" | "--check-hash-based-pycs" => {
                i += 2;
                continue;
            }
            _ => {}
        }
        if let Some(rest) = a.strip_prefix('-').filter(|r| !r.starts_with('-')) {
            // `py` launcher version selectors: -3, -3.12, -3-64, -0, -0p.
            if rest.starts_with(|c: char| c.is_ascii_digit()) {
                i += 1;
                continue;
            }
            // A cluster: -uBc CODE, -Wignore, -Xdev, -cCODE, -mmod.
            for (pos, c) in rest.char_indices() {
                match c {
                    'c' => {
                        let attached = &rest[pos + 1..];
                        inv.code = if attached.is_empty() {
                            args.get(i + 1).cloned().or(Some(String::new()))
                        } else {
                            Some(attached.to_string())
                        };
                        return inv;
                    }
                    'm' => {
                        let attached = &rest[pos + 1..];
                        inv.module = if attached.is_empty() {
                            args.get(i + 1).cloned().or(Some(String::new()))
                        } else {
                            Some(attached.to_string())
                        };
                        return inv;
                    }
                    'V' | 'h' | '?' => return Invocation::default(),
                    'W' | 'X' | 'Q' => {
                        // Value is the rest of the cluster, or the next word.
                        if rest[pos + 1..].is_empty() {
                            i += 1;
                        }
                        break;
                    }
                    _ => {}
                }
            }
            i += 1;
            continue;
        }
        if a.starts_with("--") {
            i += 1;
            continue;
        }
        inv.script = Some(a.to_string());
        return inv;
    }
    // No program named: stdin, unless it is an interactive session.
    inv.stdin = true;
    inv
}

// ------------------------------------------------------------------ source

fn sources(
    inv: &Invocation,
    seg: &Segment,
    piped_from: Option<&Segment>,
    cwd: Option<&Path>,
    opts: &ExtractOpts,
) -> Vec<Extracted> {
    let mut out = Vec::new();
    if let Some(code) = &inv.code {
        out.push(Extracted {
            source: code.clone(),
            label: "python -c".into(),
            path: None,
            segment: String::new(),
        });
        return out;
    }
    if inv.module.is_some() {
        return out;
    }
    if let Some(script) = &inv.script {
        out.push(read_script(script, cwd, opts));
        return out;
    }
    // Standard input.
    for h in &seg.heredocs {
        out.push(heredoc_item(h));
    }
    if !out.is_empty() {
        return out;
    }
    if let Some(file) = &seg.stdin_from {
        out.push(read_script(file, cwd, opts));
        return out;
    }
    if let Some(prev) = piped_from {
        let (words, _) = strip_wrappers(&prev.words);
        match words.first().map(String::as_str) {
            Some("cat") => {
                let files: Vec<&String> =
                    words[1..].iter().filter(|w| !w.starts_with('-')).collect();
                if files.is_empty() || files == [&"-".to_string()] {
                    for h in &prev.heredocs {
                        out.push(heredoc_item(h));
                    }
                    if let Some(file) = &prev.stdin_from {
                        out.push(read_script(file, cwd, opts));
                    }
                } else {
                    for f in files {
                        out.push(read_script(f, cwd, opts));
                    }
                }
            }
            Some("echo") => {
                let text: Vec<&str> = words[1..]
                    .iter()
                    .map(String::as_str)
                    .skip_while(|w| matches!(*w, "-e" | "-n" | "-E" | "-en" | "-ne"))
                    .collect();
                out.push(Extracted {
                    source: format!("{}\n", text.join(" ")),
                    label: "<pipe> echo".into(),
                    path: None,
                    segment: String::new(),
                });
            }
            Some("printf") => {
                let text = words.get(1).cloned().unwrap_or_default();
                out.push(Extracted {
                    source: text.replace("\\n", "\n"),
                    label: "<pipe> printf".into(),
                    path: None,
                    segment: String::new(),
                });
            }
            Some(other) => out.push(Extracted {
                source: String::new(),
                label: format!("<pipe> {}", basename(other)),
                path: None,
                segment: String::new(),
            }),
            None => {}
        }
    }
    out
}

fn heredoc_item(h: &Heredoc) -> Extracted {
    let label = if h.delim == "<" {
        "here-string <<<".to_string()
    } else if h.terminated {
        format!("heredoc <<{}", h.delim)
    } else {
        format!("heredoc <<{} (unterminated)", h.delim)
    };
    Extracted {
        source: h.body.clone(),
        label,
        path: None,
        segment: String::new(),
    }
}

fn read_script(script: &str, cwd: Option<&Path>, opts: &ExtractOpts) -> Extracted {
    let expanded = expand_home(script);
    let path = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        match cwd {
            Some(base) => base.join(&expanded),
            None => PathBuf::from(&expanded),
        }
    };
    let label = script.to_string();
    if !opts.read_files {
        return Extracted {
            source: String::new(),
            label,
            path: Some(path),
            segment: String::new(),
        };
    }
    match std::fs::metadata(&path) {
        Ok(meta) if meta.is_file() && meta.len() > opts.max_bytes => Extracted {
            source: String::new(),
            label: format!("{label} (too large)"),
            path: Some(path),
            segment: String::new(),
        },
        Ok(meta) if meta.is_file() => match std::fs::read(&path) {
            Ok(bytes) => Extracted {
                source: String::from_utf8_lossy(&bytes).into_owned(),
                label,
                path: Some(path),
                segment: String::new(),
            },
            Err(_) => Extracted {
                source: String::new(),
                label: format!("{label} (unreadable)"),
                path: Some(path),
                segment: String::new(),
            },
        },
        _ => Extracted {
            source: String::new(),
            label: format!("{label} (unreadable)"),
            path: Some(path),
            segment: String::new(),
        },
    }
}

fn expand_home(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            return PathBuf::from(home).join(rest).display().to_string();
        }
    }
    p.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(cmd: &str) -> Vec<Vec<String>> {
        segments(&lex(cmd))
            .into_iter()
            .map(|s| s.words)
            .filter(|w| !w.is_empty())
            .collect()
    }

    #[test]
    fn quotes_and_operators_split_as_the_shell_would() {
        assert_eq!(
            words("cd /tmp && python3 -c 'a && b' | tee x; echo \"q\\\"q\""),
            vec![
                vec!["cd", "/tmp"],
                vec!["python3", "-c", "a && b"],
                vec!["tee", "x"],
                vec!["echo", "q\"q"],
            ]
        );
    }

    #[test]
    fn a_backslash_before_an_ordinary_char_is_kept() {
        assert_eq!(
            words(r"C:\Python\python.exe s.py")[0][0],
            r"C:\Python\python.exe"
        );
        assert_eq!(words(r"echo a\ b")[0], vec!["echo", "a b"]);
    }

    #[test]
    fn redirections_are_not_arguments() {
        assert_eq!(
            words("python3 s.py > out 2>&1 < in")[0],
            vec!["python3", "s.py"]
        );
        let segs = segments(&lex("python3 < in.py"));
        assert_eq!(segs[0].stdin_from.as_deref(), Some("in.py"));
    }

    #[test]
    fn a_heredoc_inside_quotes_is_not_a_heredoc() {
        let all = extract_all("python3 -c \"x = 1 << 2\"", &ExtractOpts::default());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].source, "x = 1 << 2");
    }

    #[test]
    fn heredocs_attach_to_the_command_that_declared_them() {
        let all = extract_all(
            "python3 <<'EOF' | tee log\nprint(1)\nEOF\n",
            &ExtractOpts::default(),
        );
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].source, "print(1)\n");
        assert_eq!(all[0].label, "heredoc <<EOF");
    }

    #[test]
    fn argv_is_read_like_cpython() {
        let inv = |s: &str| classify_argv(&s.split(' ').map(String::from).collect::<Vec<_>>());
        assert_eq!(inv("-X faulthandler s.py").script.as_deref(), Some("s.py"));
        assert_eq!(inv("-W ignore -c code").code.as_deref(), Some("code"));
        assert_eq!(inv("-uBc code").code.as_deref(), Some("code"));
        assert_eq!(inv("-ccode").code.as_deref(), Some("code"));
        assert_eq!(inv("-m pytest -q").module.as_deref(), Some("pytest"));
        assert_eq!(inv("-mpip install x").module.as_deref(), Some("pip"));
        assert!(inv("-u -").stdin);
        assert!(classify_argv(&[]).stdin);
        assert_eq!(inv("-3.12 -c code").code.as_deref(), Some("code"));
        assert_eq!(inv("-Wignore s.py").script.as_deref(), Some("s.py"));
        assert_eq!(inv("-- -c").script.as_deref(), Some("-c"));
        assert!(!inv("--version").stdin);
        assert!(!inv("-V").stdin && inv("-V").script.is_none());
    }

    #[test]
    fn interpreter_names() {
        for ok in [
            "python",
            "python3",
            "python3.12",
            "/usr/bin/python3",
            "py",
            "python.exe",
            "PYTHON.EXE",
            "pypy3",
            r"C:\p\python3.exe",
        ] {
            assert!(is_python_name(ok), "{ok}");
        }
        for no in [
            "mypython3",
            "pythonic",
            "python3.abc",
            "python312",
            "perl",
            "pythons",
        ] {
            assert!(!is_python_name(no), "{no}");
        }
    }
}
