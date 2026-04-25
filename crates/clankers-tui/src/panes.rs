//! Pane registry — bridges hypertile's `PaneId` to clankers' panel types.
//!
//! Each hypertile pane gets a [`PaneKind`] that tells the renderer what
//! content to draw. The [`PaneRegistry`] owns this mapping and tracks the
//! reserved chat pane.

#![allow(unexpected_cfgs)]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        compound_assertion,
        ignored_result,
        no_unwrap,
        no_panic,
        no_todo,
        unjustified_no_todo_allow,
        no_recursion,
        unchecked_narrowing,
        unchecked_division,
        unbounded_loop,
        catch_all_on_enum,
        explicit_defaults,
        unbounded_channel,
        unbounded_collection_growth,
        assertion_density,
        raw_arithmetic_overflow,
        sentinel_fallback,
        acronym_style,
        bool_naming,
        negated_predicate,
        numeric_units,
        float_for_currency,
        function_length,
        nested_conditionals,
        platform_dependent_cast,
        usize_in_public_api,
        too_many_parameters,
        compound_condition,
        unjustified_allow,
        ambiguous_params,
        ambient_clock,
        verified_purity,
        contradictory_time,
        multi_lock_ordering,
        reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"
    )
)]

use std::collections::HashMap;

use ratatui::layout::Direction;
use ratatui_hypertile::Hypertile;
use ratatui_hypertile::PaneId;
use ratatui_hypertile::raw::Node;

use crate::panel::PanelId;

// ── PaneKind ────────────────────────────────────────────────────────────────

/// What content a hypertile pane displays.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PaneKind {
    /// The main chat/conversation area (blocks + editor + status bar).
    Chat,
    /// One of the existing side-panel types.
    Panel(PanelId),
    /// A dedicated pane for a single subagent's live output.
    Subagent(String),
    /// Placeholder pane (empty, waiting for content assignment).
    Empty,
}

impl PaneKind {
    pub fn label(&self) -> &str {
        match self {
            PaneKind::Chat => "Chat",
            PaneKind::Panel(id) => id.label(),
            PaneKind::Subagent(_) => "Subagent",
            PaneKind::Empty => "Empty",
        }
    }
}

// ── PaneRegistry ────────────────────────────────────────────────────────────

/// Maps hypertile [`PaneId`]s to their content type ([`PaneKind`]).
///
/// Invariants:
/// - Exactly one pane has `PaneKind::Chat` at all times.
/// - The chat pane's ID is tracked in `chat_pane`.
#[derive(Debug, Clone)]
pub struct PaneRegistry {
    kinds: HashMap<PaneId, PaneKind>,
    chat_pane: PaneId,
}

impl PaneRegistry {
    /// Create a new registry with the root pane as the chat pane.
    pub fn new() -> Self {
        let mut kinds = HashMap::new();
        kinds.insert(PaneId::ROOT, PaneKind::Chat);
        Self {
            kinds,
            chat_pane: PaneId::ROOT,
        }
    }

    /// The pane ID reserved for the chat area.
    pub fn chat_pane(&self) -> PaneId {
        self.chat_pane
    }

    /// Look up what kind of content a pane holds.
    pub fn kind(&self, id: PaneId) -> Option<&PaneKind> {
        self.kinds.get(&id)
    }

    /// Is this the chat pane?
    pub fn is_chat(&self, id: PaneId) -> bool {
        id == self.chat_pane
    }

    /// Register a new pane with the given kind.
    pub fn register(&mut self, id: PaneId, kind: PaneKind) {
        self.kinds.insert(id, kind);
    }

    /// Remove a pane from the registry. Returns the kind if it existed.
    /// Refuses to remove the chat pane — returns `None` without removing.
    pub fn unregister(&mut self, id: PaneId) -> Option<PaneKind> {
        if id == self.chat_pane {
            return None;
        }
        self.kinds.remove(&id)
    }

    /// Find the first pane of a given kind.
    pub fn find(&self, kind: &PaneKind) -> Option<PaneId> {
        self.kinds.iter().find(|(_, k)| *k == kind).map(|(&id, _)| id)
    }

