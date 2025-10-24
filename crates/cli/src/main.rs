use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{cursor, execute, terminal};
use owo_colors::OwoColorize;
use pre_commit_core::{Executor, Hook, PlanBuilder};
use pre_commit_dag::DagBuilder;
use pre_commit_executor_sync::SyncExecutor;
use pre_commit_parser::{extract_hooks, parse_config_file, validate_config};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "pre-commit-rs")]
#[command(about = "A fast, parallel pre-commit hook runner written in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run pre-commit hooks
    Run {
        /// Path to config file
        #[arg(short, long, default_value = ".pre-commit-config.yaml")]
        config: PathBuf,

        /// Run hooks in parallel (respecting dependencies)
        #[arg(short, long)]
        parallel: bool,

        /// Files to check (if not provided, checks all staged files)
        files: Vec<PathBuf>,
    },
    /// Install pre-commit hook
    Install {
        /// Path to git repository
        #[arg(short, long, default_value = ".")]
        repo: PathBuf,
    },
    /// Uninstall pre-commit hook
    Uninstall {
        /// Path to git repository
        #[arg(short, long, default_value = ".")]
        repo: PathBuf,
    },
}

fn get_staged_files() -> Result<Vec<PathBuf>> {
    let output = process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACM"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("Failed to get staged files from git");
    }

    let files = String::from_utf8(output.stdout)?
        .lines()
        .map(PathBuf::from)
        .collect();

    Ok(files)
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn print_dag(hooks: &[Hook]) {
    println!("{}", "Dependency Graph:".bright_blue().bold());
    println!();

    // Build dependency map and reverse dependency map
    let mut deps_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut reverse_deps: HashMap<String, Vec<String>> = HashMap::new();

    for hook in hooks {
        deps_map.insert(hook.id.clone(), hook.depends_on.clone());
        reverse_deps.entry(hook.id.clone()).or_default();

        for dep in &hook.depends_on {
            reverse_deps
                .entry(dep.clone())
                .or_default()
                .push(hook.id.clone());
        }
    }

    // Print each hook with its dependencies
    for (idx, hook) in hooks.iter().enumerate() {
        let is_last = idx == hooks.len() - 1;
        let prefix = if is_last { "└─" } else { "├─" };

        // Print the hook
        println!(
            "{} {} {}",
            prefix.cyan(),
            "●".green().bold(),
            hook.name.bold()
        );

        // Print dependencies (what this hook depends on)
        if !hook.depends_on.is_empty() {
            for (dep_idx, dep_id) in hook.depends_on.iter().enumerate() {
                let is_last_dep = dep_idx == hook.depends_on.len() - 1;
                let dep_hook = hooks.iter().find(|h| h.id == *dep_id);
                let dep_name = dep_hook.map(|h| h.name.as_str()).unwrap_or(dep_id.as_str());

                let connector = if is_last {
                    if is_last_dep {
                        "   └──▶"
                    } else {
                        "   ├──▶"
                    }
                } else if is_last_dep {
                    "│  └──▶"
                } else {
                    "│  ├──▶"
                };

                println!("{}  {}", connector.cyan(), dep_name.yellow());
            }
        }
    }

    println!();
}

enum HookStatus {
    Pending,
    Running,
    Success,
    Failed,
}

fn run_hooks(config_path: PathBuf, parallel: bool, files: Vec<PathBuf>) -> Result<()> {
    // Parse and validate config
    let config = parse_config_file(&config_path)?;
    validate_config(&config)?;

    // Extract hooks
    let hooks = extract_hooks(&config);

    if hooks.is_empty() {
        println!("No hooks to run");
        return Ok(());
    }

    // Get files to check
    let files_to_check = if files.is_empty() {
        get_staged_files()?
    } else {
        files
    };

    println!(
        "Running {} hooks on {} files...\n",
        hooks.len(),
        files_to_check.len()
    );

    // Display DAG
    print_dag(&hooks);

    // Build execution plan
    let plan = DagBuilder::new().build_plan(&hooks)?;

    // Execute hooks with live status
    let result = if parallel {
        execute_with_live_status(plan, &hooks, &files_to_check)?
    } else {
        let executor = SyncExecutor::new();
        executor.execute(&hooks, &files_to_check)?
    };

    // Display results
    for hook_result in &result.hooks {
        let status = if hook_result.success { "✓" } else { "✗" };
        println!(
            "{} {} ({}ms)",
            status, hook_result.hook_id, hook_result.duration_ms
        );

        if !hook_result.stdout.is_empty() {
            println!("  stdout: {}", hook_result.stdout.trim());
        }
        if !hook_result.stderr.is_empty() {
            println!("  stderr: {}", hook_result.stderr.trim());
        }
    }

    println!("\nTotal time: {}ms", result.total_duration_ms);

    if result.all_passed {
        println!("All hooks passed!");
        Ok(())
    } else {
        anyhow::bail!("Some hooks failed");
    }
}

