//! Looking a book up by its ISBN in a big CSV file (a public dataset, about
//! 1.3 GB and 8.7 million lines, with the columns `isbn`, `title`, `editeur`…).
//!
//! Reading the whole file for each lookup would take seconds, so the first
//! use builds a small sorted index next to the data (`isbn-index.bin`: for
//! every ISBN, where its line starts in the CSV). It takes a few dozen
//! seconds, once; after that a lookup reads 20-odd bytes of the index and one
//! line of the CSV. The index is rebuilt if the CSV changes.
//!
//! The dataset has no author, no year and no usable category: only the
//! title and, for some books, the publisher can be filled in.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use crate::tidy::{is_garbled, tidy_title};

const MAGIC: &[u8; 8] = b"BIBLOIX1";
/// Magic, size and modification time of the CSV, number of records.
const HEADER_LEN: u64 = 32;
/// An ISBN (as a number) and the offset of its line in the CSV.
const RECORD_LEN: u64 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsbnError {
    /// Not 10 or 13 digits.
    Invalid,
    /// The right number of digits, but the check digit does not fit: a typo,
    /// or a bad scan.
    BadChecksum,
    Io(String),
    /// The CSV does not look like the expected dataset.
    Format(String),
}

impl fmt::Display for IsbnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsbnError::Invalid => write!(
                f,
                "ISBN invalide : il faut 13 chiffres (ou 10), par exemple 9782070612758."
            ),
            IsbnError::BadChecksum => write!(
                f,
                "ISBN invalide : la clé de contrôle ne correspond pas (faute de frappe ?)."
            ),
            IsbnError::Io(m) => write!(f, "Erreur de fichier : {m}"),
            IsbnError::Format(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for IsbnError {}

fn io_err(e: io::Error) -> IsbnError {
    IsbnError::Io(e.to_string())
}

/// What the dataset says about a book (empty strings when unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookInfo {
    /// Tidied up (see `tidy`).
    pub title: String,
    /// Some characters of the title were lost in the database and could not
    /// be restored: it needs a look.
    pub title_garbled: bool,
    pub publisher: String,
}

// ------------------------------------------------------------ ISBN numbers

/// A scanner set to US layout on an AZERTY Windows "types" the digits as
/// `& é " ' ( - è _ ç à` (see `codes.rs`).
fn azerty_digit(c: char) -> Option<char> {
    "&é\"'(-è_çà"
        .chars()
        .position(|k| k == c)
        .map(|i| char::from(b"1234567890"[i]))
}

/// The 13-digit ISBN written in `input` (hyphens, spaces and an "ISBN" prefix
/// are fine; an ISBN-10 is converted), or why it is not one.
pub fn normalize(input: &str) -> Result<String, IsbnError> {
    let mut text: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if !text.chars().any(|c| c.is_ascii_digit()) && text.chars().all(|c| azerty_digit(c).is_some())
    {
        text = text.chars().filter_map(azerty_digit).collect();
    }
    let lower = text.to_lowercase();
    let text = match lower.strip_prefix("isbn") {
        Some(rest) => rest.trim_start_matches(':').to_string(),
        None => lower,
    };
    let text: String = text.chars().filter(|&c| c != '-').collect();
    match text.len() {
        13 if text.chars().all(|c| c.is_ascii_digit()) => {
            let digits: Vec<u32> = text.chars().filter_map(|c| c.to_digit(10)).collect();
            if ean13_check(&digits[..12]) == digits[12] {
                Ok(text)
            } else {
                Err(IsbnError::BadChecksum)
            }
        }
        10 if text[..9].chars().all(|c| c.is_ascii_digit()) => {
            let mut values: Vec<u32> = text[..9].chars().filter_map(|c| c.to_digit(10)).collect();
            values.push(match text.as_bytes()[9] {
                b'x' => 10,
                d @ b'0'..=b'9' => u32::from(d - b'0'),
                _ => return Err(IsbnError::Invalid),
            });
            let sum: u32 = values
                .iter()
                .enumerate()
                .map(|(i, v)| (10 - i as u32) * v)
                .sum();
            if !sum.is_multiple_of(11) {
                return Err(IsbnError::BadChecksum);
            }
            let mut digits = vec![9, 7, 8];
            digits.extend(&values[..9]);
            let check = ean13_check(&digits);
            digits.push(check);
            Ok(digits
                .iter()
                .filter_map(|&d| char::from_digit(d, 10))
                .collect())
        }
        _ => Err(IsbnError::Invalid),
    }
}

