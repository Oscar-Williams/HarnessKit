// PenguinHarness agent state — verified against
// github.com/Prism-Shadow/penguin-harness @ 9eebc91c09175b16150a5a0bb6ecc46ba28e066c:
// - Data root: `$PENGUIN_HOME` or `~/.penguin/data`.
// - Current layout: `<root>/<project>/agents/<agent>/agent_state/`.
// - The issue's earlier direct layout (`<root>/agents/<agent>/agent_state/`)
//   is also accepted so existing installations remain discoverable.
// - Agent state owns `AGENTS.md`, `skills/`, `memory/`, and
//   `system_config.yaml`.
// - MCP, plugins, hooks, and project-scoped extensions are intentionally
//   read-only/unsupported until their native formats have been verified.

use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry, ProjectMarker, RemoteMcpSchema};
use std::path::{Path, PathBuf};

pub struct PenguinAdapter {
    penguin_home: PathBuf,
}

impl Default for PenguinAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve the data root without mutating process state, so the path contract
/// can be tested without racing other adapter tests.
fn resolve_home(penguin_home: Option<std::ffi::OsString>, home: &Path) -> PathBuf {
    penguin_home
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".penguin").join("data"))
}

impl PenguinAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self {
            penguin_home: resolve_home(std::env::var_os("PENGUIN_HOME"), &home),
        }
    }

    /// Test constructor matching PenguinHarness's default root below `home`.
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            penguin_home: home.join(".penguin").join("data"),
        }
    }

    /// Test constructor for an explicit `PENGUIN_HOME`-style data root.
    pub fn with_root(penguin_home: PathBuf) -> Self {
        Self { penguin_home }
    }

    /// Return both layouts that have existed in the documented/issue-level
    /// contract: a direct `<root>/agents` container and the current
    /// `<root>/<project>/agents` containers. Sorting keeps scan output stable.
    fn agent_containers(&self) -> Vec<PathBuf> {
        let mut containers = Vec::new();
        let direct = self.penguin_home.join("agents");
        if direct.is_dir() {
            containers.push(direct);
        }

        if let Ok(entries) = std::fs::read_dir(&self.penguin_home) {
            for entry in entries.flatten() {
                let project_dir = entry.path();
                if !project_dir.is_dir()
                    || project_dir.file_name().is_some_and(|name| name == "agents")
                {
                    continue;
                }
                let agents = project_dir.join("agents");
                if agents.is_dir() {
                    containers.push(agents);
                }
            }
        }

        containers.sort();
        containers.dedup();
        containers
    }

    /// Resolve every existing Agent State directory, not just the default
    /// agent. PenguinHarness keeps each agent's skills and memory isolated.
    fn agent_state_dirs(&self) -> Vec<PathBuf> {
        let mut states = Vec::new();
        for agents_dir in self.agent_containers() {
            let Ok(entries) = std::fs::read_dir(agents_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let state = entry.path().join("agent_state");
                if state.is_dir() {
                    states.push(state);
                }
            }
        }
        states.sort();
        states.dedup();
        states
    }

    fn memory_files(&self) -> Vec<PathBuf> {
        let mut files = self
            .agent_state_dirs()
            .into_iter()
            .flat_map(|state| super::files_with_ext_recursive(&state.join("memory"), "md"))
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        files
    }
}

impl AgentAdapter for PenguinAdapter {
    fn name(&self) -> &str {
        "penguin"
    }

    fn base_dir(&self) -> PathBuf {
        self.penguin_home.clone()
    }

    fn detect(&self) -> bool {
        // PenguinHarness creates the data root before its first Project/Agent
        // is initialized. Presence of the root is therefore the same reliable
        // install signal used by the issue's accepted proposal.
        self.penguin_home.exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        self.agent_state_dirs()
            .into_iter()
            .map(|state| state.join("skills"))
            .collect()
    }

