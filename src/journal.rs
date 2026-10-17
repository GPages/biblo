//! Startup diagnostics. A release build has no console, so without this a
//! failed start is silent: Biblo just does not appear. Here, every start
//! writes a few lines to `biblo.log` (in the data folder), and a fatal error
//! is shown in a Windows message box, which needs no window of Biblo's own.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const LOG_FILE: &str = "biblo.log";
/// Above this size, a new log is started; the previous one is kept as `biblo.old.log`.
const MAX_LOG_BYTES: u64 = 512 * 1024;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Sets where the log goes. Called once, as early as possible.
pub fn init(dir: &Path) {
    let path = dir.join(LOG_FILE);
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = fs::rename(&path, dir.join("biblo.old.log"));
    }
    let _ = LOG_PATH.set(path);
}

/// Appends a timestamped line. Never fails: the log must not stop Biblo.
pub fn line(text: impl AsRef<str>) {
    let Some(path) = LOG_PATH.get() else { return };
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{stamp} {}", text.as_ref());
    }
}

/// Logs a fatal error and shows it to the user.
pub fn fail(summary: &str, detail: &str) {
    line(format!("ERROR: {summary} {detail}"));
    let mut text = format!("{summary}\n\n{detail}");
    if let Some(path) = LOG_PATH.get() {
        text.push_str(&format!(
            "\n\nLes détails sont enregistrés dans le fichier :\n{}",
            path.display()
        ));
    }
    message_box(&text, Icon::Error);
}

/// A crash is logged (with where it happened) and shown, instead of Biblo
/// silently disappearing. Data already saved stays saved: every change is
/// written to disk as soon as it is made.
pub fn catch_panics() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        line(format!("PANIC: {info}\n{backtrace}"));
        let what = info.payload_as_str().unwrap_or("erreur inconnue");
        fail(
            "Biblo a rencontré un problème et doit se fermer.",
            &format!(
                "{what}\n\nLes données déjà enregistrées ne sont pas perdues : \
                 chaque prêt, retour ou ajout est enregistré dès qu'il est fait."
            ),
        );
    }));
}

#[derive(Clone, Copy)]
pub enum Icon {
    Error,
    Information,
}

/// A message window that blocks until it is closed.
#[cfg(windows)]
pub fn message_box(text: &str, icon: Icon) {
    use std::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(window: *mut c_void, text: *const u16, title: *const u16, flags: u32)
            -> i32;
    }
    const MB_ICONERROR: u32 = 0x10;
    const MB_ICONINFORMATION: u32 = 0x40;
    const MB_SETFOREGROUND: u32 = 0x1_0000;
    const MB_TOPMOST: u32 = 0x4_0000;

    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let flags = match icon {
        Icon::Error => MB_ICONERROR,
        Icon::Information => MB_ICONINFORMATION,
    } | MB_SETFOREGROUND
        | MB_TOPMOST;
    let (text, title) = (wide(text), wide("Biblo"));
    // SAFETY: both strings are NUL-terminated UTF-16 and outlive the call.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), flags);
    }
}

#[cfg(not(windows))]
pub fn message_box(text: &str, _icon: Icon) {
    eprintln!("Biblo: {text}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crash_is_written_to_the_log() {
        let dir = std::env::temp_dir().join(format!("biblo-journal-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        init(&dir);
        catch_panics();
        let crashed = std::thread::spawn(|| panic!("boom")).join();
        // Back to the default hook for the other tests.
        let _ = std::panic::take_hook();
        assert!(crashed.is_err());
        let log = fs::read_to_string(dir.join(LOG_FILE)).unwrap();
        assert!(log.contains("PANIC") && log.contains("boom"), "{log}");
        assert!(log.contains("Biblo a rencontré un problème"), "{log}");
        let _ = fs::remove_dir_all(dir);
    }
}
