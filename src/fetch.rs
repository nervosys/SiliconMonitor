// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! A neofetch-style summary: logo on the left, the machine on the right.
//!
//! # What this is for
//!
//! One screen a person can read at a glance, and paste into an issue. It is the
//! least precise surface simon has and the most looked at, which is exactly why
//! it must not round an absence into a plausible number.
//!
//! # Unknown is printed as unknown
//!
//! Every other tool of this shape prints a dash, a zero, or omits the line. This
//! one prints the reason where there is room for it, because the difference
//! between "this machine has no GPU" and "simon cannot read GPUs on this
//! platform" is the difference between buying hardware and filing a bug.
//!
//! The rendering is deliberately built from [`crate::ontology::resolve`] rather
//! than from the individual monitors. One resolver means one set of answers: a
//! summary that disagreed with `simon snapshot` about the same machine would
//! make both untrustworthy, and the summary is the one people quote.

use crate::ontology::resolve::Reading;
use std::collections::BTreeMap;

/// ASCII art, chosen for the platform.
///
/// Deliberately generic shapes rather than trademarked logos: this crate is
/// AGPL and ships no artwork it does not own.
pub fn logo() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &[
            "    ####  ####    ",
            "   #####  #####   ",
            "  ######  ######  ",
            "                  ",
            "  ######  ######  ",
            "   #####  #####   ",
            "    ####  ####    ",
        ]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            "      .---.       ",
            "     /     \\      ",
            "     |()  ()|     ",
            "     |  __ |      ",
            "    /|      |\\    ",
            "   / |      | \\   ",
            "   \\_|______|_/   ",
        ]
    }
    #[cfg(target_os = "macos")]
    {
        &[
            "        .-.       ",
            "      .-'''-.     ",
            "     /       \\    ",
            "    |         |   ",
            "    |         |   ",
            "     \\       /    ",
            "      '-...-'     ",
        ]
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        &[
            "     +-------+    ",
            "     | ..... |    ",
            "     | ..... |    ",
            "     +-------+    ",
        ]
    }
}

/// One line of the summary: a label, and either a value or why there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub label: String,
    /// The rendered value, or `None` when it was not readable.
    pub value: Option<String>,
    /// Why there is no value. Present exactly when `value` is `None`.
    pub reason: Option<String>,
}

impl Line {
    /// How this line renders, unknown included.
    pub fn render(&self) -> String {
        match (&self.value, &self.reason) {
            (Some(v), _) => v.clone(),
            (None, Some(r)) => format!("unknown — {r}"),
            // Unreachable through `summary`, which always supplies one or the
            // other. Rendered rather than panicking: a display surface that
            // aborts is worse than one that says it does not know.
            (None, None) => "unknown".to_string(),
        }
    }
}

fn find<'a>(readings: &'a [Reading], id: &str) -> Option<&'a Reading> {
    readings.iter().find(|r| r.id == id)
}

/// Render one reading as a line, carrying its reason when it has no value.
fn line(
    readings: &[Reading],
    label: &str,
    id: &str,
    fmt: impl Fn(&serde_json::Value) -> String,
) -> Line {
    match find(readings, id) {
        Some(r) => match &r.value {
            Some(v) => Line {
                label: label.to_string(),
                value: Some(fmt(v)),
                reason: None,
            },
            None => Line {
                label: label.to_string(),
                value: None,
                reason: Some(
                    r.note
                        .clone()
                        .unwrap_or_else(|| "no reason recorded".to_string()),
                ),
            },
        },
        None => Line {
            label: label.to_string(),
            value: None,
            reason: Some(format!("{id} is not in this snapshot")),
        },
    }
}

fn as_text(v: &serde_json::Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| v.to_string())
}

fn as_bytes(v: &serde_json::Value) -> String {
    let b = v.as_f64().unwrap_or(0.0);
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut n = b;
    let mut i = 0;
    while n >= 1024.0 && i + 1 < UNITS.len() {
        n /= 1024.0;
        i += 1;
    }
    format!("{n:.1} {}", UNITS[i])
}

fn as_percent(v: &serde_json::Value) -> String {
    format!("{:.1}%", v.as_f64().unwrap_or(0.0))
}

/// The summary lines, in display order.
///
/// Takes readings rather than gathering them, so the same function renders a
/// live machine and a fixture.
pub fn summary(readings: &[Reading]) -> Vec<Line> {
    let mut out = vec![
        line(readings, "OS", "system.os.name", as_text),
        line(readings, "Host", "system.hostname", as_text),
        line(readings, "Kernel", "system.kernel.version", as_text),
        line(readings, "Arch", "system.architecture", as_text),
        line(readings, "Uptime", "system.uptime", |v| {
            let secs = v.as_f64().unwrap_or(0.0) as u64;
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }),
        line(readings, "CPU", "cpu.model", as_text),
        line(readings, "Cores", "cpu.cores.logical", as_text),
        line(readings, "CPU usage", "cpu.total.utilization", as_percent),
        line(readings, "Memory", "memory.total", as_bytes),
        line(readings, "Mem used", "memory.utilization", as_percent),
    ];

    // GPUs are instanced, so they are gathered rather than named. A machine with
    // none gets the `gpu.<none>` diagnostic and its reason, which is the whole
    // point: "no GPU here" and "simon cannot read GPUs here" look different.
    let gpus: BTreeMap<&str, &Reading> = readings
        .iter()
        .filter(|r| r.id.starts_with("gpu.") && r.id.ends_with(".name"))
        .map(|r| (r.id.as_str(), r))
        .collect();

    if gpus.is_empty() {
        let reason = find(readings, "gpu.<none>")
            .and_then(|r| r.note.clone())
            .unwrap_or_else(|| "no gpu readings in this snapshot".to_string());
        out.push(Line {
            label: "GPU".into(),
            value: None,
            reason: Some(reason),
        });
    } else {
        for (i, (_, r)) in gpus.iter().enumerate() {
            out.push(Line {
                label: if i == 0 {
                    "GPU".into()
                } else {
                    format!("GPU {i}")
                },
                value: r.value.as_ref().map(as_text),
                reason: r.note.clone(),
            });
        }
    }

    out
}

