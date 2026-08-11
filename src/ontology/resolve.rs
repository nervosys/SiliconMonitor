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
    resolve_disk(&mut out);
    resolve_network(&mut out);
    resolve_process(&mut out);
    resolve_thermal(&mut out);
    resolve_power(&mut out);
    resolve_virtualization(&mut out);
    resolve_numa(&mut out);
    resolve_ecc(&mut out);
    resolve_pci(&mut out);
    resolve_usb(&mut out);

    // Anything the ontology names but nothing above produced.
    let produced: std::collections::HashSet<&str> = out.iter().map(|r| r.id.as_str()).collect();
    let mut unbound: Vec<Reading> = ontology
        .entities
        .values()
        .filter(|e| {
            // Diagnostics are claims about the reading process, and a resolver emits
            // one only when its condition actually holds. Sweeping them in here
            // asserted the opposite of the truth: `network.<none>` appeared saying
            // "no resolver bound" in a snapshot where the network resolver had just
            // enumerated forty-five readings.
            e.kind != super::EntityKind::Diagnostic
                && !Ontology::is_template(&e.id)
                && !produced.contains(e.id.as_str())
        })
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

    // Templated entities need the same treatment, and used to be skipped entirely on
    // the grounds that an unexpanded `disk.{n}.model` is not a fact about a machine.
    // That reasoning was wrong in a way this module exists to prevent: a domain whose
    // entities are *all* templates simply vanished from the snapshot, so an agent
    // could not tell "this machine has no disks" from "simon does not read disks".
    // Silence is the one answer a resolver must never give. A domain that produced
    // nothing gets one row saying so, keyed on the template so the id stays traceable
    // back to the schema.
    let mut missing_domains: Vec<Reading> = Vec::new();
    for domain in super::Domain::ALL {
        let prefix = format!("{}.", domain.as_str());
        if out.iter().any(|r| r.id.starts_with(&prefix)) {
            continue;
        }
        let declares_any = ontology.entities.values().any(|e| e.domain == *domain);
        if declares_any {
            missing_domains.push(Reading::unavailable(
                format!("{}.<none>", domain.as_str()),
                None,
                "no instance of this domain was found and no resolver reported why; \
                 the ontology declares entities here but nothing was enumerated",
            ));
        }
    }
    out.append(&mut missing_domains);

    // Last gate: a resolver must not hand an agent a number its own schema calls
    // impossible. Anything outside the declared range becomes unavailable, with the
    // offending value quoted in the note — surfaced, not swallowed, and never
    // clamped. Clamping would turn a 103% core into a plausible 100% and destroy the
    // evidence that the sampling was wrong.
    //
    // This exists because the derivation above got it wrong under load and shipped
    // an impossible percentage. The specific bug is fixed; the gate means the next
    // one degrades to "unknown, and here is why" instead of to a confident lie.
    for reading in out.iter_mut() {
        let Some(value) = reading.value.as_ref().and_then(|v| v.as_f64()) else {
            continue;
        };
        let Some(entity) = lookup_template(&ontology, &reading.id) else {
            continue;
        };
        if let Some(problem) = entity.validate_range(value) {
            reading.value = None;
            reading.provenance = Provenance::Unavailable;
            reading.note = Some(format!(
                "reader produced a value the ontology rejects, so it is withheld \
                 rather than reported: {problem}"
            ));
        }
    }

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
    coverage_of(&snapshot())
}

