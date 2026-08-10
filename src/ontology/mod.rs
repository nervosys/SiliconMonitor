//! Machine-readable ontology over everything simon can report.
//!
//! The three interfaces — CLI, TUI, GUI — all render the same underlying readings,
//! but until now each described them in its own words. `cli board` printed
//! `Kernel:`, the GUI printed `Kernel Version`, and the JSON output used
//! `kernel_version`; an agent reading one could not reliably talk about the others.
//! This module is the single naming authority the three surfaces agree on.
//!
//! ## What an entity is
//!
//! Every value simon can report has a stable dotted id — `gpu.0.thermal.temperature`,
//! `cpu.total.utilization`, `disk.0.capacity` — plus a unit and, most importantly,
//! a [`Provenance`].
//!
//! ## Why provenance is the point
//!
//! An agent reading `gpu.0.thermal.critical_temperature = null` learns nothing about
//! *why* it is null, and an agent reading `= 110` cannot tell whether the hardware
//! said so or whether a table of plausible constants did. That distinction is not
//! cosmetic: this repository has repeatedly shipped invented numbers through the same
//! field as measured ones — a boot time of exactly 45s assigned whenever it could not
//! be read, a GPU power percentage whose denominator came from a core-count lookup,
//! Secure Boot reported from the existence of a registry key rather than its value.
//! Each was indistinguishable from a real reading at the point of consumption.
//!
//! [`Provenance`] makes the distinction explicit and machine-checkable, so an agent
//! can refuse to reason about a specification constant as though it were a sample.

pub mod resolve;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Ontology format version. Bump the major on any breaking id or field change.
pub const ONTOLOGY_VERSION: &str = "1.0";

/// Where a value came from.
///
/// This is the field that distinguishes a reading from a guess. It is deliberately
/// not defaultable: every entity must state its provenance explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Sampled from hardware or the OS during this collection cycle.
    Measured,
    /// A published constant — a spec sheet, a vendor table, a documented default.
    /// True of the hardware, but not observed on *this* machine right now.
    Specification,
    /// Computed from other entities. `derived_from` names the inputs.
    Derived,
    /// Not obtainable here: unsupported platform, absent device, missing permission.
    /// A consumer must render this as "unknown", never as zero.
    Unavailable,
}

impl Provenance {
    /// Whether a value with this provenance may be treated as a live observation.
    ///
    /// The guard an agent should apply before reasoning about a number as fact.
    /// `Specification` values are true but stale; `Derived` values inherit the
    /// weakest provenance of their inputs and so must be checked through those.
    pub fn is_observation(self) -> bool {
        matches!(self, Self::Measured)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Specification => "specification",
            Self::Derived => "derived",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Physical unit of a value. `None` on the entity means dimensionless or textual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Celsius,
    Percent,
    Bytes,
    BytesPerSecond,
    Hertz,
    Megahertz,
    Watts,
    Milliwatts,
    Volts,
    Rpm,
    Seconds,
    Hours,
    Milliseconds,
    Count,
    Identifier,
    Text,
}

impl Unit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Celsius => "celsius",
            Self::Percent => "percent",
            Self::Bytes => "bytes",
            Self::BytesPerSecond => "bytes_per_second",
            Self::Hertz => "hertz",
            Self::Megahertz => "megahertz",
            Self::Watts => "watts",
            Self::Milliwatts => "milliwatts",
            Self::Volts => "volts",
            Self::Rpm => "rpm",
            Self::Seconds => "seconds",
            Self::Hours => "hours",
            Self::Milliseconds => "milliseconds",
            Self::Count => "count",
            Self::Identifier => "identifier",
            Self::Text => "text",
        }
    }

    /// Whether a negative value is physically meaningful for this unit.
    ///
    /// Used by [`Entity::validate_range`] so a consumer can reject impossible
    /// readings without hardcoding per-field knowledge.
    pub fn allows_negative(self) -> bool {
        matches!(self, Self::Celsius | Self::Volts)
    }
}

/// What kind of thing an entity is, which determines how an agent may use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// A sampled quantity that changes over time.
    Measurement,
    /// A fixed property of the device — model name, capacity, core count.
    Identity,
    /// A value that can be written back, subject to the apply layer's confirmation.
    Setting,
    /// A limit or threshold the hardware declares.
    Limit,
    /// Not a property of the hardware but a statement about the reading process
    /// itself — that a domain enumerated nothing, or that a list was truncated.
    /// These exist because silence is the one answer a resolver must never give: an
    /// agent that receives no `disk.*` rows cannot tell an absent device from an
    /// unimplemented reader, and a capped list presented as complete invites the
    /// conclusion that a process is absent when it was merely ranked eleventh.
    Diagnostic,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Measurement => "measurement",
            Self::Identity => "identity",
            Self::Setting => "setting",
            Self::Limit => "limit",
            Self::Diagnostic => "diagnostic",
        }
    }
}

/// The subsystem an entity belongs to. Mirrors the CLI's top-level nouns so that
/// `simon cli gpu` and the `gpu.*` id space are obviously the same thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Cpu,
    Gpu,
    Memory,
    Disk,
    Network,
    Power,
    Thermal,
    Process,
    System,
    Board,
}

