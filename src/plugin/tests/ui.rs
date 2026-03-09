use crate::plugin::ui;

// ── UI widget serialization tests ────────────────────────────────

#[test]
fn ui_widget_text_roundtrip() {
    let widget = ui::Widget::Text {
        content: "Hello".to_string(),
        bold: true,
        color: Some("green".to_string()),
    };
    let json = serde_json::to_string(&widget).unwrap();
    let parsed: ui::Widget = serde_json::from_str(&json).unwrap();
    match parsed {
        ui::Widget::Text { content, bold, color } => {
            assert_eq!(content, "Hello");
            assert!(bold);
            assert_eq!(color, Some("green".to_string()));
        }
        _ => panic!("Expected Text widget"),
    }
}

#[test]
fn ui_widget_box_with_children() {
    let widget = ui::Widget::Box {
        children: vec![
            ui::Widget::Text {
                content: "A".to_string(),
                bold: false,
                color: None,
            },
            ui::Widget::Spacer { lines: 2 },
            ui::Widget::Text {
                content: "B".to_string(),
                bold: true,
                color: Some("red".to_string()),
            },
        ],
        direction: ui::Direction::Vertical,
    };
    let json = serde_json::to_string(&widget).unwrap();
    let parsed: ui::Widget = serde_json::from_str(&json).unwrap();
    match parsed {
        ui::Widget::Box { children, .. } => assert_eq!(children.len(), 3),
        _ => panic!("Expected Box widget"),
    }
}

#[test]
fn ui_widget_list() {
    let widget = ui::Widget::List {
        items: vec!["one".to_string(), "two".to_string(), "three".to_string()],
        selected: 1,
    };
    let json = serde_json::to_string(&widget).unwrap();
    assert!(json.contains("\"selected\":1"));
}

#[test]
fn ui_widget_input() {
    let json = r#"{"type":"Input","value":"","placeholder":"Search..."}"#;
    let widget: ui::Widget = serde_json::from_str(json).unwrap();
    match widget {
        ui::Widget::Input { value, placeholder } => {
            assert_eq!(value, "");
            assert_eq!(placeholder, "Search...");
        }
        _ => panic!("Expected Input widget"),
    }
}

#[test]
fn ui_widget_progress_roundtrip() {
    let widget = ui::Widget::Progress {
        label: "Building".to_string(),
        value: 0.75,
        color: Some("green".to_string()),
    };
    let json = serde_json::to_string(&widget).unwrap();
    let parsed: ui::Widget = serde_json::from_str(&json).unwrap();
    match parsed {
        ui::Widget::Progress { label, value, color } => {
            assert_eq!(label, "Building");
            assert!((value - 0.75).abs() < f64::EPSILON);
            assert_eq!(color, Some("green".to_string()));
        }
        _ => panic!("Expected Progress widget"),
    }
}

#[test]
fn ui_widget_table_roundtrip() {
    let widget = ui::Widget::Table {
        headers: vec!["Name".to_string(), "Status".to_string()],
        rows: vec![vec!["plugin-a".to_string(), "active".to_string()], vec![
            "plugin-b".to_string(),
            "error".to_string(),
        ]],
    };
    let json = serde_json::to_string(&widget).unwrap();
    let parsed: ui::Widget = serde_json::from_str(&json).unwrap();
    match parsed {
        ui::Widget::Table { headers, rows } => {
            assert_eq!(headers.len(), 2);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0][0], "plugin-a");
        }
        _ => panic!("Expected Table widget"),
    }
}

// ── PluginUIAction parsing ───────────────────────────────────────

#[test]
fn ui_action_set_widget_roundtrip() {
    let action = ui::PluginUIAction::SetWidget {
        plugin: "test".to_string(),
        widget: ui::Widget::Text {
            content: "hello".to_string(),
            bold: true,
            color: None,
        },
    };
    let json = serde_json::to_string(&action).unwrap();
    let parsed: ui::PluginUIAction = serde_json::from_str(&json).unwrap();
    match parsed {
        ui::PluginUIAction::SetWidget { plugin, widget } => {
            assert_eq!(plugin, "test");
            match widget {
                ui::Widget::Text { content, bold, .. } => {
                    assert_eq!(content, "hello");
                    assert!(bold);
                }
                _ => panic!("Expected Text widget"),
            }
        }
        _ => panic!("Expected SetWidget action"),
    }
}

#[test]
fn ui_action_set_status_roundtrip() {
    let action = ui::PluginUIAction::SetStatus {
        plugin: "test".to_string(),
        text: "running".to_string(),
        color: Some("green".to_string()),
    };
    let json = serde_json::to_string(&action).unwrap();
    let parsed: ui::PluginUIAction = serde_json::from_str(&json).unwrap();
    match parsed {
        ui::PluginUIAction::SetStatus { plugin, text, color } => {
            assert_eq!(plugin, "test");
            assert_eq!(text, "running");
            assert_eq!(color, Some("green".to_string()));
        }
        _ => panic!("Expected SetStatus action"),
    }
}