/// Check digit of the first 12 digits of an EAN-13.
fn ean13_check(first12: &[u32]) -> u32 {
    let sum: u32 = first12
        .iter()
        .enumerate()
        .map(|(i, d)| if i % 2 == 0 { *d } else { 3 * d })
        .sum();
    (10 - sum % 10) % 10
}

// -------------------------------------------------------------- CSV lines

/// One CSV record starting at the reader's position; quoted fields may span
/// several lines. `None` at the end of the file.
fn read_record<R: BufRead>(reader: &mut R) -> io::Result<Option<(Vec<String>, usize)>> {
    let mut buf = Vec::new();
    loop {
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 {
            return Ok(if buf.is_empty() {
                None
            } else {
                Some((parse_fields(&buf), buf.len()))
            });
        }
        // Inside a quoted field, an odd number of quotes so far.
        if buf.iter().filter(|&&b| b == b'"').count() % 2 == 0 {
            return Ok(Some((parse_fields(&buf), buf.len())));
        }
    }
}

fn parse_fields(line: &[u8]) -> Vec<String> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let mut fields = Vec::new();
    let (mut field, mut in_quotes) = (Vec::new(), false);
    let mut bytes = line.iter().copied().peekable();
    while let Some(b) = bytes.next() {
        match b {
            b'"' if in_quotes && bytes.peek() == Some(&b'"') => {
                field.push(b'"');
                bytes.next();
            }
            b'"' => in_quotes = !in_quotes,
            b',' if !in_quotes => {
                fields.push(String::from_utf8_lossy(&std::mem::take(&mut field)).into_owned())
            }
            _ => field.push(b),
        }
    }
    fields.push(String::from_utf8_lossy(&field).into_owned());
    fields
}