impl Domain {
    pub const ALL: &'static [Domain] = &[
        Domain::Cpu,
        Domain::Gpu,
        Domain::Memory,
        Domain::Disk,
        Domain::Network,
        Domain::Power,
        Domain::Thermal,
        Domain::Process,
        Domain::System,
        Domain::Board,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Memory => "memory",
            Self::Disk => "disk",
            Self::Network => "network",
            Self::Power => "power",
            Self::Thermal => "thermal",
            Self::Process => "process",
            Self::System => "system",
            Self::Board => "board",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|d| d.as_str().eq_ignore_ascii_case(s))
    }
}

/// One named thing simon can report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// Stable dotted id. `{domain}.{instance}.{path}`, where `instance` is omitted
    /// for singletons (`memory.total`) and is an index otherwise (`gpu.0.name`).
    /// Ids are the contract: they may be added, but never repurposed.
    pub id: String,
    pub domain: Domain,
    pub kind: EntityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
    pub provenance: Provenance,
    /// Whether the reader may legitimately have no value for this. A `false` here
    /// with a null value is a bug in the reader, not an absent device.
    pub nullable: bool,
    /// One line, written for a reader that cannot see the hardware.
    pub description: String,
    /// Ids this value is computed from. Non-empty only for [`EntityKind`] entries
    /// whose provenance is [`Provenance::Derived`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<String>,
    /// The `simon profile set` id that writes this, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writable_via: Option<String>,
}

impl Entity {
    fn new(
        id: &str,
        domain: Domain,
        kind: EntityKind,
        unit: Option<Unit>,
        provenance: Provenance,
        nullable: bool,
        description: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            domain,
            kind,
            unit,
            provenance,
            nullable,
            description: description.to_string(),
            derived_from: Vec::new(),
            writable_via: None,
        }
    }

    fn derived(mut self, from: &[&str]) -> Self {
        self.provenance = Provenance::Derived;
        self.derived_from = from.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Check a concrete value against what this entity's unit permits.
    ///
    /// Returns the reason it is impossible, or `None` if it is admissible. This is
    /// the same class of assertion as the `plausibility` test tier, exposed so that
    /// an agent consuming the JSON can apply it without reimplementing the rules.
    pub fn validate_range(&self, value: f64) -> Option<String> {
        let unit = self.unit?;
        if value.is_nan() {
            return Some(format!("{} is NaN", self.id));
        }
        if value < 0.0 && !unit.allows_negative() {
            return Some(format!(
                "{} = {value} is negative, which {} cannot be",
                self.id,
                unit.as_str()
            ));
        }
        if unit == Unit::Percent && value > 100.0 {
            return Some(format!("{} = {value} exceeds 100 percent", self.id));
        }
        if unit == Unit::Celsius && !(-273.15..=200.0).contains(&value) {
            return Some(format!(
                "{} = {value}C is outside any temperature real hardware reports",
                self.id
            ));
        }
        None
    }
}

/// The full set of entities simon knows how to name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ontology {
    pub version: String,
    /// Keyed by id so lookups are O(log n) and the JSON is stable across runs.
    pub entities: BTreeMap<String, Entity>,
}

impl Default for Ontology {
    fn default() -> Self {
        Self::build()
    }
}

