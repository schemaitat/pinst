//! Running a skill's `evidence:` one-liner.
//!
//! A skill declares in its own frontmatter what it produces and the command
//! that measures it, because the skill is the only thing that knows what it
//! is for. Put the measure in the checker and the two drift — which is the
//! failure the whole audit exists to catch.
//!
//! # This executes shell from the repo being checked
//!
//! `evidence:` is a `sh -c` string read out of a file in the corpus. The
//! bash harness did exactly the same, but the blast radius is not the same:
//! a script you had to clone has become a binary you installed and trust, and
//! `pinst harness skills` in a repo someone handed you will run that repo's
//! shell. Three things keep it honest, and none of them is a sandbox:
//!
//! - `--no-evidence` turns it off entirely.
//! - Every run is bounded by [`TIMEOUT`].
//! - `evidence:` is code, and reviewing it is part of reviewing the skill.
//!
//! The default is still "on", because a measurement that has to be asked for
//! is a measurement nobody takes.

use std::process::{Command, Stdio};
use std::time::Duration;

/// A measure that hangs must not hang `just review`.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// How many units of history the windowed rate looks at.
///
/// Both a windowed and an all-time rate get reported, because a rate over all
/// history is dominated by history: a regression shows up while it is still
/// one commit old instead of being buried under a hundred conformant ones.
pub const WINDOW: u32 = 20;

/// `<conforming> <total>` from one run of a measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    pub conforming: u64,
    pub total: u64,
}

impl Rate {
    /// `n/m pp%`, or `n/a` when the measure found nothing to measure.
    pub fn label(&self) -> String {
        match self.percent() {
            Some(pct) => format!("{}/{} {pct}%", self.conforming, self.total),
            None => format!("{}/{} n/a", self.conforming, self.total),
        }
    }

    pub fn percent(&self) -> Option<u64> {
        (self.total > 0).then(|| self.conforming * 100 / self.total)
    }
}

/// Runs one measure over a window, or over all history when `window` is
/// `None`.
///
/// Returns `None` for anything that is not a clean exit printing exactly two
/// integers. The caller reports that as `unmeasured` and never as `0`,
/// because a zero meaning "offline" is worse than a gap that admits it.
/// The timeout is a parameter rather than a constant read inside: a test
/// that wants a shorter one should not have to reach around the API to get
/// it, and the alternative here was a 30-second wait on every `cargo test`
/// (LESSON-018).
pub fn run(
    command: &str,
    dir: &std::path::Path,
    window: Option<u32>,
    timeout: Duration,
) -> Option<Rate> {
    // Both spellings, so a measure can use whichever fits: `$ASH_RANGE` drops
    // straight into a `git log`, `$ASH_WINDOW` into a `--limit`.
    let (range, window_value) = match window {
        Some(n) => (format!("-n {n}"), n.to_string()),
        None => (String::new(), String::new()),
    };

    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .env("ASH_RANGE", range)
        .env("ASH_WINDOW", window_value)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let output = wait_with_timeout(child, timeout)?;
    if !output.status.success() {
        return None;
    }
    parse(&String::from_utf8_lossy(&output.stdout))
}

/// Waits for the child, killing it at [`TIMEOUT`].
///
/// Polled rather than threaded: this is called a dozen times per report, and
/// a spawned reader thread per call is more machinery than a 50ms poll over a
/// 30-second ceiling deserves.
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Option<std::process::Output> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {}
            Err(_) => return None,
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Accepts exactly two integers and nothing else.
///
/// Strict on purpose: a measure that printed a diagnostic, or one number, or
/// three, has not measured anything, and guessing which number was meant is
/// how a report starts stating a rate nobody computed.
fn parse(stdout: &str) -> Option<Rate> {
    let fields: Vec<&str> = stdout.split_whitespace().collect();
    let [conforming, total] = fields.as_slice() else {
        return None;
    };
    Some(Rate {
        conforming: conforming.parse().ok()?,
        total: total.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn here() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn reads_two_integers() {
        assert_eq!(
            run("echo 3 4", &here(), None, TIMEOUT),
            Some(Rate {
                conforming: 3,
                total: 4
            })
        );
    }

    #[test]
    fn both_window_variables_reach_the_command() {
        assert_eq!(
            run("echo $ASH_WINDOW 1", &here(), Some(20), TIMEOUT),
            Some(Rate {
                conforming: 20,
                total: 1
            })
        );
        // `$ASH_RANGE` is git-ready, so it carries the flag as well.
        assert_eq!(
            run(
                "echo $(echo $ASH_RANGE | tr -dc 0-9) 1",
                &here(),
                Some(7),
                TIMEOUT
            ),
            Some(Rate {
                conforming: 7,
                total: 1
            })
        );
        // All-time leaves both empty rather than unset.
        assert_eq!(
            run("test -z \"$ASH_RANGE\" && echo 1 1", &here(), None, TIMEOUT),
            Some(Rate {
                conforming: 1,
                total: 1
            })
        );
    }

    /// Anything that is not two integers is unmeasured, which the caller must
    /// never render as a zero.
    #[test]
    fn anything_else_is_unmeasured() {
        for command in [
            "echo 3",              // one number
            "echo 3 4 5",          // three
            "echo three four",     // not numbers
            "echo 3 4; exit 1",    // non-zero exit
            "nosuchcommand-xyzzy", // does not exist
            "echo -3 4",           // negative
        ] {
            assert_eq!(run(command, &here(), None, TIMEOUT), None, "{command}");
        }
    }

    /// A measure that hangs must not hang the report — and the test for it
    /// must not hang `cargo test`, which is why the timeout is an argument.
    #[test]
    fn a_hanging_measure_is_killed_rather_than_waited_on() {
        let budget = Duration::from_millis(300);
        let started = std::time::Instant::now();
        assert_eq!(run("sleep 120", &here(), None, budget), None);
        assert!(
            started.elapsed() < budget * 10,
            "took {:?}, which is not a kill",
            started.elapsed()
        );
    }

    #[test]
    fn a_rate_labels_itself_and_never_divides_by_zero() {
        assert_eq!(
            Rate {
                conforming: 3,
                total: 4
            }
            .label(),
            "3/4 75%"
        );
        assert_eq!(
            Rate {
                conforming: 0,
                total: 0
            }
            .label(),
            "0/0 n/a"
        );
    }
}
