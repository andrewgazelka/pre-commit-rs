use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PreCommitError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Execution error: {0}")]
    Execution(String),
    #[error("Cycle detected in hook dependencies")]
    CycleDetected,
    #[error("Hook not found: {0}")]
    HookNotFound(String),
}

pub type Result<T> = std::result::Result<T, PreCommitError>;

/// Represents a single hook configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hook {
    pub id: String,
    pub name: String,
    pub entry: String,
    pub language: String,
    #[serde(default)]
    pub files: Option<String>,
    #[serde(default)]
    pub pass_filenames: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Represents a repository with hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub repo: String,
    pub hooks: Vec<Hook>,
}

/// The complete pre-commit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub repos: Vec<Repo>,
}

/// Result of executing a single hook
#[derive(Debug, Clone, Serialize)]
pub struct HookResult {
    pub hook_id: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

/// Result of executing all hooks
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionResult {
    pub hooks: Vec<HookResult>,
    pub total_duration_ms: u64,
    pub all_passed: bool,
}

/// Trait for executing hooks
pub trait Executor {
    fn execute(&self, hooks: &[Hook], files: &[PathBuf]) -> Result<ExecutionResult>;
}

/// Trait for building execution plan from hooks
pub trait PlanBuilder {
    fn build_plan(&self, hooks: &[Hook]) -> Result<ExecutionPlan>;
}

/// Execution plan with dependency ordering
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Hooks grouped by execution level (all hooks in a level can run in parallel)
    pub levels: Vec<Vec<Hook>>,
}

impl ExecutionPlan {
    pub fn new(levels: Vec<Vec<Hook>>) -> Self {
        Self { levels }
    }

    /// Get all hooks in sequential order
    pub fn sequential(&self) -> Vec<Hook> {
        self.levels
            .iter()
            .flat_map(|level| level.iter().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_creation() {
        let hook = Hook {
            id: "test".to_string(),
            name: "Test Hook".to_string(),
            entry: "cargo test".to_string(),
            language: "system".to_string(),
            files: Some("\\.rs$".to_string()),
            pass_filenames: false,
            depends_on: vec![],
        };
        assert_eq!(hook.id, "test");
        assert!(!hook.pass_filenames);
    }

    #[test]
    fn test_execution_plan_sequential() {
        let hook1 = Hook {
            id: "hook1".to_string(),
            name: "Hook 1".to_string(),
            entry: "echo 1".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec![],
        };
        let hook2 = Hook {
            id: "hook2".to_string(),
            name: "Hook 2".to_string(),
            entry: "echo 2".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec![],
        };

        let plan = ExecutionPlan::new(vec![vec![hook1.clone()], vec![hook2.clone()]]);
        let sequential = plan.sequential();
        assert_eq!(sequential.len(), 2);
        assert_eq!(sequential[0].id, "hook1");
        assert_eq!(sequential[1].id, "hook2");
    }
}