/// Coverage of an existing snapshot.
///
/// Split out because a caller wanting both the readings and their tally must not
/// take two snapshots to get them: instance counts move between calls on a live
/// machine — a USB device is unplugged, a process exits — so two snapshots
/// legitimately differ in length. A test asserting they match was passing only
/// while the box was quiet, and failed the moment the suite got busier.
pub fn coverage_of(readings: &[Reading]) -> Coverage {
    let bound = readings.iter().filter(|r| r.value.is_some()).count();
    Coverage {
        total: readings.len(),
        resolved: bound,
        unavailable: readings.len() - bound,
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
    // Cache topology is independent of the sampling below and must still resolve
    // when the CPU reader fails, so it runs before the early return.
    resolve_cpu_cache(out);

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

    // Declared since the ontology was written and never resolved — the CPU stats
    // reader counts logical processors only, so nothing filled it and the entity
    // fell through to the unbound sweep. Found by
    // `tests/ontology_conformance.rs::non_nullable_entities_are_never_null`, which
    // is the whole argument for deriving tests from the schema: a declared,
    // non-nullable id that no resolver touched is invisible to a hand-written suite.
    match crate::cpu_microarch::CpuMicroarchMonitor::new() {
        Ok(m) if m.report().physical_cores > 0 => out.push(Reading::measured(
            "cpu.cores.physical",
            serde_json::json!(m.report().physical_cores),
            Some(Unit::Count),
        )),
        Ok(_) => out.push(Reading::unavailable(
            "cpu.cores.physical",
            Some(Unit::Count),
            "the platform reported no physical core count distinct from the \
             logical one",
        )),
        Err(e) => out.push(Reading::unavailable(
            "cpu.cores.physical",
            Some(Unit::Count),
            format!("microarchitecture reader unavailable: {e}"),
        )),
    }

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
        // Derived as `100 - idle`, matching how `cpu.total.utilization` is computed.
        // Summing user and system instead let sampling jitter push the result past
        // 100 — the per-core deltas are taken across a boundary and need not add up
        // — which produced an impossible percentage under heavy load. Deriving from
        // idle is bounded by the same value the total already trusts.
        match core.idle {
            Some(idle) => out.push(Reading::derived(
                format!("{base}.utilization"),
                serde_json::json!(100.0 - idle),
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
    // Slot topology is read from SMBIOS and is independent of the usage figures
    // below, so it runs first and still resolves if those fail.
    resolve_memory_dimms(out);

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

    resolve_firmware(out);
    resolve_tpm(out);
}

fn resolve_firmware(out: &mut Vec<Reading>) {
    const IDS: [&str; 3] = [
        "board.firmware.vendor",
        "board.firmware.product",
        "board.firmware.boot_mode",
    ];

    let inventory = match crate::firmware::FirmwareInventory::new() {
        Ok(i) => i,
        Err(e) => {
            for id in IDS {
                out.push(Reading::unavailable(
                    id,
                    Some(Unit::Text),
                    format!("firmware inventory unavailable: {e}"),
                ));
            }
            return;
        }
    };

    push_text(out, "board.firmware.vendor", inventory.system_vendor());
    push_text(out, "board.firmware.product", inventory.system_product());
    push_id(
        out,
        "board.firmware.boot_mode",
        &format!("{:?}", inventory.boot_mode()).to_lowercase(),
    );

    for (i, entry) in inventory.items().iter().enumerate() {
        push_id(
            out,
            format!("board.firmware.{i}.component"),
            &format!("{:?}", entry.component).to_lowercase(),
        );
        push_text(out, format!("board.firmware.{i}.version"), &entry.version);
    }
}

fn resolve_tpm(out: &mut Vec<Reading>) {
    const DETAIL_IDS: [&str; 4] = [
        "board.tpm.version",
        "board.tpm.manufacturer",
        "board.tpm.status",
        "board.tpm.measured_boot",
    ];

    let monitor = match crate::tpm::TpmMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            // Not knowing whether a TPM exists is different from knowing there is
            // none, so `present` goes unavailable here rather than false.
            out.push(Reading::unavailable(
                "board.tpm.present",
                None,
                format!("TPM enumeration failed: {e}"),
            ));
            for id in DETAIL_IDS {
                out.push(Reading::unavailable(
                    id,
                    None,
                    format!("TPM enumeration failed: {e}"),
                ));
            }
            return;
        }
    };

    // A successful enumeration that found nothing is a reading: this machine has
    // no TPM. That is exactly the case `present` exists to state.
    out.push(Reading::measured(
        "board.tpm.present",
        serde_json::json!(monitor.has_tpm()),
        None,
    ));

    let Some(tpm) = monitor.tpm() else {
        for id in DETAIL_IDS {
            out.push(Reading::unavailable(
                id,
                None,
                "no TPM present on this machine",
            ));
        }
        return;
    };

    // `Unknown` on either of these is the reader having failed to determine the
    // value, not the device reporting "unknown". Passing it through as a measured
    // identifier would let an agent conclude a TPM's state had been established
    // when it had not — and for a security property that is the wrong way to be
    // wrong. Presence is separately reported above and stays true either way.
    use crate::tpm::{TpmStatus, TpmVersion};
    match tpm.version {
        TpmVersion::Unknown => out.push(Reading::unavailable(
            "board.tpm.version",
            Some(Unit::Identifier),
            "a TPM is present but its specification version could not be determined",
        )),
        v => push_id(out, "board.tpm.version", &format!("{v:?}").to_lowercase()),
    }
    push_text(out, "board.tpm.manufacturer", &tpm.manufacturer);
    match tpm.status {
        TpmStatus::Unknown => out.push(Reading::unavailable(
            "board.tpm.status",
            Some(Unit::Identifier),
            "a TPM is present but whether it is enabled could not be determined",
        )),
        s => push_id(out, "board.tpm.status", &format!("{s:?}").to_lowercase()),
    }
    out.push(Reading::measured(
        "board.tpm.measured_boot",
        serde_json::json!(tpm.measured_boot),
        None,
    ));
}

fn resolve_virtualization(out: &mut Vec<Reading>) {
    const IDS: [&str; 4] = [
        "system.virtualization.platform",
        "system.virtualization.hypervisor",
        "system.virtualization.detection_method",
        "system.virtualization.hardware_support",
    ];

    let monitor = match crate::virtualization::VirtMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            for id in IDS {
                out.push(Reading::unavailable(
                    id,
                    None,
                    format!("virtualization detection unavailable: {e}"),
                ));
            }
            return;
        }
    };

    let hypervisor = monitor.hypervisor();

    // Hyper-V is the one case simon cannot call. When virtualization-based
    // security is on — the Windows 11 default — the host OS runs as the Hyper-V
    // *root partition*, so CPUID reports "Microsoft Hv" on a bare-metal
    // workstation exactly as it does inside a guest. The development machine, an
    // ASUS desktop, is detected as a virtual machine for this reason, and its
    // AMD-V support reads as absent because the hypervisor masks SVM from the root
    // partition.
    //
    // Distinguishing the two needs the partition privilege mask from Hyper-V
    // CPUID leaf 0x40000003, which simon does not read yet. Until it does, saying
    // "virtual machine" here would be a guess presented as a determination — and
    // this is the entity an agent consults before trusting every other reading, so
    // it is the worst possible place for one.
    let is_hyperv = matches!(
        hypervisor.as_ref().map(|h| &h.hypervisor),
        Some(crate::virtualization::Hypervisor::HyperV)
    );

    if is_hyperv {
        out.push(Reading::unavailable(
            "system.virtualization.platform",
            Some(Unit::Identifier),
            "a Hyper-V hypervisor is present, but simon cannot yet tell a root \
             partition (bare metal with virtualization-based security) from a \
             guest VM; both report the same CPUID signature",
        ));
    } else {
        // Bare metal is a determination, not a failure to detect one.
        let platform = if monitor.is_container() {
            "container"
        } else if monitor.is_virtual_machine() {
            "virtual_machine"
        } else {
            "bare_metal"
        };
        push_id(out, "system.virtualization.platform", platform);
    }

    match hypervisor {
        Some(h) => {
            push_id(
                out,
                "system.virtualization.hypervisor",
                &format!("{:?}", h.hypervisor).to_lowercase(),
            );
            push_text(
                out,
                "system.virtualization.detection_method",
                &h.detection_method,
            );
        }
        None => {
            out.push(Reading::unavailable(
                "system.virtualization.hypervisor",
                Some(Unit::Identifier),
                "no hypervisor detected; this machine appears to be bare metal",
            ));
            out.push(Reading::unavailable(
                "system.virtualization.detection_method",
                Some(Unit::Text),
                "no hypervisor was detected, so no detection method applied",
            ));
        }
    }

    // A running hypervisor masks the virtualization bits from CPUID, so a `false`
    // here under one means "not visible", not "not supported" — reporting it would
    // tell an agent this CPU cannot virtualize while it is actively virtualizing.
    match monitor.cpu_capabilities() {
        Some(c) if !is_hyperv => out.push(Reading::measured(
            "system.virtualization.hardware_support",
            serde_json::json!(c.hardware_virt),
            None,
        )),
        Some(_) => out.push(Reading::unavailable(
            "system.virtualization.hardware_support",
            None,
            "a hypervisor is masking the CPU virtualization bits from CPUID; \
             absence here would not mean the hardware lacks them",
        )),
        None => out.push(Reading::unavailable(
            "system.virtualization.hardware_support",
            None,
            "the platform did not report CPU virtualization capabilities",
        )),
    }
}

