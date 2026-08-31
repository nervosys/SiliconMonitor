# Status

Current as of 6.0.0. This file is excluded from the published crate
(`Cargo.toml`'s `exclude` list) and is for whoever picks the work up next.

## Read this first: 6.0.0 is shipped

Merged to `master` as a fast-forward and tagged **`v6.0.0`** at `1947426`, with
**CI green on all three platforms**. Not published to crates.io — declined for
5.2.0 and 6.0.0, tag only. `master` and the tag point at the same commit.

| Commit | What |
|---|---|
| `9452a01` | Eleven clusters, five honesty defects, five nullability fixes |
| `2e6f9e9` | The `CACHE_STATS` test race |
| `b2dd07b` | `gpu.codec` declared `Measured` for rows resolving `Specification` |
| `da769bf` | A clippy 1.98 lint that would have failed CI |
| `b7bc21d` | Two more non-nullable entities a VM resolves absent |
| `099c604` | The provenance guardrail: stronger than declared is fine, weaker is not |
| `2687dbb` | `memory.bandwidth` published defaults; `services` became a summary |
| `ebab956` | Nine readers said "none" where they meant "cannot look" |
| `1947426` | MSRV raised to 1.89, true of every feature combination |
| `014e4dd` | Kernel parameters, split from this crate's opinions about them |
| `24a7314` | The macOS CPU path in the CLI backend invented almost everything |
| `7b606fe` | A GPU-holding Python process was reported as PyTorch |
| `1cf1980` | The agent-facing memory tool invented figures on macOS |

Since the tag, on `master` and green on all three platforms:

| Commit | What |
|---|---|
| `afb377d` | Secure Boot was readable all along, unelevated |
| `2f7be76` | Boot duration bound; 32 descriptions printed collapsed indentation |
| `8961a1c` | The agent surface reported an uptime of zero and a load average |
| `4dd5cce` | A test for the agent tool surface, which had none |
| `a584dd0` | Five wrong conclusions in `hardware_ai`, each with a confidence attached |
| `7607401` | A 24GB card read as 4GB, because `AdapterRAM` is 32 bits |
| `00ffef8` | The method, written down, and what the one-machine caveat costs |
| `6617020` | The nominal clock reported as the current one, on every core |
| `ecd1281` | Two more constants wearing a measurement's provenance |
| `7192187` | Four NVMe SMART fields read sixteen bytes early |
| `4846c3f` | An interlaced mode reported field lines as a resolution |
| `1e26d01` | The SATA temperature walk used the input struct's layout |
| `4af44bc` | The three scans, in the order they pay |
| `7c79f8b` | A DIMM at zero volts, and three absence words as values |
| `3a50c99` | `pstate_name` gated with its caller, after CI caught it |
| `2360068` | Watching nothing is not the same as having no baseline |
| `5931069` | The recorder stored an absent sensor as zero degrees |
| `1666a52` | A shell variable read as a debugger, and a consent UI for nothing |
| `f9e7248` | `serve --help` named a Prometheus route that 404s |
| `425ff4a` | 24 threads reported as 24 cores, one line under a 12-core name |
| `1284391` | `cli temperature` found no sensors while snapshot read five |
| `dcbd50b` | A display's mode had nowhere to be absent; five consumers printed 0x0 |
| `c14e8c6` | A gamepad counted as a Bluetooth radio, GATT services as devices |
| `9ef5ba0` | The PnP classifier gated with the only target that calls it |
| `HEAD` | Root hubs given the identifier 0000:0000, and an invented Intel hub |

**None of these were found by grepping.** The method, and why the greps missed
them, is below under *Run it and read the output*.

### An identifier is a bad place for a sentinel

`simon cli usb` printed **`[0000:0000]`** for four of forty-one entries — every
root hub and a "USB4 Virtual power coordination device". `Get-PnpDevice -Class
USB` shows why: a root hub's id is `USB\ROOT_HUB30\9&EBA7CE&0&0`, with no
`VID_` or `PID_` in it at all. `parse_vid_pid` returned `(0, 0)` for that, and
`0000:0000` is a value a device could legitimately report, so nothing downstream
could tell the two apart.

**One of the two callers already guarded `if vid != 0 || pid != 0`**, and the
TUI pane guarded it again before printing. So the sentinel was known to be
meaningless in two places out of four, and the CLI and the agent surface — which
published `"vendor_id": "0000"` — did not. Third time this sweep that one
consumer knew and another did not.

The zero also had to be *re-guessed* by the ontology resolver, which published
the identifier `"0000"` as `measured`. It now says
*"this entry carries no USB vendor id; root hubs and virtual devices have none"*.

The regression test keeps the distinction explicit, including the case that
makes it matter: `VID_0000&PID_0000` parses to `Some(0)`, not `None`. **A device
reporting zero and a device reporting nothing are different facts**, and a
sentinel in an identifier field is precisely the shape that cannot express both.

**And a second defect in the same file, worse than the first.** When the Linux
sysfs walk found nothing, the reader invented a device:

```rust
// Fallback
if self.devices.is_empty() {
    self.devices.push(UsbDevice {
        vendor_id: 0x8086, product_id: 0x0001,
        product: Some("USB Root Hub".to_string()),
        speed: UsbSpeed::High, ...
```

An Intel root hub, at high speed, read from nothing. A machine whose USB tree
could not be enumerated reported one device that does not exist, and no caller
could distinguish that from a machine with exactly one hub. Deleted; an empty
list is the honest answer. This is the same shape as the synthetic placeholder
displays `tests/plausibility.rs` already guards against — worth knowing that
family has another member, in a module that suite does not cover.

### A gamepad counted as a radio, because its name contains "Controller"

`simon cli bluetooth` reported **"Adapters: 2, Devices: 9"** on a machine with
one radio and two paired peripherals. `Get-PnpDevice -Class Bluetooth` settles it
in one command, which is how it was checked.

The classification was a regex over the *display name*, inside a PowerShell
string:

```powershell
Where-Object { $_.PNPClass -eq 'Bluetooth' -and $_.Name -match 'Radio|Adapter|Controller' }
```

An **Xbox Wireless Controller** matches `Controller`, so a gamepad was reported
as one of the machine's Bluetooth radios — and the device query excluded the
same three words, so it then vanished from the device list. The device filter's
`-notmatch` also admitted "Generic Access Profile", "Device Information
Service", "Bluetooth LE Generic Attribute Service" and "Bluetooth Device
(RFCOMM Protocol TDI)": six GATT services and a protocol driver, counted as
things the user had paired.

**The `PNPDeviceID` was in the same query object all along and separates them
without ambiguity** — `USB\` is the radio, `BTHENUM\DEV_`/`BTHLE\DEV_` is a
paired peripheral, `BTHLEDEVICE\{guid}` is a GATT service on one, `BTH\MS_` is
the stack. This is the handoff's own *guess placed in front of a reading*, with
the reading sitting in an adjacent field.

**The classifier moved from the PowerShell string into Rust**, which is the part
worth keeping. Logic inside a shell string cannot be unit-tested at all; the
same logic in Rust is tested against all eleven Bluetooth-class entries from
this desktop, verbatim, asserting one adapter and two peripherals. Ordering
matters and is now covered: `BTHLEDEVICE` also begins with `BTHLE`, so a GATT
service classifies as a peripheral if the peripheral check runs first.

### A display's mode had nowhere to be absent

`simon cli display` printed **`RX-A740 0x0 @ 0Hz Dvi`** — a Yamaha AV receiver
on HDMI, attached and named, whose current mode Windows will not report. The
ontology got this right and said so in a reason that reads, in as many words,
*"zero is not a resolution"*. Five other consumers printed the zero anyway.

`DisplayInfo` had `width: u32`, `height: u32`, `refresh_rate: f32` — the same
shape as `SystemSnapshot`'s GPU columns two commits earlier, and the same
consequence: **an absence with nowhere to go becomes a zero, and the resolver
downstream has to reconstruct it from a sentinel.** The doc comment on
`aspect_ratio` already named the diagnosis exactly — *"a return type with no way
to say nothing"* — because the sibling defect was fixed there this session. That
fix stopped at the method and left the fields.

Fixing the fields found consumers a grep did not:

- `ai_api/tools.rs` published `"resolution": "0x0"` to agents, twice.
- `tui/app.rs`, which the grep missed entirely — the compiler found it.
- `profile/display.rs` published a `0 Hz` refresh-rate row.
- `examples/display_monitor.rs` summed `0 * 0` into a total-pixels figure.

And it turned up a defect invisible on this machine: **the Linux and macOS
readers write `refresh_rate: 0.0` unconditionally**, because neither parses a
rate. Every display on both platforms reported 0 Hz as though measured.

**The zero came from the data source, not from a missing key**, which the first
attempt got wrong: `item.get("Width")` returns `Some(0)`, so making the field
`Option` left `Some(0)` and the output unchanged. Normalising is now done once
at the reader — a display is not zero pixels wide, so zero becomes `None` there
rather than at each of the five consumers.

### The command named for the reading was the one not doing it

`simon cli temperature` printed **"No temperature sensors detected"**. Seconds
later, in the same binary, `simon snapshot` read two GPU temperatures and three
NVMe drive temperatures, all `measured`. `--format json` returned
`{"sensors": {}}`.

The Windows reader behind it looks in three places — OpenHardwareMonitor's WMI
namespace, LibreHardwareMonitor's, and CIMV2 thermal-zone counters — and none is
present on a stock desktop. That part is correct and the resulting empty map is
correct. **What was wrong is the sentence.** "No temperature sensors detected"
is a claim about the machine; the truth was a claim about the reader.

It also contains, in as many words, the reason the command was incomplete:

```rust
} else {
    continue; // Skip other sensors (GPU temps come from NVML)
}
```

That comment is true of the *crate* and false of the *command*. GPU temperatures
do come from NVML — and nothing in `simon cli temperature` ever asked NVML.

`simon cli power` was the same defect exactly: `PowerStats.rails` is hwmon,
which Windows does not expose, so it printed "No power rails exposed by this
platform" while the ontology held both GPUs' draw and limit, the battery
percentage and the three power profiles. Someone had already fixed the
neighbouring half of it — there is a comment explaining why "Total Power: 0.00W"
was removed — and stopped at the rails.

Both now read `ontology::resolve::snapshot()`, which is the fifth and sixth
consumer to adopt a reader that was already right. **The selection lives in
`fetch::{temperatures,power}` rather than in `main.rs`, so it can be tested**,
and `temperatures` keys on the declared unit rather than the id shape, so a
temperature entity added anywhere is shown without touching it.

What the fixed command prints is the argument for the whole approach: ten
readable temperatures, and six unreadable ones each with its own reason —
including the two that explain the original symptom truthfully, *"on Windows
most board sensors require a signed kernel driver"*. The old code had that fact
and threw it away.

**The regression test asserts the selection spans more than one subsystem
prefix.** Narrowing it back to `thermal.*` fails with
`every temperature came from one subsystem ({"thermal"})`, which is the defect
verbatim.

**The first version of it was wrong, and CI said so on all three runners.** It
resolved `resolve::snapshot()` and I described it as hardware-independent
"because every temperature entity is declared whether or not a host can read
it". The declarations are; a *snapshot* is not. Instanced entities produce no
row at all when the instance is missing, so a runner with no GPU and no NVMe
drive resolves nothing the test was looking for. It passed on this desktop for
the same reason the defect it guards existed: the machine had the hardware.

The fix is to build the readings from `Ontology::build().entities` — one
`Reading` per declared entity, unit carried, value `None` — and run the
predicate over that. The test now takes 0.00s instead of 37s, because it never
touches hardware, and it means what I claimed the first version meant.

### CI caught a test I wrote, on prose I added after the gate

`documented_http_paths_match_the_route_table` failed on all three runners at
`708ae50`, on **HANDOFF.md** — this file, which quotes the wrong Prometheus path
three times while explaining that it is wrong. The test cannot tell a
counter-example from a recommendation.

The exemption is easy (this file is excluded from the published crate and
instructs nobody, so a mention here is a citation). **The process failure is the
part worth keeping.** The order was: fix, add test, confirm it fails on the old
text, restore, run the gate green, *then write the handoff entry*, then push.
The entry introduced the failure and was never tested, because the gate had
already been run.

**This file is an input to the test suite now** — `documentation_links` and
`source_hygiene` both read it. Editing it after the gate invalidates the gate,
in exactly the way that editing any source file would. Run the tests last, not
the prose.

That is the second time this pattern has appeared in three commits: at `7c79f8b`
a local gate that could not see feature-gated code, and here a local gate run
before the last edit. Both times the gate was correct and the sequencing was
not.

### The contradiction was on screen, two lines apart

`simon status` printed:

```
CPU        AMD Ryzen 9 9900X 12-Core Processor
Cores      24
```

`fetch::summary` labelled `cpu.cores.logical` as "Cores". The reading was right
and the label was wrong — `Win32_Processor` confirms 12 cores, 24 logical
processors — and **both counts are already measured entities**, so the fix was
to name each one rather than to read anything new. It now prints `Cores 12` and
`Threads 24`.

**This is the cheapest defect in the whole file to have found, and it survived
six major versions.** Nothing about it needed hardware knowledge or a second
tool: the name of the part and the wrong count were two lines apart, in the
output of the command most likely to be run first. It is the strongest argument
for scan #1 there is — grep cannot see it, types cannot see it, and no test that
asserts a summary *is produced* can see it.

Alongside it, `as_bytes` divided by 1024 and labelled the result GB, so 100.5 GB
of installed memory printed as "93.6 GB". 93.6 is the correct number and GB is
the wrong name for it — that quantity is 93.6 GiB. A crate that carries QUDT
units through its ontology should not lose them in the line most people read.
Now GiB.

**Deliberately not fixed, and worth stating plainly rather than leaving
implied:** the same 1024-with-decimal-labels convention runs through five other
byte formatters (`bin/main.rs::format_bytes_short`, `gui/app.rs::format_bytes`,
`memory_management::{format_bytes,format_size}`, `tsdb::format_size`) and a long
tail of internal constants and test fixtures. Only `fetch::as_bytes` reaches
`simon status`, which is the surface that was read; the rest is a crate-wide
cosmetic sweep touching the GUI, which this handoff has repeatedly deprioritised.
It is a real inconsistency and it is still there.

### A constant read without its prefix, documented as a route

`simon serve --help` said: *"Prometheus metrics are at /metrics/prometheus (not
/metrics — that route returns JSON)."* Both halves are false. `curl` returns 404
for `/metrics/prometheus` and 404 for `/metrics`; the served path is
`/api/v1/metrics/prometheus`, which the server's own startup banner and
`grafana/README.md` both state correctly.

The mechanism will recur, so it is worth naming: `routes::METRICS_PROMETHEUS` is
the string `"/metrics/prometheus"`, and the dispatcher compares it only *after*
stripping the `/api/v1` prefix. Reading the constant and documenting it verbatim
yields a path that does not exist, and nothing about the constant warns you.

**A help string is the one claim a user cannot check before acting on it.** They
run what it names and get nothing back. `documented_http_paths_match_the_route_
table` in `tests/documentation_links.rs` now composes the path from the route
constants and fails on any mention that drops the prefix; it was confirmed
against the old text first.

Two things looked wrong here and are not, which is worth recording so the next
reader does not spend the time twice:

- `simon_gpu_{0,1}_fan_speed_percent 0` at 49 °C and 37 °C is a **true zero** —
  `nvidia-smi --query-gpu=fan.speed` reports `0 %` for both. Zero-RPM idle on a
  3090 Ti. Checked in a second tool rather than assumed, per the rule below.
- The Prometheus exporter omits `simon_gpu_2_temperature_celsius` entirely
  rather than publishing a zero, which is correct for Prometheus and is the
  behaviour the recorder lacked.

**Open, not fixed:** the OpenAPI document at `/api/v1/openapi.json` advertises
three paths (`/health`, `/api/v1/context`, `/api/v1/gpus`) out of roughly twenty
that the dispatcher serves. Everything it advertises works, so it states nothing
false — but an agent handed that spec would conclude simon serves three
endpoints. Four route constants (`METRICS`, `STREAM`, `EVENTS_SUBSCRIBE`,
`PROCESS_BY_ID`) are declared and never dispatched; they are unreferenced dead
constants rather than advertised lies, which is why they were left alone.

### A privacy screen is a reading too, and this one was wrong three ways

`simon privacy` was the last unread surface. It has no hardware in it, which is
why it went unexamined for so long, and it turned out to hold the most confident
false statements in the crate.

**There is no telemetry in simon.** No endpoint, no collector, no serialiser.
`ConsentScope` is referenced only by the `privacy` subcommand that displays it;
`should_collect_data` has no callers at all outside its own module; every
outbound `.post` in the crate goes to an LLM backend the user configured. In
front of that emptiness sat five categories, twenty itemised data points
("Crash stack traces (anonymized)", "Frame time distributions"), an opt-in flow,
and a persisted config with timestamps — and `simon privacy opt-in` answered
"All data collection categories have been enabled."

**The direction of a false claim does not decide how serious it is.** Every
defect before this one overstated what simon knew; this one understated what it
did, which reads as reassuring and is therefore harder to doubt. It is still a
confident wrong answer about the program's own behaviour, in the one area a
user cannot check for themselves, and it degrades: the disclosure is already
written and already consented to on the day someone adds a collector.

Three defects, not one:

1. **The apparatus does not exist.** Fixed by saying so on every screen, and by
   `ConsentScope::is_collected`, which returns `false` for all five and is the
   single place to change. `should_collect_data` consults it, so consent to a
   category nothing implements cannot report that it should collect.
2. **`has_consent` defaulted to granted**, under a comment reading "opt-in by
   default" and a module header listing "Opt-in by default" as a principle. A
   default of granted is opt-out; the two cannot both be true. Nothing consumed
   the answer, so it was inert — and it would have started collecting from
   everyone who never answered on the day it stopped being inert. Now `false`.
3. **The module claimed compliance with GDPR and CCPA.** A library is not in a
   position to make that claim about a deployment, and default-granted consent
   is the specific thing the first of those forbids. Removed rather than
   softened.

The global `--no-telemetry` and `--offline` help lines said simon did "telemetry,
analytics, or crash reports" on **every subcommand's `--help`**, which is the
widest-reach false statement the crate had. `--offline`'s one real effect is
worth stating accurately: `agent::refuse_if_offline_and_offhost` declines a
backend that would relay the question to a vendor. That check is good, and its
doc comment records that the flag previously enforced nothing at all.

### `$_` is not a debugger, and it cost the privacy screen its credibility

Pulling the thread on why `opt-in` said "enabled" and `status` said "DENIED" on
the next line landed on `sandbox.rs`:

```rust
let debugger_vars = ["_", "LLDB_DEBUGSERVER_PATH", "GDB", "PYTHONBREAKPOINT"];
```

`$_` is set by **every POSIX shell** to the last argument of the previous
command. So every run started from bash, zsh or sh set `is_debugged`, therefore
`is_sandboxed()`, therefore `has_consent() == false` for every scope — silently,
because the display folded three unrelated causes into one word. `simon
privacy status` was reporting a shell variable as the user's privacy choice.
`cargo run --example sandbox_demo` printed **"Sandboxed: Debugger Attached"**,
"Protection Status: ACTIVE" and "Network Transmission: BLOCKED" on a bare-metal
desktop.

The other three names in that list say nothing about *this* process either: gdb
does not export `GDB`, and `PYTHONBREAKPOINT` configures a Python hook in a Rust
binary. The two real checks — `TracerPid` on Linux, `IsDebuggerPresent` on
Windows — were already there and already correct, sitting directly above. **The
heuristic was pure downside: it could only add false positives to checks that
did not need help.**

Two more in the same file, both confirmed rather than reasoned:

- The Windows Hyper-V check read `C:\Windows\System32\drivers\vmbus.sys` and
  set `is_vm` if the contents were non-empty. The file **ships with every modern
  Windows install**, host or guest, so its presence proves nothing — and it is a
  binary driver, so `read_to_string` fails on invalid UTF-8 and the check
  **never ran at all**. Both checked here: present, 210 KB, invalid UTF-8 at
  byte 2. It now calls `virtualization::detect`, which 3.10.0 already built
  correctly out of CPUID leaf 0x40000003 — *a second consumer that never
  adopted the good reader, found four sessions after the pattern was named.*
- Linux `sys_vendor` matched `"Microsoft Corporation"`, which is the sys_vendor
  of every Surface. Bare metal reported as a VM.

**The regression test was confirmed against the old code before being believed**,
per the rule below, and failed with exactly the right message:
`reported a debugger with none attached; indicators: ["Debugger env var _ detected"]`.

### The recurring string mangling, finally diagnosed and now caught

Three sessions have lost `\` line continuations inside Rust string literals,
leaving runs of source indentation embedded in user-facing text, and the handoff
has recorded it twice as unexplained. The mechanism is now clear enough to state:
**a `\` at the end of a line does not survive being written through a shell
heredoc**, so a literal authored that way arrives collapsed. It still compiles,
still lints clean, and every assertion about it still passes, because all the
words are present.

`tests/source_hygiene.rs` now fails on the signature — six or more spaces
between two lowercase letters, inside a line containing a quote, outside a
comment. It found **twelve** instances on its first run, five of them already
committed, including one shipped in `4846c3f` in an EDID setting description.

The fix is always `concat!("first part ", "second part")`, which has no
continuation to lose. That was already the crate's convention; this is why it
has to be.

### The reader was honest; the consumer threw it away

`gpu.2.thermal.temperature` resolves **"unavailable — no temperature sensor
exposed for this adapter"** in `simon snapshot`, and the Prometheus exporter
omits the series entirely, which is correct for Prometheus. The same
`Option<u32>`, on the same tick, was written into the time-series database as
`0.0` and printed by `simon record query` as `Temp: 0°C` — an integrated AMD
adapter reading as the coolest device in a box holding two 3090 Tis at 44 and
38 degrees.

**Three consumers of one already-honest reader; two respected the `Option` and
the third discarded it at the type boundary.** This is the "reader wired into
one consumer" pattern further down, in its most treatable form: nothing needed
to be measured differently, only carried. `SystemSnapshot`'s four per-GPU
columns were `Vec<f32>`/`Vec<u64>`, so there was nowhere for an absence to go,
and `unwrap_or(0)` was the only way to satisfy the type.

**A `Vec<T>` whose elements come from `Option<T>` readings is the shape to look
for.** The lossy conversion is forced by the declaration, so it never looks like
a decision anyone made. Grepping `unwrap_or(0)` does find it, among 884 others;
what singles this one out is that the value crosses into **storage**, where it
is exported to JSON, averaged, and read back later by someone who cannot ask
what the zero meant.

Worse than the sensor: a device whose query *failed on that tick* was recorded
as `0%` utilisation, `0` bytes, `0°C`, `0 mW` — a complete, plausible reading of
an idle, cold, unpowered GPU. The comment directly above that code said
"carries None for a device whose query failed this tick", and the next four
lines turned it into zeros. Four per-process columns did the same, with comments
admitting they were never tracked at all.

The fix makes the columns `Vec<Option<_>>` and bumps `DB_VERSION` to 2.
**Version 1 files are now rejected rather than migrated**, and the reason says
why: a stored zero does not record whether it was measured, so any conversion
would have to guess, reintroducing exactly what was removed. The rejection names
`simon record clear` as the recovery, and that command was checked against a
real version-1 file — it deletes without opening.

### Three scans, in the order they pay

Each found things the one before it could not, and all three are cheap:

1. **Read the output of anything that produces conclusions.** `hardware_ai`
   asserted "No SSD detected" at 0.85 confidence on a machine with three NVMe
   drives, and classified a tower as a GamingLaptop.
2. **Group readings by entity family and flag a family whose instances are all
   identical.** That is what a broken per-device reader looks like from outside.
   It found `cpu.core.{n}.frequency` reading 4400 on all 24 cores,
   `disk.controller.{n}.ports` hardcoded to 1, and
   `system.printer.{n}.color` false for every printer.
3. **Take two snapshots seconds apart and list the numbers that did not move.**
   Most are legitimately static; the interesting ones are the volatile-sounding
   survivors. That found the NVMe SMART offsets — a drive reporting 2189 power
   cycles against 43 power-on hours, a cycle every seventy seconds.

Then, once a parser is implicated, **audit the crate's other offset-based
parsers**, because the mistake is a habit rather than an accident:
`grep -rln "from_le_bytes|\[off" src/`. `nvme_log.rs` was sixteen bytes out,
`hwmon/smart.rs` used the ioctl *input* layout to read the *output*, and
`edid.rs` never read the interlace flag. `disk/ata_smart.rs` is the one that
still cannot be checked here.

**A parse test whose fixture comes from the parser is a tautology.** It is what
let four NVMe fields ship wrong under a test named
`health_log_reads_every_field_at_its_specified_offset`. Write the fixture from
the spec document, or from bytes off real hardware; give every field a distinct
non-zero value so a shift cannot pass; and **confirm the test fails against the
old code before believing it**. Both parser fixes this session were checked that
way, and both did fail first.

**Eighteen defects came out of verifying a batch that arrived type-checked,
linted and looking finished.** Two were caught by tests that already existed.
The rest came from running the gate, reading snapshot output, CI, one new test,
and a grep over comments admitting the code could not determine something. That
ratio is the argument for every rule in this file.

**Running the gate was worth it, which is the point of the rule.** The batch was
type-checked and linted and looked finished; `cargo test --all-features` failed
on `non_nullable_entities_are_never_null` with five violations. Two causes, both
wrong declarations rather than broken readers:

- The CPUID triple was left non-nullable by the very fix that made it honest.
  Withholding family/model/stepping on Windows is correct, and the entity then
  has to say it may be absent.
- `system.printer.{n}.{connection,status}` — the Windows spooler reports
  `PrinterStatus` 1 and 2, which *are* "Other" and "Unknown", and the connection
  is classified from a port string that does not cover every local printer.

Generalising, because it will recur: **a fix that makes a reader stop lying
usually makes some entity nullable, and the declaration does not follow
automatically.** Both defects here were introduced by honesty fixes.

CI then found two more of exactly that shape — `board.input.{n}.interface` and
`memory.bandwidth.generation`, `Unknown` on both runners, fine here. Seven in
total from this batch.

**Those two are worth more than their size, because they name a blind spot this
file did not have.** The recorded lesson is that a local Windows run says
nothing about Linux or macOS. These were not that: **macOS passed**, and the two
failures were on *different operating systems*. What the runners share is that
they are **virtual machines** — a virtualised input device reports an interface
nothing classifies, and a VM's SMBIOS names no memory generation — and this
desktop is bare metal.

So the axis is hardware, not platform, and **no cross-target check can find it**:
the code compiles identically either way and only the readings differ. The two
`cargo check --target` commands above, which are the recommended cheap
substitute for other platforms, would have passed. Anything resolving off
SMBIOS, a device enumeration or a bus classification can differ this way, and
CI is the only instrument here that sees it.

That loose thread was pulled, and it ran the other way. `memory.bandwidth.
{achievable,stream_triad}` resolving while `generation` was unavailable did not
mean the declaration overstated the dependency — the derivation genuinely needs
the generation and **proceeded without it**, substituting 3200 MT/s, a 64-bit bus
and a 0.75 efficiency factor. Every VM, both CI runners included, was being
handed built-in defaults dressed as `Specification` and `Derived` readings. Fixed
in `2687dbb`.

The lesson is about reading evidence, not about memory: a derived value resolving
while its declared input is absent is *more* suspicious than it looks, not less.
The comfortable reading was that the schema was too strict. The true one was that
the code was inventing numbers.

### What is verified, and what is not

Verified on stable 1.98.0: `cargo fmt --all -- --check`,
`cargo clippy --all-features --all-targets -- -D warnings`, and
`cargo test --all-features --lib --tests --no-fail-fast` — green on every
target, 826 in the lib, `ontology_conformance` 21, `honesty` 7.

Also run and green: `cargo test --all-features --doc` (73 passed),
`cargo run --example probe_readers --all-features`, and
`simon snapshot --format text`.

That last one is the check the handoff keeps insisting on, and it paid again:
**the camera fix is now confirmed against hardware rather than against a
narrowed query.** `board.camera.{0,1}` are a Lenovo 510 IR and a Lenovo 510 RGB
— one dual-sensor Windows Hello webcam, correctly two rows — and the Brother
scanner is gone. Before the fix it was a scanner wearing a camera's name.

**CI is green on all three platforms**, on `master` at `1947426` — Format,
Clippy, Check, Feature combinations, and Test on windows, macos and ubuntu.

The MSRV is built rather than inferred: `cargo +1.89 check --all-features
--all-targets` passes, which also settles that `slice::as_chunks` in `da769bf`
exists at the declared floor.

### Run it and read the output — the only method that found any of this

Grepping for `unwrap_or` finds candidate sites. It does not tell you which of
the 884 matter, and it never once pointed at the worst defects. Every serious
finding in the `hardware_ai` and `ai_api` work came from **running the thing and
reading what it said**, then checking each claim against the machine:

- The inference report said "No SSD detected — HDD will severely bottleneck
  modern workloads" at 0.85 confidence on a host with three NVMe drives.
  `Get-PhysicalDisk` settled it in one command.
- It classified a desktop tower as a GamingLaptop.
  `(Get-CimInstance Win32_SystemEnclosure).ChassisTypes` returned 3.
- It dated a Ryzen 9 9900X to 2018.
- It reported 4GB of VRAM on a 24GB card, and recommended against ML training
  on that basis.

None of those are visible in a diff, and none would be caught by a test that
asserts a report *is produced*. They are only visible when a human-or-model
reads the sentences and asks whether each is true of this machine.

**Check ground truth in a second tool before believing the fix.** Every one of
the above was confirmed with a PowerShell query independent of the code path
being fixed, and the derived `boot_time` was checked against the kernel-start
event in the System log — two seconds apart, from sources that share nothing.
The one time this session that a plausible reading went unchecked, it was the
0.61-second boot duration, which was two timestamps of the same instant
subtracted from each other.

**A fallback is how a module fails completely and still looks like it works.**
`Get-PhysicalDisk | ConvertTo-Json` serialises MediaType and BusType as the
strings `"SSD"` and `"NVMe"`; the code read them with `as_u64()` and got `None`
for every disk on every Windows machine since the module was written.
`unwrap_or(0)` turned that total failure into a confident, plausible, wrong
answer. Nothing was ever logged, nothing ever errored, and the module kept
producing a full report.

**Watch for a guess placed in front of a reading.** Twice in one module: Windows
inferred the chassis from battery presence while Linux read the real DMI code,
and then the classifier scored a battery 0.4 against a read chassis at 0.3. The
same shape appeared in the ontology the same day, where `system.boot.secure_boot`
was bound to a reader needing Administrator while the value sat in the registry,
unelevated. When two sources disagree, check which one is *read* and which is
*inferred* before tuning weights.

### Nor a target-gated one, for a different reason

`c14e8c6` failed CI's Clippy job on Linux and macOS: `classify_pnp_entry` and
its helpers are called only from `refresh_windows`, so they are dead code
everywhere else.

**The two `cargo check --target` commands below cannot catch this, and it is
worth knowing why.** `check` reports dead code as a warning; it does not deny
it. CI runs `clippy -- -D warnings` separately on each OS. So the same code
passes every local command and fails on two of three runners.

The cheap catch is to run clippy for the target rather than check:

```bash
cargo clippy --quiet --target x86_64-unknown-linux-gnu --all-targets   --no-default-features --features cpu,npu,io,network,amd,nvidia,intel -- -D warnings
```

It cannot use `--all-features` — `ring` needs a C cross-compiler — so it reports
two pre-existing dead items that the narrower feature set excludes
(`agent::backend::body_has_array`, `tuning::CLASSIFY_SYSTEM_PROMPT`). Those are
not CI failures; only *new* names in that output are.

**Anything added beside `#[cfg(target_os = ...)]` code needs this check**, which
is the same rule as the feature one above, one axis over: `src/bluetooth/`,
`src/display/`, `src/usb/` and `src/platform/` all hold per-OS readers next to
shared helpers.

