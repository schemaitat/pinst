//! Minting a plan id: `<yymmdd>-<six lowercase letters>`, e.g. `260919-qwerty`.
//!
//! Both halves earn their place, and both rationales are load-bearing enough
//! that moving this out of bash must not lose them.
//!
//! The hash half is **random, not allocated**. Work here happens in parallel
//! git worktrees branched off the same commit, so any "next number in
//! sequence" scheme has two sessions computing the same answer and colliding
//! at merge — by which time the id is already written into commit footers and
//! can no longer be changed without breaking the traceability it exists to
//! provide. Six letters is 26^6 ~= 309M, ample for a corpus of dozens.
//!
//! The date half is `yymmdd`, most-significant-first, so string order *is*
//! date order: `ls .ash/plans/` and the generated index both come out
//! chronological with no sort key. Under the friendlier-looking `ddmmyy`,
//! `011026` (1 Oct) would sort before `180926` (18 Sep).

use std::io::Read;

use color_eyre::eyre::{Context, Result, eyre};

const LETTERS: usize = 6;

/// Mints an id. `date` is an optional ISO `YYYY-MM-DD`; without one, today
/// in UTC.
pub fn mint(date: Option<&str>) -> Result<String> {
    let stamp = match date {
        Some(iso) => stamp_from_iso(iso)?,
        None => today_utc()?,
    };
    Ok(format!("{stamp}-{}", letters(LETTERS)?))
}

/// `2026-09-19` -> `260919`.
fn stamp_from_iso(iso: &str) -> Result<String> {
    let parts: Vec<&str> = iso.split('-').collect();
    let [year, month, day] = parts.as_slice() else {
        return Err(eyre!("expected a date as YYYY-MM-DD, got '{iso}'"));
    };
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return Err(eyre!("expected a date as YYYY-MM-DD, got '{iso}'"));
    }
    if !iso.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err(eyre!("expected a date as YYYY-MM-DD, got '{iso}'"));
    }
    Ok(format!("{}{month}{day}", &year[2..]))
}

fn today_utc() -> Result<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("the system clock is before the Unix epoch")?
        .as_secs();
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    Ok(format!("{:02}{month:02}{day:02}", year % 100))
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to a calendar
/// date, proleptic Gregorian, no leap seconds and no table.
///
/// Written out rather than pulled in: a date crate would be a dependency on a
/// binary tuned for size (`opt-level = "z"`, `lto`, `strip`) for the sake of
/// one call, and this arithmetic is small, exact and pinned by the tests below.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `n` lowercase letters from the OS entropy source.
///
/// Reads a fixed block and filters it rather than streaming `/dev/urandom`
/// through a take — the bash version had to do the same, for a different
/// reason (`head -c` closes the pipe early and `pipefail` turns the SIGPIPE
/// into a failure). Roughly 26/256 of bytes are usable, so 256 bytes yields
/// ~26 letters and the loop almost never runs twice.
fn letters(n: usize) -> Result<String> {
    let mut file = std::fs::File::open("/dev/urandom").context("cannot read /dev/urandom")?;
    let mut out = String::with_capacity(n);
    let mut block = [0u8; 256];
    while out.len() < n {
        file.read_exact(&mut block)
            .context("cannot read /dev/urandom")?;
        for byte in block {
            if byte.is_ascii_lowercase() {
                out.push(byte as char);
                if out.len() == n {
                    break;
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn well_formed(id: &str) -> bool {
        let Some((date, hash)) = id.split_once('-') else {
            return false;
        };
        date.len() == 6
            && date.chars().all(|c| c.is_ascii_digit())
            && hash.len() == LETTERS
            && hash.chars().all(|c| c.is_ascii_lowercase())
    }

    #[test]
    fn mints_a_well_formed_id() {
        let id = mint(None).unwrap();
        assert!(well_formed(&id), "{id}");
    }

    /// The property the random half exists for: two worktrees minting at the
    /// same moment must not collide.
    #[test]
    fn a_hundred_ids_are_all_distinct() {
        let ids: HashSet<String> = (0..100).map(|_| mint(None).unwrap()).collect();
        assert_eq!(ids.len(), 100);
    }

    #[test]
    fn an_iso_date_becomes_the_yymmdd_half() {
        assert!(mint(Some("2026-09-19")).unwrap().starts_with("260919-"));
        assert!(mint(Some("2026-01-01")).unwrap().starts_with("260101-"));
    }

    #[test]
    fn a_malformed_date_is_an_error_rather_than_a_guess() {
        for bad in ["19-09-2026", "2026-9-19", "not-a-date", "20260919"] {
            assert!(mint(Some(bad)).is_err(), "{bad} should be rejected");
        }
    }

    /// Pinned against dates whose day-number is independently known, including
    /// both sides of a leap day and an epoch boundary.
    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_716), (2026, 9, 20));
    }

    /// `yymmdd` is chosen so that string order is date order; a `ddmmyy` id
    /// would sort 1 October before 18 September and the whole index with it.
    #[test]
    fn string_order_is_date_order() {
        let mut ids = [
            mint(Some("2026-10-01")).unwrap(),
            mint(Some("2026-09-18")).unwrap(),
        ];
        ids.sort();
        assert!(ids[0].starts_with("260918-"), "{ids:?}");
    }
}