fn resolve_numa(out: &mut Vec<Reading>) {
    let monitor = match crate::numa::NumaMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            for id in ["memory.numa.nodes", "memory.numa.is_numa"] {
                out.push(Reading::unavailable(
                    id,
                    None,
                    format!("NUMA topology unavailable: {e}"),
                ));
            }
            return;
        }
    };

    let summary = monitor.summary();
    out.push(Reading::measured(
        "memory.numa.nodes",
        serde_json::json!(summary.node_count),
        Some(Unit::Count),
    ));
    out.push(Reading::measured(
        "memory.numa.is_numa",
        serde_json::json!(summary.is_numa),
        None,
    ));

    for node in monitor.nodes() {
        let base = format!("memory.numa.{}", node.id);
        out.push(Reading::measured(
            format!("{base}.cpus"),
            serde_json::json!(node.cpus.len()),
            Some(Unit::Count),
        ));
        push_opt(
            out,
            format!("{base}.memory"),
            (node.memory_total_bytes > 0).then(|| serde_json::json!(node.memory_total_bytes)),
            Some(Unit::Bytes),
            "the platform reported no memory total for this node",
        );
    }
}

fn resolve_ecc(out: &mut Vec<Reading>) {
    const IDS: [&str; 3] = [
        "memory.ecc.active",
        "memory.ecc.correctable_errors",
        "memory.ecc.uncorrectable_errors",
    ];

    let monitor = match crate::edac::EdacMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            // On Windows and macOS there is no EDAC equivalent simon reads, so
            // this is the common path. The note says so rather than implying the
            // machine has no ECC — which would be a claim about the hardware
            // rather than about the reader.
            for id in IDS {
                out.push(Reading::unavailable(
                    id,
                    None,
                    format!("no ECC error reporting interface available here: {e}"),
                ));
            }
            return;
        }
    };

    let overview = monitor.overview();
    if overview.total_controllers == 0 {
        for id in IDS {
            out.push(Reading::unavailable(
                id,
                None,
                "no memory controller exposed ECC reporting; the platform interface \
                 exists but enumerated nothing",
            ));
        }
        return;
    }

    out.push(Reading::measured(
        "memory.ecc.active",
        serde_json::json!(overview.ecc_active),
        None,
    ));
    out.push(Reading::measured(
        "memory.ecc.correctable_errors",
        serde_json::json!(overview.total_ce),
        Some(Unit::Count),
    ));
    out.push(Reading::measured(
        "memory.ecc.uncorrectable_errors",
        serde_json::json!(overview.total_ue),
        Some(Unit::Count),
    ));
}

fn resolve_pci(out: &mut Vec<Reading>) {
    let monitor = match crate::pci_devices::PciDeviceMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            out.push(Reading::unavailable(
                "pci.<none>",
                None,
                format!("PCI enumeration failed: {e}"),
            ));
            return;
        }
    };
    if monitor.devices().is_empty() {
        out.push(Reading::unavailable(
            "pci.<none>",
            None,
            "no PCI devices enumerated on this machine",
        ));
        return;
    }

    for dev in monitor.devices() {
        let base = format!("pci.{}", id_segment(&dev.address));
        push_text(out, format!("{base}.vendor"), &dev.vendor_name);
        push_text(out, format!("{base}.device"), &dev.device_name);
        push_id(
            out,
            format!("{base}.class"),
            &format!("{:?}", dev.class).to_lowercase(),
        );
        push_opt(
            out,
            format!("{base}.driver"),
            (!dev.driver.trim().is_empty()).then(|| serde_json::json!(dev.driver)),
            Some(Unit::Text),
            "no driver is bound to this device",
        );

        match &dev.link_info {
            Some(link) => {
                for (suffix, value) in [
                    ("link.width", &link.width),
                    ("link.max_width", &link.max_width),
                    ("link.speed", &link.speed),
                    ("link.max_speed", &link.max_speed),
                ] {
                    push_opt(
                        out,
                        format!("{base}.{suffix}"),
                        (!value.trim().is_empty()).then(|| serde_json::json!(value)),
                        Some(Unit::Identifier),
                        "the device did not report this link property",
                    );
                }
            }
            None => {
                for suffix in [
                    "link.width",
                    "link.max_width",
                    "link.speed",
                    "link.max_speed",
                ] {
                    out.push(Reading::unavailable(
                        format!("{base}.{suffix}"),
                        Some(Unit::Identifier),
                        "no link state: the device is not PCIe, or this platform \
                         exposes none — Windows reports link training nowhere simon \
                         can reach unelevated",
                    ));
                }
            }
        }

        // The reader uses -1 for "no affinity". Passing that through would give an
        // agent a node number that does not exist.
        push_opt(
            out,
            format!("{base}.numa_node"),
            (dev.numa_node >= 0).then(|| serde_json::json!(dev.numa_node)),
            Some(Unit::Count),
            "the platform reports no NUMA affinity for this device",
        );
    }
}

