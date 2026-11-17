//! When to raise a "running hot" notification, and when to stay quiet.

use std::collections::BTreeSet;

/// Decides which sensors deserve a notification on this poll.
///
/// A sensor alerts once when it reaches the threshold and not again until it has cooled by
/// the re-arm margin. Without that margin a reading hovering at the threshold would fire on
/// every poll.
#[derive(Debug, Clone, PartialEq)]
pub struct HotAlertPolicy {
    pub threshold_c: f64,
    pub rearm_margin_c: f64,
    alerted: BTreeSet<String>,
}

impl HotAlertPolicy {
    pub fn new(threshold_c: f64, rearm_margin_c: f64) -> HotAlertPolicy {
        HotAlertPolicy {
            threshold_c,
            rearm_margin_c,
            alerted: BTreeSet::new(),
        }
    }

    /// `readings` holds every sensor currently present as `(id, temperature)`; a `None`
    /// temperature means the sensor is there but gave no value this time.
    ///
    /// Returns the sensors to notify about, in input order.
    pub fn sensors_to_alert(
        &mut self,
        readings: &[(&str, Option<f64>)],
        enabled: bool,
    ) -> Vec<(String, f64)> {
        // A sensor that disappeared (a phone was unplugged) is forgotten, so that plugging a
        // hot one back in alerts again.
        self.alerted
            .retain(|id| readings.iter().any(|(present, _)| present == id));
        if !enabled {
            // Switching alerts off and on again re-arms everything.
            self.alerted.clear();
            return Vec::new();
        }
        let mut fire = Vec::new();
        for &(id, temperature) in readings {
            // No value is not evidence of cooling: the armed state is left alone.
            let Some(temperature) = temperature else {
                continue;
            };
            if temperature >= self.threshold_c {
                if self.alerted.insert(id.to_string()) {
                    fire.push((id.to_string(), temperature));
                }
            } else if temperature <= self.threshold_c - self.rearm_margin_c {
                self.alerted.remove(id);
            }
        }
        fire
    }
}
