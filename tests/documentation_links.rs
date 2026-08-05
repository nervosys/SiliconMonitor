//! Every relative link and image in the repository's markdown must resolve.
//!
//! The README carried six `docs/images/*.png` references for months. None of the
//! files was ever added, so the two "Screenshots" sections rendered as broken
//! images on GitHub — and, once the crate was published, on its crates.io page
//! too. Nothing caught it because a broken *image* fails silently: the markdown is
//! valid, the build is green, and the damage is only visible to a human looking at
//! the rendered page, which is exactly the thing nobody re-reads.
//!
//! A missing target is checkable, so it should be checked rather than noticed.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Markdown files tracked by git, so generated and vendored trees are skipped.
fn tracked_markdown() -> Vec<PathBuf> {
    let root = repo_root();
    let output = Command::new("git")
        .args(["ls-files", "*.md"])
        .current_dir(&root)
        .output()
        .expect("git ls-files should run inside the repository");
    assert!(output.status.success(), "git ls-files failed");

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| root.join(line))
        .collect()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Pull the target out of every `[text](target)` and `![alt](target)`.
fn link_targets(markdown: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let bytes: Vec<char> = markdown.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == ']' && i + 1 < bytes.len() && bytes[i + 1] == '(' {
            let start = i + 2;
            let mut end = start;
            while end < bytes.len() && bytes[end] != ')' && bytes[end] != '\n' {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == ')' {
                targets.push(bytes[start..end].iter().collect());
            }
            i = end;
        }
        i += 1;
    }
    targets
}

/// A target simon is responsible for: not a URL, not a bare anchor.
fn is_local(target: &str) -> bool {
    !target.starts_with("http://")
        && !target.starts_with("https://")
        && !target.starts_with("mailto:")
        && !target.starts_with('#')
        && !target.is_empty()
}

#[test]
fn every_relative_link_and_image_resolves() {
    let files = tracked_markdown();
    assert!(
        !files.is_empty(),
        "no tracked markdown found; the checker would pass vacuously"
    );

    let mut broken = Vec::new();
    let mut checked = 0usize;

    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        // Resolve relative to the file that contains the link, not to the working
        // directory. An earlier version of this check got that wrong and reported
        // a file that existed as missing, which is worse than no check at all —
        // it produced a confident, wrong claim that was then written down.
        let dir = file.parent().unwrap_or_else(|| Path::new("."));

        for target in link_targets(&text) {
            if !is_local(&target) {
                continue;
            }
            // Strip any `#anchor`; the file is what must exist.
            let path_part = target.split('#').next().unwrap_or(&target);
            if path_part.is_empty() {
                continue;
            }
            checked += 1;
            if !dir.join(path_part).exists() {
                broken.push(format!(
                    "{} -> {target}",
                    file.strip_prefix(repo_root()).unwrap_or(file).display()
                ));
            }
        }
    }

    assert!(
        broken.is_empty(),
        "{} of {checked} relative links do not resolve:\n  {}",
        broken.len(),
        broken.join("\n  ")
    );
}

/// Documentation must not carry identifiers belonging to whoever built it.
///
/// The TUI capture embedded in the README is real output, and real output from
/// this tool includes the machine's hostname. Sanitizing it was a manual step, and
/// manual steps are the ones that get skipped the next time someone refreshes the
/// capture.
#[test]
fn documentation_carries_no_machine_identifiers() {
    // Identifiers observed on the machine this was authored on. The point is not
    // this particular list — it is that pasting real tool output into docs is a
    // routine way to leak one, so the routine gets a check.
    const IDENTIFIERS: &[&str] = &["heimdall", "adamm"];

    let mut found = Vec::new();
    for file in tracked_markdown() {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let lowered = text.to_lowercase();
        for id in IDENTIFIERS {
            if lowered.contains(id) {
                found.push(format!(
                    "{} contains {id:?}",
                    file.strip_prefix(repo_root()).unwrap_or(&file).display()
                ));
            }
        }
    }

    assert!(
        found.is_empty(),
        "documentation leaks a machine identifier:\n  {}",
        found.join("\n  ")
    );
}
