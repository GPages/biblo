//! Demo data to show Biblo: about 55 books, 40 pupils (plus 4 who left the
//! school last June, for the cleanup) and a school year of loans, some of
//! them overdue. Loan dates are relative to `today`, so the overdue examples
//! stay overdue.
//!
//! Used by `cargo run --example demo_data` and by the demonstration build
//! (`--features demo`), which fills an empty data folder on its first start.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

use crate::dates::parse_date;
use crate::error::Error;
use crate::library::{BookInput, Library};

/// Title, author, collection, category, year, number of copies.
pub type DemoBook = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Option<i32>,
    usize,
);

#[rustfmt::skip] // one book per line reads better
pub const BOOKS: &[DemoBook] = &[
    ("Le Petit Prince", "Antoine de Saint-Exupéry", "Folio Junior", "Roman", Some(1943), 3),
    ("Le Petit Nicolas", "René Goscinny, Jean-Jacques Sempé", "Folio Junior", "Roman", Some(1960), 2),
    ("Les Récrés du petit Nicolas", "René Goscinny, Jean-Jacques Sempé", "Folio Junior", "Roman", Some(1961), 1),
    ("Charlie et la chocolaterie", "Roald Dahl", "Folio Junior", "Roman", Some(1964), 2),
    ("Matilda", "Roald Dahl", "Folio Junior", "Roman", Some(1988), 1),
    ("Le Bon Gros Géant", "Roald Dahl", "Folio Junior", "Roman", Some(1982), 1),
    ("Sacrées sorcières", "Roald Dahl", "Folio Junior", "Roman", Some(1983), 1),
    ("Fantastique Maître Renard", "Roald Dahl", "Folio Cadet", "Roman", Some(1970), 1),
    ("La Sorcière de la rue Mouffetard", "Pierre Gripari", "Folio Junior", "Conte", Some(1967), 2),
    ("Les Contes du chat perché", "Marcel Aymé", "Folio Junior", "Conte", Some(1939), 1),
    ("Harry Potter à l'école des sorciers", "J. K. Rowling", "Folio Junior", "Roman", Some(1998), 2),
    ("Journal d'un dégonflé", "Jeff Kinney", "Seuil Jeunesse", "Roman", None, 2),
    ("Mortelle Adèle : Tout ça finira mal", "Mr Tan, Miss Prickly", "Tourbillon", "BD", Some(2012), 2),
    ("Les Cahiers d'Esther : Histoires de mes 10 ans", "Riad Sattouf", "Allary", "BD", Some(2016), 1),
    ("Tintin au Tibet", "Hergé", "Les Aventures de Tintin", "BD", Some(1960), 1),
    ("On a marché sur la Lune", "Hergé", "Les Aventures de Tintin", "BD", Some(1954), 1),
    ("Le Lotus bleu", "Hergé", "Les Aventures de Tintin", "BD", Some(1936), 1),
    ("Astérix le Gaulois", "René Goscinny, Albert Uderzo", "Astérix", "BD", Some(1961), 1),
    ("Astérix et Cléopâtre", "René Goscinny, Albert Uderzo", "Astérix", "BD", Some(1965), 1),
    ("L'Élève Ducobu", "Zidrou, Godi", "L'Élève Ducobu", "BD", None, 1),
    ("Max et Lili ont peur", "Dominique de Saint Mars, Serge Bloch", "Max et Lili", "Album", None, 1),
    ("Max et Lili se disputent", "Dominique de Saint Mars, Serge Bloch", "Max et Lili", "Album", None, 1),
    ("Le Loup qui voulait changer de couleur", "Orianne Lallemand, Éléonore Thuillier", "Auzou", "Album", Some(2009), 1),
    ("Le Monstre des couleurs", "Anna Llenas", "Quatre Fleuves", "Album", None, 1),
    ("Le Gruffalo", "Julia Donaldson, Axel Scheffler", "Gallimard Jeunesse", "Album", Some(1999), 1),
    ("Les Trois Brigands", "Tomi Ungerer", "L'École des loisirs", "Album", None, 1),
    ("Ernest et Célestine ont perdu Siméon", "Gabrielle Vincent", "Casterman", "Album", Some(1981), 1),
    ("La Petite Poule qui voulait voir la mer", "Christian Jolibois, Christian Heinrich", "Pocket Jeunesse", "Album", Some(2000), 1),
    ("Chien Pourri !", "Colas Gutman, Marc Boutavant", "L'École des loisirs", "Roman", Some(2013), 1),
    ("Le Prince de Motordu", "Pef", "Folio Benjamin", "Album", Some(1980), 1),
    ("Mademoiselle Zazie a-t-elle un zizi ?", "Thierry Lenain, Delphine Durand", "Nathan", "Roman", Some(1998), 1),
    ("Le Hollandais sans peine", "Marie-Aude Murail", "L'École des loisirs", "Roman", Some(1989), 1),
    ("L'Enfant et la Rivière", "Henri Bosco", "Folio Junior", "Roman", Some(1945), 1),
    ("L'Œil du loup", "Daniel Pennac", "Pocket Jeunesse", "Roman", Some(1984), 1),
    ("Cabot-Caboche", "Daniel Pennac", "Pocket Jeunesse", "Roman", Some(1982), 1),
    ("Le Livre de la jungle", "Rudyard Kipling", "Folio Junior", "Roman", Some(1894), 1),
    ("Alice au pays des merveilles", "Lewis Carroll", "Folio Junior", "Roman", Some(1865), 1),
    ("Le Tour du monde en quatre-vingts jours", "Jules Verne", "Folio Junior", "Roman", None, 1),
    ("Les Malheurs de Sophie", "Comtesse de Ségur", "Bibliothèque Rose", "Roman", None, 1),
    ("Le Club des Cinq et le trésor de l'île", "Enid Blyton", "Bibliothèque Rose", "Roman", None, 1),
    ("Le Journal de Gurty : Vacances en Provence", "Bertrand Santini", "Sarbacane", "Roman", None, 1),
    ("Fables", "Jean de La Fontaine", "", "Poésie", Some(1668), 1),
    // Documentaries: no author, to show that only the title is required.
    ("Les Dinosaures", "", "", "Documentaire", None, 1),
    ("Le Corps humain", "", "", "Documentaire", None, 1),
    ("Les Volcans", "", "", "Documentaire", None, 1),
    ("Le Système solaire", "", "", "Documentaire", None, 1),
];