/// The whole thing: logo beside the summary.
pub fn render(readings: &[Reading]) -> String {
    let art = logo();
    let lines = summary(readings);
    let width = lines.iter().map(|l| l.label.len()).max().unwrap_or(0);
    let mut out = String::new();

    let rows = art.len().max(lines.len());
    for i in 0..rows {
        let left = art.get(i).copied().unwrap_or("                  ");
        match lines.get(i) {
            Some(l) => out.push_str(&format!(
                "{left}  {:<width$}  {}\n",
                l.label,
                l.render(),
                width = width
            )),
            None => out.push_str(&format!("{left}\n")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::{Provenance, Unit};

    fn measured(id: &str, v: serde_json::Value) -> Reading {
        Reading {
            id: id.into(),
            value: Some(v),
            provenance: Provenance::Measured,
            unit: Some(Unit::Text),
            note: None,
        }
    }

    fn unavailable(id: &str, why: &str) -> Reading {
        Reading {
            id: id.into(),
            value: None,
            provenance: Provenance::Unavailable,
            unit: None,
            note: Some(why.into()),
        }
    }

    /// The property that separates this from every other tool of its shape.
    #[test]
    fn an_unreadable_field_prints_the_reason_rather_than_a_dash() {
        let readings = [unavailable(
            "cpu.model",
            "the platform CPU reader failed: permission denied",
        )];
        let lines = summary(&readings);
        let cpu = lines.iter().find(|l| l.label == "CPU").unwrap();
        assert!(cpu.value.is_none());
        let rendered = cpu.render();
        assert!(
            rendered.contains("permission denied"),
            "a summary that prints a dash makes a reader guess whether the \
             machine lacks the thing or simon lacks the reader: {rendered}"
        );
    }

    /// The distinction a dash would destroy.
    #[test]
    fn no_gpu_and_no_gpu_reader_render_differently() {
        let absent = [unavailable("gpu.<none>", "no GPU detected on this machine")];
        let unreadable = [unavailable("gpu.<none>", "GPU enumeration failed")];

        let a = summary(&absent);
        let b = summary(&unreadable);
        let ga = a.iter().find(|l| l.label == "GPU").unwrap().render();
        let gb = b.iter().find(|l| l.label == "GPU").unwrap().render();

        assert_ne!(
            ga, gb,
            "buying a graphics card and filing a bug are different responses, and \
             the summary is where most people will look first"
        );
        assert!(ga.contains("no GPU detected"));
        assert!(gb.contains("enumeration failed"));
    }

    #[test]
    fn a_present_gpu_is_named() {
        let readings = [measured("gpu.0.name", serde_json::json!("RTX 4090"))];
        let lines = summary(&readings);
        let gpu = lines.iter().find(|l| l.label == "GPU").unwrap();
        assert_eq!(gpu.value.as_deref(), Some("RTX 4090"));
    }

    #[test]
    fn several_gpus_are_all_listed() {
        let readings = [
            measured("gpu.0.name", serde_json::json!("A")),
            measured("gpu.1.name", serde_json::json!("B")),
        ];
        let labels: Vec<String> = summary(&readings)
            .into_iter()
            .filter(|l| l.label.starts_with("GPU"))
            .map(|l| l.label)
            .collect();
        assert_eq!(labels, vec!["GPU", "GPU 1"]);
    }

    /// An id the snapshot never produced is distinguishable from one it produced
    /// as unavailable.
    #[test]
    fn a_missing_id_says_so_rather_than_claiming_it_was_unreadable() {
        let lines = summary(&[]);
        let cpu = lines.iter().find(|l| l.label == "CPU").unwrap();
        assert!(cpu.render().contains("not in this snapshot"));
    }

    #[test]
    fn bytes_render_at_a_readable_scale() {
        assert_eq!(as_bytes(&serde_json::json!(1024)), "1.0 KB");
        assert_eq!(as_bytes(&serde_json::json!(1_073_741_824u64)), "1.0 GB");
    }

    #[test]
    fn the_logo_and_the_summary_both_survive_being_longer() {
        // More lines than art, and more art than lines: neither may truncate the
        // other, because a summary that silently drops a row is a summary that
        // lies by omission.
        let many: Vec<Reading> = (0..20)
            .map(|i| measured(&format!("gpu.{i}.name"), serde_json::json!("G")))
            .collect();
        let rendered = render(&many);
        assert!(rendered.lines().count() >= 20);

        let none = render(&[]);
        assert!(none.lines().count() >= logo().len());
    }
}
