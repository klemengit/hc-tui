//! `screen = true` on a `[[keys.command]]`: the focused pane's visible
//! screen as a private ANSI text file whose path reaches the command in
//! `HC_FOCUSED_SCREEN`. The file lives as long as its owner (popup,
//! command pane) and is removed when the owner is dropped.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A temp file removed on drop.
#[derive(Debug)]
pub(crate) struct ScreenFile {
    path: Option<PathBuf>,
}

impl ScreenFile {
    /// Write `contents` to a new 0600 file under `$XDG_RUNTIME_DIR`
    /// (else the system temp dir).
    pub(crate) fn write(contents: &str) -> std::io::Result<Self> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|dir| dir.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        Self::write_in(&dir, contents)
    }

    pub(crate) fn write_in(dir: &Path, contents: &str) -> std::io::Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let path = dir.join(format!(
            "hc-tui-screen-{}-{}.ansi",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut handle = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        // From here the guard owns the path: a failed write removes it.
        let file = Self { path: Some(path) };
        handle.write_all(contents.as_bytes())?;
        Ok(file)
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_deref().expect("path is set until released")
    }

    /// Give up ownership: the caller removes the file.
    pub(crate) fn release(mut self) -> PathBuf {
        self.path.take().expect("path is set until released")
    }
}

impl Drop for ScreenFile {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}