fn resolve_usb(out: &mut Vec<Reading>) {
    let monitor = match crate::usb::UsbMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            out.push(Reading::unavailable(
                "usb.<none>",
                None,
                format!("USB enumeration failed: {e}"),
            ));
            return;
        }
    };
    if monitor.devices().is_empty() {
        out.push(Reading::unavailable(
            "usb.<none>",
            None,
            "no USB devices enumerated on this machine",
        ));
        return;
    }

    for dev in monitor.devices() {
        // Bus and port rather than enumeration order: an index shifts when an
        // unrelated device is unplugged, which would silently repoint every id.
        let base = format!("usb.{}_{}", dev.bus_number, dev.port_number);
        push_opt(
            out,
            format!("{base}.product"),
            dev.product
                .as_ref()
                .or(dev.description.as_ref())
                .map(|p| serde_json::json!(p)),
            Some(Unit::Text),
            "the device publishes no product string",
        );
        push_opt(
            out,
            format!("{base}.manufacturer"),
            dev.manufacturer.as_ref().map(|m| serde_json::json!(m)),
            Some(Unit::Text),
            "the device publishes no manufacturer string",
        );
        push_id(
            out,
            format!("{base}.vendor_id"),
            &format!("{:04x}", dev.vendor_id),
        );
        push_id(
            out,
            format!("{base}.product_id"),
            &format!("{:04x}", dev.product_id),
        );
        // `Unknown` on either of these is the descriptor not having been read, not
        // a device that identifies itself as unknown. Third occurrence of this
        // shape in one sweep, which is why it is called out in the handoff.
        use crate::usb::{UsbDeviceClass, UsbSpeed};
        match dev.class {
            UsbDeviceClass::Unknown => out.push(Reading::unavailable(
                format!("{base}.class"),
                Some(Unit::Identifier),
                "the device class descriptor was not readable",
            )),
            c => push_id(
                out,
                format!("{base}.class"),
                &format!("{c:?}").to_lowercase(),
            ),
        }
        match dev.speed {
            UsbSpeed::Unknown => out.push(Reading::unavailable(
                format!("{base}.speed"),
                Some(Unit::Identifier),
                "the platform did not report a negotiated bus speed",
            )),
            s => push_id(
                out,
                format!("{base}.speed"),
                &format!("{s:?}").to_lowercase(),
            ),
        }
    }
}

fn resolve_memory_dimms(out: &mut Vec<Reading>) {
    const PER_SLOT: [&str; 10] = [
        "capacity",
        "speed",
        "configured_speed",
        "type",
        "manufacturer",
        "part_number",
        "ecc",
        "data_width",
        "total_width",
        "voltage",
    ];

    let monitor = match crate::memory_topology::MemoryTopologyMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            out.push(Reading::unavailable(
                "memory.dimm.0.locator",
                Some(Unit::Text),
                format!("memory topology unavailable: {e}"),
            ));
            return;
        }
    };

    for (i, dimm) in monitor.topology().dimms.iter().enumerate() {
        let base = format!("memory.dimm.{i}");
        push_text(out, format!("{base}.locator"), &dimm.locator);
        out.push(Reading::measured(
            format!("{base}.populated"),
            serde_json::json!(dimm.populated),
            None,
        ));

        // An empty slot is a real slot with nothing in it. Reporting zeros for its
        // capacity and speed would describe a module of no size running at no
        // speed, which is not what the board is telling us.
        if !dimm.populated {
            for suffix in PER_SLOT {
                out.push(Reading::unavailable(
                    format!("{base}.{suffix}"),
                    None,
                    "this slot is empty",
                ));
            }
            continue;
        }

        push_opt(
            out,
            format!("{base}.capacity"),
            (dimm.capacity_bytes > 0).then(|| serde_json::json!(dimm.capacity_bytes)),
            Some(Unit::Bytes),
            "SMBIOS reported no capacity for a slot it marked populated",
        );
        push_opt(
            out,
            format!("{base}.speed"),
            (dimm.speed_mts > 0).then(|| serde_json::json!(dimm.speed_mts)),
            Some(Unit::Count),
            "SMBIOS reported no rated speed",
        );
        push_opt(
            out,
            format!("{base}.configured_speed"),
            (dimm.configured_speed_mts > 0).then(|| serde_json::json!(dimm.configured_speed_mts)),
            Some(Unit::Count),
            "SMBIOS reported no configured speed",
        );
        push_id(
            out,
            format!("{base}.type"),
            &format!("{:?}", dimm.memory_type).to_lowercase(),
        );
        push_text(out, format!("{base}.manufacturer"), &dimm.manufacturer);
        push_text(out, format!("{base}.part_number"), &dimm.part_number);
        push_opt(
            out,
            format!("{base}.data_width"),
            (dimm.data_width_bits > 0).then(|| serde_json::json!(dimm.data_width_bits)),
            Some(Unit::Count),
            "SMBIOS reported no data width",
        );
        push_opt(
            out,
            format!("{base}.total_width"),
            (dimm.total_width_bits > 0).then(|| serde_json::json!(dimm.total_width_bits)),
            Some(Unit::Count),
            "SMBIOS reported no total width",
        );
        // ECC is the widths differing, so it can only be stated when both are
        // known. Two zeros are equal, and would otherwise report "no ECC".
        if dimm.total_width_bits > 0 && dimm.data_width_bits > 0 {
            out.push(Reading::derived(
                format!("{base}.ecc"),
                serde_json::json!(dimm.total_width_bits > dimm.data_width_bits),
                None,
            ));
        } else {
            out.push(Reading::unavailable(
                format!("{base}.ecc"),
                None,
                "ECC is inferred from the data and total widths, and one is unknown",
            ));
        }
        push_opt(
            out,
            format!("{base}.voltage"),
            (dimm.voltage > 0.0).then(|| serde_json::json!(dimm.voltage)),
            Some(Unit::Volts),
            "SMBIOS reported no operating voltage",
        );
    }
}

