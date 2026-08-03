//! Resolving ontology ids to live values.
//!
//! [`super::Ontology`] names what simon *can* report; this module answers what it
//! *does* report, right now, on this machine. Without it the ontology is a glossary:
//! an agent can learn that `gpu.0.thermal.temperature` exists but has no way to read
//! it.
//!
//! ## Declared vs. observed provenance
//!
//! An [`super::Entity`] declares the provenance a value is *supposed* to have. A
//! [`Reading`] carries the provenance it *actually* had when taken. These differ
//! routinely and the difference is the useful part: `gpu.0.thermal.temperature` is
//! declared `measured`, but on a machine whose driver exposes no sensor the reading
//! comes back `unavailable` with a note saying so.
//!
//! The rule this module exists to enforce is that a failed read is never dressed as
//! a successful one. There is no path here that substitutes zero, a previous sample,
//! or a plausible constant for a value that could not be obtained — every such
//! substitution this repository has shipped was indistinguishable from a real
//! reading at the point of consumption, which is precisely what a consuming agent
//! cannot afford.

use serde::{Deserialize, Serialize};

use super::{Entity, Ontology, Provenance, Unit};

/// One resolved value, with the provenance it actually had.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    /// Concrete id — `gpu.0.name`, never the `gpu.{n}.name` template.
    pub id: String,
    /// `None` whenever `provenance` is [`Provenance::Unavailable`]. A consumer must
    /// render this as unknown; it is never a zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// What this reading actually is — not what the entity declares it should be.
    pub provenance: Provenance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
    /// Why a value is missing, or what qualifies it. Present whenever the observed
    /// provenance differs from the declared one, so the difference is never silent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Reading {
    fn measured(id: impl Into<String>, value: serde_json::Value, unit: Option<Unit>) -> Self {
        Self {
            id: id.into(),
            value: Some(value),
            provenance: Provenance::Measured,
            unit,
            note: None,
        }
    }

    fn derived(id: impl Into<String>, value: serde_json::Value, unit: Option<Unit>) -> Self {
        Self {
            id: id.into(),
            value: Some(value),
            provenance: Provenance::Derived,
            unit,
            note: None,
        }
    }

    /// A value that could not be obtained. The reason is mandatory: "unavailable"
    /// without a cause is the same dead end as a fabricated number.
    fn unavailable(id: impl Into<String>, unit: Option<Unit>, why: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            value: None,
            provenance: Provenance::Unavailable,
            unit,
            note: Some(why.into()),
        }
    }

    /// Whether this reading may be treated as a live observation of the hardware.
    pub fn is_observation(&self) -> bool {
        self.provenance.is_observation() && self.value.is_some()
    }
}

