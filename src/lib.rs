#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::assertion_density,
        tigerstyle::numeric_units,
        tigerstyle::function_length,
        tigerstyle::explicit_defaults,
        tigerstyle::ambient_clock,
        tigerstyle::usize_in_public_api,
        tigerstyle::unbounded_collection_growth,
        tigerstyle::raw_arithmetic_overflow,
        tigerstyle::compound_condition,
        tigerstyle::nested_conditionals,
        tigerstyle::no_unwrap,
        tigerstyle::no_panic,
        tigerstyle::unbounded_channel,
        tigerstyle::unbounded_loop,
        tigerstyle::ambiguous_params,
        tigerstyle::too_many_parameters,
        tigerstyle::bool_naming,
        tigerstyle::ignored_result,
        tigerstyle::sentinel_fallback,
        tigerstyle::unchecked_narrowing,
        tigerstyle::platform_dependent_cast,
        tigerstyle::multi_lock_ordering,
        tigerstyle::contradictory_time,
        tigerstyle::no_recursion,
        tigerstyle::catch_all_on_enum,
        tigerstyle::unjustified_allow,
        reason = "root crate is CLI/orchestration shell across existing daemon, tool, and mode contracts; behavior is covered by focused integration tests during Tigerstyle drain"
    )
)]

pub use clankers_agent_defs as agent_defs;
pub mod capability_gate;
pub mod checkpoints;
pub mod cli;
pub mod commands;
pub mod error;
pub mod event_translator;
pub mod modes;
pub mod plugin;
pub mod runtime_prompt;
pub mod runtime_services;
pub mod self_evolution;
pub(crate) mod session;
pub mod slash_commands;
pub mod soul_personality;
pub mod tool_gateway;
pub mod tools;
pub mod tui_config;
pub mod voice_mode;
pub mod worktree;
#[cfg(feature = "zellij-share")]
pub use clankers_zellij as zellij;