fn execute_with_live_status(
    plan: pre_commit_core::ExecutionPlan,
    hooks: &[Hook],
    files: &[PathBuf],
) -> Result<pre_commit_core::ExecutionResult> {
    use futures::stream::{FuturesUnordered, StreamExt};
    use std::time::Instant;

    // Track status of all hooks
    let mut statuses: HashMap<String, HookStatus> = HashMap::new();
    for hook in hooks {
        statuses.insert(hook.id.clone(), HookStatus::Pending);
    }

    let start = Instant::now();
    let mut all_results = Vec::new();

    // Create runtime for async execution
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        // Execute each level sequentially
        for level in &plan.levels {
            let mut futures = FuturesUnordered::new();

            // Mark all hooks in this level as running and display
            for hook in level {
                statuses.insert(hook.id.clone(), HookStatus::Running);
                futures.push(execute_hook_with_id(hook.clone(), files.to_vec()));
            }

            // Display current status
            display_inline_status(&statuses, hooks);

            // Execute all hooks in this level in parallel
            while let Some((hook_id, result)) = futures.next().await {
                // Update status
                let status = if result.success {
                    HookStatus::Success
                } else {
                    HookStatus::Failed
                };
                statuses.insert(hook_id, status);

                // Update display
                display_inline_status(&statuses, hooks);

                all_results.push(result);
            }
        }
    });

    // Clear the inline display
    clear_inline_status(hooks.len());

    let total_duration = start.elapsed();
    let all_passed = all_results.iter().all(|r| r.success);

    Ok(pre_commit_core::ExecutionResult {
        hooks: all_results,
        total_duration_ms: total_duration.as_millis() as u64,
        all_passed,
    })
}

async fn execute_hook_with_id(hook: Hook, files: Vec<PathBuf>) -> (String, pre_commit_core::HookResult) {
    use regex::Regex;
    use std::time::Instant;
    use tokio::process::Command;

    let hook_id = hook.id.clone();
    let start = Instant::now();

    // Filter files based on hook's file pattern
    let filtered_files = if let Some(pattern) = &hook.files {
        if let Ok(regex) = Regex::new(pattern) {
            files
                .iter()
                .filter(|f| f.to_str().map(|s| regex.is_match(s)).unwrap_or(false))
                .cloned()
                .collect()
        } else {
            files.clone()
        }
    } else {
        files.clone()
    };

    // Build command
    let mut parts = shell_words::split(&hook.entry).unwrap_or_else(|_| vec![hook.entry.clone()]);

    if hook.pass_filenames && !filtered_files.is_empty() {
        for file in &filtered_files {
            if let Some(s) = file.to_str() {
                parts.push(s.to_string());
            }
        }
    }

    // Execute command
    let result = if parts.is_empty() {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Empty command",
        ))
    } else {
        Command::new(&parts[0])
            .args(&parts[1..])
            .env("FORCE_COLOR", "1")
            .env("CLICOLOR_FORCE", "1")
            .output()
            .await
    };

    let duration = start.elapsed();

    let hook_result = match result {
        Ok(output) => pre_commit_core::HookResult {
            hook_id: hook.id.clone(),
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms: duration.as_millis() as u64,
        },
        Err(e) => pre_commit_core::HookResult {
            hook_id: hook.id.clone(),
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("Failed to execute command: {}", e),
            duration_ms: duration.as_millis() as u64,
        },
    };

    (hook_id, hook_result)
}

