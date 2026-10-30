//! The behaviour suite must pass in full; it is the parity contract with the C
//! reference implementation.

use dyetrace_core::selftest;

#[test]
fn all_behaviour_checks_pass() {
    let checks = selftest::run();

    let failed: Vec<&str> = checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| c.name)
        .collect();

    assert!(failed.is_empty(), "failed checks: {failed:?}");
    assert_eq!(checks.len(), 12, "the suite is expected to have twelve checks");
}
