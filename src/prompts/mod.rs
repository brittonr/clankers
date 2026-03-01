//! Prompt templates

pub mod discovery;

pub use discovery::PromptTemplate;
pub use discovery::discover_prompts;
pub use discovery::expand_template;
pub use discovery::format_prompts_list;
