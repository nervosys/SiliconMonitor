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

/// A per-instance metric has to say which instance it describes.
///
/// A bare `simon_network_rx_bytes_total` is not merely ambiguous: emitted once
/// per interface it becomes the duplicate-sample failure above. Anything
/// collected in a loop needs a label distinguishing the members.
#[test]
fn per_instance_metrics_carry_a_distinguishing_label() {
    let text = exported();
    let unlabelled: Vec<&str> = sample_lines(&text)
        .filter(|l| {
            let name = l.split(&['{', ' '][..]).next().unwrap_or("");
            // Genuinely one-per-machine families. A `_count` belongs here on
            // its own reasoning rather than by prefix: `simon_gpu_count` is how
            // many GPUs the machine has, which is a fact about the machine and
            // not about any one GPU, so it correctly carries no `gpu` label.
            let whole_machine = name.ends_with("_count")
                || name.starts_with("simon_memory_")
                || name.starts_with("simon_cpu_")
                || name.starts_with("simon_system_")
                || name.starts_with("simon_profile_");
            !whole_machine && !l.contains('{')
        })
        .collect();

    assert!(
        unlabelled.is_empty(),
        "a metric collected per device, per disk or per interface must carry a \
         label naming which one: {unlabelled:#?}"
    );
}
