use anyhow::{Context, Result};
use auto_commit_rs::{cache, cli, config, git, preset, prompt, provider, ui, update};
use colored::Colorize;
use inquire::{Select, Text};
use std::time::Instant;

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {:#}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = cli::parse();
    let cfg = match &cli.command {
        Some(
            cli::Command::Config
            | cli::Command::Update
            | cli::Command::History
            | cli::Command::Preset
            | cli::Command::Fallback,
        ) => None,
        _ => Some(config::AppConfig::load()?),
    };

    // On first run, ask about auto-update preference
    if let Some(ref c) = cfg {
        if c.auto_update.is_none() {
            prompt_auto_update();
        }
    }

    // Check for updates (except for config/update/history commands)
    let update_warning = match &cli.command {
        Some(
            cli::Command::Config
            | cli::Command::Update
            | cli::Command::History
            | cli::Command::Preset
            | cli::Command::Fallback,
        ) => None,
        _ => check_for_updates(cfg.as_ref()),
    };

    match &cli.command {
        Some(cli::Command::Config) => {
            run_config_command()?;
        }
        Some(cli::Command::Update) => {
            run_update_command()?;
        }
        Some(cli::Command::History) => {
            cache::interactive_history()?;
        }
        Some(cli::Command::Preset) => {
            preset::interactive_presets()?;
        }
        Some(cli::Command::Fallback) => {
            preset::interactive_fallback_order()?;
        }
        Some(cli::Command::Undo) => {
            run_undo(cfg.as_ref().expect("config should be loaded"))?;
        }
        Some(cli::Command::Alter { commits }) => {
            run_alter(
                cfg.as_ref().expect("config should be loaded"),
                &cli,
                commits,
            )?;
        }
        Some(cli::Command::Prompt) => {
            let c = cfg.as_ref().expect("config should be loaded");
            let system_prompt = prompt::build_system_prompt(c);
            println!("\n{}", "LLM system prompt:".cyan().bold());
            println!("{system_prompt}");
        }
        None => {
            run_standard_commit(cfg.as_ref().expect("config should be loaded"), &cli)?;
        }
    }

    // Show update warning at the end so it doesn't get buried
    if let Some(latest) = update_warning {
        update::print_update_warning(&latest);
    }

    Ok(())
}

fn run_standard_commit(cfg: &config::AppConfig, cli: &cli::Cli) -> Result<()> {
    ensure_api_key(cfg)?;

    let staged_files = git::list_staged_files().context("Failed to list staged files")?;
    print_staged_files(&staged_files);

    if cfg.warn_staged_files_enabled && staged_files.len() > cfg.warn_staged_files_threshold {
        let prompt = format!(
            "You have {} staged files (threshold {}). Continue with commit generation?",
            staged_files.len(),
            cfg.warn_staged_files_threshold
        );
        let should_continue = ui::confirm(&prompt, false);
        if !should_continue {
            println!("{}", "Commit cancelled.".dimmed());
            return Ok(());
        }
    }

    let gen_start = Instant::now();
    let diff = git::get_staged_diff_filtered(&cfg.diff_exclude_globs)
        .context("Failed to get staged diff")?;
    let Some((final_msg, time_to_ready)) = generate_final_message(cfg, &diff, cli.verbose, gen_start)? else {
        return Ok(());
    };
    if cli.verbose {
        if let Some(elapsed) = time_to_ready {
            println!("  {} {}", "Generated in".dimmed(), format!("{:.2}s", elapsed.as_secs_f64()).dimmed());
        }
    }

    if cli.dry_run {
        println!(
            "\n{}",
            "Dry run enabled. Commit not created.".yellow().bold()
        );
        return Ok(());
    }

    git::run_commit(&final_msg, &cli.extra_args, cfg.suppress_tool_output)
        .context("git commit failed")?;

    if cfg.track_generated_commits {
        if let Ok(repo_root) = git::find_repo_root() {
            if let Ok(hash) = cache::get_head_hash() {
                let preview: String = final_msg.chars().take(80).collect();
                let _ = cache::record_commit(&repo_root, &hash, &preview);
            }
        }
    }

    if cli.tag {
        create_semver_tag(cfg)?;
    }

    handle_post_commit_push(cfg, "Commit created. Push now?")?;
    Ok(())
}

