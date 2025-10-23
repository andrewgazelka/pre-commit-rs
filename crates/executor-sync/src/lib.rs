use pre_commit_core::{ExecutionResult, Executor, Hook, HookResult, Result};
use regex::Regex;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

/// Sequential executor that runs hooks one at a time
pub struct SyncExecutor;

impl SyncExecutor {
    pub fn new() -> Self {
        Self
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

    /// Execute a single hook
    fn execute_hook(hook: &Hook, files: &[PathBuf]) -> HookResult {
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
}

impl Default for SyncExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor for SyncExecutor {
    fn execute(&self, hooks: &[Hook], files: &[PathBuf]) -> Result<ExecutionResult> {
        let start = Instant::now();
        let mut results = Vec::new();

        for hook in hooks {
            let result = Self::execute_hook(hook, files);
            results.push(result);
        }

        let total_duration = start.elapsed();
        let all_passed = results.iter().all(|r| r.success);

        Ok(ExecutionResult {
            hooks: results,
            total_duration_ms: total_duration.as_millis() as u64,
            all_passed,
        })
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
        assert_eq!(
            shell_words::split("echo \"hello world\"").unwrap(),
            vec!["echo", "hello world"]
        );
    }

    #[test]
    fn test_filter_files_no_pattern() {
        let hook = Hook {
            id: "test".to_string(),
            name: "Test".to_string(),
            entry: "echo".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec![],
        };

        let files = vec![PathBuf::from("test.rs"), PathBuf::from("test.txt")];
        let filtered = SyncExecutor::filter_files(&hook, &files);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_files_with_pattern() {
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
        let filtered = SyncExecutor::filter_files(&hook, &files);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&PathBuf::from("test.rs")));
        assert!(filtered.contains(&PathBuf::from("main.rs")));
    }

    #[test]
    fn test_execute_simple_hook() {
        let hook = Hook {
            id: "echo-test".to_string(),
            name: "Echo Test".to_string(),
            entry: "echo hello".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec![],
        };

        let result = SyncExecutor::execute_hook(&hook, &[]);
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
    }

    #[test]
    fn test_executor_multiple_hooks() {
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

        let executor = SyncExecutor::new();
        let result = executor.execute(&hooks, &[]).unwrap();

        assert_eq!(result.hooks.len(), 2);
        assert!(result.all_passed);
        assert!(result.hooks[0].stdout.contains("first"));
        assert!(result.hooks[1].stdout.contains("second"));
    }

    #[test]
    fn test_executor_failing_hook() {
        let hooks = vec![Hook {
            id: "failing".to_string(),
            name: "Failing Hook".to_string(),
            entry: "false".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec![],
        }];

        let executor = SyncExecutor::new();
        let result = executor.execute(&hooks, &[]).unwrap();

        assert_eq!(result.hooks.len(), 1);
        assert!(!result.all_passed);
        assert!(!result.hooks[0].success);
    }
}
