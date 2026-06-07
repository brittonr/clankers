//! Root-shell discovery adapters for agent-owned prompt resource DTOs.

use clankers_agent::system_prompt::PromptDiscoveryPaths;
use clankers_agent::system_prompt::PromptResources;
use clankers_agent::system_prompt::PromptSkill;
use clankers_agent::system_prompt::PromptTemplateInfo;

/// Discover prompt resources using concrete desktop prompt/skill crates, then
/// project them into the neutral DTOs consumed by `clankers-agent`.
pub fn discover_agent_prompt_resources(paths: &PromptDiscoveryPaths) -> PromptResources {
    let skills = clankers_skills::discover_skills(&paths.global_skills_dir, Some(&paths.project_skills_dir))
        .into_iter()
        .map(prompt_skill_from_desktop)
        .collect();
    let prompts = clankers_prompts::discover_prompts(&paths.global_prompts_dir, Some(&paths.project_prompts_dir))
        .into_iter()
        .map(prompt_template_from_desktop)
        .collect();

    clankers_agent::system_prompt::discover_resources_with_catalogs(paths, skills, prompts)
}

fn prompt_skill_from_desktop(skill: clankers_skills::Skill) -> PromptSkill {
    PromptSkill {
        name: skill.name,
        description: skill.description,
        path: skill.path,
        content: skill.content,
    }
}

fn prompt_template_from_desktop(prompt: clankers_prompts::PromptTemplate) -> PromptTemplateInfo {
    PromptTemplateInfo {
        name: prompt.name,
        description: prompt.description,
        path: prompt.path,
        content: prompt.content,
        variables: prompt.variables,
    }
}
