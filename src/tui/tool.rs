use std::collections::HashSet;
use crate::config::Config;

/// Which central config field a row represents
#[derive(Debug, Clone, PartialEq)]
pub enum CentralField {
    Config,
    Prompt,
    Skills,
    Agents,
    Source,
}

/// Which tool-specific field a row represents
#[derive(Debug, Clone, PartialEq)]
pub enum ToolField {
    Prompt,
    Skills,
    Agents,
    Settings,
    Auth,
    Mcp,
}

/// A single row in the tool TUI list
#[derive(Debug, Clone)]
pub enum ToolRow {
    CentralHeader,
    CentralItem(CentralField),
    ToolHeader {
        key: String,
        name: String,
        installed: bool,
    },
    ToolItem {
        tool_key: String,
        field: ToolField,
    },
}

/// Build the flat list of rows from config state and expanded sections.
/// `expanded` contains keys of sections that are currently open ("central", tool keys).
pub fn build_rows(config: &Config, expanded: &HashSet<String>) -> Vec<ToolRow> {
    let mut rows = Vec::new();

    // Central section
    rows.push(ToolRow::CentralHeader);
    if expanded.contains("central") {
        rows.push(ToolRow::CentralItem(CentralField::Config));
        rows.push(ToolRow::CentralItem(CentralField::Prompt));
        rows.push(ToolRow::CentralItem(CentralField::Skills));
        rows.push(ToolRow::CentralItem(CentralField::Agents));
        rows.push(ToolRow::CentralItem(CentralField::Source));
    }

    // Tool sections — BTreeMap gives alphabetical order
    for (key, tool) in &config.tools {
        let installed = tool.is_installed();
        rows.push(ToolRow::ToolHeader {
            key: key.clone(),
            name: tool.name.clone(),
            installed,
        });
        if expanded.contains(key) {
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Prompt });
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Skills });
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Agents });
            if !tool.settings.is_empty() {
                rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Settings });
            }
            if !tool.auth.is_empty() {
                rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Auth });
            }
            if !tool.mcp.is_empty() {
                rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Mcp });
            }
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CentralConfig, ToolConfig};
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    fn test_config_with_tools(tools: Vec<(&str, ToolConfig)>) -> Config {
        let mut tools_map = BTreeMap::new();
        for (key, tool) in tools {
            tools_map.insert(key.to_string(), tool);
        }
        Config {
            editor: String::new(),
            central: CentralConfig {
                prompt_source: "~/.local/share/agm/prompts/MASTER.md".to_string(),
                skills_source: "~/.local/share/agm/skills".to_string(),
                agents_source: "~/.local/share/agm/agents".to_string(),
                source_dir: "~/.local/share/agm/source".to_string(),
                source_repos: vec![],
            },
            tools: tools_map,
        }
    }

    fn test_tool_config(name: &str, config_dir: &str, with_optional: bool) -> ToolConfig {
        ToolConfig {
            name: name.to_string(),
            config_dir: config_dir.to_string(),
            settings: if with_optional { vec!["settings.json".to_string()] } else { vec![] },
            auth: if with_optional { vec!["auth.json".to_string()] } else { vec![] },
            prompt_filename: "PROMPT.md".to_string(),
            skills_dir: "skills".to_string(),
            agents_dir: "agents".to_string(),
            mcp: if with_optional { vec!["mcp.json".to_string()] } else { vec![] },
        }
    }

    #[test]
    fn test_build_rows_all_collapsed() {
        let config = test_config_with_tools(vec![
            ("claude", test_tool_config("Claude Code", "/nonexistent/claude", true)),
            ("copilot", test_tool_config("Copilot CLI", "/nonexistent/copilot", true)),
        ]);
        let expanded = HashSet::new();
        let rows = build_rows(&config, &expanded);

        // Should have 1 central header + 2 tool headers
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "claude"));
        assert!(matches!(rows[2], ToolRow::ToolHeader { ref key, .. } if key == "copilot"));
    }

    #[test]
    fn test_build_rows_central_expanded() {
        let config = test_config_with_tools(vec![
            ("claude", test_tool_config("Claude Code", "/nonexistent/claude", true)),
        ]);
        let mut expanded = HashSet::new();
        expanded.insert("central".to_string());
        let rows = build_rows(&config, &expanded);

        // Should have central header + 5 central items + 1 tool header
        assert_eq!(rows.len(), 7);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::CentralItem(CentralField::Config)));
        assert!(matches!(rows[2], ToolRow::CentralItem(CentralField::Prompt)));
        assert!(matches!(rows[3], ToolRow::CentralItem(CentralField::Skills)));
        assert!(matches!(rows[4], ToolRow::CentralItem(CentralField::Agents)));
        assert!(matches!(rows[5], ToolRow::CentralItem(CentralField::Source)));
        assert!(matches!(rows[6], ToolRow::ToolHeader { ref key, .. } if key == "claude"));
    }

    #[test]
    fn test_build_rows_tool_expanded() {
        let _tmp = TempDir::new().unwrap();
        let tool_dir = _tmp.path().join("claude");
        std::fs::create_dir_all(&tool_dir).unwrap();
        
        let config = test_config_with_tools(vec![
            ("claude", test_tool_config("Claude Code", &tool_dir.to_string_lossy(), true)),
        ]);
        let mut expanded = HashSet::new();
        expanded.insert("claude".to_string());
        let rows = build_rows(&config, &expanded);

        // Should have central header + tool header + 6 tool items (Prompt, Skills, Agents, Settings, Auth, Mcp)
        assert_eq!(rows.len(), 8);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, installed: true, .. } if key == "claude"));
        assert!(matches!(rows[2], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Prompt));
        assert!(matches!(rows[3], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Skills));
        assert!(matches!(rows[4], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Agents));
        assert!(matches!(rows[5], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Settings));
        assert!(matches!(rows[6], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Auth));
        assert!(matches!(rows[7], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Mcp));
    }

    #[test]
    fn test_build_rows_empty_vec_skipped() {
        let _tmp = TempDir::new().unwrap();
        let tool_dir = _tmp.path().join("minimal");
        std::fs::create_dir_all(&tool_dir).unwrap();
        
        let config = test_config_with_tools(vec![
            ("minimal", test_tool_config("Minimal Tool", &tool_dir.to_string_lossy(), false)),
        ]);
        let mut expanded = HashSet::new();
        expanded.insert("minimal".to_string());
        let rows = build_rows(&config, &expanded);

        // Should have central header + tool header + 3 tool items (only Prompt, Skills, Agents)
        assert_eq!(rows.len(), 5);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "minimal"));
        assert!(matches!(rows[2], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Prompt));
        assert!(matches!(rows[3], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Skills));
        assert!(matches!(rows[4], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Agents));

        // Verify no Settings/Auth/Mcp items
        for row in &rows {
            if let ToolRow::ToolItem { field, .. } = row {
                assert!(!matches!(field, ToolField::Settings | ToolField::Auth | ToolField::Mcp));
            }
        }
    }

    #[test]
    fn test_build_rows_alphabetical() {
        let config = test_config_with_tools(vec![
            ("zed", test_tool_config("Zed Editor", "/nonexistent/zed", true)),
            ("alpha", test_tool_config("Alpha Tool", "/nonexistent/alpha", true)),
        ]);
        let expanded = HashSet::new();
        let rows = build_rows(&config, &expanded);

        // Should have central header + alpha tool header + zed tool header (alphabetical order)
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "alpha"));
        assert!(matches!(rows[2], ToolRow::ToolHeader { ref key, .. } if key == "zed"));
    }
}