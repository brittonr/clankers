//! Tool catalog types for the host-facing runtime facade.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use clankers_artifacts::RedactionClass;
use serde::Deserialize;
use serde::Serialize;

use crate::ExtensionRuntimeKind;
use crate::ExtensionRuntimeService;
use crate::RuntimeError;
use crate::effects::EffectAbilityClass;
use crate::effects::EffectCorrelationId;
use crate::effects::EffectHandler;
use crate::effects::EffectRequest;
use crate::effects::EffectResultStatus;
use crate::events::sanitize_metadata_value;
use crate::services::extension_kind_label;

/// Host-facing tool catalog.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCatalog {
    tools: BTreeMap<String, ToolDescriptor>,
    packs: BTreeSet<CapabilityPack>,
    omissions: Vec<ToolCatalogOmission>,
}

impl ToolCatalog {
    #[must_use]
    pub fn builder() -> ToolCatalogBuilder {
        ToolCatalogBuilder::default()
    }

    #[must_use]
    pub fn embedding_safe() -> Self {
        Self::builder().pack(CapabilityPack::ReadOnly).build().expect("read-only pack has no collisions")
    }

    #[must_use]
    pub fn desktop_default() -> Self {
        Self::builder()
            .pack(CapabilityPack::ReadOnly)
            .pack(CapabilityPack::WorkspaceMutation)
            .pack(CapabilityPack::ShellCommands)
            .build()
            .expect("built-in packs have no collisions")
    }

    #[must_use]
    pub fn tools(&self) -> impl Iterator<Item = &ToolDescriptor> {
        self.tools.values()
    }

    #[must_use]
    pub fn contains_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    #[must_use]
    pub fn packs(&self) -> &BTreeSet<CapabilityPack> {
        &self.packs
    }

    #[must_use]
    pub fn omissions(&self) -> &[ToolCatalogOmission] {
        &self.omissions
    }
}

#[derive(Debug, Clone)]
pub struct ToolCatalogBuilder {
    tools: BTreeMap<String, ToolDescriptor>,
    packs: BTreeSet<CapabilityPack>,
    disabled_tools: BTreeSet<String>,
    omissions: Vec<ToolCatalogOmission>,
    collision_policy: ToolCollisionPolicy,
}

impl Default for ToolCatalogBuilder {
    fn default() -> Self {
        Self {
            tools: BTreeMap::new(),
            packs: BTreeSet::new(),
            disabled_tools: BTreeSet::new(),
            omissions: Vec::new(),
            collision_policy: ToolCollisionPolicy::Reject,
        }
    }
}

impl ToolCatalogBuilder {
    #[must_use]
    pub fn pack(mut self, pack: CapabilityPack) -> Self {
        for descriptor in pack.descriptors() {
            if !self.disabled_tools.contains(&descriptor.name) {
                self.tools.entry(descriptor.name.clone()).or_insert(descriptor);
            }
        }
        self.packs.insert(pack);
        self
    }

    #[must_use]
    pub fn collision_policy(mut self, policy: ToolCollisionPolicy) -> Self {
        self.collision_policy = policy;
        self
    }

    pub fn custom_tool(self, descriptor: ToolDescriptor) -> Result<Self, RuntimeError> {
        self.insert_descriptor(descriptor)
    }

    pub fn extension_runtime_tools(
        mut self,
        kind: ExtensionRuntimeKind,
        runtime: &dyn ExtensionRuntimeService,
    ) -> Result<Self, RuntimeError> {
        for descriptor in runtime.publishable_tools(kind)? {
            let tool = ToolDescriptor::new(descriptor.visible_tool_name, "Host extension tool", descriptor.side_effect)
                .with_source(format!("extension:{}", extension_kind_label(kind)));
            self = self.insert_descriptor(tool)?;
        }
        Ok(self)
    }

    fn insert_descriptor(mut self, descriptor: ToolDescriptor) -> Result<Self, RuntimeError> {
        if descriptor.name.trim().is_empty() {
            return Err(RuntimeError::InvalidTool("tool name cannot be blank".to_string()));
        }
        if self.disabled_tools.contains(&descriptor.name) {
            self.omissions.push(ToolCatalogOmission::new(descriptor.name, "disabled_by_host_filter"));
            return Ok(self);
        }
        if self.tools.contains_key(&descriptor.name) {
            match self.collision_policy {
                ToolCollisionPolicy::Reject => return Err(RuntimeError::ToolNameCollision(descriptor.name)),
                ToolCollisionPolicy::KeepExisting => {
                    self.omissions.push(ToolCatalogOmission::new(descriptor.name, "name_collision_existing_kept"));
                    return Ok(self);
                }
                ToolCollisionPolicy::HostOverrides => {}
            }
        }
        self.tools.insert(descriptor.name.clone(), descriptor);
        Ok(self)
    }

    #[must_use]
    pub fn disabled_tool(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        if !name.trim().is_empty() {
            self.tools.remove(&name);
            self.disabled_tools.insert(name.clone());
            self.omissions.push(ToolCatalogOmission::new(name, "disabled_by_host_filter"));
        }
        self
    }

