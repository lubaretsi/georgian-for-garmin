//! Which part a file in `GARMIN/ExtData` belongs to, and what it is for that part, from its name
//! alone: the one rule every pre-write check and every state read uses, in both listings.
//!
//! Garmin names a part's files by its part number, `006-D9035-02`, or by a short form without
//! the `006-` prefix and the dash, `D903502` (some downloads, and the files some watches declare,
//! such as the Lily 2 Active's `D844870.tmp`). Both forms name the same part, in any case.

/// What a part's file is, by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// `.tmp`: a bitmap part waiting to be imported at the next restart.
    Waiting,
    /// `.fnt`: the font the watch imported a bitmap part's `.tmp` as.
    Imported,
    /// `.bin`: a TrueType part, which the watch loads as it is.
    Bin,
}

/// A part's file: the part number in its long form, and what the file is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PartFile {
    /// e.g. `006-D9035-02`.
    pub part: String,
    pub role: Role,
}

/// `name` as a part's file: `006-D9035-02.tmp`, `d903502.TMP` → `006-D9035-02`, waiting.
/// `None` for any other name.
pub fn resolve(name: &str) -> Option<PartFile> {
    let (stem, ext) = name.rsplit_once('.')?;
    let role = match ext.to_ascii_lowercase().as_str() {
        "tmp" => Role::Waiting,
        "fnt" => Role::Imported,
        "bin" => Role::Bin,
        _ => return None,
    };
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let (four, two) = match stem.len() {
        12 if stem.is_char_boundary(5)
            && stem[..5].eq_ignore_ascii_case("006-d")
            && stem.as_bytes()[9] == b'-'
            && digits(&stem[5..9])
            && digits(&stem[10..]) =>
        {
            (&stem[5..9], &stem[10..])
        }
        7 if stem.as_bytes()[0].eq_ignore_ascii_case(&b'd') && digits(&stem[1..]) => {
            (&stem[1..5], &stem[5..])
        }
        _ => return None,
    };
    Some(PartFile {
        part: format!("006-D{four}-{two}"),
        role,
    })
}

/// Whether `name` is `part`'s file with `role`, in either name form.
pub fn is(name: &str, part: &str, role: Role) -> bool {
    resolve(name).is_some_and(|f| f.role == role && f.part.eq_ignore_ascii_case(part))
}

/// `part`'s short form: `006-D9035-02` → `D903502`; `None` for anything that isn't a part
/// number.
pub fn short_form(part: &str) -> Option<String> {
    resolve(&format!("{part}.tmp"))
        .filter(|f| f.part.eq_ignore_ascii_case(part))
        .map(|f| format!("D{}{}", &f.part[5..9], &f.part[10..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(part: &str, role: Role) -> Option<PartFile> {
        Some(PartFile {
            part: part.into(),
            role,
        })
    }

    #[test]
    fn both_name_forms_resolve_to_the_same_part() {
        for (name, expected) in [
            ("006-D9035-02.tmp", file("006-D9035-02", Role::Waiting)),
            ("D903502.tmp", file("006-D9035-02", Role::Waiting)),
            ("d903502.TMP", file("006-D9035-02", Role::Waiting)),
            ("006-d8945-20.TMP", file("006-D8945-20", Role::Waiting)),
            ("006-D8168-13.fnt", file("006-D8168-13", Role::Imported)),
            ("D816813.fnt", file("006-D8168-13", Role::Imported)),
            ("D894734.fnt", file("006-D8947-34", Role::Imported)),
            ("006-D9971-09.bin", file("006-D9971-09", Role::Bin)),
            ("D095208.BIN", file("006-D0952-08", Role::Bin)),
            ("D844870.tmp", file("006-D8448-70", Role::Waiting)),
        ] {
            assert_eq!(resolve(name), expected, "{name}");
        }
        for name in [
            "006-D9035-3.tmp",
            "006-D12a4-03.tmp",
            "006-B9035-03.tmp",
            "006-D9035-03.bix",
            "006-D9035-03.tmpx",
            "D90350.tmp",
            "D9035022.tmp",
            "X903502.tmp",
            "006-D9035_02.tmp",
            "../D903502.tmp",
            "D903502",
            ".tmp",
            "GarminDevice.xml",
            "ДD903502.tmp",
        ] {
            assert_eq!(resolve(name), None, "{name}");
        }
    }

    #[test]
    fn matching_and_short_forms() {
        assert!(is("D903502.tmp", "006-D9035-02", Role::Waiting));
        assert!(is("006-d9035-02.TMP", "006-D9035-02", Role::Waiting));
        assert!(!is("D903502.fnt", "006-D9035-02", Role::Waiting));
        assert!(!is("D903503.tmp", "006-D9035-02", Role::Waiting));
        assert_eq!(short_form("006-D8448-70").as_deref(), Some("D844870"));
        assert_eq!(short_form("D844870"), None);
        assert_eq!(short_form("french.ln4"), None);
    }
}
