//! Writes the demo data (see `biblo::demo`) into a data folder.
//!
//!     cargo run --example demo_data             # fills target/debug/biblo-data
//!     cargo run --example demo_data -- <folder> # any other data folder
//!
//! A data folder that already holds books, pupils or loans is never touched.

use std::path::PathBuf;
use std::process::exit;

use biblo::demo::{fill, BOOKS};
use biblo::Store;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/biblo-data")
        });
    let today = biblo::today();
    let mut store = Store::open(&dir, today).unwrap_or_else(|e| {
        eprintln!("Cannot open {}: {e}", dir.display());
        exit(1);
    });
    let lib = store.lib();
    if !lib.books().is_empty() || !lib.students().is_empty() || !lib.loans().is_empty() {
        eprintln!("{} already holds data: nothing was changed.", dir.display());
        exit(1);
    }
    if let Err(e) = store.apply(|lib| fill(lib, today)) {
        eprintln!("Failed: {e}");
        exit(1);
    }

    let lib = store.lib();
    println!("Demo data written to {}", dir.display());
    println!(
        "{} books ({} titles), {} pupils, {} loans: {} still out, {} of them for 21 days or more.",
        lib.books().len(),
        BOOKS.len(),
        lib.students().len(),
        lib.loans().len(),
        lib.overdue(today, 0).len(),
        lib.overdue(today, 21).len()
    );
}
