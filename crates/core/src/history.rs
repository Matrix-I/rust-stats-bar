//! A fixed-length window of recent samples, and the geometry of the chart drawn from it.

use std::collections::VecDeque;

/// The last `capacity` samples of a metric, oldest first.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageHistory {
    capacity: usize,
    samples: VecDeque<f64>,
}

impl UsageHistory {
    /// A capacity of zero is raised to one, so the window always holds the latest sample.
    pub fn new(capacity: usize) -> UsageHistory {
        let capacity = capacity.max(1);
        UsageHistory {
            capacity,
            samples: VecDeque::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn samples(&self) -> impl Iterator<Item = f64> + '_ {
        self.samples.iter().copied()
    }

    /// Appends a sample, dropping the oldest once the window is full. NaN and infinities are
    /// ignored: a single one would otherwise poison the chart's path.
    pub fn push(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }

    /// Chart points as `(x, y)` with the origin at the top-left, as Cairo draws.
    ///
    /// The newest sample sits on the right edge and older ones step left by a fixed amount,
    /// so a window that is still filling grows in from the right instead of stretching.
    /// Values are clamped into `0..=maximum`; a NaN or non-positive `maximum` plots nothing.
    pub fn plot_points(&self, width: f64, height: f64, maximum: f64) -> Vec<(f64, f64)> {
        if maximum.is_nan() || maximum <= 0.0 || self.samples.is_empty() {
            return Vec::new();
        }
        let step = if self.capacity > 1 {
            width / (self.capacity - 1) as f64
        } else {
            0.0
        };
        let last = self.samples.len() - 1;
        self.samples
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let fraction = (value / maximum).clamp(0.0, 1.0);
                (
                    width - (last - index) as f64 * step,
                    height - fraction * height,
                )
            })
            .collect()
    }
}
