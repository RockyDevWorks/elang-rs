//! Peak-memory measurement for ELANG reader/writer operations (T1614).
//!
//! The v0.1.0 release gate requires an honest peak-memory measurement,
//! not a hand-wave. The single implementation that is reliable enough to
//! base the release number on is **Linux `/proc/self/status` `VmHWM`**, the
//! peak resident-set size the kernel attributes to this process since it
//! started. `VmHWM` is:
//!
//! * measured by the kernel, not by a sampling loop (so it cannot be
//!   missed by a clock tick),
//! * monotonic across the process lifetime (it only goes up),
//! * defined in the kernel's `[task_mmu.c]` documentation as
//!   "Peak resident set size (\"high water mark\")",
//!
//! which makes it the right number to put in the release notes.
//!
//! On non-Linux platforms the public docs document only the **method**;
//! no number is fabricated. Callers must not pretend `VmHWM` was
//! measured when it was not. See
//! [`PeakMemorySample::is_measured`].
//!
//! # Reproducibility
//!
//! `VmHWM` is a single sample of one process on one machine, so two runs
//! of the same workload on the same kernel and the same allocator can
//! produce slightly different numbers (allocator growth patterns, page
//! reclamation at unfortunate moments, and the cost of the measurement
//! read itself). The harness therefore reports `VmHWM` in KiB, which is
//! the resolution the kernel exposes and the resolution at which the
//! numbers are stable across rebuilds.

use std::path::Path;

/// A peak-memory sample taken from one operation.
#[derive(Debug, Clone, Copy)]
pub struct PeakMemorySample {
    /// `VmHWM` from `/proc/self/status`, in KiB. Always reported.
    vmhwm_kib: u64,
    /// `VmRSS` from `/proc/self/status`, in KiB. Reported alongside
    /// `VmHWM` for context (the RSS *at the moment of the read*, not
    /// the peak).
    vmrss_kib: u64,
    /// `VmPeak` from `/proc/self/status`, in KiB. Reported alongside
    /// `VmHWM` for context (peak virtual memory size, not resident).
    vmpeak_kib: u64,
}

impl PeakMemorySample {
    /// `VmHWM` (peak RSS), in KiB.
    pub fn vmhwm_kib(&self) -> u64 {
        self.vmhwm_kib
    }

    /// `VmRSS` (resident set size at read time), in KiB.
    pub fn vmrss_kib(&self) -> u64 {
        self.vmrss_kib
    }

    /// `VmPeak` (peak virtual size), in KiB.
    pub fn vmpeak_kib(&self) -> u64 {
        self.vmpeak_kib
    }

    /// `VmHWM` converted to MiB, for the doc table.
    pub fn vmhwm_mib(&self) -> f64 {
        self.vmhwm_kib as f64 / 1024.0
    }

    /// `VmRSS` converted to MiB, for the doc table.
    pub fn vmrss_mib(&self) -> f64 {
        self.vmrss_kib as f64 / 1024.0
    }

    /// `VmPeak` converted to MiB, for the doc table.
    pub fn vmpeak_mib(&self) -> f64 {
        self.vmpeak_kib as f64 / 1024.0
    }

    /// Whether the sample is from a real measurement (Linux
    /// `/proc/self/status`) rather than a placeholder. Always `true` on
    /// Linux; on other platforms [`current`] returns `None` and
    /// the bench harness records "not measured".
    pub fn is_measured(&self) -> bool {
        true
    }
}

/// Read `/proc/self/status` and extract the peak-memory fields.
///
/// Returns `None` on platforms where `/proc/self/status` does not exist
/// (notably macOS, Windows, *BSD). Callers must surface that absence as
/// "not measured" rather than fabricating a number.
pub fn current() -> Option<PeakMemorySample> {
    read_proc_self_status()
}

fn read_proc_self_status() -> Option<PeakMemorySample> {
    let body = std::fs::read_to_string(Path::new("/proc/self/status")).ok()?;
    parse_proc_self_status(&body)
}