    /// Find the pane holding a specific `PanelId`.
    pub fn find_panel(&self, panel_id: PanelId) -> Option<PaneId> {
        self.find(&PaneKind::Panel(panel_id))
    }

    /// Find the pane for a specific subagent by its string ID.
    /// Find the first subagent pane (any subagent).
    pub fn find_any_subagent_pane(&self) -> Option<PaneId> {
        self.kinds.iter().find_map(|(&pane_id, kind)| match kind {
            PaneKind::Subagent(_) => Some(pane_id),
            _ => None,
        })
    }

    /// All registered pane IDs.
    pub fn pane_ids(&self) -> Vec<PaneId> {
        self.kinds.keys().copied().collect()
    }

    /// Remove panes not present in the given set (sync after tree changes).
    pub fn retain_only(&mut self, keep: &std::collections::HashSet<PaneId>) {
        self.kinds.retain(|id, _| keep.contains(id));
        // Ensure chat pane is always present
        if !self.kinds.contains_key(&self.chat_pane) {
            self.kinds.insert(self.chat_pane, PaneKind::Chat);
        }
    }
}

impl Default for PaneRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Well-known PaneIds for the default layout ───────────────────────────────

/// Stable pane IDs for the default layout. Using fixed IDs so the registry
/// can be built independently of the tree and so layout persistence works.
///
/// `PaneId::new` is not const, so these are functions (except ROOT/CHAT).
pub mod pane_ids {
    use ratatui_hypertile::PaneId;

    pub const CHAT: PaneId = PaneId::ROOT; // 0

    pub fn todo() -> PaneId {
        PaneId::new(1)
    }
    pub fn files() -> PaneId {
        PaneId::new(2)
    }
    pub fn subagents() -> PaneId {
        PaneId::new(3)
    }
    pub fn peers() -> PaneId {
        PaneId::new(4)
    }
    /// Stable ID for the Processes panel (used by toggle and presets).
    pub fn processes() -> PaneId {
        PaneId::new(5)
    }
    /// Stable ID for the Branches panel (used by toggle and presets).
    pub fn branches() -> PaneId {
        PaneId::new(6)
    }
}

// ── Default layout builder ──────────────────────────────────────────────────

/// Build the default BSP tree matching the original three-column layout:
///
/// ```text
/// ┌──────┬────────────────────┬───────────┐
/// │ Todo │                    │ Subagents │
/// │      │       Chat         │           │
/// │──────│                    │───────────│
/// │Files │                    │   Peers   │
/// └──────┴────────────────────┴───────────┘
///   20%          50%              30%
/// ```
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_unwrap, reason = "default BSP tree construction is infallible")
)]
pub fn default_tiling() -> Hypertile {
    let tree = Node::Split {
        direction: Direction::Horizontal,
        ratio: 0.20,
        first: Box::new(Node::Split {
            direction: Direction::Vertical,
            ratio: 0.50,
            first: Box::new(Node::Pane(pane_ids::todo())),
            second: Box::new(Node::Pane(pane_ids::files())),
        }),
        second: Box::new(Node::Split {
            direction: Direction::Horizontal,
            ratio: 0.625, // 50 / (50+30) = 0.625
            first: Box::new(Node::Pane(pane_ids::CHAT)),
            second: Box::new(Node::Split {
                direction: Direction::Vertical,
                ratio: 0.50,
                first: Box::new(Node::Pane(pane_ids::subagents())),
                second: Box::new(Node::Pane(pane_ids::peers())),
            }),
        }),
    };

    let mut tiling = Hypertile::builder().with_focus_highlight(true).with_gap(0).build();
    tiling.set_root(tree).expect("default BSP tree is valid");
    // Focus the chat pane on startup
    tiling.focus_pane(pane_ids::CHAT).expect("chat pane exists");
    tiling
}