    #[must_use]
    pub fn disabled_tools<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for name in names {
            self = self.disabled_tool(name);
        }
        self
    }

    pub fn build(self) -> Result<ToolCatalog, RuntimeError> {
        let tools = self.tools.into_iter().filter(|(name, _)| !self.disabled_tools.contains(name)).collect();
        Ok(ToolCatalog {
            tools,
            packs: self.packs,
            omissions: self.omissions,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub side_effect: SideEffectLevel,
    pub requires_confirmation: bool,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCatalogOmission {
    pub name: String,
    pub reason: String,
}

impl ToolCatalogOmission {
    #[must_use]
    pub fn new(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            name: sanitize_metadata_value(name.into()),
            reason: sanitize_metadata_value(reason.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCollisionPolicy {
    #[default]
    Reject,
    KeepExisting,
    HostOverrides,
}

impl ToolDescriptor {
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>, side_effect: SideEffectLevel) -> Self {
        let side_effect = side_effect;
        Self {
            name: name.into(),
            description: description.into(),
            requires_confirmation: side_effect.requires_confirmation(),
            side_effect,
            source: "clankers".to_string(),
        }
    }

    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = sanitize_metadata_value(source.into());
        self
    }

    #[must_use]
    pub fn effect_class(&self) -> EffectAbilityClass {
        effect_class_for_tool(self.name.as_str(), self.side_effect)
    }

    #[must_use]
    pub fn effect_request(&self, correlation_id: EffectCorrelationId) -> EffectRequest {
        EffectRequest::new(self.effect_class(), correlation_id, RedactionClass::MetadataOnly)
            .with_safe_metadata("tool_name", self.name.clone())
            .with_safe_metadata("tool_source", self.source.clone())
    }

    #[must_use]
    pub fn route_through_effect_handler(
        &self,
        correlation_id: EffectCorrelationId,
        handler: &dyn EffectHandler,
    ) -> ToolEffectReceipt {
        let request = self.effect_request(correlation_id);
        ToolEffectReceipt::from_effect_result(self, handler.handle(&request))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolEffectReceipt {
    pub tool_name: String,
    pub effect_class: EffectAbilityClass,
    pub handler_status: EffectResultStatus,
    pub safe_summary: String,
}

impl ToolEffectReceipt {
    #[must_use]
    pub fn from_effect_result(descriptor: &ToolDescriptor, result: crate::EffectResult) -> Self {
        Self {
            tool_name: descriptor.name.clone(),
            effect_class: result.request.class,
            handler_status: result.status,
            safe_summary: result.safe_summary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityPack {
    ReadOnly,
    WorkspaceMutation,
    ShellCommands,
    Network,
    ExternalProcesses,
}

impl CapabilityPack {
    #[must_use]
    pub fn descriptors(self) -> Vec<ToolDescriptor> {
        match self {
            Self::ReadOnly => vec![
                ToolDescriptor::new("read", "Read files selected by the host", SideEffectLevel::ReadOnly),
                ToolDescriptor::new("search", "Search host-selected project content", SideEffectLevel::ReadOnly),
            ],
            Self::WorkspaceMutation => vec![
                ToolDescriptor::new(
                    "write",
                    "Write files in host-approved workspace roots",
                    SideEffectLevel::WorkspaceMutation,
                ),
                ToolDescriptor::new(
                    "patch",
                    "Patch files in host-approved workspace roots",
                    SideEffectLevel::WorkspaceMutation,
                ),
            ],
            Self::ShellCommands => vec![ToolDescriptor::new(
                "bash",
                "Run host-approved shell commands",
                SideEffectLevel::Dangerous,
            )],
            Self::Network => vec![ToolDescriptor::new(
                "web",
                "Fetch host-approved network resources",
                SideEffectLevel::ExternalIo,
            )],
            Self::ExternalProcesses => vec![ToolDescriptor::new(
                "process",
                "Manage host-approved background processes",
                SideEffectLevel::Dangerous,
            )],
        }
    }

    #[must_use]
    pub fn effect_classes(self) -> BTreeSet<EffectAbilityClass> {
        self.descriptors().into_iter().map(|descriptor| descriptor.effect_class()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectLevel {
    ReadOnly,
    WorkspaceMutation,
    ExternalIo,
    Dangerous,
}

impl SideEffectLevel {
    #[must_use]
    pub fn requires_confirmation(self) -> bool {
        !matches!(self, Self::ReadOnly)
    }

    #[must_use]
    pub fn default_effect_class(self) -> EffectAbilityClass {
        match self {
            Self::ReadOnly | Self::WorkspaceMutation => EffectAbilityClass::Filesystem,
            Self::ExternalIo => EffectAbilityClass::Network,
            Self::Dangerous => EffectAbilityClass::Tool,
        }
    }
}

fn effect_class_for_tool(name: &str, side_effect: SideEffectLevel) -> EffectAbilityClass {
    match name {
        "read" | "search" | "write" | "patch" => EffectAbilityClass::Filesystem,
        "bash" | "process" => EffectAbilityClass::Shell,
        "web" => EffectAbilityClass::Network,
        "tool_gateway" => EffectAbilityClass::Tool,
        "mcp" => EffectAbilityClass::Plugin,
        "browser" => EffectAbilityClass::Browser,
        "voice_mode" => EffectAbilityClass::Delivery,
        _ => side_effect.default_effect_class(),
    }
}
