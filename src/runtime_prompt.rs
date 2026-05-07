//! Bridges existing Clankers prompt assembly into the embeddable runtime prompt service.

/// Assemble an existing Clankers system-prompt section list through `clankers-runtime`.
///
/// The input sections must come from
/// `clankers_agent::system_prompt::assemble_system_prompt_sections`. Keeping this bridge separate
/// lets embedding parity tests verify the runtime prompt service preserves the same section order
/// as the current CLI/TUI/daemon prompt path.
pub fn assemble_system_sections_with_runtime_service(
    sections: &[String],
    user_prompt: impl Into<String>,
) -> Result<clankers_runtime::AssembledPrompt, clankers_runtime::RuntimeError> {
    let sources = clankers_runtime::PromptSources {
        system_prompt: None,
        host_context: sections
            .iter()
            .enumerate()
            .map(|(index, content)| clankers_runtime::HostContext {
                label: format!("clankers_system_section_{index}"),
                content: content.clone(),
            })
            .collect(),
        filesystem_context_requested: false,
        context_references: Vec::new(),
    };
    clankers_runtime::PromptAssembler::assemble(
        &clankers_runtime::PromptAssemblyPolicy::host_context_only(),
        &sources,
        user_prompt.into(),
    )
}

#[cfg(test)]
mod tests {
    use clankers_agent::system_prompt::ContextFile;
    use clankers_agent::system_prompt::PromptResources;
    use clankers_agent::system_prompt::SoulPromptAssembly;
    use clankers_agent::system_prompt::assemble_system_prompt;
    use clankers_agent::system_prompt::assemble_system_prompt_sections;

    #[test]
    fn runtime_prompt_service_preserves_clankers_system_section_order() {
        let resources = PromptResources {
            skills: Vec::new(),
            prompts: Vec::new(),
            context_files: vec!["context-one".to_string(), "context-two".to_string()],
            agents_files: vec![ContextFile {
                path: std::path::PathBuf::from("AGENTS.md"),
                content: "agent rules".to_string(),
            }],
            soul_personality: SoulPromptAssembly::default(),
            spec_context: "spec context".to_string(),
            system_prompt_override: None,
            append_system_prompt: Some("appendix".to_string()),
            include_learning_guidance: false,
        };
        let sections = assemble_system_prompt_sections("base", &resources, Some("prefix"), Some("suffix"));
        let legacy_joined = assemble_system_prompt("base", &resources, Some("prefix"), Some("suffix"));

        let assembled = super::assemble_system_sections_with_runtime_service(&sections, "user prompt")
            .expect("runtime prompt service should assemble existing Clankers sections");
        let runtime_joined =
            assembled.sections.iter().map(|section| section.content.as_str()).collect::<Vec<_>>().join("\n\n");

        assert_eq!(runtime_joined, legacy_joined);
        assert_eq!(assembled.sections.first().expect("prefix section").content, "prefix");
        assert_eq!(assembled.sections.last().expect("suffix section").content, "suffix");
    }
}
