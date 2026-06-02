//! Source-level responsibility inventory for controller command handling.
//!
//! The inventory is intentionally static and consumed by architecture rails so
//! new command behavior has to declare whether it is translation, authorization,
//! runtime dispatch, persistence, continuation, or projection work.

#[allow(dead_code)]
pub(crate) const CONTROLLER_COMMAND_RESPONSIBILITY_DRAIN_REQUIREMENT: &str =
    "r[controller-command-responsibility-drain.responsibility-map]";

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CommandResponsibilityKind {
    Translation,
    Authorization,
    CoreInputConstruction,
    RuntimeDispatch,
    Persistence,
    Continuation,
    Projection,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandResponsibilityOwner {
    pub(crate) kind: CommandResponsibilityKind,
    pub(crate) owner_module: &'static str,
    pub(crate) replacement_path: &'static str,
    pub(crate) convergence: &'static str,
}

#[allow(dead_code)]
pub(crate) const COMMAND_RESPONSIBILITY_INVENTORY: &[CommandResponsibilityOwner] = &[
    CommandResponsibilityOwner {
        kind: CommandResponsibilityKind::Translation,
        owner_module: "crates/clankers-controller/src/command.rs",
        replacement_path: "command_thinking.rs for thinking labels; command.rs/auto_test.rs for remaining SessionCommand translation",
        convergence: "split each command family into a named policy module before adding new parsing branches",
    },
    CommandResponsibilityOwner {
        kind: CommandResponsibilityKind::Authorization,
        owner_module: "crates/clankers-controller/src/command.rs",
        replacement_path: "future command_authorization.rs",
        convergence: "extract session/prompt/manage authorization guards behind a narrow controller API",
    },
    CommandResponsibilityOwner {
        kind: CommandResponsibilityKind::CoreInputConstruction,
        owner_module: "crates/clankers-controller/src/command_thinking.rs",
        replacement_path: "command_thinking.rs owns thinking CoreInput construction; command.rs/auto_test.rs remain allowed translation shells",
        convergence: "move each command cluster's CoreInput construction to the cluster owner or core adapter",
    },
    CommandResponsibilityOwner {
        kind: CommandResponsibilityKind::RuntimeDispatch,
        owner_module: "crates/clankers-controller/src/runtime_adapter.rs",
        replacement_path: "ControllerRuntimeAdapter and AgentBackedRuntimeAdapter",
        convergence: "route direct agent control through runtime_adapter instead of command branches",
    },
    CommandResponsibilityOwner {
        kind: CommandResponsibilityKind::Persistence,
        owner_module: "crates/clankers-controller/src/persistence.rs",
        replacement_path: "SessionStore/ledger persistence service",
        convergence: "keep replay/resume storage outside command-policy modules",
    },
    CommandResponsibilityOwner {
        kind: CommandResponsibilityKind::Continuation,
        owner_module: "crates/clankers-controller/src/auto_test.rs and crates/clankers-controller/src/loop_mode.rs",
        replacement_path: "auto_test.rs / loop_mode.rs continuation owners",
        convergence: "follow-up prompts and loop continuation stay outside raw SessionCommand dispatch",
    },
    CommandResponsibilityOwner {
        kind: CommandResponsibilityKind::Projection,
        owner_module: "crates/clankers-controller/src/convert.rs and crates/clankers-controller/src/transport_convert.rs",
        replacement_path: "convert.rs / transport_convert.rs projection owners",
        convergence: "command modules call projection helpers instead of reconstructing protocol DTO families",
    },
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn command_responsibility_inventory_names_required_owners() {
        let kinds = COMMAND_RESPONSIBILITY_INVENTORY.iter().map(|entry| entry.kind).collect::<BTreeSet<_>>();
        for kind in [
            CommandResponsibilityKind::Translation,
            CommandResponsibilityKind::Authorization,
            CommandResponsibilityKind::CoreInputConstruction,
            CommandResponsibilityKind::RuntimeDispatch,
            CommandResponsibilityKind::Persistence,
            CommandResponsibilityKind::Continuation,
            CommandResponsibilityKind::Projection,
        ] {
            assert!(kinds.contains(&kind), "missing command responsibility owner for {kind:?}");
        }
        assert_eq!(
            CONTROLLER_COMMAND_RESPONSIBILITY_DRAIN_REQUIREMENT,
            "r[controller-command-responsibility-drain.responsibility-map]"
        );
        assert!(COMMAND_RESPONSIBILITY_INVENTORY.iter().all(|entry| {
            !entry.owner_module.is_empty() && !entry.replacement_path.is_empty() && !entry.convergence.is_empty()
        }));
    }
}
