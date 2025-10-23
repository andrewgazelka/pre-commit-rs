use anyhow::Result;
use clap::{Parser, ValueEnum};
use pre_commit_core::{Executor, PlanBuilder};
use pre_commit_dag::DagBuilder;
use pre_commit_executor_parallel::ParallelExecutor;
use pre_commit_executor_sync::SyncExecutor;
use pre_commit_parser::{extract_hooks, parse_config_file, validate_config};
use std::path::PathBuf;
use std::process;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Json,
    Human,
}

#[derive(Parser)]
#[command(name = "pre-commit-ci")]
#[command(about = "CI-optimized pre-commit hook runner", long_about = None)]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = ".pre-commit-config.yaml")]
    config: PathBuf,

    /// Run hooks in parallel (respecting dependencies)
    #[arg(short, long)]
    parallel: bool,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value = "human")]
    format: OutputFormat,

    /// Files to check (if not provided, checks all files in repo)
    files: Vec<PathBuf>,
}

fn get_all_files() -> Result<Vec<PathBuf>> {
    let output = process::Command::new("git").args(["ls-files"]).output()?;

    if !output.status.success() {
        anyhow::bail!("Failed to get files from git");
    }

    let files = String::from_utf8(output.stdout)?
        .lines()
        .map(PathBuf::from)
        .collect();

    Ok(files)
}

fn output_json(result: &pre_commit_core::ExecutionResult) -> Result<()> {
    let json = serde_json::to_string_pretty(result)?;
    println!("{}", json);
    Ok(())
}

fn output_human(result: &pre_commit_core::ExecutionResult) {
    println!("Pre-commit Hook Results");
    println!("=======================\n");

    for hook_result in &result.hooks {
        let status = if hook_result.success { "PASS" } else { "FAIL" };
        println!("[{}] {}", status, hook_result.hook_id);
        println!("  Duration: {}ms", hook_result.duration_ms);

        if let Some(code) = hook_result.exit_code {
            println!("  Exit code: {}", code);
        }

        if !hook_result.stdout.is_empty() {
            println!("  Output:");
            for line in hook_result.stdout.lines() {
                println!("    {}", line);
            }
        }

        if !hook_result.stderr.is_empty() {
            println!("  Errors:");
            for line in hook_result.stderr.lines() {
                println!("    {}", line);
            }
        }

        println!();
    }

    println!("Summary");
    println!("-------");
    println!("Total hooks: {}", result.hooks.len());
    println!(
        "Passed: {}",
        result.hooks.iter().filter(|h| h.success).count()
    );
    println!(
        "Failed: {}",
        result.hooks.iter().filter(|h| !h.success).count()
    );
    println!("Total time: {}ms", result.total_duration_ms);
    println!(
        "\nResult: {}",
        if result.all_passed {
            "SUCCESS"
        } else {
            "FAILURE"
        }
    );
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Parse and validate config
    let config = parse_config_file(&cli.config)?;
    validate_config(&config)?;

    // Extract hooks
    let hooks = extract_hooks(&config);

    if hooks.is_empty() {
        eprintln!("No hooks to run");
        return Ok(());
    }

    // Get files to check
    let files_to_check = if cli.files.is_empty() {
        get_all_files()?
    } else {
        cli.files
    };

    // Execute hooks
    let result = if cli.parallel {
        let builder = DagBuilder::new();
        let plan = builder.build_plan(&hooks)?;
        let executor = ParallelExecutor::new(plan);
        executor.execute(&hooks, &files_to_check)?
    } else {
        let executor = SyncExecutor::new();
        executor.execute(&hooks, &files_to_check)?
    };

    // Output results
    match cli.format {
        OutputFormat::Json => output_json(&result)?,
        OutputFormat::Human => output_human(&result),
    }

    // Exit with appropriate code
    if !result.all_passed {
        process::exit(1);
    }

    Ok(())
}