fn resolve_cpu_cache(out: &mut Vec<Reading>) {
    const TOTALS: [&str; 4] = [
        "cpu.cache.l1d",
        "cpu.cache.l1i",
        "cpu.cache.l2",
        "cpu.cache.l3",
    ];

    let monitor = match crate::cpu_cache::CpuCacheMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            for id in TOTALS {
                out.push(Reading::unavailable(
                    id,
                    Some(Unit::Bytes),
                    format!("cache topology unavailable: {e}"),
                ));
            }
            return;
        }
    };

    let topology = monitor.topology();
    // The platform sources state these in KiB. Converting here keeps every
    // capacity in the ontology in bytes, so an agent never has to ask which unit a
    // particular size field happens to use.
    let kib = |v: u64| (v > 0).then(|| serde_json::json!(v * 1024));
    for (id, value) in TOTALS.iter().zip([
        topology.total_l1d_kb,
        topology.total_l1i_kb,
        topology.total_l2_kb,
        topology.total_l3_kb,
    ]) {
        push_opt(
            out,
            *id,
            kib(value),
            Some(Unit::Bytes),
            "no aggregate was reported for this level; some platforms publish a \
             combined L1 rather than separate data and instruction figures, in \
             which case the per-instance entities below carry the sizes",
        );
    }

    for (i, cache) in monitor.caches().iter().enumerate() {
        let base = format!("cpu.cache.{i}");
        push_id(
            out,
            format!("{base}.level"),
            &format!("{:?}", cache.level).to_lowercase(),
        );
        push_opt(
            out,
            format!("{base}.size"),
            kib(cache.size_kb),
            Some(Unit::Bytes),
            "the platform reported no size for this cache",
        );
        push_opt(
            out,
            format!("{base}.line_size"),
            (cache.line_size > 0).then(|| serde_json::json!(cache.line_size)),
            Some(Unit::Bytes),
            "the platform reported no line size for this cache",
        );
        push_opt(
            out,
            format!("{base}.shared_cpus"),
            (!cache.shared_cpu_list.trim().is_empty())
                .then(|| serde_json::json!(cache.shared_cpu_list)),
            Some(Unit::Text),
            "the platform does not publish which processors share this cache",
        );
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

/// Make a device name safe to use as one segment of a dotted id.
///
/// Device names are chosen by vendors and drivers, not by simon: Windows hands back
/// "Bluetooth Network Connection", and VLAN interfaces carry dots. Both break the id
/// contract — a dot creates a spurious path segment, and whitespace makes the id
/// impossible to pass to `simon get` without quoting. Substitution is lossy but
/// total, which matters more here than being reversible: an id an agent cannot type
/// is not an id.
fn id_segment(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Collapse runs so "Bluetooth Network Connection" does not become a thicket of
    // underscores, and trim the edges.
    let mut out = String::with_capacity(cleaned.len());
    let mut last_underscore = false;
    for c in cleaned.chars() {
        if c == '_' {
            if !last_underscore {
                out.push(c);
            }
            last_underscore = true;
        } else {
            out.push(c);
            last_underscore = false;
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed
    }
}

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
    ontology.template_for(concrete)
}

/// Number of processes reported. Enumerating every process would make a snapshot
/// dominated by transient noise, so the list is capped — and the cap is announced as
/// a reading rather than applied silently, because a truncated list that looks
/// complete is its own kind of wrong answer.
const PROCESS_LIMIT: usize = 10;

fn resolve_disk(out: &mut Vec<Reading>) {
    let disks = match crate::disk::enumerate_disks() {
        Ok(d) => d,
        Err(e) => {
            out.push(Reading::unavailable(
                "disk.<none>",
                None,
                format!("disk enumeration failed: {e}"),
            ));
            return;
        }
    };
    if disks.is_empty() {
        out.push(Reading::unavailable(
            "disk.<none>",
            None,
            "no block devices enumerated on this machine",
        ));
        return;
    }

    for (i, disk) in disks.iter().enumerate() {
        let base = format!("disk.{i}");
        match disk.info() {
            Ok(info) => {
                push_text(out, format!("{base}.model"), &info.model);
                if info.capacity > 0 {
                    out.push(Reading::measured(
                        format!("{base}.capacity"),
                        serde_json::json!(info.capacity),
                        Some(Unit::Bytes),
                    ));
                } else {
                    out.push(Reading::unavailable(
                        format!("{base}.capacity"),
                        Some(Unit::Bytes),
                        "device reported zero capacity",
                    ));
                }
            }
            Err(e) => {
                for suffix in ["model", "capacity"] {
                    out.push(Reading::unavailable(
                        format!("{base}.{suffix}"),
                        None,
                        format!("device info read failed: {e}"),
                    ));
                }
            }
        }

        // Throughput is documented as calculated from recent samples. One sample is
        // not two, so a single-shot query legitimately has none.
        match disk.io_stats() {
            Ok(io) => {
                push_opt(
                    out,
                    format!("{base}.read_rate"),
                    io.read_throughput.map(|v| serde_json::json!(v)),
                    Some(Unit::BytesPerSecond),
                    "a throughput needs two samples; this query took one",
                );
                push_opt(
                    out,
                    format!("{base}.write_rate"),
                    io.write_throughput.map(|v| serde_json::json!(v)),
                    Some(Unit::BytesPerSecond),
                    "a throughput needs two samples; this query took one",
                );
            }
            Err(e) => {
                for suffix in ["read_rate", "write_rate"] {
                    out.push(Reading::unavailable(
                        format!("{base}.{suffix}"),
                        Some(Unit::BytesPerSecond),
                        format!("I/O statistics read failed: {e}"),
                    ));
                }
            }
        }

        resolve_disk_health(out, &base, disk.as_ref());
    }
}

/// Health, SMART and NVMe entities for one drive.
///
/// Split out because the three readers fail independently and each has its own
/// reason to have nothing to say — a SATA drive has no NVMe log page, an NVMe
/// drive has no sector reallocation concept, and a USB bridge may tunnel neither.
/// An agent needs those told apart, so each absence carries the reason rather than
/// the row being omitted.
fn resolve_disk_health(out: &mut Vec<Reading>, base: &str, disk: &dyn crate::disk::DiskDevice) {
    // Spelled out rather than derived from `{:?}`, which lowercases `NvmeSsd` to
    // `nvmessd` — a token no reader would guess and no id elsewhere resembles.
    use crate::disk::DiskType;
    let kind = match disk.disk_type() {
        DiskType::NvmeSsd => Some("nvme_ssd"),
        DiskType::SataSsd => Some("sata_ssd"),
        DiskType::SataHdd => Some("sata_hdd"),
        DiskType::Scsi => Some("scsi"),
        DiskType::Usb => Some("usb"),
        DiskType::Virtual => Some("virtual"),
        DiskType::Unknown => None,
    };
    push_opt(
        out,
        format!("{base}.kind"),
        kind.map(|k| serde_json::json!(k)),
        Some(Unit::Text),
        "the platform did not classify this device's transport or medium",
    );

    match disk.info() {
        Ok(info) => push_opt(
            out,
            format!("{base}.serial"),
            info.serial.as_ref().map(|s| serde_json::json!(s)),
            Some(Unit::Text),
            "the platform did not disclose a serial number for this device",
        ),
        Err(e) => out.push(Reading::unavailable(
            format!("{base}.serial"),
            Some(Unit::Text),
            format!("device info read failed: {e}"),
        )),
    }

    // `Unknown` is the absence of a verdict, not a verdict of "unknown". Passing it
    // through as a measured string would let an agent record a health check it
    // never actually got — the same shape as reporting an access error as 0 °C.
    use crate::disk::DiskHealth;
    match disk.health() {
        Ok(DiskHealth::Unknown) => out.push(Reading::unavailable(
            format!("{base}.health"),
            Some(Unit::Text),
            "no counter this device exposes yielded a health verdict",
        )),
        Ok(h) => push_text(
            out,
            format!("{base}.health"),
            &format!("{h:?}").to_lowercase(),
        ),
        Err(e) => out.push(Reading::unavailable(
            format!("{base}.health"),
            Some(Unit::Text),
            format!("health read failed: {e}"),
        )),
    }

    const SMART_FIELDS: [&str; 7] = [
        "temperature",
        "smart.passed",
        "smart.power_on_hours",
        "smart.power_cycles",
        "smart.reallocated_sectors",
        "smart.pending_sectors",
        "smart.uncorrectable_sectors",
    ];

    match disk.smart_info() {
        Ok(s) => {
            push_opt(
                out,
                format!("{base}.temperature"),
                s.temperature.map(|t| serde_json::json!(t)),
                Some(Unit::Celsius),
                "this device exposes no thermal sensor",
            );
            out.push(Reading::measured(
                format!("{base}.smart.passed"),
                serde_json::json!(s.passed),
                None,
            ));
            push_opt(
                out,
                format!("{base}.smart.power_on_hours"),
                s.power_on_hours.map(|v| serde_json::json!(v)),
                Some(Unit::Hours),
                "the drive did not report a power-on hour count",
            );
            push_opt(
                out,
                format!("{base}.smart.power_cycles"),
                s.power_cycle_count.map(|v| serde_json::json!(v)),
                Some(Unit::Count),
                "the drive did not report a power cycle count",
            );
            // NVMe has no sector reallocation concept. Reporting 0 here would
            // assert a clean count that was never measured, which is exactly the
            // substitution this module exists to refuse.
            push_opt(
                out,
                format!("{base}.smart.reallocated_sectors"),
                s.reallocated_sectors.map(|v| serde_json::json!(v)),
                Some(Unit::Count),
                "not an NVMe concept; on ATA, the drive did not report attribute 5",
            );
            push_opt(
                out,
                format!("{base}.smart.pending_sectors"),
                s.pending_sectors.map(|v| serde_json::json!(v)),
                Some(Unit::Count),
                "not an NVMe concept; on ATA, the drive did not report attribute 197",
            );
            push_opt(
                out,
                format!("{base}.smart.uncorrectable_sectors"),
                s.uncorrectable_sectors.map(|v| serde_json::json!(v)),
                Some(Unit::Count),
                "the drive did not report an uncorrectable count",
            );
        }
        Err(e) => {
            // On Windows this is the unelevated case for devices that reach the
            // WMI fallback, and the error says so. Naming the reason is the whole
            // point: a caller can tell "needs elevation" from "no such sensor".
            for suffix in SMART_FIELDS {
                out.push(Reading::unavailable(
                    format!("{base}.{suffix}"),
                    None,
                    format!("SMART read failed: {e}"),
                ));
            }
        }
    }

    const NVME_FIELDS: [&str; 6] = [
        "nvme.version",
        "nvme.percentage_used",
        "nvme.data_units_written",
        "nvme.data_units_read",
        "nvme.power_state",
        "nvme.critical_warnings",
    ];

    match disk.nvme_info() {
        Ok(n) => {
            push_opt(
                out,
                format!("{base}.nvme.version"),
                n.nvme_version.as_ref().map(|v| serde_json::json!(v)),
                Some(Unit::Text),
                "the controller did not report a specification version",
            );
            push_opt(
                out,
                format!("{base}.nvme.percentage_used"),
                n.percentage_used.map(|v| serde_json::json!(v)),
                Some(Unit::Percent),
                "the controller did not report a wear figure",
            );
            push_opt(
                out,
                format!("{base}.nvme.data_units_written"),
                n.data_units_written.map(|v| serde_json::json!(v)),
                Some(Unit::Count),
                "the controller did not report a written data unit count",
            );
            push_opt(
                out,
                format!("{base}.nvme.data_units_read"),
                n.data_units_read.map(|v| serde_json::json!(v)),
                Some(Unit::Count),
                "the controller did not report a read data unit count",
            );
            push_opt(
                out,
                format!("{base}.nvme.power_state"),
                n.power_state.map(|v| serde_json::json!(v)),
                Some(Unit::Count),
                "the controller does not implement Get Features for power management",
            );
            push_opt(
                out,
                format!("{base}.nvme.critical_warnings"),
                n.critical_warnings.map(|v| serde_json::json!(v)),
                Some(Unit::Count),
                "the controller did not serve the SMART/Health log page",
            );
        }
        Err(e) => {
            // `NotSupported` here is the device declining the NVMe protocol, which
            // is how a SATA or USB device answers. That is a fact about the device,
            // not a failure of the query, and the note says which.
            for suffix in NVME_FIELDS {
                out.push(Reading::unavailable(
                    format!("{base}.{suffix}"),
                    None,
                    format!("no NVMe controller data: {e}"),
                ));
            }
        }
    }
}

fn resolve_network(out: &mut Vec<Reading>) {
    let mut monitor = match crate::network_monitor::NetworkMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            out.push(Reading::unavailable(
                "network.<none>",
                None,
                format!("network monitor unavailable: {e}"),
            ));
            return;
        }
    };
    let interfaces = match monitor.interfaces() {
        Ok(i) => i,
        Err(e) => {
            out.push(Reading::unavailable(
                "network.<none>",
                None,
                format!("interface enumeration failed: {e}"),
            ));
            return;
        }
    };
    if interfaces.is_empty() {
        out.push(Reading::unavailable(
            "network.<none>",
            None,
            "no network interfaces enumerated",
        ));
        return;
    }

    for iface in &interfaces {
        let base = format!("network.{}", id_segment(&iface.name));

        out.push(Reading::measured(
            format!("{base}.rx_bytes"),
            serde_json::json!(iface.rx_bytes),
            Some(Unit::Bytes),
        ));
        out.push(Reading::measured(
            format!("{base}.tx_bytes"),
            serde_json::json!(iface.tx_bytes),
            Some(Unit::Bytes),
        ));

        // The counters above are cumulative. Reporting them as a rate would be the
        // same category of error as calling a spec constant a measurement.
        for dir in ["rx", "tx"] {
            out.push(Reading::unavailable(
                format!("{base}.{dir}_rate"),
                Some(Unit::BytesPerSecond),
                "a rate needs two samples; this query took one — differentiate \
                 rx_bytes/tx_bytes across two snapshots",
            ));
        }

        push_opt(
            out,
            format!("{base}.link_speed"),
            iface.speed_mbps.map(|s| serde_json::json!(s)),
            Some(Unit::Count),
            "driver reports no negotiated link rate for this interface",
        );
    }
}

