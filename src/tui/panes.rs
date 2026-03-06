//! Pane registry — bridges hypertile's `PaneId` to clankers' panel types.
//!
//! Each hypertile pane gets a [`PaneKind`] that tells the renderer what
//! content to draw. The [`PaneRegistry`] owns this mapping and tracks the
//! reserved chat pane.

use std::collections::HashMap;

use ratatui::layout::Direction;
use ratatui_hypertile::raw::Node;
use ratatui_hypertile::{Hypertile, PaneId};

use crate::tui::panel::PanelId;

// ── PaneKind ────────────────────────────────────────────────────────────────

/// What content a hypertile pane displays.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PaneKind {
    /// The main chat/conversation area (blocks + editor + status bar).
    Chat,
    /// One of the existing side-panel types.
    Panel(PanelId),
    /// Placeholder pane (empty, waiting for content assignment).
    Empty,
}

impl PaneKind {
    pub fn label(&self) -> &str {
        match self {
            PaneKind::Chat => "Chat",
            PaneKind::Panel(id) => id.label(),
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
        self.kinds
            .iter()
            .find(|(_, k)| *k == kind)
            .map(|(&id, _)| id)
    }

    /// Find the pane holding a specific `PanelId`.
    pub fn find_panel(&self, panel_id: PanelId) -> Option<PaneId> {
        self.find(&PaneKind::Panel(panel_id))
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

    pub fn todo() -> PaneId { PaneId::new(1) }
    pub fn files() -> PaneId { PaneId::new(2) }
    pub fn subagents() -> PaneId { PaneId::new(3) }
    pub fn peers() -> PaneId { PaneId::new(4) }
    /// Stable ID for the Processes panel (used by toggle and presets).
    pub fn processes() -> PaneId { PaneId::new(5) }
    /// Stable ID for the Branches panel (used by toggle and presets).
    pub fn branches() -> PaneId { PaneId::new(6) }
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

    let mut tiling = Hypertile::builder()
        .with_focus_highlight(true)
        .with_gap(0)
        .build();
    tiling
        .set_root(tree)
        .expect("default BSP tree is valid");
    // Focus the chat pane on startup
    tiling
        .focus_pane(pane_ids::CHAT)
        .expect("chat pane exists");
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