/// Everything simon can currently resolve, in id order.
///
/// Entities with no resolver bound are reported as [`Provenance::Unavailable`] with
/// a note saying so, rather than omitted. An agent comparing this against
/// [`Ontology::build`] can therefore tell "this machine has no such device" apart
/// from "simon cannot read this yet" — two very different facts that an omission
/// would collapse into one.
pub fn snapshot() -> Vec<Reading> {
    let ontology = Ontology::build();
    let mut out = Vec::new();

    resolve_cpu(&mut out);
    resolve_memory(&mut out);
    resolve_system(&mut out);
    resolve_board(&mut out);
    resolve_gpu(&mut out);

    // Anything the ontology names but nothing above produced. Templates are skipped:
    // an unexpanded `gpu.{n}.name` is not a fact about this machine.
    let produced: std::collections::HashSet<&str> = out.iter().map(|r| r.id.as_str()).collect();
    let mut unbound: Vec<Reading> = ontology
        .entities
        .values()
        .filter(|e| !Ontology::is_template(&e.id) && !produced.contains(e.id.as_str()))
        .map(|e| {
            Reading::unavailable(
                e.id.clone(),
                e.unit,
                "no resolver bound on this build — the entity is defined but simon \
                 does not yet read it here",
            )
        })
        .collect();
    out.append(&mut unbound);

    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Resolve one id. Templates are rejected: `gpu.{n}.name` is not a question that has
/// an answer, `gpu.0.name` is.
pub fn get(id: &str) -> Option<Reading> {
    if Ontology::is_template(id) {
        return None;
    }
    snapshot().into_iter().find(|r| r.id == id)
}

/// Entities the ontology declares that this build can actually resolve.
///
/// Coverage is a fact about simon, not about the machine, so it is worth being able
/// to ask for directly rather than inferring from a snapshot full of nulls.
pub fn coverage() -> Coverage {
    let readings = snapshot();
    let bound = readings.iter().filter(|r| r.value.is_some()).count();
    let unavailable = readings.len() - bound;
    Coverage {
        total: readings.len(),
        resolved: bound,
        unavailable,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub total: usize,
    pub resolved: usize,
    pub unavailable: usize,
}

// ── Per-domain resolvers ─────────────────────────────────────────────────────
//
// Each pushes what it can read and an `unavailable` with a cause for what it
// cannot. None of them invent a value on failure.

fn resolve_cpu(out: &mut Vec<Reading>) {
    let stats = read_cpu_stats();

    let Some(stats) = stats else {
        out.push(Reading::unavailable(
            "cpu.total.utilization",
            Some(Unit::Percent),
            "the platform CPU reader returned an error",
        ));
        return;
    };

    out.push(Reading::derived(
        "cpu.total.utilization",
        serde_json::json!(100.0 - stats.total.idle),
        Some(Unit::Percent),
    ));
    out.push(Reading::measured(
        "cpu.total.idle",
        serde_json::json!(stats.total.idle),
        Some(Unit::Percent),
    ));
    out.push(Reading::measured(
        "cpu.cores.logical",
        serde_json::json!(stats.cores.len()),
        Some(Unit::Count),
    ));

    match stats.cores.first().map(|c| c.model.as_str()) {
        // An empty model string is a failed read, not a CPU without a name.
        Some(model) if !model.trim().is_empty() => push_text(out, "cpu.model", model),
        _ => out.push(Reading::unavailable(
            "cpu.model",
            Some(Unit::Text),
            "no core reported a model string",
        )),
    }

    for core in &stats.cores {
        let base = format!("cpu.core.{}", core.id);
        match (core.user, core.system) {
            (Some(user), Some(system)) => out.push(Reading::derived(
                format!("{base}.utilization"),
                serde_json::json!(user + system),
                Some(Unit::Percent),
            )),
            // The platform exposes no per-processor times. Reporting the system
            // average here would look per-core and be one number repeated.
            _ => out.push(Reading::unavailable(
                format!("{base}.utilization"),
                Some(Unit::Percent),
                "platform exposes no per-processor times; the system-wide average \
                 is not a per-core reading",
            )),
        }
        match &core.frequency {
            Some(f) if f.current > 0 => out.push(Reading::measured(
                format!("{base}.frequency"),
                serde_json::json!(f.current),
                Some(Unit::Megahertz),
            )),
            _ => out.push(Reading::unavailable(
                format!("{base}.frequency"),
                Some(Unit::Megahertz),
                "no per-core clock reported",
            )),
        }
        // Deliberately not derived from the maximum: a minimum is a distinct
        // property, and dividing the maximum by four produced a plausible number
        // that was not one.
        out.push(Reading::unavailable(
            format!("{base}.frequency.min"),
            Some(Unit::Megahertz),
            "platform exposes no minimum core frequency; it is not inferable from \
             the maximum",
        ));
    }
}

fn resolve_memory(out: &mut Vec<Reading>) {
    let Some(stats) = read_memory_stats() else {
        for (id, unit) in [
            ("memory.total", Unit::Bytes),
            ("memory.used", Unit::Bytes),
            ("memory.utilization", Unit::Percent),
        ] {
            out.push(Reading::unavailable(
                id,
                Some(unit),
                "the platform memory reader returned an error",
            ));
        }
        return;
    };

    // Core readers work in KB; the ontology declares bytes.
    let total = stats.ram.total.saturating_mul(1024);
    let used = stats.ram.used.saturating_mul(1024);
    out.push(Reading::measured(
        "memory.total",
        serde_json::json!(total),
        Some(Unit::Bytes),
    ));
    out.push(Reading::measured(
        "memory.used",
        serde_json::json!(used),
        Some(Unit::Bytes),
    ));
    match total {
        0 => out.push(Reading::unavailable(
            "memory.utilization",
            Some(Unit::Percent),
            "total memory reported as zero, so a percentage has no denominator",
        )),
        t => out.push(Reading::derived(
            "memory.utilization",
            serde_json::json!((used as f64 / t as f64) * 100.0),
            Some(Unit::Percent),
        )),
    }

    let swap_total = stats.swap.total.saturating_mul(1024);
    if swap_total == 0 {
        out.push(Reading::unavailable(
            "memory.swap.total",
            Some(Unit::Bytes),
            "no swap or pagefile configured",
        ));
        out.push(Reading::unavailable(
            "memory.swap.used",
            Some(Unit::Bytes),
            "no swap or pagefile configured",
        ));
    } else {
        out.push(Reading::measured(
            "memory.swap.total",
            serde_json::json!(swap_total),
            Some(Unit::Bytes),
        ));
        out.push(Reading::measured(
            "memory.swap.used",
            serde_json::json!(stats.swap.used.saturating_mul(1024)),
            Some(Unit::Bytes),
        ));
    }
}

fn resolve_system(out: &mut Vec<Reading>) {
    match crate::os_info::OsInfoMonitor::new() {
        Ok(monitor) => {
            let info = monitor.info();
            push_text(out, "system.os.name", &info.os_name);
            push_id(out, "system.os.build", &info.os_build);
            if info.uptime_seconds > 0 {
                out.push(Reading::measured(
                    "system.uptime",
                    serde_json::json!(info.uptime_seconds),
                    Some(Unit::Seconds),
                ));
            } else {
                out.push(Reading::unavailable(
                    "system.uptime",
                    Some(Unit::Seconds),
                    "uptime read as zero, which would mean the machine booted this \
                     instant",
                ));
            }
        }
        Err(e) => {
            for (id, unit) in [
                ("system.os.name", Unit::Text),
                ("system.os.build", Unit::Identifier),
                ("system.uptime", Unit::Seconds),
            ] {
                out.push(Reading::unavailable(
                    id,
                    Some(unit),
                    format!("OS info reader failed: {e}"),
                ));
            }
        }
    }
}

fn resolve_board(out: &mut Vec<Reading>) {
    match detect_board() {
        Some(board) => {
            push_text(out, "board.model", &board.hardware.model);
            match &board.hardware.module {
                Some(m) => push_text(out, "board.manufacturer", m),
                None => out.push(Reading::unavailable(
                    "board.manufacturer",
                    Some(Unit::Text),
                    "SMBIOS reported no baseboard manufacturer",
                )),
            }
        }
        None => {
            for id in ["board.model", "board.manufacturer"] {
                out.push(Reading::unavailable(
                    id,
                    Some(Unit::Text),
                    "no board detection on this platform",
                ));
            }
        }
    }
}

fn resolve_gpu(out: &mut Vec<Reading>) {
    let Ok(monitor) = crate::SiliconMonitor::new() else {
        out.push(Reading::unavailable(
            "gpu.0.name",
            Some(Unit::Text),
            "GPU enumeration failed",
        ));
        return;
    };
    let Ok(gpus) = monitor.snapshot_gpus() else {
        out.push(Reading::unavailable(
            "gpu.0.name",
            Some(Unit::Text),
            "GPU snapshot failed",
        ));
        return;
    };
    if gpus.is_empty() {
        // Absent hardware is a fact, not a failure — and not a zero-valued GPU.
        out.push(Reading::unavailable(
            "gpu.0.name",
            Some(Unit::Text),
            "no GPU detected on this machine",
        ));
        return;
    }

    for (i, gpu) in gpus.iter().enumerate() {
        let base = format!("gpu.{i}");
        push_text(out, format!("{base}.name"), &gpu.static_info.name);
        out.push(Reading::measured(
            format!("{base}.vendor"),
            serde_json::json!(format!("{:?}", gpu.static_info.vendor).to_lowercase()),
            Some(Unit::Identifier),
        ));

        let dynamic = &gpu.dynamic_info;
        // Not an Option: the collection layer already flattens an absent counter to
        // zero, so this cannot distinguish "idle" from "not reported" and must not
        // pretend to.
        out.push(Reading::measured(
            format!("{base}.utilization"),
            serde_json::json!(dynamic.utilization),
            Some(Unit::Percent),
        ));
        push_opt(
            out,
            format!("{base}.thermal.temperature"),
            dynamic.thermal.temperature.map(|t| serde_json::json!(t)),
            Some(Unit::Celsius),
            "no temperature sensor exposed for this adapter",
        );
        push_opt(
            out,
            format!("{base}.thermal.max_temperature"),
            dynamic
                .thermal
                .max_temperature
                .map(|t| serde_json::json!(t)),
            Some(Unit::Celsius),
            "vendor publishes no thermal limit for this adapter",
        );
        push_opt(
            out,
            format!("{base}.thermal.critical_temperature"),
            dynamic
                .thermal
                .critical_temperature
                .map(|t| serde_json::json!(t)),
            Some(Unit::Celsius),
            "vendor publishes no shutdown threshold for this adapter",
        );
        push_opt(
            out,
            format!("{base}.power.draw"),
            dynamic.power.draw.map(|p| serde_json::json!(p)),
            Some(Unit::Milliwatts),
            "driver reports no power telemetry",
        );
        push_opt(
            out,
            format!("{base}.power.limit"),
            dynamic.power.limit.map(|p| serde_json::json!(p)),
            Some(Unit::Milliwatts),
            "driver exposes no enforced power cap",
        );
        push_opt(
            out,
            format!("{base}.clocks.graphics"),
            dynamic.clocks.graphics.map(|c| serde_json::json!(c)),
            Some(Unit::Megahertz),
            "driver reports no graphics clock",
        );
        push_opt(
            out,
            format!("{base}.clocks.graphics.max"),
            dynamic.clocks.graphics_max.map(|c| serde_json::json!(c)),
            Some(Unit::Megahertz),
            "vendor publishes no graphics clock ceiling",
        );

        let mem = &dynamic.memory;
        if mem.total > 0 {
            out.push(Reading::measured(
                format!("{base}.memory.total"),
                serde_json::json!(mem.total),
                Some(Unit::Bytes),
            ));
            out.push(Reading::measured(
                format!("{base}.memory.used"),
                serde_json::json!(mem.used),
                Some(Unit::Bytes),
            ));
        } else {
            for suffix in ["total", "used"] {
                out.push(Reading::unavailable(
                    format!("{base}.memory.{suffix}"),
                    Some(Unit::Bytes),
                    "adapter reports no discrete video memory (unified memory parts \
                     report none)",
                ));
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn push_text(out: &mut Vec<Reading>, id: impl Into<String>, value: &str) {
    let id = id.into();
    if value.trim().is_empty() {
        out.push(Reading::unavailable(
            id,
            Some(Unit::Text),
            "reader returned an empty string",
        ));
    } else {
        out.push(Reading::measured(
            id,
            serde_json::json!(value),
            Some(Unit::Text),
        ));
    }
}

fn push_id(out: &mut Vec<Reading>, id: impl Into<String>, value: &str) {
    let id = id.into();
    if value.trim().is_empty() {
        out.push(Reading::unavailable(
            id,
            Some(Unit::Identifier),
            "reader returned an empty string",
        ));
    } else {
        out.push(Reading::measured(
            id,
            serde_json::json!(value),
            Some(Unit::Identifier),
        ));
    }
}

fn push_opt(
    out: &mut Vec<Reading>,
    id: impl Into<String>,
    value: Option<serde_json::Value>,
    unit: Option<Unit>,
    why_absent: &str,
) {
    match value {
        Some(v) => out.push(Reading::measured(id, v, unit)),
        None => out.push(Reading::unavailable(id, unit, why_absent)),
    }
}

fn read_cpu_stats() -> Option<crate::core::cpu::CpuStats> {
    #[cfg(windows)]
    {
        crate::platform::windows::read_cpu_stats().ok()
    }
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::read_cpu_stats().ok()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

fn read_memory_stats() -> Option<crate::core::memory::MemoryStats> {
    #[cfg(windows)]
    {
        crate::platform::windows::read_memory_stats().ok()
    }
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::read_memory_stats().ok()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

fn detect_board() -> Option<crate::core::platform_info::BoardInfo> {
    #[cfg(windows)]
    {
        crate::platform::windows::detect_platform().ok()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Check a snapshot against the ontology's declared ranges.
///
/// Returns one message per reading that is physically impossible. An empty result
/// is the assertion this repository's plausibility tier makes, available to any
/// consumer rather than only to the test suite.
pub fn validate(readings: &[Reading]) -> Vec<String> {
    let ontology = Ontology::build();
    let mut problems = Vec::new();
    for r in readings {
        let Some(value) = r.value.as_ref().and_then(|v| v.as_f64()) else {
            continue;
        };
        if let Some(entity) = lookup_template(&ontology, &r.id) {
            if let Some(problem) = entity.validate_range(value) {
                problems.push(problem);
            }
        }
    }
    problems
}

/// Find the entity for a concrete id, matching `gpu.0.name` against `gpu.{n}.name`.
fn lookup_template<'a>(ontology: &'a Ontology, concrete: &str) -> Option<&'a Entity> {
    if let Some(e) = ontology.get(concrete) {
        return Some(e);
    }
    let parts: Vec<&str> = concrete.split('.').collect();
    ontology.entities.values().find(|e| {
        let tparts: Vec<&str> = e.id.split('.').collect();
        tparts.len() == parts.len()
            && tparts
                .iter()
                .zip(&parts)
                .all(|(t, c)| t.starts_with('{') || t == c)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The invariant the whole module exists for: a missing value is never a zero,
    /// and never silent about why.
    #[test]
    fn unavailable_readings_carry_no_value_and_state_a_reason() {
        for r in snapshot() {
            if r.provenance == Provenance::Unavailable {
                assert!(
                    r.value.is_none(),
                    "{} is unavailable but carries a value: {:?}",
                    r.id,
                    r.value
                );
                assert!(
                    r.note.as_ref().is_some_and(|n| !n.trim().is_empty()),
                    "{} is unavailable with no reason given",
                    r.id
                );
            } else {
                assert!(
                    r.value.is_some(),
                    "{} claims provenance {:?} but has no value",
                    r.id,
                    r.provenance
                );
            }
        }
    }

    #[test]
    fn only_measured_readings_count_as_observations() {
        for r in snapshot() {
            let expected = r.provenance == Provenance::Measured && r.value.is_some();
            assert_eq!(r.is_observation(), expected, "{}", r.id);
        }
    }

    /// Every resolved id must be one the ontology names — otherwise the schema an
    /// agent fetched does not describe what it receives.
    #[test]
    fn every_reading_maps_to_a_declared_entity() {
        let ontology = Ontology::build();
        for r in snapshot() {
            assert!(
                lookup_template(&ontology, &r.id).is_some(),
                "{} was resolved but is not declared in the ontology",
                r.id
            );
            assert!(
                !Ontology::is_template(&r.id),
                "{} is a template, not a concrete reading",
                r.id
            );
        }
    }

    /// Live values must satisfy the ranges the ontology declares. This is the
    /// plausibility tier applied to the resolver's own output.
    #[test]
    fn live_readings_are_physically_possible() {
        let problems = validate(&snapshot());
        assert!(problems.is_empty(), "impossible readings: {problems:#?}");
    }

    #[test]
    fn ids_are_unique_within_a_snapshot() {
        let readings = snapshot();
        let mut seen = std::collections::HashSet::new();
        for r in &readings {
            assert!(seen.insert(r.id.clone()), "duplicate reading id: {}", r.id);
        }
    }

    #[test]
    fn get_rejects_templates_and_finds_concrete_ids() {
        assert!(
            get("gpu.{n}.name").is_none(),
            "a template is not a question with an answer"
        );
        // memory.total resolves on every supported platform; on an unsupported one
        // it is still present, as an unavailable reading.
        assert!(get("memory.total").is_some());
        assert!(get("no.such.entity").is_none());
    }

    #[test]
    fn coverage_accounts_for_every_reading() {
        let c = coverage();
        assert_eq!(c.total, c.resolved + c.unavailable);
        assert_eq!(c.total, snapshot().len());
    }
}