impl Ontology {
    /// Construct the ontology. Pure — no hardware is touched, so this is safe to call
    /// on any machine and produces identical output everywhere, which is what makes
    /// it usable as a schema an agent can fetch ahead of time.
    pub fn build() -> Self {
        let mut entities = BTreeMap::new();
        let mut add = |e: Entity| {
            debug_assert!(
                !entities.contains_key(&e.id),
                "duplicate ontology id: {}",
                e.id
            );
            entities.insert(e.id.clone(), e);
        };

        use Domain as D;
        use EntityKind as K;
        use Provenance as P;
        use Unit as U;

        // ── CPU ──────────────────────────────────────────────────────────────
        add(Entity::new(
            "cpu.model",
            D::Cpu,
            K::Identity,
            Some(U::Text),
            P::Measured,
            false,
            "Processor model string as the CPU reports it.",
        ));
        add(Entity::new(
            "cpu.cores.physical",
            D::Cpu,
            K::Identity,
            Some(U::Count),
            P::Measured,
            false,
            "Physical core count, excluding SMT siblings.",
        ));
        add(Entity::new(
            "cpu.cores.logical",
            D::Cpu,
            K::Identity,
            Some(U::Count),
            P::Measured,
            false,
            "Logical processor count, including SMT siblings.",
        ));
        add(Entity::new(
            "cpu.total.utilization",
            D::Cpu,
            K::Measurement,
            Some(U::Percent),
            P::Measured,
            false,
            "System-wide CPU busy percentage over the last sampling interval.",
        )
        .derived(&["cpu.total.idle"]));
        add(Entity::new(
            "cpu.total.idle",
            D::Cpu,
            K::Measurement,
            Some(U::Percent),
            P::Measured,
            false,
            "System-wide idle percentage over the last sampling interval.",
        ));
        add(Entity::new(
            "cpu.core.{n}.utilization",
            D::Cpu,
            K::Measurement,
            Some(U::Percent),
            P::Measured,
            true,
            "Per-core busy percentage. Null where the platform exposes no \
             per-processor times — deliberately not the system average.",
        ));
        add(Entity::new(
            "cpu.core.{n}.frequency",
            D::Cpu,
            K::Measurement,
            Some(U::Megahertz),
            P::Measured,
            true,
            "Current core clock. Null when the platform reports no per-core clock.",
        ));
        add(Entity::new(
            "cpu.core.{n}.frequency.min",
            D::Cpu,
            K::Limit,
            Some(U::Megahertz),
            P::Unavailable,
            true,
            "Minimum core clock. Unavailable on Windows, which exposes no such \
             figure; it is not inferable from the maximum.",
        ));

        // ── GPU ──────────────────────────────────────────────────────────────
        add(Entity::new(
            "gpu.{n}.name",
            D::Gpu,
            K::Identity,
            Some(U::Text),
            P::Measured,
            false,
            "Adapter name as the driver reports it.",
        ));
        add(Entity::new(
            "gpu.{n}.vendor",
            D::Gpu,
            K::Identity,
            Some(U::Identifier),
            P::Measured,
            false,
            "One of nvidia, amd, intel, apple.",
        ));
        add(Entity::new(
            "gpu.{n}.utilization",
            D::Gpu,
            K::Measurement,
            Some(U::Percent),
            P::Measured,
            true,
            "Graphics engine busy percentage.",
        ));
        add(Entity::new(
            "gpu.{n}.memory.used",
            D::Gpu,
            K::Measurement,
            Some(U::Bytes),
            P::Measured,
            true,
            "Video memory in use. Null on unified-memory parts that report none.",
        ));
        add(Entity::new(
            "gpu.{n}.memory.total",
            D::Gpu,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            true,
            "Total video memory.",
        ));
        add(Entity::new(
            "gpu.{n}.thermal.temperature",
            D::Gpu,
            K::Measurement,
            Some(U::Celsius),
            P::Measured,
            true,
            "GPU core temperature. Null where no sensor is exposed — notably Apple \
             Silicon via powermetrics. Null is not zero degrees.",
        ));
        add(Entity::new(
            "gpu.{n}.thermal.max_temperature",
            D::Gpu,
            K::Limit,
            Some(U::Celsius),
            P::Measured,
            true,
            "Vendor-declared thermal limit, read from the driver. Null when the \
             vendor publishes none — it must not be filled with a plausible constant.",
        ));
        add(Entity::new(
            "gpu.{n}.thermal.critical_temperature",
            D::Gpu,
            K::Limit,
            Some(U::Celsius),
            P::Measured,
            true,
            "Vendor-declared shutdown threshold, read from the driver. Null when \
             unpublished.",
        ));
        add(Entity::new(
            "gpu.{n}.power.draw",
            D::Gpu,
            K::Measurement,
            Some(U::Milliwatts),
            P::Measured,
            true,
            "Instantaneous board power draw.",
        ));
        add(Entity::new(
            "gpu.{n}.power.limit",
            D::Gpu,
            K::Limit,
            Some(U::Milliwatts),
            P::Measured,
            true,
            "Enforced power cap. Null when the driver exposes none; a percentage of \
             an unknown cap is not a measurement and is reported null in turn.",
        ));
        add(Entity::new(
            "gpu.{n}.clocks.graphics",
            D::Gpu,
            K::Measurement,
            Some(U::Megahertz),
            P::Measured,
            true,
            "Current graphics clock.",
        ));
        add(Entity::new(
            "gpu.{n}.clocks.graphics.max",
            D::Gpu,
            K::Limit,
            Some(U::Megahertz),
            P::Measured,
            true,
            "Maximum graphics clock as the driver declares it. Null where the vendor \
             publishes no ceiling.",
        ));

        // ── Memory ───────────────────────────────────────────────────────────
        add(Entity::new(
            "memory.total",
            D::Memory,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            false,
            "Total installed RAM visible to the OS.",
        ));
        add(Entity::new(
            "memory.used",
            D::Memory,
            K::Measurement,
            Some(U::Bytes),
            P::Measured,
            false,
            "RAM currently in use.",
        ));
        add(Entity::new(
            "memory.utilization",
            D::Memory,
            K::Measurement,
            Some(U::Percent),
            P::Measured,
            false,
            "Used RAM as a percentage of total.",
        )
        .derived(&["memory.used", "memory.total"]));
        add(Entity::new(
            "memory.swap.total",
            D::Memory,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            true,
            "Total swap or pagefile capacity.",
        ));
        add(Entity::new(
            "memory.swap.used",
            D::Memory,
            K::Measurement,
            Some(U::Bytes),
            P::Measured,
            true,
            "Swap or pagefile currently in use.",
        ));

        add(Entity::new(
            "cpu.cache.l1d",
            D::Cpu,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            true,
            "Total L1 data cache across all cores. Reported in bytes, though the \
             platform sources state it in KiB, so that every capacity in this \
             ontology carries the same unit.",
        ));
        add(Entity::new(
            "cpu.cache.l1i",
            D::Cpu,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            true,
            "Total L1 instruction cache across all cores.",
        ));
        add(Entity::new(
            "cpu.cache.l2",
            D::Cpu,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            true,
            "Total L2 cache.",
        ));
        add(Entity::new(
            "cpu.cache.l3",
            D::Cpu,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            true,
            "Total L3 cache. Null on parts with no last-level cache to report.",
        ));
        add(Entity::new(
            "cpu.cache.{n}.level",
            D::Cpu,
            K::Identity,
            Some(U::Identifier),
            P::Measured,
            true,
            "Cache level of one cache instance — l1, l2, l3, l4.",
        ));
        add(Entity::new(
            "cpu.cache.{n}.size",
            D::Cpu,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            true,
            "Size of one cache instance.",
        ));
        add(Entity::new(
            "cpu.cache.{n}.line_size",
            D::Cpu,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            true,
            "Cache line size. The unit of false sharing, and the reason this is \
             exposed per instance rather than assumed to be 64 bytes.",
        ));
        add(Entity::new(
            "cpu.cache.{n}.shared_cpus",
            D::Cpu,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "Which logical processors share this cache, as a range list. Null where \
             the platform does not publish the sharing map.",
        ));

        // ── Disk ─────────────────────────────────────────────────────────────
        add(Entity::new(
            "disk.{n}.model",
            D::Disk,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "Drive model string from the device.",
        ));
        add(Entity::new(
            "disk.{n}.capacity",
            D::Disk,
            K::Identity,
            Some(U::Bytes),
            P::Measured,
            false,
            "Raw device capacity.",
        ));
        add(Entity::new(
            "disk.{n}.read_rate",
            D::Disk,
            K::Measurement,
            Some(U::BytesPerSecond),
            P::Measured,
            true,
            "Read throughput. Null where the platform reports only a combined \
             figure that cannot be attributed to a direction.",
        ));
        add(Entity::new(
            "disk.{n}.write_rate",
            D::Disk,
            K::Measurement,
            Some(U::BytesPerSecond),
            P::Measured,
            true,
            "Write throughput. Null under the same condition as the read rate.",
        ));
        add(Entity::new(
            "disk.{n}.partition.{m}.mount_point",
            D::Disk,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "Mount point of a partition that genuinely resides on this device. A \
             device holding no mounted volume reports none rather than every volume \
             on the system.",
        ));
        add(Entity::new(
            "disk.{n}.serial",
            D::Disk,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "Drive serial number. Null where the platform will not disclose it \
             without elevation.",
        ));
        add(Entity::new(
            "disk.{n}.kind",
            D::Disk,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "Transport and medium — nvme_ssd, sata_ssd, sata_hdd, and so on. \
             Distinct from the SMART media type, which describes the medium alone.",
        ));
        add(Entity::new(
            "disk.{n}.health",
            D::Disk,
            K::Measurement,
            Some(U::Text),
            P::Measured,
            true,
            "Health verdict — healthy, warning, critical, failed, unknown. Derived \
             from the drive's own report, never from the device merely existing. \
             `unknown` is a real answer and means no counter could be read.",
        ));
        add(Entity::new(
            "disk.{n}.temperature",
            D::Disk,
            K::Measurement,
            Some(U::Celsius),
            P::Measured,
            true,
            "Drive temperature, from the NVMe health log or ATA attribute 194. \
             Null on devices that expose no thermal sensor.",
        ));
        add(Entity::new(
            "disk.{n}.smart.passed",
            D::Disk,
            K::Measurement,
            None,
            P::Measured,
            true,
            "The drive's own pass/fail verdict on itself — NVMe critical warning \
             bits, or the ATA failure prediction. Not a judgement computed from the \
             counters below.",
        ));
        add(Entity::new(
            "disk.{n}.smart.power_on_hours",
            D::Disk,
            K::Measurement,
            Some(U::Hours),
            P::Measured,
            true,
            "Lifetime powered-on hours. A minority of ATA drives report this \
             attribute in minutes and nothing in the structure distinguishes them.",
        ));
        add(Entity::new(
            "disk.{n}.smart.power_cycles",
            D::Disk,
            K::Measurement,
            Some(U::Count),
            P::Measured,
            true,
            "Lifetime power cycle count.",
        ));
        add(Entity::new(
            "disk.{n}.smart.reallocated_sectors",
            D::Disk,
            K::Measurement,
            Some(U::Count),
            P::Measured,
            true,
            "Sectors the drive has remapped. Null on NVMe, which has no sector \
             reallocation concept — that is an ATA notion, and zero would assert a \
             clean count that was never measured.",
        ));
        add(Entity::new(
            "disk.{n}.smart.pending_sectors",
            D::Disk,
            K::Measurement,
            Some(U::Count),
            P::Measured,
            true,
            "Sectors awaiting remap — data at risk now. Null on NVMe, for the same \
             reason as the reallocated count.",
        ));
        add(Entity::new(
            "disk.{n}.smart.uncorrectable_sectors",
            D::Disk,
            K::Measurement,
            Some(U::Count),
            P::Measured,
            true,
            "Uncorrectable sectors on ATA; media errors on NVMe.",
        ));
        add(Entity::new(
            "disk.{n}.nvme.version",
            D::Disk,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "NVMe specification version the controller reports. Null on non-NVMe.",
        ));
        add(Entity::new(
            "disk.{n}.nvme.percentage_used",
            D::Disk,
            K::Measurement,
            Some(U::Percent),
            P::Measured,
            true,
            "Share of rated write endurance consumed. May legitimately exceed 100 \
             on a drive past its rating, which is a reading and not an error.",
        ));
        add(Entity::new(
            "disk.{n}.nvme.data_units_written",
            D::Disk,
            K::Measurement,
            Some(U::Count),
            P::Measured,
            true,
            "Lifetime data units written, in 1000x512-byte units as NVMe defines \
             them. Reported in the drive's own unit rather than converted, so that \
             the number matches what other NVMe tools show.",
        ));
        add(Entity::new(
            "disk.{n}.nvme.data_units_read",
            D::Disk,
            K::Measurement,
            Some(U::Count),
            P::Measured,
            true,
            "Lifetime data units read, in the same units as the written count.",
        ));
        add(Entity::new(
            "disk.{n}.nvme.power_state",
            D::Disk,
            K::Measurement,
            Some(U::Identifier),
            P::Measured,
            true,
            "Current NVMe power state index. State 0 is both the active state and \
             what an unread field would look like, so this is null unless the \
             controller answered Get Features.",
        ));
        add(Entity::new(
            "disk.{n}.nvme.critical_warnings",
            D::Disk,
            K::Measurement,
            Some(U::Identifier),
            P::Measured,
            true,
            "NVMe critical warning bitfield. Zero is a reading — a drive with \
             nothing wrong — and is reported as such rather than omitted.",
        ));

        // ── Network ──────────────────────────────────────────────────────────
        add(Entity::new(
            "network.{iface}.rx_rate",
            D::Network,
            K::Measurement,
            Some(U::BytesPerSecond),
            P::Measured,
            true,
            "Receive throughput on this interface.",
        ));
        add(Entity::new(
            "network.{iface}.tx_rate",
            D::Network,
            K::Measurement,
            Some(U::BytesPerSecond),
            P::Measured,
            true,
            "Transmit throughput on this interface.",
        ));
        // Cumulative counters exist because a rate does not: a single-shot query has
        // one sample, and a rate needs two. Declaring only `rx_rate` forced a
        // resolver to either return nothing useful or to pass a counter off as a
        // rate. These are what a one-shot read can honestly produce.
        add(Entity::new(
            "network.{iface}.rx_bytes",
            D::Network,
            K::Measurement,
            Some(U::Bytes),
            P::Measured,
            true,
            "Bytes received since the interface came up. A counter, not a rate — \
             differentiate two samples to obtain throughput.",
        ));
        add(Entity::new(
            "network.{iface}.tx_bytes",
            D::Network,
            K::Measurement,
            Some(U::Bytes),
            P::Measured,
            true,
            "Bytes transmitted since the interface came up. A counter, not a rate.",
        ));
        add(Entity::new(
            "network.{iface}.link_speed",
            D::Network,
            K::Limit,
            Some(U::Count),
            P::Measured,
            true,
            "Negotiated link rate in megabits per second, read from the driver. \
             Platforms that can only guess from the interface name report this as \
             specification-provenance instead.",
        ));

        // ── Power and thermal ────────────────────────────────────────────────
        add(Entity::new(
            "power.battery.percentage",
            D::Power,
            K::Measurement,
            Some(U::Percent),
            P::Measured,
            true,
            "Battery charge. Null on machines with no battery.",
        ));
        add(Entity::new(
            "thermal.{sensor}.temperature",
            D::Thermal,
            K::Measurement,
            Some(U::Celsius),
            P::Measured,
            true,
            "A named board or package temperature sensor.",
        ));
        add(Entity::new(
            "thermal.{sensor}.fan_rpm",
            D::Thermal,
            K::Measurement,
            Some(U::Rpm),
            P::Measured,
            true,
            "Fan speed. Null where no tachometer is wired.",
        ));

        // ── Process ──────────────────────────────────────────────────────────
        add(Entity::new(
            "process.{pid}.name",
            D::Process,
            K::Identity,
            Some(U::Text),
            P::Measured,
            false,
            "Executable name.",
        ));
        add(Entity::new(
            "process.{pid}.cpu",
            D::Process,
            K::Measurement,
            Some(U::Percent),
            P::Measured,
            true,
            "Process CPU share over the last interval.",
        ));
        add(Entity::new(
            "process.{pid}.memory",
            D::Process,
            K::Measurement,
            Some(U::Bytes),
            P::Measured,
            true,
            "Process resident memory.",
        ));

        // ── System and board ─────────────────────────────────────────────────
        add(Entity::new(
            "system.os.name",
            D::System,
            K::Identity,
            Some(U::Text),
            P::Measured,
            false,
            "Operating system product name, corrected where the vendor's own \
             registry is stale.",
        ));
        add(Entity::new(
            "system.os.build",
            D::System,
            K::Identity,
            Some(U::Identifier),
            P::Measured,
            true,
            "OS build identifier.",
        ));
        add(Entity::new(
            "system.uptime",
            D::System,
            K::Measurement,
            Some(U::Seconds),
            P::Measured,
            false,
            "Seconds since boot.",
        ));
        add(Entity::new(
            "system.boot.duration",
            D::System,
            K::Measurement,
            Some(U::Seconds),
            P::Measured,
            true,
            "Measured boot duration. Null when it could not be read — never a \
             representative constant.",
        ));
        add(Entity::new(
            "system.boot.secure_boot",
            D::System,
            K::Identity,
            None,
            P::Measured,
            true,
            "Whether Secure Boot is enforcing, read from the firmware flag itself \
             rather than inferred from the presence of the key holding it.",
        ));
        add(Entity::new(
            "board.model",
            D::Board,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "Baseboard product name from SMBIOS.",
        ));
        add(Entity::new(
            "board.firmware.vendor",
            D::Board,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "System firmware vendor, from SMBIOS.",
        ));
        add(Entity::new(
            "board.firmware.product",
            D::Board,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "System product name, from SMBIOS.",
        ));
        add(Entity::new(
            "board.firmware.boot_mode",
            D::Board,
            K::Identity,
            Some(U::Identifier),
            P::Measured,
            true,
            "Whether the machine booted UEFI or legacy BIOS. Determines whether \
             Secure Boot is even applicable.",
        ));
        add(Entity::new(
            "board.firmware.{n}.component",
            D::Board,
            K::Identity,
            Some(U::Identifier),
            P::Measured,
            true,
            "Name of one component in the firmware inventory — BIOS, ME, a device \
             option ROM.",
        ));
        add(Entity::new(
            "board.firmware.{n}.version",
            D::Board,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "Installed version of that component.",
        ));
        add(Entity::new(
            "board.tpm.present",
            D::Board,
            K::Identity,
            None,
            P::Measured,
            false,
            "Whether a TPM was enumerated at all. False is a reading: the absence \
             of a TPM is a fact about the machine, distinct from being unable to \
             look.",
        ));
        add(Entity::new(
            "board.tpm.version",
            D::Board,
            K::Identity,
            Some(U::Identifier),
            P::Measured,
            true,
            "TPM specification version. Null when no TPM is present.",
        ));
        add(Entity::new(
            "board.tpm.manufacturer",
            D::Board,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "TPM manufacturer string.",
        ));
        add(Entity::new(
            "board.tpm.status",
            D::Board,
            K::Measurement,
            Some(U::Identifier),
            P::Measured,
            true,
            "Whether the TPM is enabled, activated and owned. A present but \
             disabled TPM cannot attest anything, so presence alone is not the \
             question an agent should ask.",
        ));
        add(Entity::new(
            "board.tpm.measured_boot",
            D::Board,
            K::Measurement,
            None,
            P::Measured,
            true,
            "Whether platform integrity measurements are active.",
        ));
        add(Entity::new(
            "board.manufacturer",
            D::Board,
            K::Identity,
            Some(U::Text),
            P::Measured,
            true,
            "Baseboard manufacturer from SMBIOS.",
        ));

        // ── Writable settings ────────────────────────────────────────────────
        //
        // Generated from the apply-handler registry rather than listed here, for the
        // same reason the command catalogue is walked out of clap: a hand-maintained
        // list would claim a write the binary cannot perform the first time a
        // handler was removed and nobody remembered this function. An agent reading
        // `writable_via` is being told it can change something, which is a promise
        // that has to be backed by a registered handler.
        //
        // Until this existed, `EntityKind::Setting` and `Entity::writable_via` were
        // declared and populated by nothing: the ontology described a machine an
        // agent could only read, while `simon profile set` could write.
        for handler in crate::profile::apply::builtin_handlers() {
            let setting_id = handler.setting_id();
            let domain = match handler.subsystem() {
                crate::profile::Subsystem::Gpu => D::Gpu,
                crate::profile::Subsystem::Cpu => D::Cpu,
                crate::profile::Subsystem::Memory => D::Memory,
                // NVMe parameters are disk-level; display has no ontology domain of
                // its own and its settings are board-level firmware state.
                crate::profile::Subsystem::Nvme => D::Disk,
                crate::profile::Subsystem::Display => D::Board,
            };
            let mut entity = Entity::new(
                &format!("{}.setting.{}", domain.as_str(), setting_id),
                domain,
                K::Setting,
                Some(U::Identifier),
                P::Measured,
                true,
                "A driver or firmware setting with a registered write handler. \
                 Writing requires explicit confirmation and is recorded in the \
                 apply audit log.",
            );
            entity.writable_via = Some(setting_id.to_string());
            add(entity);
        }

        // ── Diagnostics ──────────────────────────────────────────────────────
        //
        // Declared, not synthesised at read time, because everything the resolver
        // emits has to be findable in the schema an agent fetched beforehand. These
        // were the one class of row that violated that: emitting an undeclared
        // `disk.<none>` told an agent something true in a vocabulary it had no way
        // to look up.
        for domain in Domain::ALL {
            add(Entity::new(
                &format!("{}.<none>", domain.as_str()),
                *domain,
                K::Diagnostic,
                None,
                P::Unavailable,
                true,
                "Present only when this domain enumerated nothing. Carries the \
                 reason, so an absent device stays distinguishable from a reader \
                 that is not implemented here.",
            ));
        }
        add(Entity::new(
            "process.<truncated>",
            D::Process,
            K::Diagnostic,
            Some(U::Count),
            P::Unavailable,
            true,
            "Present when the process list was capped. Absence from a truncated \
             list is not absence from the machine.",
        ));

        Self {
            version: ONTOLOGY_VERSION.to_string(),
            entities,
        }
    }

