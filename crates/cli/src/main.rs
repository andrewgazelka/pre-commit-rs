use anyhow::Result;
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use pre_commit_core::{Executor, Hook, PlanBuilder};
use pre_commit_dag::DagBuilder;
use pre_commit_executor_parallel::ParallelExecutor;
use pre_commit_executor_sync::SyncExecutor;
use pre_commit_parser::{extract_hooks, parse_config_file, validate_config};
use std::collections::HashMap;
use std::fs;
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

    // Execute hooks
    let result = if parallel {
        let executor = ParallelExecutor::new(plan);
        executor.execute(&hooks, &files_to_check)?
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

fn install_hook(repo_path: PathBuf) -> Result<()> {
    let hooks_dir = repo_path.join(".git").join("hooks");
    if !hooks_dir.exists() {
        anyhow::bail!("Not a git repository");
    }

    let pre_commit_hook = hooks_dir.join("pre-commit");

    let hook_content = r#"#!/bin/sh
# pre-commit-rs hook
exec pre-commit-rs run
"#;

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
