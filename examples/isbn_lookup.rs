//! Looks an ISBN up in the CSV database, building the index if needed.
//!
//!     cargo run --release --example isbn_lookup -- <csv> <index-file> <isbn>...

use std::path::PathBuf;
use std::time::Instant;

use biblo::isbn::{normalize, IsbnDb, Opened, Progress};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [csv, index, isbns @ ..] = args.as_slice() else {
        eprintln!("usage: isbn_lookup <csv> <index-file> <isbn>...");
        std::process::exit(2);
    };
    let (csv, index) = (PathBuf::from(csv), PathBuf::from(index));
    let start = Instant::now();
    let db = match IsbnDb::open(&csv, &index) {
        Ok(Opened::Ready(db)) => db,
        Ok(Opened::NeedsIndex) => {
            eprintln!("building the index…");
            IsbnDb::build_index(&csv, &index, &Progress::default()).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            })
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    println!("{} ISBNs, ready in {:?}", db.count(), start.elapsed());
    for isbn in isbns {
        let start = Instant::now();
        let result = normalize(isbn).map(|isbn| (isbn.clone(), db.lookup(&isbn)));
        match result {
            Ok((isbn, Ok(Some(info)))) => println!("{isbn}: {info:?} ({:?})", start.elapsed()),
            Ok((isbn, Ok(None))) => println!("{isbn}: not found ({:?})", start.elapsed()),
            Ok((isbn, Err(e))) => println!("{isbn}: {e}"),
            Err(e) => println!("{isbn}: {e}"),
        }
    }
}