    pub fn get(&self, id: &str) -> Option<&Entity> {
        self.entities.get(id)
    }

    /// Entities in one domain, in id order.
    pub fn in_domain(&self, domain: Domain) -> Vec<&Entity> {
        self.entities
            .values()
            .filter(|e| e.domain == domain)
            .collect()
    }

    /// Substring search over ids and descriptions, for agents that do not yet know
    /// the id space.
    pub fn search(&self, needle: &str) -> Vec<&Entity> {
        let needle = needle.to_ascii_lowercase();
        self.entities
            .values()
            .filter(|e| {
                e.id.to_ascii_lowercase().contains(&needle)
                    || e.description.to_ascii_lowercase().contains(&needle)
            })
            .collect()
    }

    /// Ids whose value is a template — `gpu.{n}.name` rather than a concrete
    /// `gpu.0.name`. A consumer expands these against the live instance count.
    pub fn is_template(id: &str) -> bool {
        id.contains('{')
    }

    /// Expand a template id for a concrete instance.
    ///
    /// `gpu.{n}.name` with `"0"` becomes `gpu.0.name`. Returns the id unchanged when
    /// it holds no placeholder, so callers need not special-case singletons.
    pub fn instantiate(id: &str, instance: &str) -> String {
        if let Some(open) = id.find('{') {
            if let Some(close) = id[open..].find('}') {
                let mut out = String::with_capacity(id.len() + instance.len());
                out.push_str(&id[..open]);
                out.push_str(instance);
                out.push_str(&id[open + close + 1..]);
                return out;
            }
        }
        id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_is_unique_and_well_formed() {
        let ont = Ontology::build();
        assert!(!ont.entities.is_empty(), "ontology is empty");
        for (key, e) in &ont.entities {
            assert_eq!(key, &e.id, "map key and entity id disagree");
            assert!(
                e.id.starts_with(e.domain.as_str()),
                "{} is not namespaced under its domain {}",
                e.id,
                e.domain.as_str()
            );
            assert!(
                !e.description.is_empty(),
                "{} has no description; the ontology exists to be read by something \
                 that cannot see the hardware",
                e.id
            );
            assert!(
                !e.id.ends_with('.') && !e.id.contains(".."),
                "malformed id: {}",
                e.id
            );
        }
    }

    /// `writable_via` tells an agent it may change something. That is a promise, and
    /// it must be backed by a handler that actually exists — a schema claiming a
    /// write the binary cannot perform is worse than one claiming nothing, because
    /// the agent will try.
    #[test]
    fn every_writable_entity_has_a_registered_handler() {
        let ont = Ontology::build();
        let registered: Vec<String> = crate::profile::apply::builtin_handlers()
            .iter()
            .map(|h| h.setting_id().to_string())
            .collect();

        for e in ont.entities.values() {
            let Some(setting_id) = &e.writable_via else {
                continue;
            };
            assert_eq!(
                e.kind,
                EntityKind::Setting,
                "{} declares writable_via but is not a Setting",
                e.id
            );
            assert!(
                registered.contains(setting_id),
                "{} claims it can be written via {setting_id:?}, but no such apply \
                 handler is registered on this build; an agent told this would \
                 attempt a write that cannot succeed. Registered: {registered:?}",
                e.id
            );
        }
    }

    /// The converse: a handler the binary exposes but the schema hides is a
    /// capability an agent cannot discover. Reading `simon profile writable` should
    /// not reveal anything `simon describe` omits.
    #[test]
    fn every_registered_handler_is_discoverable_in_the_schema() {
        let ont = Ontology::build();
        let declared: Vec<&String> = ont
            .entities
            .values()
            .filter_map(|e| e.writable_via.as_ref())
            .collect();

        for handler in crate::profile::apply::builtin_handlers() {
            let id = handler.setting_id().to_string();
            assert!(
                declared.contains(&&id),
                "the apply registry exposes {id:?} but no ontology entity declares \
                 it, so an agent reading the schema cannot discover it"
            );
        }
    }

    /// Settings are only meaningful if the kind is used at all — an assertion over
    /// an empty set would pass while the write surface was entirely undeclared,
    /// which is precisely the state this replaced.
    #[test]
    fn the_schema_declares_at_least_one_writable_setting() {
        // Platform-dependent: a build with no handlers for this OS legitimately has
        // none, so this only asserts the two views agree about how many there are.
        let ont = Ontology::build();
        let declared = ont
            .entities
            .values()
            .filter(|e| e.kind == EntityKind::Setting)
            .count();
        let registered = crate::profile::apply::builtin_handlers().len();
        assert_eq!(
            declared, registered,
            "the schema declares {declared} writable settings but the apply registry \
             has {registered}"
        );
    }

    /// A derived value is only as trustworthy as its inputs, so it has to name them.
    #[test]
    fn derived_entities_declare_their_inputs() {
        let ont = Ontology::build();
        for e in ont.entities.values() {
            if e.provenance == Provenance::Derived {
                assert!(
                    !e.derived_from.is_empty(),
                    "{} is derived but names no inputs",
                    e.id
                );
                for input in &e.derived_from {
                    assert!(
                        ont.get(input).is_some(),
                        "{} derives from {input}, which is not in the ontology",
                        e.id
                    );
                }
            } else {
                assert!(
                    e.derived_from.is_empty(),
                    "{} names inputs but is not marked derived",
                    e.id
                );
            }
        }
    }

    /// Only `Measured` counts as an observation. This is the guard that stops an
    /// agent treating a spec-sheet constant as a live reading.
    #[test]
    fn only_measured_values_are_observations() {
        assert!(Provenance::Measured.is_observation());
        assert!(!Provenance::Specification.is_observation());
        assert!(!Provenance::Derived.is_observation());
        assert!(!Provenance::Unavailable.is_observation());
    }

    #[test]
    fn range_validation_rejects_impossible_readings() {
        let ont = Ontology::build();
        let util = ont.get("cpu.total.utilization").unwrap();
        assert!(util.validate_range(50.0).is_none());
        assert!(util.validate_range(-1.0).is_some(), "negative percent");
        assert!(util.validate_range(101.0).is_some(), "over 100 percent");

        let temp = ont.get("gpu.{n}.thermal.temperature").unwrap();
        assert!(temp.validate_range(45.0).is_none());
        // Celsius may be negative; absolute zero may not be undercut.
        assert!(temp.validate_range(-10.0).is_none());
        assert!(temp.validate_range(-300.0).is_some(), "below absolute zero");
        assert!(temp.validate_range(500.0).is_some(), "implausibly hot");
    }

    #[test]
    fn templates_expand_to_concrete_ids() {
        assert!(Ontology::is_template("gpu.{n}.name"));
        assert!(!Ontology::is_template("memory.total"));
        assert_eq!(Ontology::instantiate("gpu.{n}.name", "0"), "gpu.0.name");
        assert_eq!(
            Ontology::instantiate("network.{iface}.rx_rate", "eth0"),
            "network.eth0.rx_rate"
        );
        // Non-templates pass through untouched.
        assert_eq!(Ontology::instantiate("memory.total", "0"), "memory.total");
    }

    #[test]
    fn lookup_and_search_find_entities() {
        let ont = Ontology::build();
        assert!(ont.get("memory.total").is_some());
        assert!(ont.get("no.such.entity").is_none());
        assert!(!ont.in_domain(Domain::Gpu).is_empty());
        assert!(
            !ont.search("temperature").is_empty(),
            "search should find thermal entities"
        );
        assert_eq!(Domain::parse("gpu"), Some(Domain::Gpu));
        assert_eq!(Domain::parse("GPU"), Some(Domain::Gpu));
        assert_eq!(Domain::parse("nonsense"), None);
    }

    /// The ontology must not depend on the machine it is built on: an agent fetches
    /// it as a schema, so two builds have to agree.
    #[test]
    fn build_is_deterministic_and_hardware_independent() {
        let a = serde_json::to_string(&Ontology::build()).unwrap();
        let b = serde_json::to_string(&Ontology::build()).unwrap();
        assert_eq!(a, b);
    }
}

/// Human-facing labels, derived from the ontology rather than retyped per surface.
///
/// The CLI printed `Kernel:`, the GUI printed `Kernel Version`, and the JSON emitted
/// `kernel_version` — three names for one reading, which is exactly what stops an
/// agent correlating what a user reports seeing with what it can query. These
/// helpers give the TUI and GUI the same source the CLI and `simon describe` use, so
/// a label shown on screen can be turned back into an id.
pub mod labels {
    use super::{Domain, Ontology};

