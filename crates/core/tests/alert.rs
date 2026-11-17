use statsbar_core::alert::HotAlertPolicy;

fn policy() -> HotAlertPolicy {
    HotAlertPolicy::new(39.0, 2.0)
}

#[test]
fn alerts_once_when_the_threshold_is_reached() {
    let mut policy = policy();
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(38.9))], true)
            .is_empty()
    );
    assert_eq!(
        policy.sensors_to_alert(&[("phone", Some(39.0))], true),
        vec![("phone".to_string(), 39.0)]
    );
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(41.0))], true)
            .is_empty()
    );
}

#[test]
fn hovering_just_below_the_threshold_does_not_rearm() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    // 37.1 is below the threshold but inside the 2-degree margin.
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(37.1))], true)
            .is_empty()
    );
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(39.5))], true)
            .is_empty()
    );
}

#[test]
fn cooling_by_the_margin_rearms() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(37.0))], true)
            .is_empty()
    );
    assert_eq!(
        policy.sensors_to_alert(&[("phone", Some(39.2))], true),
        vec![("phone".to_string(), 39.2)]
    );
}

#[test]
fn a_missing_reading_neither_alerts_nor_rearms() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    assert!(policy.sensors_to_alert(&[("phone", None)], true).is_empty());
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(40.0))], true)
            .is_empty()
    );
}

#[test]
fn a_sensor_that_disappears_is_forgotten() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    assert!(policy.sensors_to_alert(&[], true).is_empty());
    assert_eq!(
        policy.sensors_to_alert(&[("phone", Some(40.0))], true),
        vec![("phone".to_string(), 40.0)]
    );
}

#[test]
fn sensors_are_tracked_independently_and_reported_in_input_order() {
    let mut policy = policy();
    assert_eq!(
        policy.sensors_to_alert(
            &[
                ("tablet", Some(45.0)),
                ("cpu", Some(20.0)),
                ("phone", Some(39.0))
            ],
            true
        ),
        vec![("tablet".to_string(), 45.0), ("phone".to_string(), 39.0)]
    );
    assert_eq!(
        policy.sensors_to_alert(
            &[
                ("tablet", Some(45.0)),
                ("cpu", Some(50.0)),
                ("phone", Some(39.0))
            ],
            true
        ),
        vec![("cpu".to_string(), 50.0)]
    );
}

#[test]
fn disabling_silences_alerts_and_rearms_everything() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(40.0))], false)
            .is_empty()
    );
    assert_eq!(
        policy.sensors_to_alert(&[("phone", Some(40.0))], true),
        vec![("phone".to_string(), 40.0)]
    );
}
