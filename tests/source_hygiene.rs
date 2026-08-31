//! Checks on the source text itself, for a defect class that type-checking,
//! clippy and every runtime test are blind to.

use std::path::{Path, PathBuf};

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// A string literal must not carry a run of source indentation inside it.
///
/// Rust's `\` line continuation eats the newline *and* the leading whitespace
/// of the next line, so a multi-line literal written that way is correct in
/// the file and correct on screen. It stops being correct when the backslash
/// goes missing in editing — the literal stays valid Rust and silently gains
/// the indentation, and the user reads a sentence with twenty spaces in the
/// middle of it.
///
/// Nothing else catches this. It compiles, it lints clean, and every assertion
/// about the value still passes because the words are all still there. Four
/// instances were introduced in one session and a fifth had already shipped in
/// `4846c3f`, in an EDID setting description, before this test existed.
///
/// The fix is always the same, and is the convention in this crate: write the
/// literal as `concat!("first part ", "second part")`, which cannot collapse
/// because there is no continuation to lose.
#[test]
fn string_literals_carry_no_collapsed_indentation() {
    let mut files = Vec::new();
    rust_sources(Path::new("src"), &mut files);
    rust_sources(Path::new("tests"), &mut files);
    rust_sources(Path::new("examples"), &mut files);
    assert!(!files.is_empty(), "found no Rust sources to check");

    let mut bad = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            // Comments legitimately hold aligned prose and ASCII tables.
            if trimmed.starts_with("//") {
                continue;
            }
            if !line.contains('"') {
                continue;
            }
            // Six spaces between two lowercase letters. Deliberate alignment
            // pads columns or draws boxes; it does not sit between two words
            // in the middle of a sentence.
            let bytes: Vec<char> = line.chars().collect();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i].is_ascii_lowercase() || bytes[i] == ',' {
                    let mut j = i + 1;
                    while j < bytes.len() && bytes[j] == ' ' {
                        j += 1;
                    }
                    if j - i > 6 && j < bytes.len() && bytes[j].is_ascii_lowercase() {
                        bad.push(format!("{}:{}", path.display(), n + 1));
                        break;
                    }
                    i = j;
                } else {
                    i += 1;
                }
            }
        }
    }

    assert!(
        bad.is_empty(),
        concat!(
            "string literals with source indentation collapsed into them, ",
            "probably a lost line continuation. Rewrite each as ",
            "concat!(..., ...):\n  {}"
        ),
        bad.join("\n  ")
    );
}