/// Build the default pane registry matching [`default_tiling`].
pub fn default_registry() -> PaneRegistry {
    let mut reg = PaneRegistry {
        kinds: HashMap::new(),
        chat_pane: pane_ids::CHAT,
    };
    reg.register(pane_ids::CHAT, PaneKind::Chat);
    reg.register(pane_ids::todo(), PaneKind::Panel(PanelId::Todo));
    reg.register(pane_ids::files(), PaneKind::Panel(PanelId::Files));
    reg.register(pane_ids::subagents(), PaneKind::Panel(PanelId::Subagents));
    reg.register(pane_ids::peers(), PaneKind::Panel(PanelId::Peers));
    reg
}

// ── Preset layouts ──────────────────────────────────────────────────────────

/// Chat-only layout (no side panels).
pub fn focused_tiling() -> (Hypertile, PaneRegistry) {
    let tiling = Hypertile::new();
    // ROOT is already a single pane
    let reg = PaneRegistry::new(); // ROOT → Chat
    (tiling, reg)
}

/// Wide chat layout: thin left sidebar only.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_unwrap, reason = "tiling construction is infallible")
)]
pub fn wide_chat_tiling() -> (Hypertile, PaneRegistry) {
    let tree = Node::Split {
        direction: Direction::Horizontal,
        ratio: 0.20,
        first: Box::new(Node::Split {
            direction: Direction::Vertical,
            ratio: 0.33,
            first: Box::new(Node::Pane(pane_ids::todo())),
            second: Box::new(Node::Split {
                direction: Direction::Vertical,
                ratio: 0.50,
                first: Box::new(Node::Pane(pane_ids::files())),
                second: Box::new(Node::Pane(pane_ids::subagents())),
            }),
        }),
        second: Box::new(Node::Pane(pane_ids::CHAT)),
    };

    let mut tiling = Hypertile::new();
    tiling.set_root(tree).expect("valid tree");
    tiling.focus_pane(pane_ids::CHAT).expect("chat exists");

    let mut reg = PaneRegistry {
        kinds: HashMap::new(),
        chat_pane: pane_ids::CHAT,
    };
    reg.register(pane_ids::CHAT, PaneKind::Chat);
    reg.register(pane_ids::todo(), PaneKind::Panel(PanelId::Todo));
    reg.register(pane_ids::files(), PaneKind::Panel(PanelId::Files));
    reg.register(pane_ids::subagents(), PaneKind::Panel(PanelId::Subagents));
    (tiling, reg)
}

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_unwrap, reason = "tiling construction is infallible")
)]
/// Right-heavy layout: everything on the right.
pub fn right_heavy_tiling() -> (Hypertile, PaneRegistry) {
    let tree = Node::Split {
        direction: Direction::Horizontal,
        ratio: 0.70,
        first: Box::new(Node::Pane(pane_ids::CHAT)),
        second: Box::new(Node::Split {
            direction: Direction::Vertical,
            ratio: 0.25,
            first: Box::new(Node::Pane(pane_ids::todo())),
            second: Box::new(Node::Split {
                direction: Direction::Vertical,
                ratio: 0.33,
                first: Box::new(Node::Pane(pane_ids::files())),
                second: Box::new(Node::Split {
                    direction: Direction::Vertical,
                    ratio: 0.50,
                    first: Box::new(Node::Pane(pane_ids::subagents())),
                    second: Box::new(Node::Pane(pane_ids::peers())),
                }),
            }),
        }),
    };

    let mut tiling = Hypertile::new();
    tiling.set_root(tree).expect("valid tree");
    tiling.focus_pane(pane_ids::CHAT).expect("chat exists");

    let mut reg = PaneRegistry {
        kinds: HashMap::new(),
        chat_pane: pane_ids::CHAT,
    };
    reg.register(pane_ids::CHAT, PaneKind::Chat);
    reg.register(pane_ids::todo(), PaneKind::Panel(PanelId::Todo));
    reg.register(pane_ids::files(), PaneKind::Panel(PanelId::Files));
    reg.register(pane_ids::subagents(), PaneKind::Panel(PanelId::Subagents));
    reg.register(pane_ids::peers(), PaneKind::Panel(PanelId::Peers));
    (tiling, reg)
}

