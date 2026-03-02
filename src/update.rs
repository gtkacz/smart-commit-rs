use anyhow::{Context, Result};
use colored::Colorize;
use std::time::Duration;

const GITHUB_REPO: &str = "gtkacz/smart-commit-rs";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct VersionCheck {
    pub latest: String,
    pub current: String,
    pub update_available: bool,
}

/// Fetch the latest release tag from GitHub API with a short timeout
pub fn fetch_latest_version() -> Result<String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    let response: serde_json::Value = agent
        .get(&url)
        .set("User-Agent", "cgen")
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .context("Failed to reach GitHub API")?
        .into_json()
        .context("Failed to parse GitHub API response")?;

    let tag = response["tag_name"]
        .as_str()
        .context("No tag_name in GitHub release response")?;

    Ok(tag.to_string())
}

/// Parse a version string (strips leading 'v' if present) into (major, minor, patch)
pub fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let v = version.strip_prefix('v').unwrap_or(version);
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

/// Check if a newer version is available on GitHub
pub fn check_version() -> Result<VersionCheck> {
    let latest = fetch_latest_version()?;
    let current = CURRENT_VERSION.to_string();

    let update_available = match (parse_semver(&latest), parse_semver(&current)) {
        (Some(latest_v), Some(current_v)) => latest_v > current_v,
        _ => false,
    };

    Ok(VersionCheck {
        latest,
        current,
        update_available,
    })
}

/// Run the appropriate update command for the current platform
pub fn run_update() -> Result<()> {
    if is_cargo_available() {
        println!("{}", "Updating via cargo...".cyan().bold());
        let status = std::process::Command::new("cargo")
            .args(["install", "smart-commit-rs"])
            .status()
            .context("Failed to run cargo install")?;

        if !status.success() {
            anyhow::bail!("cargo install failed with exit code {}", status);
        }
    } else {
        run_platform_installer()?;
    }

    println!("{}", "Update complete!".green().bold());
    Ok(())
}

fn is_cargo_available() -> bool {
    std::process::Command::new("cargo")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_platform_installer() -> Result<()> {
    if cfg!(target_os = "windows") {
        println!("{}", "Updating via PowerShell installer...".cyan().bold());
        let status = std::process::Command::new("powershell")
            .args([
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "irm https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.ps1 | iex",
            ])
            .status()
            .context("Failed to run PowerShell installer")?;

        if !status.success() {
            anyhow::bail!("PowerShell installer failed");
        }
    } else {
        println!("{}", "Updating via install script...".cyan().bold());
        let status = std::process::Command::new("bash")
            .args([
                "-c",
                "curl -fsSL https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.sh | bash",
            ])
            .status()
            .context("Failed to run install script")?;

        if !status.success() {
            anyhow::bail!("Install script failed");
        }
    }

    Ok(())
}

/// Print a warning that a newer version is available
pub fn print_update_warning(latest: &str) {
    eprintln!(
        "\n{}  {} → {}  (run {} to update)",
        "Update available!".yellow().bold(),
        CURRENT_VERSION.dimmed(),
        latest.green(),
        "cgen update".cyan(),
    );
}

pub fn current_version() -> &'static str {
    CURRENT_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_repo_constant() {
        assert_eq!(GITHUB_REPO, "gtkacz/smart-commit-rs");
    }

    #[test]
    fn test_current_version_not_empty() {
        assert!(!CURRENT_VERSION.is_empty());
    }

    #[test]
    fn test_current_version_is_semver() {
        let version = current_version();
        assert!(parse_semver(version).is_some());
    }

    #[test]
    fn test_parse_semver_basic() {
        let v = parse_semver("1.2.3").unwrap();
        assert_eq!(v, (1, 2, 3));
    }

    #[test]
    fn test_parse_semver_with_v() {
        let v = parse_semver("v1.2.3").unwrap();
        assert_eq!(v, (1, 2, 3));
    }

    #[test]
    fn test_parse_semver_invalid_parts() {
        assert!(parse_semver("1.2").is_none());
        assert!(parse_semver("1").is_none());
        assert!(parse_semver("").is_none());
        assert!(parse_semver("1.2.3.4").is_none());
    }

    #[test]
    fn test_parse_semver_non_numeric() {
        assert!(parse_semver("a.b.c").is_none());
        assert!(parse_semver("1.2.x").is_none());
    }

    #[test]
    fn test_parse_semver_large_numbers() {
        let v = parse_semver("100.200.300").unwrap();
        assert_eq!(v, (100, 200, 300));
    }

    #[test]
    fn test_parse_semver_zeros() {
        let v = parse_semver("0.0.0").unwrap();
        assert_eq!(v, (0, 0, 0));
    }

    #[test]
    fn test_version_check_struct() {
        let check = VersionCheck {
            latest: "2.0.0".into(),
            current: "1.0.0".into(),
            update_available: true,
        };
        assert_eq!(check.latest, "2.0.0");
        assert_eq!(check.current, "1.0.0");
        assert!(check.update_available);
    }

    #[test]
    fn test_version_comparison_logic() {
        // Simulate the comparison logic used in check_version
        let latest = "2.0.0";
        let current = "1.5.0";
        let update_available = match (parse_semver(latest), parse_semver(current)) {
            (Some(latest_v), Some(current_v)) => latest_v > current_v,
            _ => false,
        };
        assert!(update_available);
    }

    #[test]
    fn test_version_comparison_no_update() {
        let latest = "1.0.0";
        let current = "1.5.0";
        let update_available = match (parse_semver(latest), parse_semver(current)) {
            (Some(latest_v), Some(current_v)) => latest_v > current_v,
            _ => false,
        };
        assert!(!update_available);
    }

    #[test]
    fn test_version_comparison_same() {
        let latest = "1.5.0";
        let current = "1.5.0";
        let update_available = match (parse_semver(latest), parse_semver(current)) {
            (Some(latest_v), Some(current_v)) => latest_v > current_v,
            _ => false,
        };
        assert!(!update_available);
    }

    #[test]
    fn test_version_comparison_invalid() {
        let latest = "invalid";
        let current = "1.0.0";
        let update_available = match (parse_semver(latest), parse_semver(current)) {
            (Some(latest_v), Some(current_v)) => latest_v > current_v,
            _ => false,
        };
        assert!(!update_available); // Falls back to false for invalid
    }

    #[test]
    fn test_print_update_warning_no_panic() {
        // Just ensure it doesn't panic
        print_update_warning("2.0.0");
        print_update_warning("v1.5.0");
        print_update_warning("");
    }
}