/// Last name, first name, birth date (fictional pupils, CP to CM2).
const PUPILS: &[(&str, &str, &str)] = &[
    ("Martin", "Léa", "14/05/2017"),
    ("Martin", "Léa", "02/01/2019"), // same name, other birth date: another pupil
    ("Durand", "Tom", "03/11/2016"),
    ("Durand", "Jade", "21/06/2019"),
    ("Bénard", "Chloé", "27/02/2018"),
    ("Lefèvre", "Zoé", "09/09/2016"),
    ("Petit", "Lucas", "12/09/2016"),
    ("Moreau", "Gabriel", "30/03/2015"),
    ("Dubois", "Emma", "17/07/2017"),
    ("Laurent", "Hugo", "05/12/2018"),
    ("Simon", "Inès", "23/04/2016"),
    ("Michel", "Arthur", "11/10/2019"),
    ("Leroy", "Louise", "08/08/2015"),
    ("Roux", "Nathan", "19/01/2017"),
    ("David", "Alice", "28/05/2020"),
    ("Bertrand", "Raphaël", "14/02/2016"),
    ("Morel", "Lina", "03/06/2018"),
    ("Fournier", "Jules", "25/11/2015"),
    ("Girard", "Rose", "16/03/2019"),
    ("Bonnet", "Adam", "07/07/2016"),
    ("Dupont", "Mia", "12/12/2017"),
    ("Lambert", "Noah", "01/04/2020"),
    ("Fontaine", "Ambre", "22/09/2015"),
    ("Rousseau", "Maël", "13/05/2018"),
    ("Vincent", "Juliette", "29/10/2016"),
    ("Muller", "Sacha", "18/02/2019"),
    ("Faure", "Agathe", "06/01/2015"),
    ("André", "Timéo", "24/08/2017"),
    ("Mercier", "Romane", "15/04/2020"),
    ("Blanc", "Ethan", "02/11/2015"),
    ("Guérin", "Lou", "10/06/2018"),
    ("Boyer", "Paul", "31/01/2017"),
    ("Garnier", "Anna", "20/12/2019"),
    ("Chevalier", "Nolan", "04/03/2016"),
    ("Benali", "Yanis", "26/07/2018"),
    ("Diallo", "Aïssatou", "11/05/2017"),
    ("Nguyen", "Kim", "09/02/2016"),
    ("Haddad", "Nour", "17/10/2019"),
    ("Da Silva", "Gabin", "05/05/2015"),
    ("Le Gall", "Maëlys", "14/09/2018"),
];