fn display_inline_status(statuses: &HashMap<String, HookStatus>, hooks: &[Hook]) {
    let mut stdout = io::stdout();

    // Move cursor up to the start of the status display
    if !hooks.is_empty() {
        execute!(stdout, cursor::MoveUp(hooks.len() as u16)).ok();
    }
    execute!(stdout, cursor::MoveToColumn(0)).ok();

    // Display each hook with its current status
    for (idx, hook) in hooks.iter().enumerate() {
        let status = statuses.get(&hook.id).unwrap();
        let is_last = idx == hooks.len() - 1;
        let prefix = if is_last { "└─" } else { "├─" };

        let (symbol, color_name) = match status {
            HookStatus::Pending => ("●", "dim"),
            HookStatus::Running => {
                let frame_idx = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    / 100) as usize
                    % SPINNER_FRAMES.len();
                (SPINNER_FRAMES[frame_idx], "cyan")
            }
            HookStatus::Success => ("✓", "green"),
            HookStatus::Failed => ("✗", "red"),
        };

        let line = format!("{} {} {}", prefix.cyan(), symbol, hook.name);
        let colored_line = match color_name {
            "dim" => line.dimmed().to_string(),
            "cyan" => line.cyan().to_string(),
            "green" => line.green().to_string(),
            "red" => line.red().to_string(),
            _ => line,
        };

        // Clear the line and print
        execute!(stdout, terminal::Clear(terminal::ClearType::CurrentLine)).ok();
        println!("{}", colored_line);
    }

    stdout.flush().ok();
}

fn clear_inline_status(num_lines: usize) {
    let mut stdout = io::stdout();
    if num_lines > 0 {
        execute!(stdout, cursor::MoveUp(num_lines as u16)).ok();
        for _ in 0..num_lines {
            execute!(
                stdout,
                terminal::Clear(terminal::ClearType::CurrentLine),
                cursor::MoveDown(1)
            )
            .ok();
        }
        execute!(stdout, cursor::MoveUp(num_lines as u16)).ok();
    }
    stdout.flush().ok();
}

// Helper module for parsing shell commands
mod shell_words {
    pub fn split(input: &str) -> Result<Vec<String>, &'static str> {
        let mut words = Vec::new();
        let mut current = String::new();
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '\'' if !in_double_quote => {
                    in_single_quote = !in_single_quote;
                }
                '"' if !in_single_quote => {
                    in_double_quote = !in_double_quote;
                }
                ' ' | '\t' if !in_single_quote && !in_double_quote => {
                    if !current.is_empty() {
                        words.push(current.clone());
                        current.clear();
                    }
                }
                '\\' if !in_single_quote => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                _ => {
                    current.push(c);
                }
            }
        }

        if !current.is_empty() {
            words.push(current);
        }

        Ok(words)
    }
}

fn install_hook(repo_path: PathBuf) -> Result<()> {
    let git_dir = repo_path.join(".git");
    if !git_dir.exists() {
        anyhow::bail!("Not a git repository");
    }

    let hooks_dir = git_dir.join("hooks");
    if !hooks_dir.exists() {
        fs::create_dir(&hooks_dir)?;
    }

    let pre_commit_hook = hooks_dir.join("pre-commit");

    // Get the absolute path to the current executable
    let current_exe = std::env::current_exe()?;
    let exe_path = current_exe.display();

    let hook_content = format!(
        r#"#!/usr/bin/env sh
# pre-commit-rs hook
exec "{}" run -p
"#,
        exe_path
    );

    fs::write(&pre_commit_hook, hook_content)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&pre_commit_hook)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&pre_commit_hook, perms)?;
    }

    println!("pre-commit hook installed successfully!");
    Ok(())
}

fn uninstall_hook(repo_path: PathBuf) -> Result<()> {
    let pre_commit_hook = repo_path.join(".git").join("hooks").join("pre-commit");

    if !pre_commit_hook.exists() {
        println!("No pre-commit hook found");
        return Ok(());
    }

    fs::remove_file(&pre_commit_hook)?;
    println!("pre-commit hook uninstalled successfully!");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            config,
            parallel,
            files,
        } => {
            run_hooks(config, parallel, files)?;
        }
        Commands::Install { repo } => {
            install_hook(repo)?;
        }
        Commands::Uninstall { repo } => {
            uninstall_hook(repo)?;
        }
    }

    Ok(())
}
