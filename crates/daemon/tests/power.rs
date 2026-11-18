use statsbard::power::is_power_saver;

#[test]
fn only_the_power_saver_profile_counts() {
    assert!(is_power_saver("power-saver"));
    for profile in ["balanced", "performance", "", "Power-Saver"] {
        assert!(!is_power_saver(profile), "{profile:?}");
    }
}
