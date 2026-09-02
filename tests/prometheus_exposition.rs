//! The exporter's output has to parse as Prometheus text exposition.
//!
//! This surface had no test file. A defect here is not a wrong number on a
//! dashboard — Prometheus rejects a scrape containing duplicate samples, so one
//! malformed section discards every metric the endpoint serves, including the
//! correct ones.

use std::collections::BTreeMap;

use simonlib::prometheus::PrometheusExporter;

fn exported() -> String {
    let mut exporter = PrometheusExporter::new("simon");
    exporter.collect_system_metrics();
    exporter.export()
}

fn sample_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
}

/// One `# HELP` and one `# TYPE` per metric name, which the format requires.
///
/// `PrometheusExporter::add` pushed a new family per sample, so the headers
/// repeated once per disk, per GPU and per network interface — twenty times for
/// `simon_network_rx_bytes_total` on the machine this was written on.
#[test]
fn each_metric_name_is_declared_once() {
    let text = exported();
    let mut helps: BTreeMap<&str, usize> = BTreeMap::new();
    let mut types: BTreeMap<&str, usize> = BTreeMap::new();

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# HELP ") {
            *helps
                .entry(rest.split(' ').next().unwrap_or(""))
                .or_default() += 1;
        } else if let Some(rest) = line.strip_prefix("# TYPE ") {
            *types
                .entry(rest.split(' ').next().unwrap_or(""))
                .or_default() += 1;
        }
    }

    let repeated: Vec<String> = helps
        .iter()
        .chain(types.iter())
        .filter(|(_, count)| **count > 1)
        .map(|(name, count)| format!("{name} declared {count} times"))
        .collect();

    assert!(
        repeated.is_empty(),
        "the text format allows one HELP and one TYPE per metric name: {repeated:#?}"
    );
}

/// No two samples may share a name and a label set in one scrape.
///
/// This is the fatal one. `collect_network_metrics` built an `interface` label
/// and then called the unlabelled `MetricFamily::counter`, so every interface
/// emitted `simon_network_rx_bytes_total` with no labels and its own value —
/// twenty identical series, which Prometheus rejects as duplicate samples,
/// taking the whole endpoint down rather than just the network metrics.
#[test]
fn no_two_samples_share_a_name_and_labels() {
    let text = exported();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();

    for line in sample_lines(&text) {
        // Everything up to the final space is the series identity.
        let Some((identity, _value)) = line.rsplit_once(' ') else {
            continue;
        };
        *seen.entry(identity).or_default() += 1;
    }

    let duplicated: Vec<String> = seen
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(identity, count)| format!("{identity} appears {count} times"))
        .collect();

    assert!(
        duplicated.is_empty(),
        "duplicate samples make Prometheus reject the entire scrape, so these \
         discard every other metric on the endpoint too: {duplicated:#?}"
    );
}

// Why there is no "every per-instance metric carries a label" test here.
//
// There was one, and it was deleted rather than kept green. The rule it tried
// to express -- a metric collected in a loop must be labelled -- is real, but
// a test cannot see the loop, so it guessed from the name and needed an
// exemption every time a legitimately whole-machine metric appeared:
// `simon_gpu_count`, then `simon_swap_used_bytes`, then
// `simon_uptime_seconds`. Three exemptions in one sitting, none of them a
// defect.
//
// **A test that needs a growing allowlist to stay green has stopped testing an
// invariant and started describing the current output.** The failure it was
// built to catch -- an unlabelled metric emitted once per device -- produces
// duplicate samples, and `no_two_samples_share_a_name_and_labels` above
// catches that exactly, with no heuristic and no exemptions.