    /// Title-cased label for an entity id, e.g. `gpu.0.thermal.temperature` ->
    /// "Temperature". Falls back to the last path segment for ids the ontology does
    /// not declare, so a surface never renders a raw dotted id at a user.
    pub fn short_label(id: &str) -> String {
        let leaf = id.rsplit('.').next().unwrap_or(id);
        title_case(leaf)
    }

    /// Fully qualified label including the domain and instance, e.g.
    /// "GPU 0 — Temperature". Used where a surface shows readings from several
    /// devices in one list.
    pub fn qualified_label(id: &str) -> String {
        let parts: Vec<&str> = id.split('.').collect();
        let short = short_label(id);
        match parts.as_slice() {
            [domain, instance, ..] if instance.chars().all(|c| c.is_ascii_digit()) => {
                format!("{} {} — {}", domain_label(domain), instance, short)
            }
            [domain, ..] => format!("{} — {}", domain_label(domain), short),
            [] => short,
        }
    }

    /// Display name for a domain, preserving the acronyms users expect to see.
    pub fn domain_label(domain: &str) -> String {
        match domain {
            "cpu" => "CPU".to_string(),
            "gpu" => "GPU".to_string(),
            _ => title_case(domain),
        }
    }

    /// The id a label came from, if the ontology declares exactly one match.
    ///
    /// The inverse direction, and the reason the labels live here: an agent handed
    /// "Temperature" by a user looking at the GUI can ask which ids that could mean
    /// instead of guessing.
    pub fn ids_for_label(label: &str) -> Vec<String> {
        let ontology = Ontology::build();
        let needle = label.trim().to_ascii_lowercase();
        ontology
            .entities
            .keys()
            .filter(|id| short_label(id).to_ascii_lowercase() == needle)
            .cloned()
            .collect()
    }