    // PenguinHarness has no generic global MCP file yet. These placeholder
    // paths satisfy the adapter contract, while the scope resolver below keeps
    // install/toggle paths unavailable instead of writing an invented format.
    fn mcp_config_path(&self) -> PathBuf {
        self.penguin_home.join("mcp.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.penguin_home.join("hooks.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        vec![]
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::Unsupported
    }

    fn supports_global_hook_install(&self) -> bool {
        false
    }

    fn mcp_config_path_for(&self, _scope: &crate::models::ConfigScope) -> Option<PathBuf> {
        None
    }

    fn hook_config_path_for(&self, _scope: &crate::models::ConfigScope) -> Option<PathBuf> {
        None
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        self.agent_state_dirs()
            .into_iter()
            .map(|state| state.join("AGENTS.md"))
            .collect()
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        self.memory_files()
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        self.agent_state_dirs()
            .into_iter()
            .map(|state| state.join("system_config.yaml"))
            .collect()
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        // PenguinHarness stores Projects under its own data root rather than
        // marking user workspaces, so it has no project marker to claim.
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn resolve_home_honors_override_and_default() {
        let home = Path::new("/home/u");
        assert_eq!(
            resolve_home(Some("/custom/penguin".into()), home),
            PathBuf::from("/custom/penguin")
        );
        assert_eq!(
            resolve_home(None, home),
            PathBuf::from("/home/u/.penguin/data")
        );
    }

    #[test]
    fn detect_requires_data_root() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PenguinAdapter::with_root(tmp.path().join("data"));
        assert!(!adapter.detect());
        std::fs::create_dir_all(adapter.base_dir()).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn scans_current_and_direct_agent_layouts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("penguin-data");
        let current = root.join("default_project/agents/researcher/agent_state");
        let direct = root.join("agents/reviewer/agent_state");
        std::fs::create_dir_all(current.join("skills/one")).unwrap();
        std::fs::create_dir_all(current.join("memory/user")).unwrap();
        std::fs::create_dir_all(direct.join("skills/two")).unwrap();
        std::fs::create_dir_all(direct.join("memory/workspace")).unwrap();
        std::fs::write(current.join("AGENTS.md"), "research rules").unwrap();
        std::fs::write(current.join("memory/user/MEMORY.md"), "- one").unwrap();
        std::fs::write(current.join("memory/user/preferences.md"), "prefers tests").unwrap();
        std::fs::write(direct.join("AGENTS.md"), "review rules").unwrap();
        std::fs::write(direct.join("memory/workspace/MEMORY.md"), "- two").unwrap();
        std::fs::write(direct.join("system_config.yaml"), "system_prompt: test").unwrap();

        let adapter = PenguinAdapter::with_root(root);
        let skills = adapter.skill_dirs();
        assert_eq!(skills.len(), 2);
        assert!(skills.contains(&current.join("skills")));
        assert!(skills.contains(&direct.join("skills")));

        let rules = adapter.global_rules_files();
        assert_eq!(rules.len(), 2);
        assert!(rules.contains(&current.join("AGENTS.md")));
        assert!(rules.contains(&direct.join("AGENTS.md")));

        let memory = adapter.global_memory_files();
        assert_eq!(memory.len(), 3);
        assert!(memory.contains(&current.join("memory/user/MEMORY.md")));
        assert!(memory.contains(&current.join("memory/user/preferences.md")));
        assert!(memory.contains(&direct.join("memory/workspace/MEMORY.md")));

        assert!(
            adapter
                .global_settings_files()
                .contains(&direct.join("system_config.yaml"))
        );
    }

    #[test]
    fn unsupported_formats_are_not_writable() {
        let adapter = PenguinAdapter::with_root(PathBuf::from("/tmp/penguin"));
        assert!(adapter.read_mcp_servers().is_empty());
        assert!(adapter.read_hooks().is_empty());
        assert!(adapter.plugin_dirs().is_empty());
        assert_eq!(adapter.hook_format(), HookFormat::None);
        assert!(!adapter.supports_global_hook_install());
        assert!(
            adapter
                .mcp_config_path_for(&crate::models::ConfigScope::Global)
                .is_none()
        );
        assert!(
            adapter
                .hook_config_path_for(&crate::models::ConfigScope::Global)
                .is_none()
        );
        assert!(adapter.project_markers().is_empty());
        assert!(adapter.project_skill_dirs().is_empty());
    }
}