/// Every metric the bundled Grafana dashboards query must be published.
///
/// `grafana/` ships three dashboards, and they are a contract: a panel querying
/// a name nothing exports renders empty against a live server, which looks like
/// broken hardware rather than a broken dashboard. `http_server.rs` records
/// that this whole class of failure already happened once — the names lacked
/// the `simon_` prefix and all three dashboards were blank — and nothing
/// prevented it recurring. Six queried names were unpublished when this was
/// written, including `simon_gpu_clock_graphics_mhz`, which the exporter
/// published as `simon_gpu_clock_core_mhz`: the same number under a name no
/// dashboard asks for.
///
/// **Two earlier versions of this test were wrong, in opposite directions.**
///
/// The first searched the publishers' source text and passed while the defect
/// was restored, because the comment explaining the defect contained the name
/// it was looking for. A test a comment can satisfy is not a test.
///
/// The second searched the rendered output, and flagged
/// `simon_cpu_temperature_celsius` on a machine whose CPU exposes no readable
/// sensor — a hardware absence, not a missing metric. Output cannot distinguish
/// "the exporter does not know this name" from "it knows it and had nothing to
/// report", and only the first is a defect.
///
/// So: the source, with comments stripped. That is exactly the question worth
/// asking — does the code contain a publisher for this name — and neither prose
/// nor an absent sensor can answer it for us.
#[test]
fn every_dashboard_metric_is_published_somewhere() {
    use std::collections::BTreeSet;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let Ok(entries) = std::fs::read_dir(root.join("grafana")) else {
        eprintln!("skipping: no grafana/ directory");
        return;
    };

    let mut queried: BTreeSet<String> = BTreeSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (idx, _) in text.match_indices("simon_") {
            queried.insert(
                text[idx..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect(),
            );
        }
    }
    assert!(
        !queried.is_empty(),
        "no metric names found in grafana/, so this test checks nothing"
    );

    // Both publishers, with every `//` comment removed so that documentation
    // mentioning a name cannot stand in for code emitting it.
    let mut code = String::new();
    for file in ["src/prometheus.rs", "src/http_server.rs"] {
        let Ok(text) = std::fs::read_to_string(root.join(file)) else {
            continue;
        };
        for line in text.lines() {
            let without_comment = match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            };
            code.push_str(without_comment);
            code.push('\n');
        }
    }

    let missing: Vec<&String> = queried
        .iter()
        .filter(|name| {
            // Emitted either as the full name or through `prefixed("...")`,
            // which prepends `simon_`.
            let bare = name.strip_prefix("simon_").unwrap_or(name);
            !code.contains(&format!("\"{name}\"")) && !code.contains(&format!("\"{bare}\""))
        })
        .collect();

    assert!(
        missing.is_empty(),
        "the bundled dashboards query metrics no publisher emits, so those \
         panels render empty against a live server: {missing:#?}"
    );
}

/// The other Prometheus renderer — the one the HTTP server actually serves.
///
/// `/api/v1/metrics/prometheus` does not serve `PrometheusExporter`. It serves
/// `MetricCollector::export_prometheus`, a second implementation, and nothing
/// outside `src/prometheus.rs` referenced the first one at all: the complete,
/// correctly formatted exporter was unreachable and the served renderer was the
/// smaller one.
///
/// `record_with_labels` encodes a labelled series into its storage key as
/// `name:{gpu=0}` — a map key, not exposition syntax. Rendered verbatim that is
/// invalid: Prometheus needs `name{gpu="0"}`, quoted and without the colon.
#[test]
fn the_served_renderer_emits_valid_label_syntax() {
    use simonlib::observability::MetricCollector;

    let collector = MetricCollector::new();
    collector.record("simon_uptime_seconds", 1234.0);
    collector.record_with_labels("simon_gpu_temperature_celsius", 42.0, &[("gpu", "0")]);
    collector.record_with_labels(
        "simon_gpu_temperature_celsius",
        37.0,
        &[("gpu", "1"), ("vendor", "NVIDIA")],
    );

    let text = collector.export_prometheus();

    assert!(
        !text.contains(":{"),
        "the storage key leaked into the exposition, which no scraper parses: {text}"
    );
    assert!(
        text.contains("simon_gpu_temperature_celsius{gpu=\"0\"} 42"),
        "a single label should render as gpu=\"0\": {text}"
    );
    assert!(
        text.contains("simon_gpu_temperature_celsius{gpu=\"1\",vendor=\"NVIDIA\"} 37"),
        "several labels should render comma-separated and quoted: {text}"
    );
    assert!(
        text.contains("simon_uptime_seconds 1234"),
        "an unlabelled series should render unchanged: {text}"
    );
}
