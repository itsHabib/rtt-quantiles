use tdigest::TDigest;

type RttMicros = u32;

pub struct Summary {
    digest: TDigest,
    count: u64,
}

impl Summary {
    /// Return a RttSummary with 100 centroids
    pub fn new() -> Self {
        Summary {
            digest: TDigest::new_with_size(100),
            count: 0,
        }
    }

    /// Add a rtt measurement to the digest
    pub fn add_rtt(&mut self, rtt: RttMicros) {
        let rtt_ms = rtt as f64 / 1000.0;
        self.digest = self.digest.merge_unsorted(vec![rtt_ms]);
        self.count += 1;
    }

    pub fn digest(&self) -> TDigest {
        self.digest.clone()
    }

    /// Returns the p99 quantile of the given rtt samples
    pub fn p99(&self) -> f64 {
        self.quantile(0.99)
    }

    /// Returns the p95 quantile of the given rtt samples
    pub fn p95(&self) -> f64 {
        self.quantile(0.95)
    }

    /// Returns the p90 quantile of the given rtt samples
    pub fn p90(&self) -> f64 {
        self.quantile(0.90)
    }

    /// Returns the p75 quantile of the given rtt samples
    pub fn p75(&self) -> f64 {
        self.quantile(0.75)
    }

    /// Returns the p50 quantile of the given rtt samples
    pub fn p50(&self) -> f64 {
        self.quantile(0.50)
    }

    /// Return a quantile 0.0->1.0 e.g. p99 (0.99), p90 (.90)
    fn quantile(&self, q: f64) -> f64 {
        self.digest.estimate_quantile(q)
    }

    /// Get the number of samples added
    pub fn count(&self) -> u64 {
        self.count
    }
}

impl Default for Summary {
    fn default() -> Self {
        Self::new()
    }
}