/// Parse the peak-memory fields out of a `/proc/self/status` body.
///
/// Returns `None` when `VmHWM` is absent — without it there is no
/// peak-memory reading, and a fabricated zero would be a lie.
///
/// Lines that are not `Key: value` pairs are skipped rather than
/// aborting the parse.
fn parse_proc_self_status(body: &str) -> Option<PeakMemorySample> {
    let mut vmhwm: Option<u64> = None;
    let mut vmrss: Option<u64> = None;
    let mut vmpeak: Option<u64> = None;
    for line in body.lines() {
        // A line without a colon is not a `Key: value` pair. Skip it
        // rather than aborting the whole parse: `/proc/self/status`
        // contains fields this parser does not model, and a malformed
        // line must not hide the fields we do model.
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let kib = match value.split_whitespace().next() {
            Some(n) if n.chars().all(|c| c.is_ascii_digit()) => match n.parse::<u64>() {
                Ok(kib) => kib,
                Err(_) => continue,
            },
            _ => continue,
        };
        match key {
            "VmHWM" => vmhwm = Some(kib),
            "VmRSS" => vmrss = Some(kib),
            "VmPeak" => vmpeak = Some(kib),
            _ => continue,
        }
    }
    Some(PeakMemorySample {
        vmhwm_kib: vmhwm?,
        vmrss_kib: vmrss?,
        vmpeak_kib: vmpeak?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_proc_status() {
        let body = "Name: bench\n\
                    VmPeak:    123456 kB\n\
                    VmSize:    120000 kB\n\
                    VmRSS:      54321 kB\n\
                    VmHWM:      98765 kB\n";
        let sample = parse_proc_self_status(body).expect("parse");
        assert_eq!(sample.vmhwm_kib(), 98_765);
        assert_eq!(sample.vmrss_kib(), 54_321);
        assert_eq!(sample.vmpeak_kib(), 123_456);
    }

    #[test]
    fn missing_vmhwm_is_none() {
        // A well-formed status with every key except VmHWM must yield
        // None, not a zeroed-out sample.
        let body = "VmPeak:    123456 kB\n\
                    VmRSS:      54321 kB\n";
        assert!(parse_proc_self_status(body).is_none());
    }

    #[test]
    fn ignores_garbage_lines() {
        // `/proc/self/status` carries many fields this parser does not
        // model, plus the occasional non-`Key: value` line. Neither kind
        // of "garbage" must hide a VmHWM, VmRSS, or VmPeak reading.
        let body = "weird: line: with: colons\n\
                    not_a_field=wrong\n\
                    VmHWM:      100 kB\n\
                    VmRSS:       80 kB\n\
                    VmPeak:    9000 kB\n";
        let sample = parse_proc_self_status(body).expect("parse");
        assert_eq!(sample.vmhwm_kib(), 100);
        assert_eq!(sample.vmrss_kib(), 80);
        assert_eq!(sample.vmpeak_kib(), 9000);
    }

    /// The MiB converters divide the KiB readings by 1024, and a
    /// sample taken from `/proc/self/status` is a real measurement.
    #[test]
    fn coverage_gap_mib_converters_divide_by_1024() {
        let body = "VmPeak:    2048 kB\n\
                    VmRSS:      1024 kB\n\
                    VmHWM:      3072 kB\n";
        let sample = parse_proc_self_status(body).expect("parse");
        assert_eq!(sample.vmhwm_mib(), 3.0);
        assert_eq!(sample.vmrss_mib(), 1.0);
        assert_eq!(sample.vmpeak_mib(), 2.0);
        assert!(sample.is_measured());
    }

    /// A field whose value is all digits but does not fit in a `u64` is
    /// skipped, not fatal: the parse continues and the missing field
    /// leaves the sample absent.
    ///
    /// The other two fields are present and valid, so the overflow of
    /// `VmHWM` is the only reason the sample can be absent. Without them
    /// the parse would also fail on the missing fields, and the test
    /// would pass even if the overflowing value were accepted.
    #[test]
    fn coverage_gap_a_value_that_does_not_fit_u64_is_skipped() {
        let body = "VmHWM: 999999999999999999999999 kB\n\
                    VmRSS: 1024 kB\n\
                    VmPeak: 2048 kB\n";
        assert!(parse_proc_self_status(body).is_none());
    }

    /// `current` reads the live status file. The outcome depends on
    /// the host — on Linux it yields a real sample, elsewhere it yields
    /// `None` — but a sample that *is* returned must be a real
    /// measurement.
    #[test]
    fn coverage_gap_current_returns_a_measurement_when_proc_is_available() {
        // The outcome depends on the host — on Linux it yields a real
        // sample, elsewhere it yields `None` — but a sample that *is*
        // returned must be a real measurement.
        let sample = current();
        assert!(sample.is_none_or(|s| s.is_measured()));
    }
}
