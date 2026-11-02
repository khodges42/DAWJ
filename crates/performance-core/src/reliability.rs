// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in non-realtime test utilities; never used by the audio callback.
/// Fail closed: automated playback must never silently choose another output.
pub fn test_output(names: &[String], requested: &str) -> Result<usize, String> {
    let matches: Vec<_> = names
        .iter()
        .enumerate()
        .filter(|(_, name)| name.to_lowercase().contains(&requested.to_lowercase()))
        .collect();
    match matches.as_slice() {
        [(index, _)] if !requested.trim().is_empty() => Ok(*index),
        _ => Err(format!(
            "Test output {requested:?} missing or ambiguous: {names:?}"
        )),
    }
}
/// Fixed memory, 0.1 ms histogram. Quantiles above 1000 ms are lower bounds.
pub struct Timing {
    bins: Box<[u64]>,
    n: u64,
    max: f64,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            bins: vec![0; 10001].into_boxed_slice(),
            n: 0,
            max: 0.0,
        }
    }
}
impl Timing {
    pub fn record(&mut self, ms: f64) {
        if ms.is_finite() && ms >= 0.0 {
            self.n += 1;
            self.max = self.max.max(ms);
            self.bins[((ms * 10.0).ceil() as usize).min(10000)] += 1;
        }
    }
    pub fn summary(&self) -> serde_json::Value {
        let percentile = |p: f64| {
            let target = (self.n as f64 * p).ceil() as u64;
            let mut total = 0;
            for (i, n) in self.bins.iter().enumerate() {
                total += n;
                if total >= target {
                    return i as f64 / 10.0;
                }
            }
            0.0
        };
        serde_json::json!({"n":self.n,"median":percentile(0.5),"p95":percentile(0.95),"worst":self.max})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_selection_never_falls_back() {
        let names = vec!["Sim rig".into(), "Headphones (HyperX)".into()];
        assert_eq!(test_output(&names, "hyperx").unwrap(), 1);
        for name in ["", "absent"] {
            assert!(test_output(&names, name).is_err())
        }
        assert!(test_output(&["HyperX one".into(), "HyperX two".into()], "hyperx").is_err());
    }
    #[test]
    fn timing_is_bounded_and_retains_outliers() {
        let mut t = Timing::default();
        for n in 1..=100 {
            t.record(f64::from(n));
        }
        t.record(f64::NAN);
        let s = t.summary();
        assert_eq!(s["median"], 50.0);
        assert_eq!(s["p95"], 95.0);
        t.record(2000.0);
        assert_eq!(t.summary()["worst"], 2000.0);
        assert_eq!(t.bins.len(), 10001);
    }
}