### The local gate cannot see a feature-gated break

`cargo clippy --all-features`, `cargo test --all-features` and **both**
cross-target checks share one blind spot: every one of them enables `nvidia`.
The cross-checks name it in their feature lists explicitly. So a helper written
at module scope without `#[cfg(feature = "nvidia")]` passes the entire local
gate and fails CI's *Feature combinations* job, which builds each feature alone.
That happened at `7c79f8b`, on a `pstate_name` helper — the fix for one defect
breaking a build the gate does not cover.

Before pushing anything that touches a vendor-gated module, check the feature
alone:

```bash
cargo check --quiet --no-default-features --all-targets --features cpu
```

The full loop CI runs is twenty features and takes far longer than a local
session usually allows; the cheap approximation is `(none)`, `cpu` and `cli`,
plus whichever feature the file you touched is *not* gated by. `src/profile/`,
`src/hwmon/` and `src/backend.rs` all hold vendor-gated code beside ungated
code, which is where this is easy to get wrong.

### The MSRV is 1.89, and one value now covers every feature combination

It was 1.88, which held for a default build and not with `vault`:
`ironvault` requires 1.89 and **all fourteen of its published versions do**, so
there was nothing to pin back to. The choice was between claiming 1.88 with a
footnote or claiming what is true everywhere.