fn resolve_process(out: &mut Vec<Reading>) {
    let mut monitor = match crate::process_monitor::ProcessMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            out.push(Reading::unavailable(
                "process.<none>",
                None,
                format!("process monitor unavailable: {e}"),
            ));
            return;
        }
    };
    let procs = match monitor.processes_by_memory() {
        Ok(p) => p,
        Err(e) => {
            out.push(Reading::unavailable(
                "process.<none>",
                None,
                format!("process enumeration failed: {e}"),
            ));
            return;
        }
    };
    if procs.is_empty() {
        out.push(Reading::unavailable(
            "process.<none>",
            None,
            "no processes enumerated",
        ));
        return;
    }

    let total = procs.len();
    for p in procs.iter().take(PROCESS_LIMIT) {
        let base = format!("process.{}", p.pid);
        push_text(out, format!("{base}.name"), &p.name);
        out.push(Reading::measured(
            format!("{base}.cpu"),
            serde_json::json!(p.cpu_percent),
            Some(Unit::Percent),
        ));
        out.push(Reading::measured(
            format!("{base}.memory"),
            serde_json::json!(p.memory_bytes),
            Some(Unit::Bytes),
        ));
    }

    // Announce the truncation. A capped list presented as complete would let an agent
    // conclude a process is absent when it was merely ranked eleventh.
    if total > PROCESS_LIMIT {
        out.push(Reading::unavailable(
            "process.<truncated>",
            Some(Unit::Count),
            format!(
                "{total} processes exist; this snapshot reports the {PROCESS_LIMIT} \
                 largest by memory. Absence from this list is not absence from the \
                 machine — use `simon cli processes` for the full table"
            ),
        ));
    }
}