fn run_alter(cfg: &config::AppConfig, cli: &cli::Cli, commits: &[String]) -> Result<()> {
    ensure_api_key(cfg)?;

    let (target, diff) = match commits {
        [single] => (
            single.to_string(),
            git::get_commit_diff(single).context("Failed to get commit diff")?,
        ),
        [older, newer] => (
            newer.to_string(),
            git::get_range_diff(older, newer).context("Failed to get range diff")?,
        ),
        _ => anyhow::bail!("Expected one or two commit hashes."),
    };

    let target_is_head = git::is_head_commit(&target)?;
    let target_is_pushed = git::commit_is_pushed(&target)?;
    if target_is_pushed {
        let proceed = ui::confirm(
            "Target commit appears to be pushed already. Rewriting history may require a force push. Continue?",
            false,
        );
        if !proceed {
            println!("{}", "Alter cancelled.".dimmed());
            return Ok(());
        }
    }

    let gen_start = Instant::now();
    let Some((final_msg, time_to_ready)) = generate_final_message(cfg, &diff, cli.verbose, gen_start)? else {
        return Ok(());
    };
    if cli.verbose {
        if let Some(elapsed) = time_to_ready {
            println!("  {} {}", "Generated in".dimmed(), format!("{:.2}s", elapsed.as_secs_f64()).dimmed());
        }
    }

    if cli.dry_run {
        println!(
            "\n{}",
            "Dry run enabled. Commit message was generated but history was not rewritten."
                .yellow()
                .bold()
        );
        return Ok(());
    }

    git::rewrite_commit_message(&target, &final_msg, cfg.suppress_tool_output)
        .context("Failed to rewrite commit message")?;

    if cfg.track_generated_commits {
        if let Ok(repo_root) = git::find_repo_root() {
            if let Ok(hash) = cache::get_head_hash() {
                let preview: String = final_msg.chars().take(80).collect();
                let _ = cache::record_commit(&repo_root, &hash, &preview);
            }
        }
    }

    if target_is_pushed {
        let should_push = ui::confirm(
            "History was rewritten on a pushed commit. Attempt `git push` now?",
            false,
        );
        if should_push {
            if !target_is_head {
                println!(
                    "{}",
                    "Note: a non-HEAD rewrite may require `git push --force-with-lease`."
                        .yellow()
                        .bold()
                );
            }
            git::run_push(cfg.suppress_tool_output).context("git push failed")?;
        } else {
            println!(
                "{}",
                "Skipped push after history rewrite. Push manually when ready.".dimmed()
            );
        }
    } else {
        handle_post_commit_push(cfg, "Commit message altered. Push now?")?;
    }

    Ok(())
}

fn ensure_api_key(cfg: &config::AppConfig) -> Result<()> {
    if cfg.api_key.is_empty() {
        anyhow::bail!(
            "No API key configured. Run {} or set {}",
            "cgen config".yellow(),
            "ACR_API_KEY".yellow()
        );
    }
    Ok(())
}