/// Pupils who left last June: last name, first name, birth date, and how many
/// school years ago they were registered. The first one never returned a book.
const FORMER_PUPILS: &[(&str, &str, &str, i32)] = &[
    ("Perrin", "Théo", "15/06/2014", 5),
    ("Garcia", "Lucas", "08/02/2014", 5),
    ("Robin", "Manon", "23/11/2013", 6),
    ("Fabre", "Camille", "02/09/2013", 6),
];

/// Pupils were first registered when the school started using Biblo,
/// this many school years ago (or when they entered CP, if later).
const BIBLO_SINCE: i32 = 3;

/// How far back the loan history goes.
const HISTORY_DAYS: i64 = 280;

/// Small deterministic generator (xorshift), so every run gives the same data.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// Library sessions: Tuesday and Friday, outside summer and Christmas holidays.
fn is_library_day(d: NaiveDate) -> bool {
    let session = matches!(d.weekday(), Weekday::Tue | Weekday::Fri);
    let summer = matches!(d.month(), 7 | 8);
    let christmas = (d.month() == 12 && d.day() >= 20) || (d.month() == 1 && d.day() <= 4);
    session && !summer && !christmas
}

/// Fills an empty library with the demo books, pupils and loans.
pub fn fill(lib: &mut Library, today: NaiveDate) -> Result<(), Error> {
    for &(title, author, collection, category, year, copies) in BOOKS {
        let input = BookInput {
            title: title.into(),
            author: author.into(),
            collection: collection.into(),
            category: category.into(),
            year,
            isbn: String::new(),
        };
        lib.add_copies(input, copies)?;
    }
    // Registration: the 1st of September of a school year.
    let this_school_year = if today.month() >= 9 {
        today.year()
    } else {
        today.year() - 1
    };
    let september = |year: i32| NaiveDate::from_ymd_opt(year, 9, 1).expect("valid date");
    let mut pupils = Vec::new();
    for &(last, first, birth) in PUPILS {
        let birth = parse_date(birth).expect("dates in PUPILS are valid");
        let enters_cp = birth.year() + 6;
        let registered = september(enters_cp.max(this_school_year - BIBLO_SINCE));
        pupils.push(lib.add_student(last, first, birth, registered)?);
    }
    let mut former = Vec::new();
    for &(last, first, birth, years_ago) in FORMER_PUPILS {
        let birth = parse_date(birth).expect("dates in FORMER_PUPILS are valid");
        let registered = september(this_school_year - years_ago);
        former.push(lib.add_student(last, first, birth, registered)?);
    }

    // Simulate every session: returns first, then new loans. A pupil holds
    // one book at a time; about 1 loan in 5 is "forgotten" for 4 to 9 weeks.
    let books: Vec<u32> = lib.books().iter().map(|b| b.id).collect();
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut out: Vec<(u32, NaiveDate)> = Vec::new(); // (book, planned return)
    let mut day = today - Duration::days(HISTORY_DAYS);
    let mut first_session = true;
    while day <= today {
        if is_library_day(day) {
            if std::mem::take(&mut first_session) {
                // The book the first former pupil never brought back.
                lib.lend(*books.last().expect("demo books"), former[0], day)?;
            }
            let (back, still_out) = out.into_iter().partition(|&(_, due)| due <= day);
            out = still_out;
            for (book, _) in back {
                lib.return_book(book, day)?;
            }
            for _ in 0..8 + rng.below(5) {
                let pupil = pupils[rng.below(pupils.len())];
                let book = books[rng.below(books.len())];
                if lib.open_loan_for_book(book).is_some()
                    || !lib.open_loans_of_student(pupil).is_empty()
                {
                    continue;
                }
                lib.lend(book, pupil, day)?;
                let days = if rng.below(5) == 0 {
                    28 + rng.below(35)
                } else {
                    7 + rng.below(11)
                };
                out.push((book, day + Duration::days(days as i64)));
            }
        }
        day += Duration::days(1);
    }
    Ok(())
}
