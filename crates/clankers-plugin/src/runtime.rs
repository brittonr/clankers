use std::sync::Arc;
use std::sync::Mutex;

use crate::PluginManager;
use crate::PluginState;
use crate::manifest::PluginKind;
use crate::stdio_runtime;

pub(crate) enum PluginRuntimeAfterGuardDrop {
    None,
    StartStdio,
}

impl PluginRuntimeAfterGuardDrop {
    pub(crate) fn run(self, manager: &Arc<Mutex<PluginManager>>, name: &str) -> Result<(), String> {
        match self {
            Self::None => Ok(()),
            Self::StartStdio => stdio_runtime::start_stdio_plugin(manager, name),
        }
    }
}

pub(crate) trait PluginRuntimeLifecycle: Sync {
    fn enable(&self, manager: &mut PluginManager, name: &str) -> Result<PluginRuntimeAfterGuardDrop, String>;
    fn disable(&self, manager: &mut PluginManager, name: &str) -> Result<(), String>;
    fn reload(&self, manager: &mut PluginManager, name: &str) -> Result<PluginRuntimeAfterGuardDrop, String>;
}

struct ExtismRuntime;
struct StdioRuntime;

static EXTISM_RUNTIME: ExtismRuntime = ExtismRuntime;
static STDIO_RUNTIME: StdioRuntime = StdioRuntime;

pub(crate) fn plugin_runtime_for_kind(kind: &PluginKind) -> &'static dyn PluginRuntimeLifecycle {
    match kind {
        PluginKind::Stdio => &STDIO_RUNTIME,
        PluginKind::Extism | PluginKind::Zellij => &EXTISM_RUNTIME,
    }
}

impl PluginRuntimeLifecycle for ExtismRuntime {
    fn enable(&self, manager: &mut PluginManager, name: &str) -> Result<PluginRuntimeAfterGuardDrop, String> {
        manager.load_wasm(name)?;
        Ok(PluginRuntimeAfterGuardDrop::None)
    }

    fn disable(&self, manager: &mut PluginManager, name: &str) -> Result<(), String> {
        manager.extism_runtime.instances.remove(name);
        if let Some(info) = manager.plugins.get_mut(name) {
            info.state = PluginState::Disabled;
        }
        Ok(())
    }

    fn reload(&self, manager: &mut PluginManager, name: &str) -> Result<PluginRuntimeAfterGuardDrop, String> {
        manager.extism_runtime.instances.remove(name);
        manager.load_wasm(name)?;
        Ok(PluginRuntimeAfterGuardDrop::None)
    }
}

impl PluginRuntimeLifecycle for StdioRuntime {
    fn enable(&self, manager: &mut PluginManager, name: &str) -> Result<PluginRuntimeAfterGuardDrop, String> {
        if let Some(info) = manager.plugins.get_mut(name) {
            info.state = PluginState::Loaded;
        }
        Ok(PluginRuntimeAfterGuardDrop::StartStdio)
    }

    fn disable(&self, manager: &mut PluginManager, name: &str) -> Result<(), String> {
        stdio_runtime::stop_stdio_plugin(manager, name, "plugin disabled", PluginState::Disabled);
        if let Some(info) = manager.plugins.get_mut(name) {
            info.state = PluginState::Disabled;
        }
        Ok(())
    }

    fn reload(&self, manager: &mut PluginManager, name: &str) -> Result<PluginRuntimeAfterGuardDrop, String> {
        stdio_runtime::stop_stdio_plugin(manager, name, "plugin reload", PluginState::Loaded);
        if let Some(info) = manager.plugins.get_mut(name) {
            info.state = PluginState::Loaded;
        }
        Ok(PluginRuntimeAfterGuardDrop::StartStdio)
    }
}