Raised to 1.89. A field meaning "1.88, except…" is the same shape of defect as an
entity declaring a provenance stronger than a row can carry — the reader has to
know the exception to avoid being misled. `cargo +1.89 check --all-features
--all-targets` now verifies the whole thing in one command, instead of two with
one expected to fail.

Note what found it: the documented MSRV check builds default features, CI builds
`--all-features` on stable, and the failing combination was the one neither ran.
That is the *Feature combinations* lesson one level down, in the dependency graph.

### A reader wired into one consumer leaves the others fabricating

`platform::macos::read_cpu_stats` and `read_memory_stats` were added in 5.2.0 and
wired into the ontology resolver. **`MonitoringBackend` — what the `simon` CLI
runs — kept its own hand-rolled macOS arm for two releases**, and almost every
field it produced was invented: the system-wide figure repeated across every
core, a fixed 60/40 user/system split, `governor: "performance"` on a platform
with no governors, `min = max/2`, and `"Apple CPU"` when the brand string was
missing. A failed load-average parse fell to 0.0 and published 100% idle.

So one Mac had two CPU paths that disagreed — one measured, one invented — and
`update_memory` had no macOS arm at all, so `simon` reported no memory there.
Fixed in `24a7314`.

**Three things worth carrying:**

- Adding a reader is not the same as adopting it. Grep for the other consumers
  of whatever the new reader replaces, because the old fabrication keeps running
  where it was not swapped in, and no test compares the two paths. **There were
  three consumers of the macOS memory path, not two** — the ontology adopted the
  real reader in 5.2.0, `MonitoringBackend` kept fabricating (`24a7314`), and
  `ai_api::tools` kept a third hand-rolled copy (`1cf1980`) with a hardcoded page
  size. Count the consumers before assuming a migration finished. **Then there
  was a fourth**, and it was CPU rather than memory: `ai_api::tools`'
  `get_cpu_status` shelled out to `sysctl vm.loadavg` and published
  `load / ncpu * 100` as `usage_percent`, next to `read_cpu_stats` it never
  called. Load average is not utilization — it counts runnable *and*
  uninterruptible tasks, so a machine blocked on I/O reads high while its cores
  idle. Grepping for the reader's name finds the consumers that adopted it; the
  ones still fabricating do not mention it, so grep for what they call instead.
- **A fabricated number is more dangerous than an absent one.** The nine silent
  readers reported nothing; this reported a believable 43% on every core, which
  a user cannot tell from a measurement.