fn resolve_thermal(out: &mut Vec<Reading>) {
    let temps = match crate::motherboard::get_system_temperatures() {
        Ok(t) => t,
        Err(e) => {
            out.push(Reading::unavailable(
                "thermal.<none>",
                Some(Unit::Celsius),
                format!("temperature read failed: {e}"),
            ));
            return;
        }
    };

    for (name, value) in [
        ("cpu", temps.cpu),
        ("gpu", temps.gpu),
        ("motherboard", temps.motherboard),
    ] {
        match value {
            Some(v) => out.push(Reading::measured(
                format!("thermal.{name}.temperature"),
                serde_json::json!(v),
                Some(Unit::Celsius),
            )),
            None => out.push(Reading::unavailable(
                format!("thermal.{name}.temperature"),
                Some(Unit::Celsius),
                "no sensor exposed for this component — on Windows most board \
                 sensors require a signed kernel driver, and a virtual machine \
                 usually exposes none at all",
            )),
        }
    }

    for (device, value) in temps.storage.iter().chain(temps.network.iter()) {
        out.push(Reading::measured(
            format!("thermal.{}.temperature", id_segment(device)),
            serde_json::json!(value),
            Some(Unit::Celsius),
        ));
    }

    // There is deliberately no `thermal.<none>` row when every sensor comes back
    // empty. `<none>` means the domain enumerated nothing, and this resolver always
    // enumerates cpu, gpu and motherboard — it emits an `unavailable` row for each,
    // which states the same fact per component and more precisely. Emitting both
    // produced a snapshot that said "the domain enumerated nothing" beside three
    // thermal rows. It only showed up on a machine where no sensor reads at all,
    // which is why the first macOS CI run found it and no Windows run ever did.
    //
    // The read-failure path above still emits `<none>`, and correctly: it returns
    // before pushing anything, so nothing contradicts it.
}

