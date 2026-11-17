//! Network throughput: interface byte counters in, rates and session totals out.

/// One reading of an interface's cumulative counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Reading {
    /// Bytes since the session baseline.
    pub download_total: u64,
    pub upload_total: u64,
    /// Bytes per second over the last measured interval.
    pub download_rate: f64,
    pub upload_rate: f64,
}

/// Turns successive counter samples into rates. The caller supplies the timestamp from a
/// monotonic clock, which keeps this type free of I/O and lets tests choose the interval.
#[derive(Debug, Clone, Default)]
pub struct ThroughputTracker {
    baseline: Option<Sample>,
    last_rx: u64,
    last_tx: u64,
    last_sample_nanos: Option<u64>,
    reading: Reading,
}

impl ThroughputTracker {
    /// Two samples closer together than this give a rate dominated by timer jitter, so the
    /// previous rate is kept until enough time has passed.
    pub const MINIMUM_INTERVAL_SECONDS: f64 = 0.2;

    pub fn reading(&self) -> Reading {
        self.reading
    }

    fn prime(&mut self, sample: &Sample, now_nanos: u64) {
        // A new session has no interval yet: a rate carried over would be the previous
        // interface's, shown against the new one.
        self.reading.download_rate = 0.0;
        self.reading.upload_rate = 0.0;
        self.baseline = Some(sample.clone());
        self.last_rx = sample.rx_bytes;
        self.last_tx = sample.tx_bytes;
        self.last_sample_nanos = Some(now_nanos);
    }

    /// Feeds the next sample. `None` means there is no active interface: the rates drop to
    /// zero while the session totals are kept.
    pub fn update(&mut self, sample: Option<&Sample>, now_nanos: u64) -> Reading {
        let Some(sample) = sample else {
            self.reading.download_rate = 0.0;
            self.reading.upload_rate = 0.0;
            self.last_sample_nanos = None;
            return self.reading;
        };
        // A different interface, or counters below the baseline (the interface was reset),
        // starts a new session: totals measured against the old baseline would be garbage.
        let restart = match &self.baseline {
            None => true,
            Some(base) => {
                base.interface != sample.interface
                    || sample.rx_bytes < base.rx_bytes
                    || sample.tx_bytes < base.tx_bytes
            }
        };
        if restart {
            self.prime(sample, now_nanos);
        }
        let (base_rx, base_tx) = self
            .baseline
            .as_ref()
            .map_or((sample.rx_bytes, sample.tx_bytes), |b| {
                (b.rx_bytes, b.tx_bytes)
            });
        self.reading.download_total = sample.rx_bytes.saturating_sub(base_rx);
        self.reading.upload_total = sample.tx_bytes.saturating_sub(base_tx);

        let Some(last) = self.last_sample_nanos else {
            self.last_sample_nanos = Some(now_nanos);
            self.last_rx = sample.rx_bytes;
            self.last_tx = sample.tx_bytes;
            return self.reading;
        };
        if now_nanos <= last {
            return self.reading;
        }
        let seconds = (now_nanos - last) as f64 / 1_000_000_000.0;
        if seconds <= Self::MINIMUM_INTERVAL_SECONDS {
            return self.reading;
        }
        self.reading.download_rate = sample.rx_bytes.saturating_sub(self.last_rx) as f64 / seconds;
        self.reading.upload_rate = sample.tx_bytes.saturating_sub(self.last_tx) as f64 / seconds;
        self.last_rx = sample.rx_bytes;
        self.last_tx = sample.tx_bytes;
        self.last_sample_nanos = Some(now_nanos);
        self.reading
    }
}
