use statsbar_core::history::UsageHistory;

#[test]
fn keeps_only_the_newest_samples_once_full() {
    let mut history = UsageHistory::new(3);
    for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
        history.push(value);
    }
    assert_eq!(history.samples().collect::<Vec<_>>(), vec![3.0, 4.0, 5.0]);
    assert_eq!(history.len(), 3);
    assert_eq!(history.capacity(), 3);
}

#[test]
fn ignores_values_that_are_not_finite() {
    let mut history = UsageHistory::new(3);
    history.push(f64::NAN);
    history.push(f64::INFINITY);
    history.push(f64::NEG_INFINITY);
    assert!(history.is_empty());
    history.push(7.0);
    assert_eq!(history.samples().collect::<Vec<_>>(), vec![7.0]);
}

#[test]
fn a_zero_capacity_is_raised_to_one() {
    let mut history = UsageHistory::new(0);
    history.push(1.0);
    history.push(2.0);
    assert_eq!(history.capacity(), 1);
    assert_eq!(history.samples().collect::<Vec<_>>(), vec![2.0]);
}

#[test]
fn a_full_window_spans_the_whole_width() {
    let mut history = UsageHistory::new(3);
    for value in [0.0, 50.0, 100.0] {
        history.push(value);
    }
    // y grows downwards: 0 % is the bottom edge, 100 % the top.
    assert_eq!(
        history.plot_points(100.0, 40.0, 100.0),
        vec![(0.0, 40.0), (50.0, 20.0), (100.0, 0.0)]
    );
}

#[test]
fn a_filling_window_grows_in_from_the_right() {
    let mut history = UsageHistory::new(5);
    history.push(100.0);
    history.push(50.0);
    assert_eq!(
        history.plot_points(100.0, 40.0, 100.0),
        vec![(75.0, 0.0), (100.0, 20.0)]
    );
}

#[test]
fn values_outside_the_scale_are_clamped_to_the_chart() {
    let mut history = UsageHistory::new(2);
    history.push(-20.0);
    history.push(250.0);
    assert_eq!(
        history.plot_points(10.0, 40.0, 100.0),
        vec![(0.0, 40.0), (10.0, 0.0)]
    );
}

#[test]
fn nothing_is_plotted_without_samples_or_a_positive_scale() {
    let mut history = UsageHistory::new(3);
    assert!(history.plot_points(100.0, 40.0, 100.0).is_empty());
    history.push(10.0);
    assert!(history.plot_points(100.0, 40.0, 0.0).is_empty());
    assert!(history.plot_points(100.0, 40.0, f64::NAN).is_empty());
}

#[test]
fn a_single_slot_window_plots_on_the_right_edge() {
    let mut history = UsageHistory::new(1);
    history.push(25.0);
    assert_eq!(history.plot_points(80.0, 40.0, 100.0), vec![(80.0, 30.0)]);
}
