use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use pre_commit_core::{ExecutionPlan, Hook, PlanBuilder, PreCommitError, Result};
use std::collections::HashMap;

/// Builds an execution plan from a list of hooks with dependencies
pub struct DagBuilder;

impl DagBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Build a directed acyclic graph from hooks
    fn build_graph(hooks: &[Hook]) -> Result<DiGraph<Hook, ()>> {
        let mut graph = DiGraph::new();
        let mut hook_indices: HashMap<String, NodeIndex> = HashMap::new();

        // Add all hooks as nodes
        for hook in hooks {
            let idx = graph.add_node(hook.clone());
            hook_indices.insert(hook.id.clone(), idx);
        }

        // Add edges for dependencies
        for hook in hooks {
            let hook_idx = hook_indices[&hook.id];
            for dep_id in &hook.depends_on {
                let dep_idx = hook_indices
                    .get(dep_id)
                    .ok_or_else(|| PreCommitError::HookNotFound(dep_id.clone()))?;
                // Edge from dependency to dependent (dep must run before hook)
                graph.add_edge(*dep_idx, hook_idx, ());
            }
        }

        Ok(graph)
    }

    /// Compute execution levels where all hooks in a level can run in parallel
    fn compute_levels(graph: &DiGraph<Hook, ()>) -> Result<Vec<Vec<Hook>>> {
        // Topologically sort the graph
        let sorted = toposort(graph, None).map_err(|_| PreCommitError::CycleDetected)?;

        // Compute the depth of each node
        let mut depths: HashMap<NodeIndex, usize> = HashMap::new();

        for &node_idx in &sorted {
            let incoming_edges = graph.edges_directed(node_idx, petgraph::Direction::Incoming);
            let max_parent_depth = incoming_edges
                .map(|edge| depths.get(&edge.source()).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);

            depths.insert(node_idx, max_parent_depth + 1);
        }

        // Group hooks by depth
        let max_depth = depths.values().max().copied().unwrap_or(0);
        let mut levels: Vec<Vec<Hook>> = vec![Vec::new(); max_depth];

        for (node_idx, depth) in depths {
            let hook = &graph[node_idx];
            levels[depth - 1].push(hook.clone());
        }

        Ok(levels)
    }
}

impl Default for DagBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanBuilder for DagBuilder {
    fn build_plan(&self, hooks: &[Hook]) -> Result<ExecutionPlan> {
        if hooks.is_empty() {
            return Ok(ExecutionPlan::new(vec![]));
        }

        let graph = Self::build_graph(hooks)?;
        let levels = Self::compute_levels(&graph)?;

        Ok(ExecutionPlan::new(levels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hook(id: &str, depends_on: Vec<&str>) -> Hook {
        Hook {
            id: id.to_string(),
            name: format!("Hook {}", id),
            entry: format!("echo {}", id),
            language: "system".to_string(),
            files: None,
            pass_filenames: false,
            depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_no_dependencies() {
        let hooks = vec![
            make_hook("a", vec![]),
            make_hook("b", vec![]),
            make_hook("c", vec![]),
        ];

        let builder = DagBuilder::new();
        let plan = builder.build_plan(&hooks).unwrap();

        // All hooks should be in the same level
        assert_eq!(plan.levels.len(), 1);
        assert_eq!(plan.levels[0].len(), 3);
    }

    #[test]
    fn test_linear_dependencies() {
        let hooks = vec![
            make_hook("a", vec![]),
            make_hook("b", vec!["a"]),
            make_hook("c", vec!["b"]),
        ];

        let builder = DagBuilder::new();
        let plan = builder.build_plan(&hooks).unwrap();

        // Should have 3 levels
        assert_eq!(plan.levels.len(), 3);
        assert_eq!(plan.levels[0][0].id, "a");
        assert_eq!(plan.levels[1][0].id, "b");
        assert_eq!(plan.levels[2][0].id, "c");
    }

    #[test]
    fn test_parallel_dependencies() {
        //     a
        //    / \
        //   b   c
        //    \ /
        //     d
        let hooks = vec![
            make_hook("a", vec![]),
            make_hook("b", vec!["a"]),
            make_hook("c", vec!["a"]),
            make_hook("d", vec!["b", "c"]),
        ];

        let builder = DagBuilder::new();
        let plan = builder.build_plan(&hooks).unwrap();

        // Should have 3 levels
        assert_eq!(plan.levels.len(), 3);
        assert_eq!(plan.levels[0].len(), 1); // a
        assert_eq!(plan.levels[1].len(), 2); // b and c
        assert_eq!(plan.levels[2].len(), 1); // d

        assert_eq!(plan.levels[0][0].id, "a");
        assert!(plan.levels[1].iter().any(|h| h.id == "b"));
        assert!(plan.levels[1].iter().any(|h| h.id == "c"));
        assert_eq!(plan.levels[2][0].id, "d");
    }

    #[test]
    fn test_cycle_detection() {
        let hooks = vec![make_hook("a", vec!["b"]), make_hook("b", vec!["a"])];

        let builder = DagBuilder::new();
        let result = builder.build_plan(&hooks);

        assert!(matches!(result, Err(PreCommitError::CycleDetected)));
    }

    #[test]
    fn test_missing_dependency() {
        let hooks = vec![make_hook("a", vec!["nonexistent"])];

        let builder = DagBuilder::new();
        let result = builder.build_plan(&hooks);

        assert!(matches!(result, Err(PreCommitError::HookNotFound(_))));
    }

    #[test]
    fn test_empty_hooks() {
        let hooks = vec![];
        let builder = DagBuilder::new();
        let plan = builder.build_plan(&hooks).unwrap();

        assert_eq!(plan.levels.len(), 0);
    }

    #[test]
    fn test_complex_dag() {
        //       a
        //      / \
        //     b   c
        //     |   |\
        //     d   e f
        //      \ /
        //       g
        let hooks = vec![
            make_hook("a", vec![]),
            make_hook("b", vec!["a"]),
            make_hook("c", vec!["a"]),
            make_hook("d", vec!["b"]),
            make_hook("e", vec!["c"]),
            make_hook("f", vec!["c"]),
            make_hook("g", vec!["d", "e"]),
        ];

        let builder = DagBuilder::new();
        let plan = builder.build_plan(&hooks).unwrap();

        // Verify structure
        assert_eq!(plan.levels[0].len(), 1); // a
        assert_eq!(plan.levels[1].len(), 2); // b, c

        // d, e, f at level 2
        assert!(plan.levels[2].iter().any(|h| h.id == "d"));
        assert!(plan.levels[2].iter().any(|h| h.id == "e"));
        assert!(plan.levels[2].iter().any(|h| h.id == "f"));

        // g depends on both d and e, so it should be after them
        let g_level = plan
            .levels
            .iter()
            .position(|level| level.iter().any(|h| h.id == "g"))
            .unwrap();
        let d_level = plan
            .levels
            .iter()
            .position(|level| level.iter().any(|h| h.id == "d"))
            .unwrap();
        let e_level = plan
            .levels
            .iter()
            .position(|level| level.iter().any(|h| h.id == "e"))
            .unwrap();

        assert!(g_level > d_level);
        assert!(g_level > e_level);
    }
}