- It was found by grepping comments — `requires admin`, `assume`, `approximation`,
  `hardcoded`, `placeholder` — next to returned values:

  ```bash
  grep -rn --include=*.rs -iE "//.*(requires admin|assume|approximation|hardcoded|placeholder|can'?t (detect|read))" src/
  ```

  **A comment admitting the code cannot determine something, sitting beside a
  returned value, is a reliable marker for a fabricated reading.** Run it before
  trusting any reader.

  **A second grep finds a different and larger population** — the defects nobody
  bothered to comment on:

  ```bash
  grep -rn --include=*.rs -E "unwrap_or\((0|0\.0|false|true|1)\)" src/
  ```

  This one needs triage rather than blanket treatment: `unwrap_or(true)` as "no
  filter given, include everything" is correct and common. **The signal is a
  fallback in a *reader* path**, where the value flows outward as a reading. It
  found the worst defect of the sweep — a hardcoded page size on the agent-facing
  memory tool that would report memory four times too large on an Intel Mac.

### What that sweep found, and the one class it turned up

A second pass over the four modules the grep flagged found one live defect and
two latent ones. The distinction matters and is easy to overstate:

- **Live: `ai_workload` reported any GPU-holding Python process as PyTorch.** The
  guess reached users through `signals.ai_frameworks`, which `tuning::classify`
  publishes beside the evidence string *"an AI framework was identified in a
  running process"* at 0.8–0.9 confidence. Fixed in `7b606fe`.

  **This is a different failure mode from the rest.** Every other defect this
  round invented a *number*; this invented an *identity*, and a second module
  then attested to it. A wrong number invites a sanity check; a plausible
  framework name does not — and `simon tune` is where acting on it has
  consequences.

- **Latent, fixed:** `dma_engine` claimed memcpy support whenever the `cap` file
  was unreadable. Nothing calls it.
- **Latent, deliberately not fixed:** `cpufreq::is_turbo` returns `bool` where it
  means "cannot tell" — with no base frequency it guesses from ">95% of max", and
  with neither base nor max it returns `false`, encoding unknown as "no turbo".
  It has no callers, and the fix is `Option<bool>`, which is breaking. **This is
  the pattern in *Queued for the next major version* recurring exactly as that
  section predicted**, and it is the first entry for 7.0.0.

`src/audio/mod.rs` was checked and is clean — the "placeholder" comment is in a
test, not a reader.

### Two local failures that were the machine, not the code

Both look exactly like defects and neither is. Recognising them saves an hour:

- **`LNK1104: cannot open file …-<hash>.exe`** means a stale test binary from a
  killed run is still holding it. `ps -W | grep <testname>` finds it; it will
  reproduce on every build until killed. Nothing to do with the quarantine below,
  though the symptom is similar.
- **`memory allocation of N bytes failed`** during `cargo test --lib` is an OOM
  abort, not an assertion. `--no-fail-fast` runs every target's binary and each
  spawns a thread per core. `-- --test-threads=4` completes cleanly.

Same lesson as the red pipeline, one level down: **a broken instrument and a
real failure produce similar-looking output**, and the difference is in the
message rather than the exit code.

### The environment ate most of this session

Windows Defender was quarantining Rust binaries and build artifacts live.
Symptoms, so the next person recognises it in one minute rather than an hour:
`rustc.exe` vanishing from a toolchain *while running*; builds dying at
`Compiling silicon-monitor` with exit 127; `EPERM` on deleting anything under
`~/.rustup`; every toolchain's `.rlib` files gone (21 left of 81,034 files) and
then partially reappearing with identical hashes. `rustup` cannot self-repair
through it, because it cannot delete what it cannot delete.

```powershell
Add-MpPreference -ExclusionPath "$HOME\.rustup","$HOME\.cargo","<repo>\target"
Get-MpThreatDetection | Select-Object -Last 20
```

**Do not attempt a `rustup toolchain uninstall` to fix it.** It was tried here
and made things worse: the uninstall half-completed against undeletable files,
leaving stable with no manifest and no `bin/`. It recovered on its own; the
lesson is that the failure is underneath rustup and rustup has no answer to it.

The version is 6.0.0 and is **not tagged**; `v5.2.0` is many commits behind.
Publishing to crates.io was explicitly declined by the maintainer for 6.0.0 and
5.2.0 — tag only.

## Released

| Version | State |
|---|---|
| 3.0.0 | Published, tagged `v3.0.0`. SMART and NVMe support. |
| 3.0.1 | Feature-combination fix — see CHANGELOG. |
| 3.1.0 | Windows NVMe passthrough (unelevated) and macOS CPU/memory. Published, tagged. |
| 3.2.0 | macOS per-core CPU, NVMe current power state, `--all-targets` in the feature job. Published, tagged. |
| 3.3.0 | Windows ATA SMART (unelevated), shared SMART collector, ontology to 94 entities. Published, tagged `v3.3.0`. The ATA parse is unverified against real SATA hardware — see open work 1. |
| 3.4.0 | DIMM slot topology and USB in the ontology (113 entities). Published, tagged `v3.4.0`. |
| 3.5.0 | Ontology-driven conformance tests, Windows PCI reader fixed, PCI domain (123 entities). Published, tagged `v3.5.0`. |
| 3.6.0 | PCIe link state on Windows via cfgmgr32; the PCI domain fully resolves. Published, tagged `v3.6.0`. |
| 3.7.0 | Virtualization, NUMA and ECC in the ontology (134 entities); two misleading virtualization readings withdrawn. Published, tagged `v3.7.0`. |
| 3.8.0 | `simon tune`: use-case detection and profile recommendations, with an automatic server. Recommend-only by default. Published, tagged `v3.8.0`. |
| 3.9.0 | Headless GUI reads four previously unreadable tabs; racy coverage test fixed. Published, tagged `v3.9.0`. |
| 3.10.0 | Hyper-V root partition distinguished from a guest via CPUID leaf 0x40000003; bare metal no longer reported as a VM. Published, tagged `v3.10.0`. |
| 4.0.0 | GUI rebuilt on Dewey; the ~10k-line egui implementation and eframe deleted. MSRV 1.85. Published, tagged `v4.0.0`. **Withdrawn in 5.0.0.** |
| 4.0.1 | GUI moved to Dewey's `agpu` backend over a wgpu Vulkan spec violation. Published, tagged `v4.0.1`. Withdrawn. |
| 4.0.2 | Dewey GUI made legible and its tabs clickable. Published, tagged `v4.0.2`. Withdrawn. |
| 4.0.3 | Dewey GUI restyled onto the recovered `CyberColors` palette. Published, tagged `v4.0.3`. Withdrawn. |
| 4.0.4 | Dewey Overview rebuilt from the egui widget vocabulary. Published, tagged `v4.0.4`. Withdrawn. |
| 5.0.0 | **The Dewey port is withdrawn and the egui GUI restored.** Claimed MSRV back to 1.70 — which was not true, see 5.2.0. Published, tagged `v5.0.0`. See open work 9. |
| 5.1.0 | Applied settings are reversible: `read_current`, `ApplyOutcome.previous`, `revert_setting`, `revert_cycle`. Published, tagged `v5.1.0`. See open work 13. |
| 5.2.0 | `simon ai models` reads an IronVault vault, behind an optional `vault` feature. **MSRV corrected to 1.88**, having been wrong since 5.0.0. The tuning loop is closed (open work 13). **CI made green after twelve consecutive failures**; macOS CPU/memory wired into the resolver. Tagged `v5.2.0`, **not published**. |
| 6.0.0 | **Zero-constructors renamed to `empty()`** and the `Option` refactor on `SwapInfo`/`RamInfo` done — all three "queued for the next major version" items closed. Ontology grew a capability and vocabulary layer with tests derived from the declarations; JSON-LD output with QUDT units; `simon status` (coloured, per-OS ASCII art); network and file intrusion detection; the tuning ledger. **Not tagged, not published.** |
| 2.1.5 | Committed, never published. Documentation only; superseded by 3.0.0. |

## Verification that is worth repeating

```bash
cargo fmt --all -- --check
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features
cargo test                       # default features: the `vault` tests must drop out
cargo +1.89 check --all-features --all-targets  # the declared MSRV, actually built
cargo check --target x86_64-unknown-linux-gnu --all-targets --no-default-features --features cpu,npu,io,network,amd,nvidia,intel
cargo check --target aarch64-apple-darwin --all-targets --no-default-features --features cpu,npu,io,network,apple,nvidia,num_cpus
```

**The cross-checks cannot use `--all-features`, and the reason is not obvious.**
`gui` pulls `egui_extras → ehttp → ureq → rustls → ring`, and `ring`'s build
script needs a C cross-compiler for the target. Without one the check dies in a
build script having compiled none of this crate's own code, which reads at a
glance like a broken toolchain rather than a missing one. Keep the feature lists
above; `cargo tree -i ring --target <t> -e normal,dev` is how that was traced.

The corollary is that a feature combination the cross-check *can* build is one
CI never tries — CI checks `--all-features` and each feature *alone*, never the
subset above. `examples/cpu_monitor.rs` failed to compile on macOS without the
`apple` feature for exactly that reason and nothing noticed until a cross-check
ran with `--all-targets`.

**Build the MSRV, do not declare it.** `rust-version` said 1.70 from 3.x through
5.1.0 and had been false since 5.0.0: restoring the egui GUI brought in
`image 0.25.10` and `home 0.5.12`, which declare 1.88 themselves, and `image`
reaches `pxfm`, an edition-2024 crate that pre-1.85 cargo cannot even parse. Two
releases shipped a floor nothing had ever compiled against, because nothing did.
`cargo +<msrv> check` is one command and it is the only thing that makes the
field mean anything.

The corollary bit in the other direction too: raising the declared floor to 1.88
un-gated ten clippy lints that `incompatible_msrv` had been silently suppressing
at 1.70. A wrong `rust-version` does not merely mislead readers — it changes
which lints run.

**Read CI before believing a green local run.** Every job had failed on every
run for over a week — twelve consecutive, spanning the 5.0.0 and 5.1.0 releases —
while local suites on Windows were green throughout. Seven defects were hiding in
there, and each was invisible until the one before it was fixed, because a
failing job cancels the rest of the matrix.

Three of the seven could not be reproduced on this machine on principle: two
needed a box with no GPU, one needed a Mac. Two more lived in
`cfg(target_os = "linux")` code that a Windows compiler never reads. **A local
`cargo test` says nothing whatsoever about the other two platforms**, and saying
"the full suite passes" on the strength of one is how those shipped.

```bash
gh run list --limit 5    # before claiming anything is green
```

The failure mode to recognise: a pipeline that is always red and a test suite
that checks nothing are the same instrument. Both return a constant, and a
constant carries no information. This project has now been bitten three times —
839 passing tests over a GUI that reported no GPUs on a three-GPU machine, an
intermittent failure at three runs in five that read as noise, and this.

The two cross-target checks need only `rustup target add …` — no C toolchain, no
VM. They are the fastest way to catch platform breakage from a Windows box, and
they are how the 114 compile errors behind the 3.0.0 manifest bug were found.

**`--all-targets` is not optional there.** Without it `cargo check` builds the
library and skips the test targets, and `tests/macos_readers.rs` is
`cfg(target_os = "macos")` — so a signature change that broke it was invisible to
both the local suite and the cross-check, and reached CI. The `Option` fields on
`SwapInfo` did exactly that.

**A test can be wrong about units, and it fails on the machine that is right.**
The macOS job went red at `2b8bf16` on `memory_totals_are_internally_consistent`,
which asserted `ram.total >= 1 GiB` with the constant written in bytes — while
`read_memory_stats` divides `hw.memsize` by 1024 before storing it. The runner's
correct 7 GiB reading, 7340032 KB, read as 7 MB and failed the floor. The reader
was right and the assertion was wrong, and the failure message printed the word
"bytes" beside a KB figure, which is what made it take longer than it should
have.

Worth generalising: this crate stores memory in KB in `RamInfo` and in bytes in
several readers, and the compiler cannot tell them apart because both are `u64`.
Before believing a plausibility bound, check which unit the field is actually in.

CI additionally checks every feature in isolation (job **Feature combinations**).
That job exists because `--all-features` cannot catch a feature that only builds
because another supplies what it is missing — which is exactly how the `cli`
feature stayed broken through eight published versions.

