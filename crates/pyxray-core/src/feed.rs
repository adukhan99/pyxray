//! The event feed: one JSON object per line, appended by whatever is watching
//! Python go past, tailed by `pyx watch`.
//!
//! A harness can fire several snippets a second, and drawing a report per
//! snippet into the terminal the agent is already talking through is both
//! unreadable and, in most harnesses, fed straight back to the model as tool
//! output. So the render goes somewhere else: a file here, a second pane
//! there. This module is the contract between the two ends.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::model::Event;

/// Keep the log bounded. A busy session appends a few hundred bytes per
/// snippet; at this cap that is tens of thousands of them, and the oldest
/// half is dropped rather than growing without limit on a machine whose home
/// quota may be nearly full.
pub const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Where the feed lives when nobody said otherwise.
///
/// Prefers the per-user runtime directory: it is a tmpfs on every systemd
/// machine, so a chatty session costs no disk quota and the log dies with the
/// login, which is the right lifetime for it.
pub fn default_path() -> PathBuf {
    if let Ok(explicit) = std::env::var("PYXRAY_LOG") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        if !runtime.is_empty() {
            return PathBuf::from(runtime).join("pyxray").join("feed.jsonl");
        }
    }
    let uid = std::env::var("UID").unwrap_or_else(|_| "user".into());
    std::env::temp_dir()
        .join(format!("pyxray-{uid}"))
        .join("feed.jsonl")
}

/// Append one event. Creates the parent directory on first use, and trims the
/// file from the front once it passes [`MAX_BYTES`].
pub fn append(path: &Path, event: &Event) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(event).map_err(std::io::Error::other)?;
    line.push('\n');

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.flush()?;

    if file.metadata()?.len() > MAX_BYTES {
        trim(path)?;
    }
    Ok(())
}

/// Drop the oldest half of the log, keeping whole lines.
fn trim(path: &Path) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path)?;
    let cut = text.len() / 2;
    let keep = match text[cut..].find('\n') {
        Some(offset) => &text[cut + offset + 1..],
        None => "",
    };
    std::fs::write(path, keep)
}

/// Read every event currently in the log, ignoring lines that do not parse —
/// a half-written line from a concurrent append is normal, not an error.
pub fn read_all(path: &Path) -> std::io::Result<Vec<Event>> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Ok(event) = serde_json::from_str::<Event>(&line) {
            out.push(event);
        }
    }
    Ok(out)
}

/// A cursor over a growing log, for tailing it.
pub struct Tail {
    path: PathBuf,
    offset: u64,
    /// Carried over when a read lands mid-line.
    partial: String,
    /// The first bytes of the file at the last poll. Length alone cannot spot
    /// a log that was trimmed and refilled to the same size, which is exactly
    /// what a rotation looks like from here.
    signature: Vec<u8>,
}

impl Tail {
    /// Start at the end of whatever is already there.
    pub fn at_end(path: &Path) -> Tail {
        let offset = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        Tail {
            path: path.to_path_buf(),
            offset,
            partial: String::new(),
            signature: head_bytes(path),
        }
    }

    pub fn at_start(path: &Path) -> Tail {
        Tail {
            path: path.to_path_buf(),
            offset: 0,
            partial: String::new(),
            signature: head_bytes(path),
        }
    }

    /// Every whole event appended since the last call. A file that shrank was
    /// trimmed or replaced, so the cursor restarts from the beginning.
    pub fn poll(&mut self) -> Vec<Event> {
        let Ok(mut file) = File::open(&self.path) else {
            return Vec::new();
        };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let signature = head_bytes(&self.path);
        if len < self.offset || signature != self.signature {
            self.offset = 0;
            self.partial.clear();
        }
        self.signature = signature;
        if len == self.offset {
            return Vec::new();
        }
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut buf = String::new();
        let read = {
            use std::io::Read;
            let mut bytes = Vec::new();
            match file.take(len - self.offset).read_to_end(&mut bytes) {
                Ok(n) => {
                    buf = String::from_utf8_lossy(&bytes).into_owned();
                    n as u64
                }
                Err(_) => 0,
            }
        };
        self.offset += read;

        let mut text = std::mem::take(&mut self.partial);
        text.push_str(&buf);
        let mut out = Vec::new();
        let ends_clean = text.ends_with('\n');
        let mut pieces: Vec<&str> = text.split('\n').collect();
        if !ends_clean {
            self.partial = pieces.pop().unwrap_or("").to_string();
        }
        for piece in pieces {
            if piece.is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<Event>(piece) {
                out.push(event);
            }
        }
        out
    }
}

/// The first bytes of a file, used as a cheap identity check between polls.
fn head_bytes(path: &Path) -> Vec<u8> {
    use std::io::Read;
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let mut buf = Vec::new();
    let _ = file.take(64).read_to_end(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xray;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pyxray-test-{name}-{}.jsonl", std::process::id()))
    }

    #[test]
    fn appends_and_reads_back() {
        let path = tmp("roundtrip");
        let _ = std::fs::remove_file(&path);
        for source in ["print(1)", "import os\nos.remove('x')"] {
            append(&path, &xray(source, "t").event("test")).unwrap();
        }
        let events = read_all(&path).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[1].risk > events[0].risk);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn tail_sees_only_new_events() {
        let path = tmp("tail");
        let _ = std::fs::remove_file(&path);
        append(&path, &xray("print(1)", "a").event("test")).unwrap();
        let mut tail = Tail::at_end(&path);
        assert!(tail.poll().is_empty());
        append(&path, &xray("print(2)", "b").event("test")).unwrap();
        let seen = tail.poll();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].name, "b");
        assert!(tail.poll().is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_truncated_log_restarts_the_cursor() {
        let path = tmp("trunc");
        let _ = std::fs::remove_file(&path);
        append(&path, &xray("print(1)", "a").event("test")).unwrap();
        let mut tail = Tail::at_end(&path);
        std::fs::write(&path, "").unwrap();
        append(&path, &xray("print(2)", "b").event("test")).unwrap();
        assert_eq!(tail.poll().len(), 1);
        std::fs::remove_file(&path).ok();
    }
}