// ── BSP tree manipulation utilities ─────────────────────────────────────────

/// Remove a pane from the BSP tree, returning the pruned tree.
/// Returns `None` if the pane is the only node (root leaf).
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_recursion, reason = "BSP tree depth bounded by MAX_SPLIT_DEPTH")
)]
pub fn remove_pane_from_tree(node: Node, target: PaneId) -> Option<Node> {
    match node {
        Node::Pane(id) => {
            if id == target {
                None
            } else {
                Some(Node::Pane(id))
            }
        }
        Node::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let first_pruned = remove_pane_from_tree(*first, target);
            let second_pruned = remove_pane_from_tree(*second, target);
            match (first_pruned, second_pruned) {
                (Some(f), Some(s)) => Some(Node::Split {
                    direction,
                    ratio,
                    first: Box::new(f),
                    second: Box::new(s),
                }),
                (Some(f), None) => Some(f),
                (None, Some(s)) => Some(s),
                (None, None) => None,
            }
        }
    }
}

/// Insert a new pane beside an existing pane in the BSP tree.
/// Splits the target pane, keeping the target in the `first` slot.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_recursion, reason = "BSP tree depth bounded by MAX_SPLIT_DEPTH")
)]
pub fn insert_pane_beside(
    node: Node,
    target: PaneId,
    new_pane: PaneId,
    direction: Direction,
    ratio: f32,
) -> Option<Node> {
    match node {
        Node::Pane(id) => {
            if id == target {
                Some(Node::Split {
                    direction,
                    ratio,
                    first: Box::new(Node::Pane(id)),
                    second: Box::new(Node::Pane(new_pane)),
                })
            } else {
                Some(Node::Pane(id))
            }
        }
        Node::Split {
            direction: d,
            ratio: r,
            first,
            second,
        } => {
            let first_result = insert_pane_beside(*first.clone(), target, new_pane, direction, ratio);
            if let Some(new_first) = first_result {
                let has_first_changed = !nodes_equal(&new_first, &first);
                if has_first_changed {
                    return Some(Node::Split {
                        direction: d,
                        ratio: r,
                        first: Box::new(new_first),
                        second,
                    });
                }
            }
            let second_result = insert_pane_beside(*second, target, new_pane, direction, ratio);
            second_result.map(|new_second| Node::Split {
                direction: d,
                ratio: r,
                first,
                second: Box::new(new_second),
            })
        }
    }
}

/// Quick structural equality check for BSP nodes.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_recursion, reason = "BSP tree depth bounded by MAX_SPLIT_DEPTH")
)]
fn nodes_equal(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (Node::Pane(a_id), Node::Pane(b_id)) => a_id == b_id,
        (
            Node::Split {
                direction: da,
                ratio: ra,
                first: fa,
                second: sa,
            },
            Node::Split {
                direction: db,
                ratio: rb,
                first: fb,
                second: sb,
            },
        ) => da == db && (ra - rb).abs() < f32::EPSILON && nodes_equal(fa, fb) && nodes_equal(sa, sb),
        _ => false,
    }
}

/// Auto-split the BSP tree to make room for a new subagent pane.
///
/// Strategy:
/// 1. If there's an existing subagent pane, split it vertically (stack them)
/// 2. Else if the Subagents overview panel exists, split it vertically
/// 3. Else split the chat pane horizontally (chat keeps 75%)
pub fn auto_split_for_subagent(tiling: &mut Hypertile, registry: &PaneRegistry, new_pane_id: PaneId) {
    // Try to find an existing subagent pane to stack beside
    let target = registry.find_any_subagent_pane().or_else(|| registry.find_panel(PanelId::Subagents));

    let (target_pane, direction, ratio) = if let Some(t) = target {
        (t, Direction::Vertical, 0.5)
    } else {
        // No subagent area — split chat horizontally
        (registry.chat_pane(), Direction::Horizontal, 0.75)
    };

    let new_root = insert_pane_beside(tiling.root().clone(), target_pane, new_pane_id, direction, ratio);
    if let Some(root) = new_root {
        tiling.set_root(root).ok();
    }
}