#[test]
fn ui_action_notify_roundtrip() {
    let action = ui::PluginUIAction::Notify {
        plugin: "test".to_string(),
        message: "Build done!".to_string(),
        level: "info".to_string(),
    };
    let json = serde_json::to_string(&action).unwrap();
    let parsed: ui::PluginUIAction = serde_json::from_str(&json).unwrap();
    match parsed {
        ui::PluginUIAction::Notify { plugin, message, level } => {
            assert_eq!(plugin, "test");
            assert_eq!(message, "Build done!");
            assert_eq!(level, "info");
        }
        _ => panic!("Expected Notify action"),
    }
}

#[test]
fn ui_action_clear_widget() {
    let json = r#"{"action":"clear_widget","plugin":"test"}"#;
    let parsed: ui::PluginUIAction = serde_json::from_str(json).unwrap();
    match parsed {
        ui::PluginUIAction::ClearWidget { plugin } => assert_eq!(plugin, "test"),
        _ => panic!("Expected ClearWidget"),
    }
}

#[test]
fn ui_action_clear_status() {
    let json = r#"{"action":"clear_status","plugin":"test"}"#;
    let parsed: ui::PluginUIAction = serde_json::from_str(json).unwrap();
    match parsed {
        ui::PluginUIAction::ClearStatus { plugin } => assert_eq!(plugin, "test"),
        _ => panic!("Expected ClearStatus"),
    }
}

// ── PluginUIState tests ──────────────────────────────────────────

#[test]
fn plugin_ui_state_set_and_clear_widget() {
    let mut state = ui::PluginUIState::new();
    assert!(!state.has_content());

    ui::apply_ui_action(&mut state, ui::PluginUIAction::SetWidget {
        plugin: "test".to_string(),
        widget: ui::Widget::Text {
            content: "hello".to_string(),
            bold: false,
            color: None,
        },
    });
    assert!(state.has_content());
    assert!(state.widgets.contains_key("test"));

    ui::apply_ui_action(&mut state, ui::PluginUIAction::ClearWidget {
        plugin: "test".to_string(),
    });
    assert!(!state.widgets.contains_key("test"));
}

#[test]
fn plugin_ui_state_set_and_clear_status() {
    let mut state = ui::PluginUIState::new();

    ui::apply_ui_action(&mut state, ui::PluginUIAction::SetStatus {
        plugin: "test".to_string(),
        text: "building".to_string(),
        color: Some("yellow".to_string()),
    });
    assert!(state.has_content());
    let seg = &state.status_segments["test"];
    assert_eq!(seg.text, "building");
    assert_eq!(seg.color, Some("yellow".to_string()));

    ui::apply_ui_action(&mut state, ui::PluginUIAction::ClearStatus {
        plugin: "test".to_string(),
    });
    assert!(!state.status_segments.contains_key("test"));
}

#[test]
fn plugin_ui_state_notify_and_gc() {
    let mut state = ui::PluginUIState::new();

    ui::apply_ui_action(&mut state, ui::PluginUIAction::Notify {
        plugin: "test".to_string(),
        message: "hello".to_string(),
        level: "info".to_string(),
    });
    assert_eq!(state.notifications.len(), 1);
    assert_eq!(state.notifications[0].message, "hello");

    // Fresh notifications should survive GC
    state.gc_notifications();
    assert_eq!(state.notifications.len(), 1);
}

#[test]
fn plugin_ui_state_multiple_plugins() {
    let mut state = ui::PluginUIState::new();

    ui::apply_ui_action(&mut state, ui::PluginUIAction::SetWidget {
        plugin: "plugin-a".to_string(),
        widget: ui::Widget::Text {
            content: "A".to_string(),
            bold: false,
            color: None,
        },
    });
    ui::apply_ui_action(&mut state, ui::PluginUIAction::SetWidget {
        plugin: "plugin-b".to_string(),
        widget: ui::Widget::Text {
            content: "B".to_string(),
            bold: false,
            color: None,
        },
    });
    ui::apply_ui_action(&mut state, ui::PluginUIAction::SetStatus {
        plugin: "plugin-a".to_string(),
        text: "ok".to_string(),
        color: None,
    });

    assert_eq!(state.widgets.len(), 2);
    assert_eq!(state.status_segments.len(), 1);
}

#[test]
fn plugin_ui_state_widget_replacement() {
    let mut state = ui::PluginUIState::new();

    ui::apply_ui_action(&mut state, ui::PluginUIAction::SetWidget {
        plugin: "test".to_string(),
        widget: ui::Widget::Text {
            content: "v1".to_string(),
            bold: false,
            color: None,
        },
    });
    ui::apply_ui_action(&mut state, ui::PluginUIAction::SetWidget {
        plugin: "test".to_string(),
        widget: ui::Widget::Text {
            content: "v2".to_string(),
            bold: true,
            color: None,
        },
    });

    assert_eq!(state.widgets.len(), 1);
    match &state.widgets["test"] {
        ui::Widget::Text { content, bold, .. } => {
            assert_eq!(content, "v2");
            assert!(*bold);
        }
        _ => panic!("Expected Text widget"),
    }
}
