use pre_commit_core::{ExecutionPlan, ExecutionResult, Executor, Hook, HookResult, Result};
use regex::Regex;
use std::path::PathBuf;
use std::time::Instant;
use tokio::process::Command;

/// Parallel executor that runs hooks respecting dependencies
pub struct ParallelExecutor {
    plan: ExecutionPlan,
}

impl ParallelExecutor {
    pub fn new(plan: ExecutionPlan) -> Self {
        Self { plan }
    }

    /// Filter files based on the hook's file pattern
    fn filter_files(hook: &Hook, files: &[PathBuf]) -> Vec<PathBuf> {
        if let Some(pattern) = &hook.files {
            if let Ok(regex) = Regex::new(pattern) {
                return files
                    .iter()
                    .filter(|f| f.to_str().map(|s| regex.is_match(s)).unwrap_or(false))
                    .cloned()
                    .collect();
            }
        }
        files.to_vec()
    }

    /// Execute a single hook asynchronously
    async fn execute_hook_async(hook: &Hook, files: &[PathBuf]) -> HookResult {
        let start = Instant::now();

        // Filter files if needed
        let filtered_files = Self::filter_files(hook, files);

        // Build command
        let mut parts =
            shell_words::split(&hook.entry).unwrap_or_else(|_| vec![hook.entry.clone()]);

        if hook.pass_filenames && !filtered_files.is_empty() {
            for file in &filtered_files {
                if let Some(s) = file.to_str() {
                    parts.push(s.to_string());
                }
            }
        }

        // Execute command with color support
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

        match result {
            Ok(output) => HookResult {
                hook_id: hook.id.clone(),
                success: output.status.success(),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                duration_ms: duration.as_millis() as u64,
            },
            Err(e) => HookResult {
                hook_id: hook.id.clone(),
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("Failed to execute command: {}", e),
                duration_ms: duration.as_millis() as u64,
            },
        }
    }

    /// Execute all hooks in a level in parallel
    async fn execute_level(hooks: &[Hook], files: &[PathBuf]) -> Vec<HookResult> {
        let futures = hooks
            .iter()
            .map(|hook| Self::execute_hook_async(hook, files));

        futures::future::join_all(futures).await
    }

    /// Execute the plan with proper dependency ordering
    pub async fn execute_async(&self, files: &[PathBuf]) -> Result<ExecutionResult> {
        let start = Instant::now();
        let mut all_results = Vec::new();

        // Execute each level sequentially, but hooks within a level in parallel
        for level in &self.plan.levels {
            let level_results = Self::execute_level(level, files).await;
            all_results.extend(level_results);
        }

        let total_duration = start.elapsed();
        let all_passed = all_results.iter().all(|r| r.success);

        Ok(ExecutionResult {
            hooks: all_results,
            total_duration_ms: total_duration.as_millis() as u64,
            all_passed,
        })
    }
}

impl Executor for ParallelExecutor {
    fn execute(&self, _hooks: &[Hook], files: &[PathBuf]) -> Result<ExecutionResult> {
        // Use tokio runtime to execute async code
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.execute_async(files))
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use pre_commit_core::ExecutionPlan;

    #[test]
    fn test_shell_words_split() {
        assert_eq!(
            shell_words::split("echo hello world").unwrap(),
            vec!["echo", "hello", "world"]
        );
        assert_eq!(
            shell_words::split("echo 'hello world'").unwrap(),
            vec!["echo", "hello world"]
        );
    }

    #[tokio::test]
    async fn test_execute_hook_async() {
        let hook = Hook {
            id: "echo-test".to_string(),
            name: "Echo Test".to_string(),
            entry: "echo hello".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec![],
        };

        let result = ParallelExecutor::execute_hook_async(&hook, &[]).await;
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_level() {
        let hooks = vec![
            Hook {
                id: "hook1".to_string(),
                name: "Hook 1".to_string(),
                entry: "echo first".to_string(),
                language: "system".to_string(),
                files: None,
                pass_filenames: false,
                depends_on: vec![],
            },
            Hook {
                id: "hook2".to_string(),
                name: "Hook 2".to_string(),
                entry: "echo second".to_string(),
                language: "system".to_string(),
                files: None,
                pass_filenames: false,
                depends_on: vec![],
            },
        ];

        let results = ParallelExecutor::execute_level(&hooks, &[]).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
    }

    #[tokio::test]
    async fn test_parallel_executor() {
        let hooks = vec![
            Hook {
                id: "hook1".to_string(),
                name: "Hook 1".to_string(),
                entry: "echo first".to_string(),
                language: "system".to_string(),
                files: None,
                pass_filenames: false,
                depends_on: vec![],
            },
            Hook {
                id: "hook2".to_string(),
                name: "Hook 2".to_string(),
                entry: "echo second".to_string(),
                language: "system".to_string(),
                files: None,
                pass_filenames: false,
                depends_on: vec![],
            },
        ];

        let plan = ExecutionPlan::new(vec![hooks]);
        let executor = ParallelExecutor::new(plan);
        let result = executor.execute_async(&[]).await.unwrap();

        assert_eq!(result.hooks.len(), 2);
        assert!(result.all_passed);
    }

    #[test]
    fn test_filter_files() {
        let hook = Hook {
            id: "test".to_string(),
            name: "Test".to_string(),
            entry: "echo".to_string(),
            language: "system".to_string(),
            files: Some("\\.rs$".to_string()),
            pass_filenames: false,
            depends_on: vec![],
        };

        let files = vec![
            PathBuf::from("test.rs"),
            PathBuf::from("test.txt"),
            PathBuf::from("main.rs"),
        ];
        let filtered = ParallelExecutor::filter_files(&hook, &files);
        assert_eq!(filtered.len(), 2);
    }
}
