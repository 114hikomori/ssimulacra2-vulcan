// Phase H / H0: full-process stage timing, host-side only. Disabled by default:
// `time` is a bare pass-through then, so no timing behavior enters normal runs.
// The report carries a RESIDUAL line (wall minus stage sum) precisely so that no
// phase of the process can hide unmeasured (the dssim-vulkan "host prep"
// hypothesis must be testable against 100% of wall time, not a stage subset).
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Profile {
    on: bool,
    map: BTreeMap<&'static str, (Duration, u32)>,
}

impl Profile {
    pub fn enabled(on: bool) -> Self {
        Self {
            on,
            ..Default::default()
        }
    }

    pub fn time<T>(&mut self, name: &'static str, f: impl FnOnce() -> T) -> T {
        if !self.on {
            return f();
        }
        let t = Instant::now();
        let r = f();
        self.add(name, t.elapsed());
        r
    }

    pub fn add(&mut self, name: &'static str, d: Duration) {
        if !self.on {
            return;
        }
        let e = self.map.entry(name).or_insert((Duration::ZERO, 0));
        e.0 += d;
        e.1 += 1;
    }

    pub fn report(&self, wall: Duration) {
        if !self.on {
            return;
        }
        let mut sum = Duration::ZERO;
        eprintln!("profile stages:");
        for (k, (d, c)) in &self.map {
            sum += *d;
            eprintln!("  {k:<16} {:>9.3} ms  x{c}", d.as_secs_f64() * 1e3);
        }
        let w = wall.as_secs_f64() * 1e3;
        let s = sum.as_secs_f64() * 1e3;
        eprintln!("  {:<16} {:>9.3} ms", "SUM", s);
        let res = w - s;
        eprintln!(
            "  RESIDUAL      {:>9.3} ms  ({:.1}% of wall {w:.1} ms)",
            res,
            res / w * 100.0
        );
    }
}
