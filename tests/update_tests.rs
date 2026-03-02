use auto_commit_rs::update::{current_version, parse_semver, print_update_warning, VersionCheck};

#[test]
fn parse_semver_handles_plain_versions() {
    assert_eq!(parse_semver("1.0.0"), Some((1, 0, 0)));
    assert_eq!(parse_semver("0.1.0"), Some((0, 1, 0)));
    assert_eq!(parse_semver("12.34.56"), Some((12, 34, 56)));
}

#[test]
fn parse_semver_strips_v_prefix() {
    assert_eq!(parse_semver("v1.0.0"), Some((1, 0, 0)));
    assert_eq!(parse_semver("v0.2.3"), Some((0, 2, 3)));
}

#[test]
fn parse_semver_rejects_invalid_formats() {
    assert_eq!(parse_semver(""), None);
    assert_eq!(parse_semver("1.0"), None);
    assert_eq!(parse_semver("1.0.0.0"), None);
    assert_eq!(parse_semver("abc"), None);
    assert_eq!(parse_semver("v1.x.0"), None);
    assert_eq!(parse_semver("not-a-version"), None);
}

#[test]
fn semver_comparison_works_for_update_detection() {
    let current = parse_semver("1.0.0").unwrap();
    let newer_patch = parse_semver("1.0.1").unwrap();
    let newer_minor = parse_semver("1.1.0").unwrap();
    let newer_major = parse_semver("2.0.0").unwrap();
    let same = parse_semver("1.0.0").unwrap();
    let older = parse_semver("0.9.0").unwrap();

    assert!(newer_patch > current);
    assert!(newer_minor > current);
    assert!(newer_major > current);
    assert!(!(same > current));
    assert!(!(older > current));
}

#[test]
fn current_version_is_valid_semver() {
    let version = current_version();
    assert!(
        parse_semver(version).is_some(),
        "CARGO_PKG_VERSION '{}' should be valid semver",
        version
    );
}

#[test]
fn current_version_is_not_empty() {
    let version = current_version();
    assert!(!version.is_empty(), "version should not be empty");
}

#[test]
fn version_check_struct_fields() {
    let check = VersionCheck {
        latest: "1.2.0".to_string(),
        current: "1.1.0".to_string(),
        update_available: true,
    };

    assert_eq!(check.latest, "1.2.0");
    assert_eq!(check.current, "1.1.0");
    assert!(check.update_available);
}

#[test]
fn version_check_no_update_available() {
    let check = VersionCheck {
        latest: "1.0.0".to_string(),
        current: "1.0.0".to_string(),
        update_available: false,
    };

    assert!(!check.update_available);
}

#[test]
fn parse_semver_large_numbers() {
    assert_eq!(parse_semver("100.200.300"), Some((100, 200, 300)));
    assert_eq!(parse_semver("0.0.1"), Some((0, 0, 1)));
}

#[test]
fn parse_semver_with_leading_zeros() {
    // Leading zeros in version numbers should still parse
    assert_eq!(parse_semver("01.02.03"), Some((1, 2, 3)));
}

#[test]
fn semver_equality_comparison() {
    let v1 = parse_semver("1.2.3").unwrap();
    let v2 = parse_semver("1.2.3").unwrap();
    assert_eq!(v1, v2);
}

#[test]
fn semver_less_than_comparison() {
    let v1 = parse_semver("1.2.3").unwrap();
    let v2 = parse_semver("1.2.4").unwrap();
    assert!(v1 < v2);

    let v3 = parse_semver("1.3.0").unwrap();
    assert!(v1 < v3);

    let v4 = parse_semver("2.0.0").unwrap();
    assert!(v1 < v4);
}

#[test]
fn print_update_warning_does_not_panic() {
    // Just verify it doesn't panic
    print_update_warning("2.0.0");
    print_update_warning("v1.5.0");
    print_update_warning("999.999.999");
}
