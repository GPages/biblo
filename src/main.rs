// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gui;
mod journal;

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use eframe::egui;
use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu;

use journal::Icon;

/// The demonstration build says so in the window title.
const TITLE: &str = if cfg!(feature = "demo") {
    "Biblo (démonstration)"
} else {
    "Biblo"
};

/// A data folder with this name next to the executable is used if it exists.
const PORTABLE_FOLDER: &str = "biblo-data";
/// Folder name in `%LOCALAPPDATA%`; the demonstration keeps its data apart.
const APP_FOLDER: &str = if cfg!(feature = "demo") {
    "Biblo-demo"
} else {
    "Biblo"
};

/// Where the data lives:
/// 1. a `biblo-data` folder next to the executable, if there is one (use from
///    a USB key, and data written there by earlier versions);
/// 2. otherwise `%LOCALAPPDATA%\Biblo`: always writable, not synced by
///    OneDrive, and not lost if the executable is moved, or was started from
///    a zip file or from a read-only folder;
/// 3. without `%LOCALAPPDATA%` (development on Linux): next to the executable.
fn choose_data_dir(exe_dir: Option<&Path>, local_app_data: Option<&Path>) -> PathBuf {
    let portable = exe_dir.map(|dir| dir.join(PORTABLE_FOLDER));
    if let Some(dir) = portable.as_ref().filter(|dir| dir.is_dir()) {
        return dir.clone();
    }
    if let Some(local) = local_app_data {
        return local.join(APP_FOLDER);
    }
    portable.unwrap_or_else(|| PathBuf::from(PORTABLE_FOLDER))
}

fn data_dir() -> PathBuf {
    let exe = std::env::current_exe().ok();
    let exe_dir = exe.as_deref().and_then(Path::parent);
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    choose_data_dir(exe_dir, local.as_deref())
}

/// Keeps a second Biblo from opening the same data (a double click too many):
/// each would save its own copy and undo the other's loans. The lock goes
/// away with the process, even after a crash. `Err` if Biblo is already open.
fn lock_data(dir: &Path) -> Result<Option<File>, ()> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("biblo.lock"));
    let file = match file {
        Ok(file) => file,
        Err(e) => {
            journal::line(format!("no lock file ({e}): starting anyway"));
            return Ok(None);
        }
    };
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Err(()),
        Err(TryLockError::Error(e)) => {
            journal::line(format!("cannot lock ({e}): starting anyway"));
            Ok(None)
        }
    }
}

/// Graphics: on Windows, DirectX 12 then OpenGL, without Vulkan, whose
/// drivers on older school computers are the most likely to fail or hang at
/// start; and the built-in graphics chip rather than a separate graphics
/// card, which starts faster. `WGPU_BACKEND` and `WGPU_POWER_PREF` still
/// override both, to try something else without rebuilding.
fn wgpu_options() -> WgpuConfiguration {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    if cfg!(windows) && wgpu::Backends::from_env().is_none() {
        setup.instance_descriptor.backends = wgpu::Backends::DX12 | wgpu::Backends::GL;
    }
    if wgpu::PowerPreference::from_env().is_none() {
        setup.power_preference = wgpu::PowerPreference::LowPower;
    }
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

/// Demonstration build: the first start fills the new data folder with sample
/// data; later starts keep whatever was changed.
#[cfg(feature = "demo")]
fn seed_demo(dir: &Path) {
    if dir.join(biblo::store::DATA_FILE).exists() {
        return;
    }
    let today = biblo::today();
    let result = biblo::Store::open(dir, today)
        .and_then(|mut store| store.apply(|lib| biblo::demo::fill(lib, today)));
    // A failure only leaves an empty library: the app still opens.
    match result {
        Ok(()) => journal::line("demo data written"),
        Err(e) => journal::line(format!("demo data not written: {e}")),
    }
}

fn main() -> ExitCode {
    let data_dir = data_dir();
    if let Err(e) = fs::create_dir_all(&data_dir) {
        // Somewhere to write why.
        journal::init(&std::env::temp_dir());
        journal::fail(
            "Biblo ne peut pas créer son dossier de données :",
            &format!("{}\n\n{e}", data_dir.display()),
        );
        return ExitCode::FAILURE;
    }
    journal::init(&data_dir);
    journal::catch_panics();
    journal::line(format!(
        "=== {TITLE} {} starting ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    journal::line(format!("executable: {:?}", std::env::current_exe().ok()));
    journal::line(format!("data folder: {}", data_dir.display()));

    let Ok(_lock) = lock_data(&data_dir) else {
        journal::line("already running: this second start stops here");
        journal::message_box(
            "Biblo est déjà ouvert, ou en train de s'ouvrir.\n\n\
             Attendez quelques secondes : la fenêtre va apparaître.\n\
             Si rien n'apparaît après une minute, redémarrez l'ordinateur.",
            Icon::Information,
        );
        return ExitCode::SUCCESS;
    };

    #[cfg(feature = "demo")]
    seed_demo(&data_dir);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([800.0, 500.0]),
        wgpu_options: wgpu_options(),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Biblo",
        options,
        Box::new(move |cc| {
            if let Some(render) = &cc.wgpu_render_state {
                journal::line(format!("graphics: {:?}", render.adapter.get_info()));
            }
            Ok(Box::new(gui::App::new(&cc.egui_ctx, data_dir)))
        }),
    );
    match result {
        Ok(()) => {
            journal::line("closed");
            ExitCode::SUCCESS
        }
        Err(e) => {
            journal::fail(
                "Biblo n'a pas pu ouvrir sa fenêtre.",
                &format!(
                    "{e}\n\nCela vient en général de la carte graphique ou de son pilote \
                     (programme qui la fait fonctionner)."
                ),
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty folder for one test.
    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("biblo-main-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn data_goes_to_local_app_data_unless_a_portable_folder_exists() {
        let exe_dir = temp("exe");
        let local = temp("local");
        assert_eq!(
            choose_data_dir(Some(&exe_dir), Some(&local)),
            local.join(APP_FOLDER)
        );
        // a folder kept next to the executable wins (USB key, earlier versions)
        fs::create_dir(exe_dir.join(PORTABLE_FOLDER)).unwrap();
        assert_eq!(
            choose_data_dir(Some(&exe_dir), Some(&local)),
            exe_dir.join(PORTABLE_FOLDER)
        );
        // no %LOCALAPPDATA% (Linux): next to the executable
        let bare = temp("bare");
        assert_eq!(
            choose_data_dir(Some(&bare), None),
            bare.join(PORTABLE_FOLDER)
        );
        for dir in [exe_dir, local, bare] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn a_second_start_finds_the_data_locked() {
        let dir = temp("lock");
        let first = lock_data(&dir);
        assert!(matches!(first, Ok(Some(_))));
        assert!(lock_data(&dir).is_err());
        drop(first);
        assert!(matches!(lock_data(&dir), Ok(Some(_))));
        let _ = fs::remove_dir_all(dir);
    }
}