## Open work

1. **`hardware_ai` was audited on one machine, and only one.** Every conclusion
   corrected in `a584dd0` and `7607401` was verifiably wrong on this desktop, and
   each fix was checked against a second source. That is not the same as being
   right in general. Two things specifically want a second machine before they
   are trusted:

   - **The classifier weights were rebalanced against a single data point.** A
     read chassis now scores 0.6 where a battery scores 0.25, and the Desktop
     branch is no longer gated on `!has_battery`. The *direction* is defensible
     on principle — a reading should outrank a guess — but the numbers are still
     tuned to one host. `test_laptop_bottleneck` and
     `test_gaming_desktop_classification` pass, and both use synthetic features
     rather than hardware. **Run `HardwareInferenceEngine::new()?.full_analysis()`
     on a real laptop and read the report.** If it does not say Laptop, the
     weights are wrong in the other direction now.
   - **`adapter_vram_bytes` was tested against NVIDIA and AMD adapters in one
     box.** Intel Arc and older drivers may not publish
     `HardwareInformation.qwMemorySize`. Those adapters get no VRAM reading at
     all, which is the intended failure — but nobody has watched it happen.

   Also still imprecise, in a heuristic table rather than a reader: the RTX 3090
   Ti dates to 2020 because it matches the RTX 30 series rule, and the Ti shipped
   in 2022. `infer_gpu_year` and the TDP tables were not audited.

2. **Per-core CPU frequency is unreported on Windows, and could be reported.**
   `CallNtPowerInformation` returns `CurrentMhz == MaxMhz` whatever the cores are
   doing, so `cpu.core.{n}.frequency` now declines rather than publishing the
   nominal clock as a measurement. The real figure is
   `\Processor Information(*)\% Processor Performance` multiplied by the nominal
   clock, which is what Task Manager shows: on this machine the cores read
   105–119% of nominal while the API insisted on 4400 for all 24.

   It needs `PdhGetFormattedCounterArrayW` for the wildcard instance, and it is a
   rate counter, so it wants **two collections with an interval between them** —
   which is why this was not simply done: it puts a sleep in the snapshot path,
   and `snapshot` currently returns without one. `src/hwmon/cpu_temp.rs` has the
   single-value PDH pattern to copy. The provenance would be `Derived` from the
   counter and the nominal clock, not `Measured`.

3. **The Windows ATA SMART path has never met a SATA drive.** 3.3.0 reads the
   attribute table unelevated through `IOCTL_STORAGE_PREDICT_FAILURE`, and the
   parse in `src/disk/ata_smart.rs` is tested only against buffers this project
   built. This machine has three NVMe drives and a USB gadget; on all four the
   path returns `NotSupported`, which exercises the decline and nothing else.

   **If you have a SATA SSD or HDD, that is the thing to use it for.** One run of
   `cargo run --example disk_monitor` against `smartctl -A` settles it. Watch
   power-on hours in particular: a minority of drives report that attribute in
   minutes, and nothing in the structure says which.

   **This is no longer hypothetical: the NVMe parser had exactly this bug.**
   `nvme_log.rs` omitted Controller Busy Time (111:96), so every field below it
   read sixteen bytes early — `power_cycles` returned Power On Hours,
   `power_on_hours` returned Unsafe Shutdowns, and `media_errors`, the field a
   person uses to judge whether a drive is dying, returned the error-log entry
   count. It shipped because `health_log_reads_every_field_at_its_specified_offset`
   built its fixture from *the parser's own offsets*: the test asserted that the
   code agreed with itself, under a name claiming it checked the specification.
   **A parse test whose fixture comes from the parser is a tautology.** Write the
   fixture from the spec document, give every field a distinct non-zero value,
   and confirm the test fails against the old code before believing it.

   Note for anyone re-reading the earlier plan: `IOCTL_ATA_PASS_THROUGH` was the
   scoped approach and it is a dead end. Its `CTL_CODE` carries `FILE_READ_ACCESS
   | FILE_WRITE_ACCESS`, so the access check happens before the driver is reached
   and a read/write handle on `\\.\PhysicalDriveN` needs Administrator — measured,
   `ERROR_ACCESS_DENIED` on all four drives. Whether an ioctl needs elevation is a
   property of its `CTL_CODE`, and is worth reading off the definition before
   planning around it.

4. **macOS GPU, power and temperature are still unimplemented.** CPU (per-core,
   with nice time), memory, swap, uptime and board info work — `Simon::cpu()`,
   `memory()`, `uptime()`. `Simon::snapshot()` still fails, because it requires
   every reader. Power and temperature need `powermetrics`, which requires root,
   so they may not be reachable unelevated at all; that is worth establishing
   before writing anything.

   **If you have a Mac, that is the thing to use it for.** These readers were
   written by cross-compilation and are verified only by `tests/macos_readers.rs`
   on `macos-latest`, which checks that readings are *plausible* — not that they
   are *correct*. Comparing `Simon::cpu()` and `memory()` against Activity Monitor
   once would settle whether the `vm_stat` accounting matches what a user sees.

   5.2.0 closed part of this. `platform::macos::read_cpu_stats` and
   `read_memory_stats` now exist and are wired into the ontology resolver, which
   previously had no macOS arm at all and reported "the platform CPU reader
   returned an error" for readers it had never called. Eight entities were
   affected, four of them non-nullable. They resolve on `macos-latest` now — the
   first time this code has executed anywhere.

   Still true, and still the reason this item is open: *plausible* is not
   *correct*. Nothing has compared these numbers against Activity Monitor, and
   the `vm_stat` used/free split is a judgement about which pages count as in
   use, which a conformance test cannot check.

5. **The Linux SMART/NVMe paths have executed exactly once**, in CI on
   `33ee241` — 733 tests, 0 failures. No one has run them against real Linux
   hardware. The sysfs paths (`/sys/class/nvme/<ctrl>/{model,serial,firmware_rev,cntlid}`)
   are documented kernel ABI, but tests are not a substitute for a drive.

6. ~~**`smart_disk()` spawns a subprocess per call.**~~ Fixed in 3.3.0 by
   `SmartMonitor::cached_disks()`, which shares one sweep process-wide for 2 s.
   A sweep is 1.23 s on this machine, and a four-drive pass could take twelve of
   them. Two things narrowed the problem before it was fixed: NVMe and SATA drives
   are now answered by their passthrough and never reach the collector at all, so
   what remains to benefit is USB storage — and every Linux machine, where a
   sweep spawns `smartctl` once per drive and the old shape was quadratic.

7. **The ontology names ~232 entities; the library has ~88 subsystem modules.**
   The running list of which clusters exist, which readers answer on which
   machine, and what is left is under **plan item F** below — it is kept in one
   place rather than two, because the last time it lived in both they disagreed.

   The standing rule is here because it governs every addition: **none should be
   declared until someone can watch its resolver answer on hardware that has the
   thing** — or, failing that, watch it decline with a true reason. A cluster
   that resolves to `unavailable` carrying a real explanation satisfies this. One
   that resolves to silence does not.

   **`tests/ontology_conformance.rs` now checks the things that used to need an
   eye.** Five entities across 3.3.0 and 3.4.0 shipped an enum's `Unknown` through
   as a *measured* value, each caught in review; that class is now caught by
   construction, along with non-nullable nulls, unit/JSON-type mismatches and
   absences with no stated reason. Run it before adding a domain and after — it
   found three defects on its first run, one of them an entity declared since the
   ontology was written that no resolver had ever touched.

   An id that resolves to a confident guess is worse for an agent than one that
   does not exist, so add a domain only when its resolver can say why each absence
   is absent.

8. ~~**`VirtMonitor::is_virtual_machine()` returns true on a Hyper-V root
   partition.**~~ Fixed in 3.10.0 via Hyper-V CPUID leaf 0x40000003: the
   partition privilege mask holds `CreatePartitions` and `CpuManagement` only on
   a root partition. `hypervisor_indicates_vm()` is now the single path, so
   `detect_platform()` and the ontology entity were corrected at the same time,
   and the 3.7.0 ontology workaround is withdrawn.

   **The guest side is still unverified against a real Hyper-V VM.** The root
   side is measured on this desktop (`ebx=0x002bb9ff`). If you have a Hyper-V
   guest, one run of `simon get system.virtualization.platform` settles it —
   expect `virtual_machine`. CI's Windows runners are Azure Hyper-V guests and
   exercise the arm, but no assertion pins the value there because the same test
   must pass on this bare-metal desktop.

9. ~~**The Windows PCI reader blocks the PCI ontology domain.**~~ Fixed in 3.5.0.
   The reader reported a device-instance path as the address and never reported a
   bound driver, so 3.4.0 withheld the domain rather than ship unstable ids and a
   false "no driver is bound". Both now come from
   `HKLM\SYSTEM\CurrentControlSet\Enum`, which is readable unelevated:
   `LocationInformation`'s parenthesised tail carries the bus/device/function
   triple, and `Service` carries the driver. Addresses are conventional BDF
   (`0000:7a:00.0`) and the domain ships.

   PCIe link width and speed followed in 3.6.0, from the device node property
   store via `CM_Get_DevNode_PropertyW`. Two notes for anyone extending this:
   `Get-PnpDeviceProperty` reads the same values and costs 0.4 s per device (25 s
   here, against 0.62 s for the whole monitor via `cfgmgr32`), and the registry
   property store under `Enum\...\Properties` is ACL-blocked unelevated, so
   neither is a shortcut. The remaining `DEVPKEY_PciDevice_*` properties — payload
   sizes, AER capability, ARI and ATS support, SR-IOV — are readable by the same
   two calls with a different pid, if anyone wants them.

10. **`simon tune`'s policy table covers five settings, and its game detection is
   a name table.** Both are deliberate first cuts, and both are where the feature
   grows.

   The policy table in `tuning::policy_for` is keyed on setting id, not on a
   category, because two vendors' "performance mode" are not interchangeable —
   pretending otherwise is how a tuner writes one vendor's value into another's
   register. Adding a setting means adding an apply handler first: a policy for
   something unwritable produces advice nobody can take, which is why the planner
   records it under `skipped` rather than recommending it.

   Game detection matches launcher and engine process names, which will miss most
   games. It is used only as corroborating evidence and carries lower confidence
   than an identified AI framework for that reason. The right answer is to ask
   the graphics driver what is presenting full-screen; that is not implemented.

   **The rule to preserve:** a proposed value comes from what the driver declared
   — a `choices` entry or the reported `default` — never from this crate and never
   from a model. `tuning::tests::a_recommendation_never_proposes_a_value_the_driver_did_not_offer`
   is the test that keeps it true. A model may classify; it may not pick numbers.

11. **The Dewey port was tried across 4.0.0–4.0.4 and withdrawn in 5.0.0.**
   `src/gui/` is the egui application again — `app.rs`, `widgets.rs`, `theme.rs`,
   `profile_tab.rs`, `headless.rs`, `mod.rs`, restored from `927ffaa^`. The
   `deweygui` dependency, the `dewey-gui` feature and `simonlib::gui_dewey` are
   gone rather than left as dead aliases.

   **Read this before proposing the port again.** The argument for it was sound:
   the 3.9.0 spinner bug came from a headless path that diverged from the
   interactive one, and Dewey's `Command::Task` removes that class of bug. What
   the argument missed is what the egui GUI actually does. Side by side, the
   original shows nine metric cards including live GPU temperatures, six
   sparkline charts with axes and min/max, a threshold legend, an uptime and task
   status line, an AI ask bar and JSON/CSV export — and on the development
   machine it detects three NVIDIA GPUs where the port reported
   `Accelerators: unavailable — Failed to initialize COM`.

   So the port was not merely unfinished to look at. **It was reading less
   hardware, and the tests said nothing about either fact.** They asserted that
   every tab emitted named ontology nodes, which the port did faithfully while
   displaying a fraction of what it replaced. A contract that checks the shape of
   the output and never its substance will certify a regression as complete. If
   this is attempted again, the acceptance criterion is a screenshot of both
   GUIs side by side and a diff of what each reports on the same machine — not a
   node count.

   Four releases went into making the replacement presentable and it never got
   close. Budget accordingly.

