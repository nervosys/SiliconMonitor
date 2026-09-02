//! Per-core delivered performance, from the PDH performance counters.
//!
//! `CallNtPowerInformation`'s `CurrentMhz` is the nominal clock on Windows 10
//! and later — it reads the same value for every core, idle and under load, so
//! `platform::windows::get_cpu_frequency` reports no current clock rather than
//! publishing a specification behind a measurement's provenance. That left
//! `cpu.core.{n}.frequency` absent on every Windows machine.
//!
//! `\Processor Information(*)\% Processor Performance` is what Task Manager
//! reads. It is derived by the kernel from the APERF and MPERF counters, so it
//! reflects clock actually delivered, it differs per core, and it exceeds 100%
//! when a core boosts — on the development machine it ranges from 60% to 124%
//! of a 4400 MHz nominal, which is a real 2.6–5.4 GHz spread.
//!
//! **It is a rate, so it needs two collections separated in time.** That is the
//! whole reason this was deferred rather than written inline: a single call
//! cannot produce it without sleeping, and sleeping in a monitor's refresh path
//! is not acceptable. So the query is opened once and kept, and each call
//! collects against the previous collection. **The first call after opening
//! returns nothing**, which is the same contract the NPU utilization reader
//! already has, and for the same reason.

use std::sync::{Mutex, OnceLock};

use windows::core::w;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
};

/// An open PDH query and the counter within it.
///
/// The handles are process-wide integers rather than pointers, but
/// `PdhCollectQueryData` is not safe to call concurrently against one query, so
/// the whole thing lives behind a mutex.
struct PerformanceQuery {
    query: isize,
    counter: isize,
    /// Whether a first collection has happened. Until it has, there is no
    /// interval to compute a rate over.
    primed: bool,
}

// SAFETY: `query` and `counter` are PDH handles, which are process-wide and not
// bound to the opening thread. Every use is serialised by the mutex below.
unsafe impl Send for PerformanceQuery {}

static QUERY: OnceLock<Option<Mutex<PerformanceQuery>>> = OnceLock::new();

fn query() -> Option<&'static Mutex<PerformanceQuery>> {
    QUERY
        .get_or_init(|| {
            let mut query: isize = 0;
            if unsafe { PdhOpenQueryW(None, 0, &mut query) } != 0 {
                return None;
            }
            let mut counter: isize = 0;
            // The English name is used explicitly: the localised counter path
            // differs by system language, and `PdhAddCounterW` would fail on a
            // machine that is not running in English.
            let added = unsafe {
                PdhAddEnglishCounterW(
                    query,
                    w!("\\Processor Information(*)\\% Processor Performance"),
                    0,
                    &mut counter,
                )
            };
            if added != 0 {
                return None;
            }
            Some(Mutex::new(PerformanceQuery {
                query,
                counter,
                primed: false,
            }))
        })
        .as_ref()
}

/// Delivered performance per logical processor, as a percentage of nominal.
///
/// Indexed by the processor number PDH reports, which is `"<group>,<cpu>"` —
/// the `_Total` rows and any per-group totals are skipped. Returns `None` when
/// the counter is unavailable, and on the first call, which only primes the
/// interval.
pub(crate) fn processor_performance_percent() -> Option<Vec<Option<f64>>> {
    let lock = query()?;
    let mut state = lock.lock().ok()?;

    if unsafe { PdhCollectQueryData(state.query) } != 0 {
        return None;
    }
    if !state.primed {
        // No previous sample to difference against. Reporting anything here
        // would be reporting the counter's raw value as though it were a rate.
        state.primed = true;
        return None;
    }

    let mut size: u32 = 0;
    let mut count: u32 = 0;
    // The first call reports the buffer size it needs and returns
    // PDH_MORE_DATA; anything else is a real failure.
    unsafe {
        PdhGetFormattedCounterArrayW(state.counter, PDH_FMT_DOUBLE, &mut size, &mut count, None)
    };
    if size == 0 {
        return None;
    }

    let mut buffer = vec![0u8; size as usize];
    let fetched = unsafe {
        PdhGetFormattedCounterArrayW(
            state.counter,
            PDH_FMT_DOUBLE,
            &mut size,
            &mut count,
            Some(buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W),
        )
    };
    if fetched != 0 {
        return None;
    }

    let items = unsafe {
        std::slice::from_raw_parts(
            buffer.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W,
            count as usize,
        )
    };

    let mut out: Vec<Option<f64>> = Vec::new();
    for item in items {
        let name = unsafe { item.szName.to_string() }.unwrap_or_default();
        // "0,5" is group 0 processor 5. "_Total" and "0,_Total" are aggregates.
        let Some((_group, cpu)) = name.split_once(',') else {
            continue;
        };
        let Ok(index) = cpu.parse::<usize>() else {
            continue;
        };
        if out.len() <= index {
            out.resize(index + 1, None);
        }
        out[index] = Some(unsafe { item.FmtValue.Anonymous.doubleValue });
    }

    (!out.is_empty()).then_some(out)
}
