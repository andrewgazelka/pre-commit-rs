use pre_commit_core::{Config, Hook, PreCommitError, Result};
use std::fs;
use std::path::Path;

/// Parse a pre-commit configuration from a file
pub fn parse_config_file<P: AsRef<Path>>(path: P) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    parse_config(&content)
}

/// Parse a pre-commit configuration from a string
pub fn parse_config(content: &str) -> Result<Config> {
    serde_yaml::from_str(content)
        .map_err(|e| PreCommitError::Parse(format!("Failed to parse YAML: {}", e)))
}

/// Extract all hooks from a configuration
pub fn extract_hooks(config: &Config) -> Vec<Hook> {
    config
        .repos
        .iter()
        .flat_map(|repo| repo.hooks.iter().cloned())
        .collect()
}

/// Validate that all hook IDs are unique
pub fn validate_unique_ids(hooks: &[Hook]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for hook in hooks {
        if !seen.insert(&hook.id) {
            return Err(PreCommitError::Parse(format!(
                "Duplicate hook ID: {}",
                hook.id
            )));
        }
    }
    Ok(())
}

/// Validate that all dependencies exist
pub fn validate_dependencies(hooks: &[Hook]) -> Result<()> {
    let ids: std::collections::HashSet<_> = hooks.iter().map(|h| &h.id).collect();

    for hook in hooks {
        for dep in &hook.depends_on {
            if !ids.contains(dep) {
                return Err(PreCommitError::HookNotFound(format!(
                    "Hook '{}' depends on non-existent hook '{}'",
                    hook.id, dep
                )));
            }
        }
    }
    Ok(())
}

/// Validate an entire configuration
pub fn validate_config(config: &Config) -> Result<()> {
    let hooks = extract_hooks(config);
    validate_unique_ids(&hooks)?;
    validate_dependencies(&hooks)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pre_commit_core::Repo;

    #[test]
    fn test_parse_simple_config() {
        let yaml = r#"
repos:
  - repo: local
    hooks:
      - id: test-hook
        name: Test Hook
        entry: echo "test"
        language: system
        files: \.rs$
        pass_filenames: false
"#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(config.repos.len(), 1);
        assert_eq!(config.repos[0].hooks.len(), 1);
        assert_eq!(config.repos[0].hooks[0].id, "test-hook");
    }

    #[test]
    fn test_parse_with_dependencies() {
        let yaml = r#"
repos:
  - repo: local
    hooks:
      - id: hook1
        name: Hook 1
        entry: echo "1"
        language: system
      - id: hook2
        name: Hook 2
        entry: echo "2"
        language: system
        depends_on:
          - hook1
"#;
        let config = parse_config(yaml).unwrap();
        let hooks = extract_hooks(&config);
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[1].depends_on, vec!["hook1"]);
    }

    #[test]
    fn test_validate_unique_ids() {
        let hook1 = Hook {
            id: "test".to_string(),
            name: "Test".to_string(),
            entry: "echo".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec![],
        };
        let hook2 = hook1.clone();

        let result = validate_unique_ids(&[hook1, hook2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_dependencies_success() {
        let hook1 = Hook {
            id: "hook1".to_string(),
            name: "Hook 1".to_string(),
            entry: "echo".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec![],
        };
        let hook2 = Hook {
            id: "hook2".to_string(),
            name: "Hook 2".to_string(),
            entry: "echo".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec!["hook1".to_string()],
        };

        let result = validate_dependencies(&[hook1, hook2]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_dependencies_missing() {
        let hook = Hook {
            id: "hook1".to_string(),
            name: "Hook 1".to_string(),
            entry: "echo".to_string(),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: vec!["nonexistent".to_string()],
        };

        let result = validate_dependencies(&[hook]);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_hooks() {
        let config = Config {
            repos: vec![
                Repo {
                    repo: "local".to_string(),
                    hooks: vec![Hook {
                        id: "hook1".to_string(),
                        name: "Hook 1".to_string(),
                        entry: "echo".to_string(),
                        language: "system".to_string(),
                        files: None,
                        pass_filenames: false,
                        depends_on: vec![],
                    }],
                },
                Repo {
                    repo: "local".to_string(),
                    hooks: vec![Hook {
                        id: "hook2".to_string(),
                        name: "Hook 2".to_string(),
                        entry: "echo".to_string(),
                        language: "system".to_string(),
                        files: None,
                        pass_filenames: false,
                        depends_on: vec![],
                    }],
                },
            ],
        };

        let hooks = extract_hooks(&config);
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].id, "hook1");
        assert_eq!(hooks[1].id, "hook2");
    }
}