12. ~~**`CpuStats::new()` and `MemoryStats::new()` are zero-constructors with
   constructor-shaped names.**~~ Fixed in 6.0.0. Both are `empty()` returning
   `Self`, and `tests/zero_constructors.rs` holds an empty list: no `new()` in
   this crate fabricates a reading.

   **Five defects came out of this one pattern, across three separate
   discoveries** — two GUI call sites, `SiliconMonitor::snapshot_cpu` and
   `snapshot_memory` returning zeros from the *public* API, `SystemHealth::check`
   computing CPU usage from 100% idle so no threshold could ever be crossed, and
   the Prometheus exporter publishing 0% CPU on every scrape. Three of those were
   found only because the rename forced every call site to be read.

   The rule that outlived the bug: **assume any `T::new()` in this crate may be a
   zero-constructor until checked**, and prefer `empty()` for a builder base. The
   name is the whole defect — every call site was written by someone who
   reasonably believed `new()` constructs a thing from the system.

13. **Verify with `--lib --tests` when the disk is tight.** `cargo test
   --all-features` links every example. That is affordable again now the duplicate
   egui is gone, but if it ever fails with `link.exe` 1318, the split is
   `cargo test --all-features --lib --tests` for execution plus
   `cargo clippy --all-features --all-targets` for type-checking the examples.
   Note `--lib --tests` skips doc-tests; run those before a release.


14. **Two Dewey bugs found during the port, recorded because they are real
   and unfixed — but no longer reachable from this crate.** Neither affects simon
   now that `deweygui` is gone. Both are for whoever works on Dewey itself, or
   for anyone who reconsiders open work 9.

   *wgpu Vulkan semaphore reuse.* Dewey's default eframe path reuses one binary
   semaphore across swapchain images, which the validation layer rejects as
   `VUID-vkQueueSubmit-pSignalSemaphores-00067` on every frame. Undefined
   behaviour by the spec rather than noise, though it renders. The fault is
   wgpu's, below both Dewey and eframe. Dewey's `agpu` backend does not hit it.

   *Intermittent blank render.* Resizing an `agpu` window — including a
   `SetWindowPos` to the size it already was — sometimes leaves it entirely blank
   with the process alive and nothing logged. It reproduces in Dewey's own
   `counter_agpu` example. Instrumenting `agpu_backend.rs` showed winit
   delivering a spurious `Resized(<the configured size>)` immediately after the
   true size, shrinking the surface while the window stays large. Three fixes
   failed and were reverted rather than left in place: `ControlFlow::WaitUntil`,
   requesting a redraw on resize, and reconciling against `window.inner_size()`
   each frame — the last because winit's own `inner_size()` reports the stale
   value too. That is the detail any further attempt should start from. Repro:
   three `eprintln!` calls in `resize`, the surface-acquire error arm and the
   frame-area computation in `render`, driven by `ShowWindow(hwnd, 3)`.

15. **Applied settings are reversible; the tuning loop is not yet closed.**
   `ApplyHandler::read_current()` reads a setting before it is written,
   `ApplyOutcome.previous` carries what was overwritten, `revert_setting()` puts
   it back through the same confirmed and audit-logged path, and
   `tuning::serve::revert_cycle()` undoes everything one cycle applied, in
   reverse.

   Before this, `simon tune --apply` could change a machine and had no way to
   change it back: the trait could only write, and the outcome recorded only what
   was requested. That is worth remembering as a shape of bug — the write path
   was complete and correct on its own terms, and useless for anything that
   needed to undo it.

   **The rule to preserve:** when no prior value was recorded, `revert_setting`
   refuses rather than writing a default. Putting a machine into a state it was
   never in is a worse failure than leaving it where the caller put it. A handler
   without `read_current` makes its setting explicitly one-way; the default
   returns `None` so existing handlers still compile, which means *adding* a
   handler without it silently gives up reversibility. Implement it whenever the
   source can be read back.

   Verified on Windows: `read_current` returns the same GUID that
   `simon profile explain active_scheme_guid` reports through unrelated code in
   `profile::cpu`. **The Linux sysfs readers are written by inspection and have
   never run** — there was no Linux machine in the session that added them, and
   CI compiles the path without exercising a sysfs write.

   **The loop was closed in 5.2.0**, in `tuning::verify`: measure, write, settle,
   measure, revert on a demonstrated regression. `serve::cycle_verified` runs a
   pass that way and each `AppliedOutcome` carries its `Verdict`.

   The mechanism was the easy half, as expected. The hard half went the way the
   warning above predicted, and the result is worth stating plainly: **the metric
   registry ships empty.** The obvious metric for a power-scheme change is
   achieved clock speed, and on Windows it does not exist —
   `CallNtPowerInformation(ProcessorInformation)` reports a nominal figure.
   Sixteen spinning threads took system idle from 79.7% to 11.4% and every core
   reported exactly 4400 MHz throughout. A verifier built on that number would
   have reported "no change" for every power scheme ever written and nobody would
   have questioned it.

   So `metric_for` returns `None` for every setting, `Unverifiable` is the normal
   verdict, and `no_setting_claims_a_metric_it_has_not_earned` exists to make
   adding one a deliberate act. **The bar for adding a metric is a demonstration,
   on real hardware, that the number moves when the setting changes — in the
   commit message.** A registry entry added because the mapping seemed sensible
   is precisely the invented criterion the module was built to refuse.

   **The NVML lead was followed, and the answer is that a live metric exists but
   nothing on Windows can be verified with it.** Six samples via `nvidia-smi`
   half a second apart: GPU 0 moved 540 → 495 → 495 → 555 → 585 → 645 MHz while
   its utilisation moved 26 → 38%. That is a genuinely live reading, in direct
   contrast to the Windows CPU clock, which does not move at all. So the metric
   source is sound.

   It has nothing to verify. **Every setting whose effect a GPU clock could
   measure is Linux-only:**

   | Setting | Platform | Metric |
   |---|---|---|
   | `active_scheme_guid` | Windows | none — the CPU clock reads nominal, see above |
   | `scaling_governor` | Linux | plausible: `scaling_cur_freq` is live on Linux, unlike the Windows equivalent. Untested. |
   | `persistence_mode` | Linux + NVIDIA | none plausible — it governs driver load latency, not throughput |
   | `perf_level` | Linux + **AMD** | GPU core clock |
   | `gt_max_freq_mhz` | Linux + Intel GPU | GPU clock |

   So the verification loop **cannot be exercised end to end on Windows at all**:
   the one applicable setting has no live metric, and every setting with a
   plausible metric is on the other platform. `perf_level` compounds it by being
   AMD, where the GPUs here are NVIDIA — so even the vendor does not line up.

   No metric was registered, and no metric source was written, on purpose.
   Writing an NVML clock reader to verify an AMD sysfs setting on a Linux machine
   that does not exist here would be plausible, untestable, and exactly the class
   of work this project has been bitten by twice — the Linux sysfs readers in
   `profile::apply` are already written by inspection and have never run.

   **What actually unblocks this is a Linux box**, ideally with an AMD GPU. On
   one, the demonstration is cheap: sample the GPU clock, write `perf_level=high`,
   sample again, and show the number move. That measurement belongs in the commit
   that adds the registry entry.

   **A timed benchmark was tried as a way round the missing Windows metric, and
   it is a dead end twice over.** The idea: if the reported clock is nominal,
   measure achieved throughput instead. A fixed single-threaded integer workload,
   nine samples, median of each:

   | Scheme | Median | MAD |
   |---|---|---|
   | Balanced | 236.8 ms | 2.2 |
   | High performance | 239.2 ms | 2.3 |

   A delta of 2.4 ms against a noise band of 4.5 ms, with the sign backwards —
   High performance nominally *slower*. By `verify`'s own threshold rule that is
   `Unchanged`. A lightly loaded desktop boosts a single thread to the same place
   under both schemes, so there is nothing for the scheme to change.

   The second reason is the one that closes the avenue regardless of what a
   sustained all-core run would have shown: **a metric that requires running a
   benchmark is unusable in an autonomous tuner, because it competes with the
   workload it is tuning for.** Burning every core for two seconds to find out
   whether the machine got faster is self-defeating on a machine that was
   already busy, and meaningless on one that was not.

   So the metric has to be *passive observation of the real workload's own rate*.
   AI workloads and their frameworks are already detected; tokens per second off
   a running inference server is the shape of thing that would work — it is the
   user's own work, measured without perturbing it. That is the direction worth
   taking, and it is a larger piece of work than a registry entry.

   (Note for whoever repeats the benchmark: a constant seed makes the workload
   pure and LLVM eliminates every call after the first. The first attempt printed
   266 ms followed by eight zeros. `black_box` the inputs, not just the result.)

## Queued for the next major version

Three fixes that are correct, identified, and breaking. They live in code
comments at their sites, which is where the person fixing them will look and not
where the person planning a release will. Collected here for that reason.

Each is the same underlying fault: **a type with no way to say "not read"**, so
absence is encoded as zero and becomes indistinguishable from a measurement of
zero. That is the one distinction this crate exists to preserve, and these three
places cannot preserve it without an API change.

1. ~~**Rename the zero-constructors.**~~ Done in 6.0.0. They are `empty()`
   returning `Self`, and `tests/zero_constructors.rs` now holds an empty list:
   no `new()` in this crate fabricates a reading.

   **The rename was not cosmetic, which is worth remembering before deferring
   the next one.** Going to do it turned up three live defects the misleading
   name had been hiding: `SiliconMonitor::snapshot_cpu` and `snapshot_memory`
   returned zeros from the public API, `SystemHealth::check` computed CPU usage
   from 100% idle so no threshold could ever be crossed, and the Prometheus
   exporter published 0% CPU on every scrape. Five defects total from this one
   pattern, over three separate discoveries.

2. ~~**`Option` fields on `SwapInfo`.**~~ Done in 6.0.0. `total`, `used` and
   `cached` are `Option<u64>`, with `total_or_zero()`, `used_or_zero()`,
   `cached_or_zero()`, `is_reported()` and `swap_usage_percent() -> Option<f32>`
   for callers that genuinely want the old behaviour.

   The 62 read sites were the reason this was deferred, and they were the cheap
   part. What the change actually bought: it **ended three workarounds and one
   unit bug**. The macOS reader no longer fails the entire memory read to avoid
   claiming a machine has no swap — it says `None` and keeps the RAM figures.
   Linux's missing-`SwapTotal:` error path and the resolver's two-way swap
   branch both collapsed into the type. Deferring on cost was the wrong call:
   the cost was in the mechanical part and the value was in the parts that had
   been bent around the missing case.

3. ~~**`Option` fields on `RamInfo`.**~~ Done in 6.0.0. `shared` is
   `Option<u64>`; `buffers` stays a plain `u64` and stays 0 on macOS, because
   that platform keeps no buffer pool distinct from the file cache and the zero
   is a fact about the platform rather than a missing reading. That asymmetry is
   deliberate — do not "finish the job" by making `buffers` an `Option` too.

All three are closed. The section is kept because the pattern will recur: **a
type with no way to say "not read"** encodes absence as zero, and zero is
indistinguishable from a measurement. When the next one appears, the lesson from
items 1 and 2 is that the rename or the `Option` is not the work — the defects
it uncovers are, and there are always more than expected. Item 1 turned up five.

**It has recurred, and this is the 7.0.0 list.**

4. **`BatteryInfo::charge_percent` reports 0% when the charge was not read.**
   *This one is live and is the first to fix.* `power_supply.rs` models it
   correctly as `Option<u8>` — its own doc comment demonstrates
   `if let Some(capacity) = supply.capacity_percent` — and `battery/mod.rs:102`
   discards that with `unwrap_or(0)` into an `f32` field that cannot say "not
   read". The ontology then publishes it: `resolve.rs` pushes `charge_percent`
   as `Reading::measured` with no guard, so `power.battery.percentage` resolves
   **0% as a measurement** on a laptop whose capacity is unreadable.

   0% is not a neutral wrong answer. It means "about to shut down", and an agent
   acting on it would defer work or warn a user. Wants `Option<f32>` on
   `BatteryInfo` and a `push_opt` in the resolver; both are breaking, which is
   the only reason it is here rather than fixed. **No non-breaking mitigation
   exists** — the `Option` is destroyed at construction, so nothing downstream
   can recover it.