/// The ISBN at the start of a line (`"9782070612758","Le Petit…`), as a number.
fn leading_isbn(line: &[u8]) -> Option<u64> {
    let rest = line.strip_prefix(b"\"")?;
    let end = rest.iter().position(|&b| b == b'"')?;
    let digits = &rest[..end];
    if !(10..=13).contains(&digits.len()) || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// Spaces collapsed; the publisher comes as `[Gallimard]`, and `[]` or
/// `[???]` when unknown.
fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clean_publisher(text: &str) -> String {
    let inner = text.trim().trim_start_matches('[').trim_end_matches(']');
    let inner = clean(inner);
    if inner.chars().all(|c| c == '?') {
        String::new()
    } else {
        inner
    }
}

// ------------------------------------------------------------------ index

/// Which column holds what, read from the header line.
#[derive(Debug, Clone, Copy)]
struct Columns {
    isbn: usize,
    title: usize,
    publisher: Option<usize>,
}

fn read_columns(csv: &Path) -> Result<Columns, IsbnError> {
    let file = File::open(csv).map_err(io_err)?;
    let mut reader = BufReader::new(file);
    let (header, _) = read_record(&mut reader)
        .map_err(io_err)?
        .ok_or_else(|| IsbnError::Format("Le fichier est vide.".into()))?;
    let find = |name: &str| {
        header
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let (Some(isbn), Some(title)) = (find("isbn"), find("title")) else {
        return Err(IsbnError::Format(
            "Ce fichier ne ressemble pas à la base ISBN attendue : il faut au moins les colonnes « isbn » et « title » sur la première ligne."
                .into(),
        ));
    };
    Ok(Columns {
        isbn,
        title,
        publisher: find("editeur"),
    })
}

/// Size and modification time of the CSV: if either changes, the index is stale.
fn stamp(csv: &Path) -> Result<(u64, u64), IsbnError> {
    let meta = fs::metadata(csv).map_err(io_err)?;
    let secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Ok((meta.len(), secs))
}

/// Progress of `IsbnDb::build_index`, readable from another thread.
#[derive(Debug, Default)]
pub struct Progress {
    done: AtomicU64,
    total: AtomicU64,
}

impl Progress {
    /// 0.0 to 1.0.
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (self.done.load(Ordering::Relaxed) as f32 / total as f32).min(1.0)
    }
}

/// The result of `IsbnDb::open`.
pub enum Opened {
    Ready(IsbnDb),
    /// No index yet, or the CSV changed: call `IsbnDb::build_index`.
    NeedsIndex,
}

#[derive(Debug)]
pub struct IsbnDb {
    csv: PathBuf,
    index: PathBuf,
    columns: Columns,
    count: u64,
}

impl IsbnDb {
    /// Checks the CSV and uses its index if it is up to date.
    pub fn open(csv: &Path, index: &Path) -> Result<Opened, IsbnError> {
        let columns = read_columns(csv)?;
        let (len, mtime) = stamp(csv)?;
        let Ok(mut file) = File::open(index) else {
            return Ok(Opened::NeedsIndex);
        };
        let mut header = [0u8; HEADER_LEN as usize];
        if file.read_exact(&mut header).is_err() || &header[..8] != MAGIC {
            return Ok(Opened::NeedsIndex);
        }
        let number = |i: usize| u64::from_le_bytes(header[i..i + 8].try_into().unwrap_or_default());
        let (csv_len, csv_mtime, count) = (number(8), number(16), number(24));
        let index_len = file.metadata().map_err(io_err)?.len();
        if csv_len != len || csv_mtime != mtime || index_len != HEADER_LEN + count * RECORD_LEN {
            return Ok(Opened::NeedsIndex);
        }
        Ok(Opened::Ready(IsbnDb {
            csv: csv.to_path_buf(),
            index: index.to_path_buf(),
            columns,
            count,
        }))
    }

    /// Reads the whole CSV once and writes the index. Takes a while for a
    /// big file: meant for a background thread, with `progress` to show.
    pub fn build_index(csv: &Path, index: &Path, progress: &Progress) -> Result<IsbnDb, IsbnError> {
        let columns = read_columns(csv)?;
        let (len, mtime) = stamp(csv)?;
        progress.total.store(len, Ordering::Relaxed);
        progress.done.store(0, Ordering::Relaxed);

        let mut reader = BufReader::with_capacity(1 << 20, File::open(csv).map_err(io_err)?);
        let mut records: Vec<(u64, u64)> = Vec::new();
        // The header record (it may span lines) is not a book.
        let (_, header_len) = read_record(&mut reader)
            .map_err(io_err)?
            .ok_or_else(|| IsbnError::Format("Le fichier est vide.".into()))?;
        let (mut offset, mut line, mut lines) = (header_len as u64, Vec::new(), 0u64);
        loop {
            line.clear();
            let n = reader.read_until(b'\n', &mut line).map_err(io_err)?;
            if n == 0 {
                break;
            }
            if let Some(isbn) = leading_isbn(&line) {
                records.push((isbn, offset));
            }
            offset += n as u64;
            lines += 1;
            if lines % 4096 == 0 {
                progress.done.store(offset, Ordering::Relaxed);
            }
        }
        records.sort_unstable();
        // Several lines for one ISBN: the first one.
        records.dedup_by_key(|r| r.0);

        let tmp = index.with_extension("tmp");
        {
            let mut out = BufWriter::new(File::create(&tmp).map_err(io_err)?);
            out.write_all(MAGIC).map_err(io_err)?;
            for value in [len, mtime, records.len() as u64] {
                out.write_all(&value.to_le_bytes()).map_err(io_err)?;
            }
            for (isbn, offset) in &records {
                out.write_all(&isbn.to_le_bytes()).map_err(io_err)?;
                out.write_all(&offset.to_le_bytes()).map_err(io_err)?;
            }
            out.flush().map_err(io_err)?;
        }
        fs::rename(&tmp, index).map_err(io_err)?;
        progress.done.store(len, Ordering::Relaxed);
        Ok(IsbnDb {
            csv: csv.to_path_buf(),
            index: index.to_path_buf(),
            columns,
            count: records.len() as u64,
        })
    }

    /// Number of ISBNs in the database.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// The book with this ISBN-13 (see `normalize`), if the dataset has it.
    pub fn lookup(&self, isbn13: &str) -> Result<Option<BookInfo>, IsbnError> {
        let Ok(wanted) = isbn13.parse::<u64>() else {
            return Ok(None);
        };
        let Some(offset) = self.find_offset(wanted)? else {
            return Ok(None);
        };
        let mut reader = BufReader::new(File::open(&self.csv).map_err(io_err)?);
        reader.seek(SeekFrom::Start(offset)).map_err(io_err)?;
        let Some((fields, _)) = read_record(&mut reader).map_err(io_err)? else {
            return Ok(None);
        };
        let field = |i: usize| fields.get(i).map_or("", String::as_str);
        // The CSV may have been edited since the index was built.
        if field(self.columns.isbn) != isbn13 {
            return Ok(None);
        }
        let title = tidy_title(field(self.columns.title));
        Ok(Some(BookInfo {
            title_garbled: is_garbled(&title),
            title,
            publisher: self
                .columns
                .publisher
                .map(|i| clean_publisher(field(i)))
                .unwrap_or_default(),
        }))
    }

    /// Binary search in the index file, without loading it.
    fn find_offset(&self, wanted: u64) -> Result<Option<u64>, IsbnError> {
        let mut file = File::open(&self.index).map_err(io_err)?;
        let (mut low, mut high) = (0u64, self.count);
        let mut record = [0u8; RECORD_LEN as usize];
        while low < high {
            let mid = low + (high - low) / 2;
            file.seek(SeekFrom::Start(HEADER_LEN + mid * RECORD_LEN))
                .map_err(io_err)?;
            file.read_exact(&mut record).map_err(io_err)?;
            let isbn = u64::from_le_bytes(record[..8].try_into().unwrap_or_default());
            match isbn.cmp(&wanted) {
                std::cmp::Ordering::Equal => {
                    return Ok(Some(u64::from_le_bytes(
                        record[8..].try_into().unwrap_or_default(),
                    )))
                }
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder removed at the end of the test.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            let dir =
                std::env::temp_dir().join(format!("biblo-isbn-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const CSV: &str = "\"isbn\",\"title\",\"last_updated\",\"editeur\"\n\
        \"9782070612758\",\"Le Petit Prince\",\"1\",\"[Gallimard]\"\n\
        \"9782355991097\",\"L'épreuve \"\"du\"\" Feu\",\"2\",\"[???]\"\n\
        \"9780000000002\",\"Titre sur\ndeux lignes\",\"3\",\"\"\n\
        \"9782070612758\",\"Doublon\",\"4\",\"[Autre]\"\n\
        \"9781234567897\",\"Sans éditeur\",\"5\",\"[]\"\r\n";

    fn build(dir: &TempDir) -> IsbnDb {
        let csv = dir.0.join("db.csv");
        fs::write(&csv, CSV).unwrap();
        IsbnDb::build_index(&csv, &dir.0.join("index.bin"), &Progress::default()).unwrap()
    }

    #[test]
    fn normalizes_isbn_13_and_10() {
        assert_eq!(normalize("9782070612758").as_deref(), Ok("9782070612758"));
        assert_eq!(
            normalize(" ISBN 978-2-07-061275-8 ").as_deref(),
            Ok("9782070612758")
        );
        // the same book as an ISBN-10, with an X in another one
        assert_eq!(normalize("2-07-061275-9").as_deref(), Ok("9782070612758"));
        assert_eq!(normalize("080442957X").as_deref(), Ok("9780804429573"));
        assert_eq!(normalize("9782070612759"), Err(IsbnError::BadChecksum));
        assert_eq!(normalize("2070612750"), Err(IsbnError::BadChecksum));
        assert_eq!(normalize("12345"), Err(IsbnError::Invalid));
        assert_eq!(normalize(""), Err(IsbnError::Invalid));
        assert_eq!(normalize("97820706127ab"), Err(IsbnError::Invalid));
    }

    #[test]
    fn a_scan_typed_on_the_wrong_keyboard_layout_is_understood() {
        // 9782070612758 typed by a US-layout scanner on an AZERTY Windows
        assert_eq!(normalize("çè_éàèà-&éè(_").as_deref(), Ok("9782070612758"));
    }

    #[test]
    fn finds_a_book_through_the_index() {
        let dir = TempDir::new("find");
        let db = build(&dir);
        // 4 distinct ISBNs: the repeated one counts once
        assert_eq!(db.count(), 4);
        assert_eq!(
            db.lookup("9782070612758").unwrap(),
            Some(BookInfo {
                title: "Le Petit Prince".into(),
                title_garbled: false,
                publisher: "Gallimard".into()
            })
        );
        // quotes, unknown publisher, a title over two lines, an empty publisher
        let found = db.lookup("9782355991097").unwrap().unwrap();
        assert_eq!(found.title, "L'épreuve \"du\" Feu");
        assert_eq!(found.publisher, "");
        let found = db.lookup("9780000000002").unwrap().unwrap();
        assert_eq!(found.title, "Titre sur deux lignes");
        let found = db.lookup("9781234567897").unwrap().unwrap();
        assert_eq!(
            (found.title.as_str(), found.publisher.as_str()),
            ("Sans éditeur", "")
        );
        assert_eq!(db.lookup("9780306406157").unwrap(), None);
    }

    #[test]
    fn an_up_to_date_index_is_reused_and_a_changed_csv_is_not() {
        let dir = TempDir::new("stale");
        build(&dir);
        let (csv, index) = (dir.0.join("db.csv"), dir.0.join("index.bin"));
        match IsbnDb::open(&csv, &index).unwrap() {
            Opened::Ready(db) => assert!(db.lookup("9782070612758").unwrap().is_some()),
            Opened::NeedsIndex => panic!("the index should be up to date"),
        }
        fs::write(
            &csv,
            format!("{CSV}\"9780306406157\",\"Nouveau\",\"6\",\"\"\n"),
        )
        .unwrap();
        assert!(matches!(IsbnDb::open(&csv, &index), Ok(Opened::NeedsIndex)));
        // no index at all
        assert!(matches!(
            IsbnDb::open(&csv, &dir.0.join("autre.bin")),
            Ok(Opened::NeedsIndex)
        ));
    }

    #[test]
    fn a_file_that_is_not_the_dataset_is_refused() {
        let dir = TempDir::new("format");
        let csv = dir.0.join("autre.csv");
        fs::write(&csv, "nom,prenom\nA,B\n").unwrap();
        assert!(matches!(
            IsbnDb::open(&csv, &dir.0.join("i.bin")),
            Err(IsbnError::Format(_))
        ));
        assert!(matches!(
            IsbnDb::open(&dir.0.join("absent.csv"), &dir.0.join("i.bin")),
            Err(IsbnError::Io(_))
        ));
    }
}