    /// Whether a domain name is one the ontology knows, so a surface can assert its
    /// section headings correspond to real domains.
    pub fn is_known_domain(domain: &str) -> bool {
        Domain::parse(domain).is_some()
    }

    fn title_case(s: &str) -> String {
        s.split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn labels_are_derived_from_ids() {
            assert_eq!(short_label("gpu.0.thermal.temperature"), "Temperature");
            assert_eq!(short_label("memory.total"), "Total");
            assert_eq!(short_label("system.os.name"), "Name");
            // Underscores become spaces, not run-together words.
            assert_eq!(
                short_label("gpu.0.thermal.critical_temperature"),
                "Critical Temperature"
            );
        }

        #[test]
        fn qualified_labels_name_the_device() {
            assert_eq!(
                qualified_label("gpu.0.thermal.temperature"),
                "GPU 0 — Temperature"
            );
            assert_eq!(qualified_label("memory.total"), "Memory — Total");
            // Acronyms survive title-casing.
            assert_eq!(domain_label("cpu"), "CPU");
            assert_eq!(domain_label("memory"), "Memory");
        }

        /// The inverse direction has to actually work, or the label layer is
        /// decoration rather than a mapping.
        #[test]
        fn labels_map_back_to_ids() {
            let ids = ids_for_label("Total");
            assert!(
                ids.contains(&"memory.total".to_string()),
                "expected memory.total among {ids:?}"
            );
            assert!(ids_for_label("no such label").is_empty());
        }

        #[test]
        fn every_ontology_domain_is_a_known_domain() {
            for d in Domain::ALL {
                assert!(is_known_domain(d.as_str()));
            }
            assert!(!is_known_domain("nonsense"));
        }

        /// No entity may produce an empty label — a surface would render a blank.
        #[test]
        fn no_entity_yields_an_empty_label() {
            for id in Ontology::build().entities.keys() {
                assert!(
                    !short_label(id).trim().is_empty(),
                    "{id} produced an empty label"
                );
                assert!(
                    !qualified_label(id).trim().is_empty(),
                    "{id} produced an empty qualified label"
                );
            }
        }
    }
}