fn generate_final_message(
    cfg: &config::AppConfig,
    diff: &str,
    verbose: bool,
    gen_start: Instant,
) -> Result<Option<(String, Option<std::time::Duration>)>> {
    let system_prompt = prompt::build_system_prompt(cfg);
    if verbose {
        println!("\n{}", "LLM system prompt:".cyan().bold());
        println!("{system_prompt}\n");
    }
    let (mut message, fallback_name) =
        provider::call_llm_with_fallback(cfg, &system_prompt, diff)
            .context("LLM API call failed")?;

    if let Some(ref name) = fallback_name {
        println!(
            "  {} Used fallback preset: {}",
            "note:".yellow().bold(),
            name
        );
    }

    let mut time_to_ready: Option<std::time::Duration> = None;

    let final_msg = if cfg.review_commit {
        loop {
            let candidate = cfg.commit_template.replace("$msg", message.trim());

            if time_to_ready.is_none() {
                time_to_ready = Some(gen_start.elapsed());
            }
            println!("\n{}", "Commit message:".green().bold());
            println!("  {}\n", candidate);

            match review_message()? {
                ReviewAction::Accept => break candidate,
                ReviewAction::Regenerate => {
                    let (new_msg, fb) =
                        provider::call_llm_with_fallback(cfg, &system_prompt, diff)
                            .context("LLM API call failed")?;
                    message = new_msg;
                    if let Some(ref name) = fb {
                        println!(
                            "  {} Used fallback preset: {}",
                            "note:".yellow().bold(),
                            name
                        );
                    }
                }
                ReviewAction::Edit => {
                    let edited = Text::new("Edit commit message:")
                        .with_default(&candidate)
                        .prompt()?;
                    break edited;
                }
                ReviewAction::Cancel => {
                    println!("{}", "Commit cancelled.".dimmed());
                    return Ok(None);
                }
            }
        }
    } else {
        let final_msg = cfg.commit_template.replace("$msg", message.trim());
        time_to_ready = Some(gen_start.elapsed());
        println!("\n{} {}", "Commit message:".green().bold(), final_msg);
        final_msg
    };

    Ok(Some((final_msg, time_to_ready)))
}

fn create_semver_tag(cfg: &config::AppConfig) -> Result<()> {
    let latest = git::get_latest_tag().context("Failed to inspect existing tags")?;
    let next_tag = git::compute_next_minor_tag(latest.as_deref())?;

    let should_create = if cfg.confirm_new_version {
        let prompt = match latest.as_deref() {
            Some(tag) => format!("Create new tag {next_tag} (latest: {tag})?"),
            None => format!("Create initial tag {next_tag}?"),
        };
        ui::confirm(&prompt, true)
    } else {
        true
    };

    if !should_create {
        println!("{}", "Tag creation skipped.".dimmed());
        return Ok(());
    }

    git::create_tag(&next_tag, cfg.suppress_tool_output).context("Failed to create git tag")?;
    println!("{} {}", "Created tag:".green().bold(), next_tag);
    Ok(())
}

enum ReviewAction {
    Accept,
    Regenerate,
    Edit,
    Cancel,
}

fn review_message() -> Result<ReviewAction> {
    let choices = vec!["Accept", "Regenerate", "Edit", "Cancel"];

    let answer = Select::new("", choices).without_help_message().prompt();

    match answer {
        Ok("Accept") => Ok(ReviewAction::Accept),
        Ok("Regenerate") => Ok(ReviewAction::Regenerate),
        Ok("Edit") => Ok(ReviewAction::Edit),
        _ => Ok(ReviewAction::Cancel),
    }
}

fn print_staged_files(staged_files: &[String]) {
    println!(
        "\n{} {}",
        "Staged files:".green().bold(),
        staged_files.len()
    );
    if staged_files.is_empty() {
        println!("  {}", "(none)".dimmed());
        return;
    }

    let last = staged_files.len() - 1;
    for (i, file) in staged_files.iter().enumerate() {
        let connector = if i == last {
            "\u{2514}\u{2500}\u{2500}"
        } else {
            "\u{251C}\u{2500}\u{2500}"
        };
        println!("  {} {}", connector, file);
    }
}

fn handle_post_commit_push(cfg: &config::AppConfig, ask_prompt: &str) -> Result<()> {
    match cfg.post_commit_push.as_str() {
        "never" => {}
        "always" => {
            git::run_push(cfg.suppress_tool_output).context("git push failed")?;
        }
        _ => {
            let should_push = ui::confirm(ask_prompt, true);
            if should_push {
                git::run_push(cfg.suppress_tool_output).context("git push failed")?;
            }
        }
    }
    Ok(())
}