5. **`cpufreq::is_turbo` returns `bool` where it means "cannot tell".** With no
   base frequency it guesses turbo from `current > 95% of max`; with neither base
   nor max it returns `false`, so "unknown" and "turbo is off" are the same
   answer. Wants `Option<bool>`, which is breaking. It has no callers today,
   which is the only reason it was left — a `false` nobody reads misleads nobody,
   and 6.0.0 had been tagged an hour earlier. Fix it before something calls it.

6. **Historical metrics for the agent surface.** `get_historical_data` and
   `compare_metrics` were advertised in the tool catalogue — full descriptions,
   worked examples, `minutes_ago` parameters — and neither was ever implemented.
   An agent that read the catalogue and called one got "Unknown tool", which it
   would read as its own mistake. They were removed rather than stubbed, because
   a stub is the same lie with more code. Building them for real needs three
   things: timestamps on `backend::HistoryBuffer` (it is a bare `VecDeque` today,
   so "five minutes ago" is not answerable from it), a handle to that buffer from
   `AiDataApi` (which holds only `historical_context: Option<String>`, a blob a
   caller sets), and a decision about what happens when the window does not reach
   back as far as the question — which is an absence with a reason, not an
   interpolation.

**Both were found by the fallback grep, and it is nowhere near exhausted:** 780
`unwrap_or(0|false|…)` / `unwrap_or_default()` sites outside tests and the GUI.
Most are legitimate. Triage by consequence rather than by count — a fallback in a
reader path that reaches the ontology or the agent tools is worth reading; one in
a filter predicate is not. **Triaging by blast radius rather than in file order
is what found the agent-surface defects:** a `0` in a TUI gauge is cosmetic, a
`0` handed to an LLM becomes a premise it reasons from. The Linux `/proc` parse
sites in `system_stats.rs` are deliberately untouched — they cannot be run or
observed here, and reworking unrunnable readers is the debt this project already
carries.

**The agent tool surface had no test at all until `tests/ai_tool_surface.rs`.**
That is why three fabrications lived in `ai_api/tools.rs` undisturbed. The suite
asserts about shape rather than about particular hardware, so it says the same
thing on a desktop and on a CI runner with no GPU. It found the two phantom tools
above on its first run.

## The plan for what is left

Items A–D are blocked on hardware this project does not have, and are
*verification* tasks with concrete recipes. E and F are ordinary software work
that anyone can pick up on any machine.

(This section previously opened by claiming everything left was hardware-blocked.
That was true when it was written and stopped being true one release later, which
is worth noticing: the sentence survived a commit that invalidated it.)

**A. Verify the ATA path against a real SATA drive.** *Needs: any SATA SSD or
HDD, on Windows. Half an hour.* **This is now retrospective.** The
recommendation was to hold the release until this check had been made; the
decision was to publish anyway, so 3.3.0 is on crates.io with the ATA parse
unverified. That raises the stakes rather than removing them: a defect found here
is a yank or a 3.3.1, not an unreleased fix.

1. Attach the drive — internal, or a USB-SATA enclosure whose bridge tunnels
   SMART (JMicron and ASMedia generally do; the cheapest ones do not).
2. `cargo run --example disk_monitor`, and `smartctl -A /dev/sdX` for the same
   drive.
3. Compare, in this order of importance: reallocated (5), pending (197) and
   uncorrectable (198) sector counts, since those are what `health()` grades on
   and what no other Windows path can reach; then temperature; then power-on
   hours.
4. If nothing appears at all, the parse refused the structure. The likely cause
   is the checksum — `AtaSmartData::parse` declines a structure whose final byte
   does not make all 512 sum to zero. Dump `VendorSpecific` and check by hand
   before loosening it, and if it must be loosened, note that smartmontools
   warns-and-continues here and this deliberately does not.
5. Watch power-on hours specifically. A minority of drives report that attribute
   in minutes rather than hours and nothing in the structure says which; if the
   number is 60× what it should be, that is what happened, and it is a per-vendor
   quirk table, not a bug in the parse.

Done when a real drive's attributes match `smartctl -A`. If they do not, the fix
ships as 3.3.1; if the readings are actively wrong rather than merely absent,
yank 3.3.0, because a wrong SMART attribute is the class of error that gets acted
on.

**B. Establish what macOS power and temperature can reach unelevated.** *Needs: a
Mac. An hour, before any code.*

This is research, not implementation, and doing it first is the point.
`powermetrics` requires root, which would make the obvious implementation
unusable in the same way `IOCTL_ATA_PASS_THROUGH` would have been — and the ATA
work is the argument for checking first. Run, as a normal user:

- `powermetrics -n1 --samplers cpu_power` — expected to fail; confirm it does.
- `ioreg -rc AppleSmartBattery` and `ioreg -rc IOPMPowerSource` for power.
- `ioreg -rc AppleSMC` and the `SMC` keys for temperature.
- On Apple silicon, `IOHIDEventSystemClient` temperature sensors, which are
  readable unelevated where SMC keys are not.

Write down which of these answer without root *before* writing a reader. If none
do, the honest outcome is a documented `NotSupported`, not an implementation that
only works under `sudo`.

**C. Validate the macOS readers that already exist.** *Needs: a Mac. Twenty
minutes.* Compare `Simon::cpu()` and `Simon::memory()` against Activity Monitor.
`tests/macos_readers.rs` checks readings are *plausible*, not *correct*; the
specific thing in doubt is whether the `vm_stat` accounting matches what a user
sees under "Memory Used", which is a different figure from free-plus-inactive.

**D. Run the Linux SMART/NVMe paths on real hardware.** *Needs: a Linux box with
an NVMe drive. Twenty minutes.* `cargo run --example disk_monitor` against
`nvme list` and `smartctl -A`. The sysfs paths are documented kernel ABI and the
code has passed CI, but CI has no drive. The quadratic collector shape fixed in
3.3.0 was worst on Linux, so this is also the first real check of that.

**E.** ~~Read PCIe link state on Windows.~~ Done in 3.6.0; see open work 7 for the
two routes that do not work and why.

**F. Continue the ontology sweep.** *Needs: nothing. Ongoing.* NUMA,
virtualization and EDAC landed in 3.7.0. **RAPL landed in 6.0.0** —
`power.rapl.{n}.{name,energy,max_energy_range,power_limit,enabled}` plus a
`power.rapl.<none>` diagnostic.

Two things that sweep turned up, both worth repeating for the next cluster:

- The reader returned `Ok(vec![])` on Windows and macOS with a comment
  explaining why there was nothing. An empty list is what a Linux box with all
  zones disabled returns, so the reason lived in a comment where no caller could
  reach it. It is now an error carrying the reason, and the resolver turns that
  into `power.rapl.<none>` with the text.
- The `<none>` diagnostic is declared per *domain*, and RAPL is a cluster inside
  `power`. Reusing `power.<none>` would have claimed the whole domain enumerated
  nothing on a machine whose battery reads fine, so the sub-cluster gets its own
  declared diagnostic. Expect the same for the next cluster inside an existing
  domain.

**Sensors landed too**, as `board.sensor.{n}.*` with a `board.sensor.<none>`
diagnostic. It was left out of the first pass on the grounds that the reader
returns nothing on this machine — which was inconsistent, because RAPL returns
nothing here either and was declared anyway. A cluster that resolves to
`unavailable` carrying a true reason *does* satisfy the standing instruction;
what it must not do is resolve to silence.

Doing it turned up two defects in that reader. Every failure path collapsed to
the same empty list — a missing powershell, non-UTF-8 output, unparseable JSON
and a machine with genuinely no sensors were indistinguishable — so `note()` now
records which. And an unnamed sensor was being called `"Unknown"`, a literal that
reads as the sensor's actual name; those are skipped instead.

Item F is complete for every cluster **the plan named** — NUMA, RAPL, sensors,
virtualization and EDAC. That is not the same as every reader having a schema,
and an earlier version of this line said so, which was an overclaim.

### Probe before declaring: `examples/probe_readers.rs`

Adding clusters one at a time and guessing which readers answer was slow and got
one of them wrong. The example constructs every uncovered monitor and prints
what each produced, which turns "which cluster next" into a table. On this
Windows desktop:

| Answers here | Silent here |
|---|---|
| `input` 6, `services` 311, `storage_controller` 9, `power_profile` 3, `audio` 12, `bluetooth` 2, `camera` 2, `codec` 15, `printer` 4, `memory_bandwidth` 1, `memory_topology` 2, `cpu_microarch` 8, `crypto_accel` 11, `interconnect` 1 | `iommu`, `interrupt_map`, `io_scheduler`, `dma_engine`, `gpu_topology`, `thermal_zone`, `voltage_regulator`, `watchdog`, `security_mitigations`, `kernel_params` |

The silent column is almost entirely Linux sysfs. Those are not blocked in the
way the hardware items A–D are — a Linux box would let all nine be verified in
one sitting, and item F's remaining work is mostly that.

**Read the silent column carefully, because it was over-read once already.** The
probe originally printed `ok 0` for those nine, which says only that the monitor
constructed and enumerated nothing — not that this machine lacks the hardware.
This desktop certainly has an IOMMU and thermal zones. The table now prints
`none` for that case and `ok` only when something was enumerated, but the
underlying readers still return `Ok(vec![])` rather than an error carrying a
reason, so `none` remains ambiguous between "absent here" and "unimplemented
here". Establishing which, per reader, is the first step of that Linux sitting —
and giving each an error with a reason, as RAPL got, is the fix.

### Clusters added in the uncommitted batch

`memory.bandwidth.*`, `cpu.microarch.*`, `cpu.crypto.*` (features and RNG
sources), `board.input.*`, `board.audio.*`, `board.camera.*`, `system.printer.*`,
`network.bluetooth.*`, `disk.controller.*`, `power.profile.*`, `gpu.codec.*`.
Each has its own `<none>` diagnostic, per the sub-cluster rule above.

Three judgement calls in there worth preserving, because each is a place where
publishing *more* would have been publishing worse:

- **Scores were left out on purpose.** `cpu_microarch` computes a 0–100
  `single_thread_score` and `crypto_accel` an `acceleration_score`. Both are
  table lookups keyed on a name, not measurements of the processor in front of
  you. There is no provenance that fits: `Measured` lies about the method and
  `Derived` lies about the inputs. An agent choosing between machines on a
  number called a performance score would be trusting a guess wearing a
  benchmark's authority. The extension list beside it is the part that is a
  fact, and facts are what got published.
- **`gpu.codec.*` picks its provenance per row**, and it is the only cluster
  that does. The reader records whether it asked the driver (`DirectQuery`) or
  concluded the capability from the GPU model, so a queried row resolves
  `Measured` and an inferred one `Specification`. This is the clearest
  illustration in the crate of why the field exists — the two look identical
  until you ask, and only one survives a driver update.
- **Serial numbers and Bluetooth MACs are readable and are not published.** They
  identify a unit rather than describe it, no agent task needs one, and a
  hardware report carrying them is harder to share than one that does not.

### Five defects the batch turned up, all found by reading the output

None of these were found by a test. Every one came from running
`simon snapshot --format text` and looking at the rows.

1. **The existing `memory.dimm.*` cluster declared SMBIOS data as `Measured`.**
   A part number was not measured off the module — the firmware said so. Now
   `Specification` throughout, which is what that provenance is for. A board
   that lies about its own DIMMs makes simon repeat the lie, and a consumer
   deciding whether to trust the figure needs to know that before it decides.
2. **`cpu.microarch.name` emitted `{:?}` of an entire struct** — braces, nested
   quotes, `None` — into a reading declared as an identifier. It parsed as a
   value and was one only in the sense that a screenshot of a table is a table.
   Now split into `name`, `codename`, `vendor`, `isa`, `process`, `year`,
   `hybrid`.