fn resolve_power(out: &mut Vec<Reading>) {
    match crate::battery::BatteryMonitor::new() {
        Ok(monitor) => {
            let batteries = monitor.batteries();
            if batteries.is_empty() {
                out.push(Reading::unavailable(
                    "power.battery.percentage",
                    Some(Unit::Percent),
                    "no battery present (desktop or always-AC machine)",
                ));
            } else {
                out.push(Reading::measured(
                    "power.battery.percentage",
                    serde_json::json!(batteries[0].charge_percent),
                    Some(Unit::Percent),
                ));
            }
        }
        Err(e) => out.push(Reading::unavailable(
            "power.battery.percentage",
            Some(Unit::Percent),
            format!("battery monitor unavailable: {e}"),
        )),
    }
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

    /// Live values must satisfy the ranges the ontology declares.
    ///
    /// This held at rest and failed under the load of the full test suite, when a
    /// per-core utilization derived as `user + system` drifted past 100 percent.
    /// Two things changed: the derivation now uses `100 - idle`, and `snapshot`
    /// withholds anything the ontology rejects. The assertion is therefore now
    /// structural rather than a matter of timing.
    #[test]
    fn live_readings_are_physically_possible() {
        let problems = validate(&snapshot());
        assert!(problems.is_empty(), "impossible readings: {problems:#?}");
    }

    /// The gate must withhold rather than clamp. A clamped value looks like a
    /// reading and is not one, which is the failure mode this module exists to
    /// prevent — and it would destroy the evidence that sampling went wrong.
    #[test]
    fn out_of_range_values_are_withheld_with_the_offending_value_named() {
        use crate::ontology::{Domain, EntityKind};

        let ontology = Ontology::build();
        let entity = ontology.get("cpu.total.utilization").unwrap();
        assert_eq!(entity.domain, Domain::Cpu);
        assert_eq!(entity.kind, EntityKind::Measurement);

        // The range check the gate relies on, exercised directly: an impossible
        // percentage must be reported as a problem, not silently corrected.
        let problem = entity
            .validate_range(103.0)
            .expect("103 percent should be rejected");
        assert!(
            problem.contains("103"),
            "the rejection must name the offending value so it can be diagnosed, \
             got: {problem}"
        );
        assert!(entity.validate_range(99.9).is_none());
    }

    /// An id an agent cannot type is not an id.
    ///
    /// Device names come from vendors, not from simon: the first version of the
    /// network resolver emitted `network.Bluetooth Network Connection.rx_bytes`,
    /// which cannot be passed to `simon get` without quoting and whose spaces make
    /// the dotted structure ambiguous. Segments are sanitised at construction; this
    /// asserts the whole snapshot honours it.
    #[test]
    fn every_id_is_shell_safe_and_well_structured() {
        for r in snapshot() {
            assert!(
                !r.id.contains(char::is_whitespace),
                "{} contains whitespace, so it cannot be passed as a bare argument",
                r.id
            );
            assert!(
                !r.id.contains("..") && !r.id.starts_with('.') && !r.id.ends_with('.'),
                "malformed id: {}",
                r.id
            );
            for segment in r.id.split('.') {
                assert!(!segment.is_empty(), "{} has an empty path segment", r.id);
            }
        }
    }

    #[test]
    fn id_segments_are_sanitised_without_collapsing_distinct_names() {
        assert_eq!(
            id_segment("Bluetooth Network Connection"),
            "Bluetooth_Network_Connection"
        );
        assert_eq!(id_segment("eth0.100"), "eth0_100");
        assert_eq!(id_segment("  spaced  "), "spaced");
        // A name made entirely of separators still has to yield a usable segment.
        assert_eq!(id_segment("///"), "unnamed");
        // Distinct names must not collide into one id.
        assert_ne!(id_segment("eth0"), id_segment("eth1"));
    }

    /// A diagnostic must only appear when its condition holds. `disk.<none>` in a
    /// snapshot that also lists eight disk readings is a contradiction, and one that
    /// would teach an agent to distrust the diagnostics generally.
    #[test]
    fn diagnostics_do_not_contradict_the_readings_beside_them() {
        let readings = snapshot();
        for domain in crate::ontology::Domain::ALL {
            let none_id = format!("{}.<none>", domain.as_str());
            let has_none = readings.iter().any(|r| r.id == none_id);
            if !has_none {
                continue;
            }
            let real_rows = readings
                .iter()
                .filter(|r| {
                    r.id.starts_with(&format!("{}.", domain.as_str())) && !r.id.contains('<')
                })
                .count();
            assert_eq!(
                real_rows,
                0,
                "{none_id} claims the domain enumerated nothing, but {real_rows} \
                 {} rows are present alongside it",
                domain.as_str()
            );
        }
    }

    /// Every domain the ontology declares must appear in a snapshot, even when the
    /// machine has no such device. Silence would leave an agent unable to tell "no
    /// disks here" from "simon does not read disks".
    #[test]
    fn no_declared_domain_is_silently_absent() {
        let readings = snapshot();
        for domain in crate::ontology::Domain::ALL {
            let prefix = format!("{}.", domain.as_str());
            assert!(
                readings.iter().any(|r| r.id.starts_with(&prefix)),
                "domain {} produced no rows at all — an agent cannot distinguish an \
                 absent device from an unimplemented reader",
                domain.as_str()
            );
        }
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
        // One snapshot, tallied. The previous version compared `coverage()`
        // against a second, independent `snapshot()` and asserted equal lengths —
        // which is a race on a live machine, since a USB device or a process can
        // come and go between the two calls. It passed for as long as the suite
        // was quiet enough, then failed under parallel load for no reason
        // connected to what it was testing.
        let readings = snapshot();
        let c = coverage_of(&readings);
        assert_eq!(c.total, c.resolved + c.unavailable);
        assert_eq!(c.total, readings.len());
    }
}
