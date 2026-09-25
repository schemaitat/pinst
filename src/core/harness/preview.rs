//! What is safe to show in the Harness reading pane.
//!
//! A skill or command can be text, a multi-megabyte reference, or a binary
//! someone dropped into a vendor directory. The pane must never dump the last
//! two into the terminal, so this module is the one place that decides what a
//! preview is. Everything except `read`/`from_bytes` is pure, so the decision
//! is unit-testable without a frame.

use std::path::Path;

/// The most source or target bytes the pane will show. Past this the head is
/// shown with an explicit notice; the asset on disk is untouched.
pub const MAX_PREVIEW_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preview {
    /// UTF-8 text. `truncated` says the body is only the first
    /// `MAX_PREVIEW_BYTES` of `total_bytes`.
    Text {
        body: String,
        total_bytes: usize,
        truncated: bool,
    },
    /// Zero bytes at the path.
    Empty,
    /// The path holds nothing.
    Missing,
    /// Bytes unsafe to print: a NUL byte or invalid UTF-8.
    Binary { bytes: usize },
    /// A read failed for a reason other than absence.
    Error(String),
}

impl Preview {
    pub fn classify(bytes: &[u8]) -> Self {
        if bytes.is_empty() {
            return Preview::Empty;
        }
        if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
            return Preview::Binary { bytes: bytes.len() };
        }
        let text = std::str::from_utf8(bytes).expect("checked above");
        if bytes.len() > MAX_PREVIEW_BYTES {
            let end = floor_char_boundary(text, MAX_PREVIEW_BYTES);
            Preview::Text {
                body: text[..end].to_string(),
                total_bytes: bytes.len(),
                truncated: true,
            }
        } else {
            Preview::Text {
                body: text.to_string(),
                total_bytes: bytes.len(),
                truncated: false,
            }
        }
    }

    /// Classifies bytes already read by another helper — `asset::content`, for
    /// the embedded tree — where the read result is an `eyre` error.
    pub fn from_bytes(bytes: color_eyre::eyre::Result<Vec<u8>>) -> Self {
        match bytes {
            Ok(bytes) => Self::classify(&bytes),
            Err(error) => Self::Error(error.to_string()),
        }
    }

    /// Reads and classifies a path on disk, distinguishing "not there" from
    /// "could not read".
    pub fn read(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => Self::classify(&bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::Missing,
            Err(error) => Self::Error(error.to_string()),
        }
    }
}

/// The largest index `<= max` that is a UTF-8 character boundary in `text`.
/// Clamping a byte cap this way is what keeps truncation from panicking or
/// emitting a replacement character.
fn floor_char_boundary(text: &str, max: usize) -> usize {
    let mut end = max.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_utf8_is_text() {
        assert_eq!(
            Preview::classify(b"hello\n"),
            Preview::Text {
                body: "hello\n".to_string(),
                total_bytes: 6,
                truncated: false,
            }
        );
    }

    #[test]
    fn nothing_is_empty_and_a_vanished_path_is_missing() {
        assert_eq!(Preview::classify(b""), Preview::Empty);
        assert_eq!(
            Preview::read(Path::new("/nonexistent/pinst-preview-path")),
            Preview::Missing
        );
    }

    #[test]
    fn nul_or_invalid_utf8_is_binary() {
        assert_eq!(Preview::classify(b"ab\0cd"), Preview::Binary { bytes: 5 });
        assert_eq!(
            Preview::classify(&[0xff, 0xfe]),
            Preview::Binary { bytes: 2 }
        );
    }

    #[test]
    fn oversized_content_is_truncated_on_a_char_boundary_with_a_count() {
        let mut bytes = "é".repeat(MAX_PREVIEW_BYTES).into_bytes();
        bytes.extend_from_slice(b"tail");
        let preview = Preview::classify(&bytes);

        match preview {
            Preview::Text {
                body,
                total_bytes,
                truncated,
            } => {
                assert!(truncated);
                assert_eq!(total_bytes, bytes.len());
                assert!(body.len() <= MAX_PREVIEW_BYTES);
                assert!(body.is_char_boundary(body.len()), "must not split a char");
            }
            other => panic!("expected truncated text, got {other:?}"),
        }
    }
}