fn prompt_auto_update() {
    println!(
        "  {}",
        "You can change this later with `cgen config`".dimmed()
    );
    let yes = ui::confirm(
        "Would you like to enable automatic updates for cgen?",
        true,
    );
    if let Err(e) = config::save_auto_update_preference(yes) {
        eprintln!(
            "{} Failed to save auto-update preference: {}",
            "warning:".yellow().bold(),
            e
        );
    } else {
        let status = if yes { "enabled" } else { "disabled" };
        println!(
            "{} Auto-updates {}.\n",
            "done!".green().bold(),
            status
        );
    }
}

/// Check for updates and either auto-update or return the latest version for a warning.
/// Returns Some(latest_version) if a warning should be shown, None otherwise.
fn check_for_updates(cfg: Option<&config::AppConfig>) -> Option<String> {
    let version_check = match update::check_version() {
        Ok(v) => v,
        Err(_) => return None, // silently ignore network errors
    };

    if !version_check.update_available {
        return None;
    }

    let auto_update = cfg.and_then(|c| c.auto_update).unwrap_or(false);

    if auto_update {
        println!(
            "{} {} → {}",
            "Auto-updating cgen...".cyan().bold(),
            version_check.current.dimmed(),
            version_check.latest.green(),
        );
        if let Err(e) = update::run_update() {
            eprintln!(
                "{} Auto-update failed: {}",
                "warning:".yellow().bold(),
                e
            );
            return Some(version_check.latest);
        }
        println!(
            "{} Restart cgen to use the new version.\n",
            "note:".yellow().bold()
        );
        return None;
    }

    Some(version_check.latest)
}

fn run_config_command() -> Result<()> {
    match git::find_repo_root() {
        Ok(_) => {
            let choices = vec!["Local (.env in repo)", "Global (TOML config)"];
            let answer = Select::new("Configure global or local settings?", choices).prompt();
            match answer {
                Ok(choice) => {
                    let global = choice.contains("Global");
                    cli::interactive_config(global)?;
                }
                Err(_) => {
                    println!("{}", "Cancelled.".dimmed());
                }
            }
        }
        Err(_) => {
            cli::interactive_config(true)?;
        }
    }
    Ok(())
}

fn run_update_command() -> Result<()> {
    println!("{}", "Checking for updates...".cyan().bold());

    match update::check_version() {
        Ok(v) if v.update_available => {
            println!(
                "{} {} → {}",
                "New version available!".green().bold(),
                v.current.dimmed(),
                v.latest.green(),
            );
            update::run_update()?;
        }
        Ok(v) => {
            println!(
                "{} You are already on the latest version ({}).",
                "Up to date!".green().bold(),
                v.current,
            );
        }
        Err(e) => {
            anyhow::bail!("Failed to check for updates: {}", e);
        }
    }
    Ok(())
}

fn run_undo(cfg: &config::AppConfig) -> Result<()> {
    git::ensure_head_exists()?;

    if git::head_is_merge_commit()? {
        let proceed_merge = ui::confirm(
            "Latest commit is a merge commit. Undo it with git reset --soft HEAD~1?",
            false,
        );
        if !proceed_merge {
            println!("{}", "Undo cancelled.".dimmed());
            return Ok(());
        }
    }

    if !git::has_upstream_branch()? {
        println!(
            "{}",
            "No upstream branch detected. Assuming latest commit is not pushed."
                .yellow()
                .bold()
        );
    } else if git::is_head_pushed()? {
        let proceed_pushed = ui::confirm(
            "Latest commit appears to be pushed already. Undo locally anyway?",
            false,
        );
        if !proceed_pushed {
            println!("{}", "Undo cancelled.".dimmed());
            return Ok(());
        }
    }

    git::undo_last_commit_soft(cfg.suppress_tool_output).context("Failed to undo latest commit")?;
    println!("{}", "Latest commit undone (soft reset).".green().bold());
    Ok(())
}
