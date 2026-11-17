use statsbar_core::throughput::{Reading, Sample, ThroughputTracker};

const SECOND: u64 = 1_000_000_000;

fn sample(interface: &str, rx_bytes: u64, tx_bytes: u64) -> Sample {
    Sample {
        interface: interface.to_string(),
        rx_bytes,
        tx_bytes,
    }
}

#[test]
fn the_first_sample_sets_the_baseline_and_reports_nothing() {
    let mut tracker = ThroughputTracker::default();
    let reading = tracker.update(Some(&sample("wlo1", 5_000, 1_000)), 10 * SECOND);
    assert_eq!(reading, Reading::default());
}

#[test]
fn rate_is_the_counter_delta_over_the_elapsed_time() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 5_000, 1_000)), 10 * SECOND);
    let reading = tracker.update(Some(&sample("wlo1", 9_000, 2_000)), 12 * SECOND);
    assert_eq!(reading.download_rate, 2_000.0);
    assert_eq!(reading.upload_rate, 500.0);
    assert_eq!(reading.download_total, 4_000);
    assert_eq!(reading.upload_total, 1_000);
    assert_eq!(tracker.reading(), reading);
}

#[test]
fn a_sample_inside_the_minimum_interval_keeps_the_previous_rate() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 0, 0)), 0);
    tracker.update(Some(&sample("wlo1", 1_000, 0)), SECOND);
    // 0.1 s later: too short to measure, so the rate stays and the total still advances.
    let reading = tracker.update(Some(&sample("wlo1", 9_000, 0)), SECOND + SECOND / 10);
    assert_eq!(reading.download_rate, 1_000.0);
    assert_eq!(reading.download_total, 9_000);
    // The skipped sample did not move the reference point: the next rate covers 1.0 s → 2.0 s.
    let reading = tracker.update(Some(&sample("wlo1", 11_000, 0)), 2 * SECOND);
    assert_eq!(reading.download_rate, 10_000.0);
}

#[test]
fn a_clock_that_did_not_advance_keeps_the_previous_rate() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 0, 0)), SECOND);
    tracker.update(Some(&sample("wlo1", 500, 0)), 2 * SECOND);
    let reading = tracker.update(Some(&sample("wlo1", 900, 0)), 2 * SECOND);
    assert_eq!(reading.download_rate, 500.0);
}

#[test]
fn switching_interface_restarts_the_session() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 1_000, 1_000)), 0);
    tracker.update(Some(&sample("wlo1", 3_000, 2_000)), SECOND);
    let reading = tracker.update(Some(&sample("eno2", 70_000, 80_000)), 2 * SECOND);
    assert_eq!(reading, Reading::default()); // no rate carried over from wlo1
    let reading = tracker.update(Some(&sample("eno2", 71_000, 80_500)), 3 * SECOND);
    assert_eq!(reading.download_rate, 1_000.0);
    assert_eq!(reading.upload_rate, 500.0);
    assert_eq!(reading.download_total, 1_000);
}

#[test]
fn counters_below_the_baseline_restart_the_session() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 50_000, 50_000)), 0);
    tracker.update(Some(&sample("wlo1", 60_000, 50_000)), SECOND);
    // The interface went down and up: its counters restarted near zero.
    let reading = tracker.update(Some(&sample("wlo1", 100, 40)), 2 * SECOND);
    assert_eq!(reading, Reading::default());
}

#[test]
fn an_upload_counter_below_the_baseline_alone_restarts_the_session() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 50_000, 50_000)), 0);
    tracker.update(Some(&sample("wlo1", 60_000, 60_000)), SECOND);
    // Only the transmit counter restarted; receive stays above its baseline.
    let reading = tracker.update(Some(&sample("wlo1", 70_000, 40)), 2 * SECOND);
    assert_eq!(reading, Reading::default());
}

#[test]
fn no_interface_zeroes_the_rates_and_keeps_the_totals() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 0, 0)), 0);
    tracker.update(Some(&sample("wlo1", 4_000, 2_000)), SECOND);
    let reading = tracker.update(None, 2 * SECOND);
    assert_eq!(reading.download_rate, 0.0);
    assert_eq!(reading.upload_rate, 0.0);
    assert_eq!(reading.download_total, 4_000);
    assert_eq!(reading.upload_total, 2_000);
}

#[test]
fn the_first_sample_after_a_gap_does_not_report_a_rate_across_the_gap() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 0, 0)), 0);
    tracker.update(Some(&sample("wlo1", 1_000, 0)), SECOND);
    tracker.update(None, 2 * SECOND);
    // An hour offline, during which the counter moved: no rate may be derived from it.
    let reading = tracker.update(Some(&sample("wlo1", 900_000, 0)), 3_600 * SECOND);
    assert_eq!(reading.download_rate, 0.0);
    let reading = tracker.update(Some(&sample("wlo1", 903_000, 0)), 3_601 * SECOND);
    assert_eq!(reading.download_rate, 3_000.0);
}