3. **CPUID family/model/stepping published `0` as a reading** on Windows, where
   the triple is never decoded. Family 0 identifies no x86 part that has shipped,
   so it means "not read". All three now go together or not at all: stepping 0 is
   a legitimate value, and publishing it while family is absent would present a
   default as a measurement.
4. **The Windows camera reader enumerated scanners.** Its WMI query accepted
   PnP class `Image` as well as `Camera`, so a Brother MFC-L2900DW appeared as
   two cameras. That is not a slightly generous enumeration; it is a wrong answer
   to "can this machine see". Windows 10 1703 and later put every webcam under
   `Camera`, so narrowing the query loses nothing real.
5. **`disk.controller.pci_address` published device instance paths.** Windows
   returns things like `ROOT\SPACEPORT\0000`, which is not a PCI address and
   cannot be joined against `pci.*`. A consumer looking for a link width would
   have been sent to a bus the device is not on. `looks_like_pci_address` now
   gates it, and the absence names what was actually found.

That is five defects in one afternoon from one habit. **The conformance tests
catch what a value *is* — null, wrong JSON type, an absence with no reason. They
cannot catch what it *means*.** A debug dump is a non-empty string; a scanner is
a plausible camera name; a device path is a plausible identifier; zero is a
plausible integer. Read the rows.

### What is left

**`services` is done** — counts plus named failures, not an enumeration. See
`2687dbb`. **The nine silent readers now decline with reasons** rather than
returning an empty list; see `ebab956`. Verifying what they *read* still needs a
Linux box, but the Windows behaviour is now honest and was verified here.

Two readers remain, and neither is the simple schema job the old wording implied:

- **`interconnect` ships nothing, deliberately.** Decided rather than deferred.
  Its numbers come from `name.contains("GRANITE")` against a table of constants,
  and no provenance fits: `Measured` lies about the method, `Specification` lies
  about the inputs, because a spec sheet describes a part rather than a guess
  keyed on its marketing name. Publishing only the structural fields would be
  worse than publishing nothing — a legitimate-looking `type` and `generation`
  lend credibility to a cluster whose numbers are invented.
- **`kernel_params` is done**, as a split rather than a subset — `name`,
  `value`, `category` published; the scores and recommendations left to the
  tuning surface. See `014e4dd`.

  **It also found a guess inside the probe table.** `kernel_params` was counted
  by `|_| 1` — a literal — so the table reported "answers here, 1 item" for a
  reader that answers nothing on this machine, and the count propagated into the
  table above. The `Get-NetTCPSetting` cmdlet it depends on *fails* here, and the
  reader was pushing a parameter with an empty value regardless.

  That table exists because guessing which readers answer "was slow and got one
  of them wrong". Worth keeping in mind before trusting it again: two other
  readers had the same hardcoded count. **A measurement nobody checks is a
  constant, and a constant carries no information** — the same sentence this file
  already applies to a permanently-red pipeline, one more level down.

**Two of those three are traps, and "answers here" is what makes them look
safe.** The probe table says a reader produced rows; it does not say the rows
are readings. Both were checked before declaring anything, and neither is ready:

- **`interconnect` measures nothing at all.** `InterconnectMonitor::infer_topology`
  is a string match on the CPU marketing name — `name.contains("GRANITE")` —
  returning hardcoded constants for link type, width, `speed_gts` and
  coherence protocol, with `bandwidth_gbs` arithmetic over those constants and
  `latency_ns` documented in its own field comment as "estimated". Not one
  value comes from the hardware. It is the same defect as the withheld
  `single_thread_score`, except a field called `bandwidth_gbs` reads as a
  measurement in a way a field called `score` does not. If any of it is ever
  declared, the structural facts (type, generation, coherence protocol) are
  `Specification` at best, `sockets` and `is_numa` are the only genuinely
  measured fields, and the bandwidth and latency numbers should not be
  published in any provenance — there is no honest one.
- **`kernel_params` is half fact and half opinion.** `name` and `value` are real
  sysctl reads. `is_recommended`, `recommended`, `security_score`,
  `network_score` and `recommendations` are this crate's judgement about what
  the value ought to be — which is precisely what `simon tune`'s standing rule
  forbids publishing as fact ("a proposed value comes from what the driver
  declared, never from this crate"). Declare the first two; the rest belong to
  the tuning surface if anywhere.

The generalisable point, since this is the third time the same shape has come
up in this file: **a reader that returns rows on this machine has cleared the
availability bar and nothing else.** Whether each field is a reading is a
separate question, answered by reading the reader, and a plausible unit is not
evidence — `bandwidth_gbs`, `latency_ns` and `single_thread_score` all look
exactly like measurements at the call site.

Still uncovered and not probed: `wsl`, `drm_monitor`, `hardware_ai`, `scheduler`.

Add a cluster per change, verify each field resolves to a true provenance on the
machine at hand, and check the absent variant first — see open work 5 for why
that is the standing instruction.

**Deliberately not planned:** SMART failure thresholds on Windows. They come from
SMART READ THRESHOLDS, which has no `IOCTL_STORAGE_*` equivalent, so the only
route is an elevated pass-through — giving back exactly what 3.3.0 bought. The
drive's own `PredictFailure` verdict is the same judgement the thresholds would
have produced, and is already reported.

## How the ontology is put together

Three layers, added in 6.0.0, each answering a different question. Knowing which
layer a change belongs in saves rediscovering the split:

- **`Entity`** (`src/ontology/mod.rs`) — *what a value means.* Id, domain, kind,
  unit, declared provenance, nullability, prose. Templated ids use `{n}`.
- **`Capability`** (`src/ontology/capability.rs`) — *whether simon can produce it
  on this platform, on this build.* `Support` is `Implemented`, `Partial`,
  `Unimplemented` or `Unverified`, and the last three carry a reason. This is the
  layer that stops the docs from claiming a feature the build does not have.
- **`Vocabulary`** (`src/ontology/vocabulary.rs`) — *what values are possible.*
  Closed sets for provenance, verdict, severity, scan status and the rest, so an
  agent can enumerate the range without parsing prose.

`src/ontology/resolve.rs` produces `Reading`s against the entities;
`src/ontology/jsonld.rs` maps the result to JSON-LD with QUDT units, mapped only
where an exact equivalent exists rather than approximated.

**Write conformance tests against invariants, not against today's state.** Three
tests here had to be rewritten because they asserted a fact that was true when
written: one required every reading marked usable to actually resolve, which
failed on a GPU-less runner and again on a battery-less one; another asserted
that *all* detection capabilities were stranded without commands, and broke the
moment detection gained one. The durable forms are "all-or-nothing" and "these
two counts agree" — properties that survive the code getting better. A test that
fails when a gap is *closed* is a test that punishes progress.

## Guardrails, and why each exists

| Test | Catches |
|---|---|
| `tests/manifest_portability.rs` | A feature depending on a target-gated crate — the bug that meant the crate never built on Linux or macOS |
| CI job *Feature combinations* | A feature that does not build in isolation — found `cargo install` broken on Linux on its first run |
| `tests/macos_readers.rs` | macOS readings that are not plausible readings; runs on `macos-latest`, which is the only Mac this project has |
| `tests/documentation_links.rs` | Broken relative links; machine identifiers in docs; documented `simon …` commands that do not exist |
| `smart::tests::a_drive_with_no_readable_counters_is_not_graded_healthy` | Health graded from an empty scorecard |
| `smart::tests::a_self_reported_failure_survives_inference` | A drive predicting its own failure being scored back to `Good` from clean-looking counters |
| `disk::ata_smart::tests` | ATA structure misparses: bad checksum accepted, zero-filled buffer read as a clean drive, temperature taken from the full 48-bit raw, table truncated at the first gap |
| `tests/plausibility.rs` | Physically impossible readings |
| `tests/agentic_contract.rs` | Schema/resolver disagreement on the agent surface |
| `tests/ontology_conformance.rs` | Anything a new entity can get wrong: unreachable ids, absences with no reason, non-nullable nulls, values whose JSON type contradicts their unit, `Unknown` dressed as a measurement. Derives its cases from the ontology, so a domain added later is covered without editing it |
| `tests/capability_conformance.rs` | A declared capability with no command behind it, or a surface whose handler count and declaration disagree. Checked in both directions, so an implementation with no declaration fails too |
| `tests/vocabulary_conformance.rs` | A closed value set drifting from the enum it describes. Caught `Provenance::Reported` — a variant that does not exist — being written into a resolver twice |
| `tests/honesty.rs` | Absences without usable reasons, and readings claiming a provenance the resolver did not actually have |
| `tests/zero_constructors.rs` | A `new()` that fabricates a reading instead of constructing from the system. The list is empty and must stay empty |

**One gap in that table, found by reading declarations rather than by a test.**
Nothing compares a *reading's* provenance against its *entity's declared*
provenance. `gpu.codec.*` resolves per row — `Measured` where the driver was
queried, `Specification` where the capability was inferred from the GPU model —
while all five entities declared `Measured`, so the schema promised a live
observation for values that had never been observed. Fixed in `b2dd07b` by
declaring the weakest provenance a row can carry, which is the rule `Derived`
already follows for its inputs.

The class is still uncaught. `Entity` holds one provenance and cannot say
"varies per row", and a `Varies` variant is not the answer — it would poison
`is_observation()`, the guard an agent applies before treating a value as fact,
for every consumer of every entity. The test worth writing asserts the safe
direction rather than equality: **a reading may resolve stronger than its
declaration, never weaker.** `honesty.rs` is where it belongs.

**A flaky test is the same instrument as a red pipeline.**
`profile::cache::tests::invalidate_forces_refresh` asserted on `CACHE_STATS`, a
process-global, while a sibling test reset it — so it failed on thread
scheduling, passed single-threaded, passed three times in isolation, and only
failed under the full 825-test parallel load. That profile is exactly what makes
one get dismissed as noise, and this project has already lost time to
"an intermittent failure at three runs in five". Fixed in `2e6f9e9`. When a test
touches a global, every test in the module that moves that global has to
serialise, including the ones that assert nothing about it.

Each was checked against a deliberate break before being kept, because a test
that cannot fail is worse than none.

**A green suite is evidence about the surface it covers, and nothing else.**
The Dewey port passed 839 tests through four releases while its window opened at
800x600 titled "Dewey App", clipped every value off the right edge, responded to
no clicks at all, and reported no GPUs on a machine with three. The tests
asserted that each tab emitted named ontology nodes; the port did that
faithfully. Nothing failed because nothing was asked. The bugs were found by
screenshotting the window and looking at it — and each round of looking found
another one. If a change affects something a person sees, look at it before
reporting it done.

## Two things worth knowing

**Don't run `cargo clippy --fix` per target in this crate.** Fixing for macOS
deleted `mut` and imports that only Linux and Windows need, silently breaking
both (21 and 14 errors). A `cfg` block that is empty on the target being fixed
makes bindings look unused. Six files had to be reverted.

**Whether a Windows ioctl needs elevation is written in its `CTL_CODE`.** The
access bits in the definition, and the mask the handle was opened with, decide it
— not how privileged the operation sounds. Two capabilities were scoped as
"needs Administrator" on the second reading and turned out not to (NVMe in 3.1.0,
ATA in 3.3.0), and one was scoped as reachable and is not
(`IOCTL_ATA_PASS_THROUGH`). Reading the `CTL_CODE` first would have settled all
three in a minute each.

**The absence-word guard lives in one place, and that is why it only had to be
written once.** `push_text`, `push_id` and `push_opt` in `resolve.rs` all funnel
through `push_str_as`, which rejects empty strings and the words "unknown",
"unspecified", "undetermined", "n/a" and "none". An enum's `Unknown` variant
lowercased into a measured identifier is the single most repeated defect in this
crate — it has been caught in five entities, in three different readers, across
three releases. Push string readings through the helpers, never
`Reading::measured` directly.

**A manual grep is not a substitute for asking the binary.** A hand sweep for
documented-but-nonexistent commands missed 29 of 38 cases; a test comparing docs
against `simon describe --commands` found them all, including a `CLI.md` at the
repo root nobody had thought to include. The same lesson produced the feature
sweep above: the `cli` breakage was found by running every combination, not by
reading the manifest.
