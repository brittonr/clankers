//! Tests for clankers capability-based authorization.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use iroh::SecretKey;

use super::*;

/// Counter for generating unique secret keys.
static KEY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Helper to create a unique test secret key.
fn test_secret_key() -> SecretKey {
    let counter = KEY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&counter.to_le_bytes());
    SecretKey::from(seed)
}

// ============================================================================
// Capability Authorization Tests
// ============================================================================

// r[verify ucan.auth.wildcard-matches-all]
#[test]
fn test_capability_prompt() {
    let cap = Capability::Prompt;
    assert!(cap.authorizes(&Operation::Prompt { text: "hello".into() }));
    assert!(!cap.authorizes(&Operation::ToolUse {
        tool_name: "read".into()
    }));
}

#[test]
fn test_capability_tool_use_exact() {
    let cap = Capability::ToolUse {
        tool_pattern: "read".into(),
    };
    assert!(cap.authorizes(&Operation::ToolUse {
        tool_name: "read".into()
    }));
    assert!(!cap.authorizes(&Operation::ToolUse {
        tool_name: "write".into()
    }));
}

#[test]
fn test_capability_tool_use_comma_separated() {
    let cap = Capability::ToolUse {
        tool_pattern: "read,grep,find".into(),
    };
    assert!(cap.authorizes(&Operation::ToolUse {
        tool_name: "read".into()
    }));
    assert!(cap.authorizes(&Operation::ToolUse {
        tool_name: "grep".into()
    }));
    assert!(cap.authorizes(&Operation::ToolUse {
        tool_name: "find".into()
    }));
    assert!(!cap.authorizes(&Operation::ToolUse {
        tool_name: "bash".into()
    }));
}

// r[verify ucan.auth.wildcard-matches-all]
#[test]
fn test_capability_tool_use_wildcard() {
    let cap = Capability::ToolUse {
        tool_pattern: "*".into(),
    };
    assert!(cap.authorizes(&Operation::ToolUse {
        tool_name: "read".into()
    }));
    assert!(cap.authorizes(&Operation::ToolUse {
        tool_name: "bash".into()
    }));
    assert!(cap.authorizes(&Operation::ToolUse {
        tool_name: "anything".into()
    }));
}

// r[verify ucan.auth.wildcard-matches-all]
#[test]
fn test_capability_shell_execute_wildcard() {
    let cap = Capability::ShellExecute {
        command_pattern: "*".into(),
        working_dir: None,
    };
    assert!(cap.authorizes(&Operation::ShellExecute {
        command: "ls -la".into(),
        working_dir: Some("/home".into()),
    }));
    assert!(cap.authorizes(&Operation::ShellExecute {
        command: "grep foo bar".into(),
        working_dir: None,
    }));
}

#[test]
fn test_capability_shell_execute_glob() {
    let cap = Capability::ShellExecute {
        command_pattern: "pg_*".into(),
        working_dir: None,
    };
    assert!(cap.authorizes(&Operation::ShellExecute {
        command: "pg_dump".into(),
        working_dir: None,
    }));
    assert!(cap.authorizes(&Operation::ShellExecute {
        command: "pg_restore".into(),
        working_dir: None,
    }));
    assert!(!cap.authorizes(&Operation::ShellExecute {
        command: "mysql".into(),
        working_dir: None,
    }));
}

#[test]
fn test_capability_shell_execute_working_dir() {
    let cap = Capability::ShellExecute {
        command_pattern: "*".into(),
        working_dir: Some("/home/user".into()),
    };

    // Command in allowed directory
    assert!(cap.authorizes(&Operation::ShellExecute {
        command: "ls".into(),
        working_dir: Some("/home/user/project".into()),
    }));

    // Command in disallowed directory
    assert!(!cap.authorizes(&Operation::ShellExecute {
        command: "ls".into(),
        working_dir: Some("/etc".into()),
    }));

    // Command with no working dir specified (not allowed when cap has constraint)
    assert!(!cap.authorizes(&Operation::ShellExecute {
        command: "ls".into(),
        working_dir: None,
    }));
}

#[test]
fn test_capability_file_access_read_write() {
    let cap = Capability::FileAccess {
        prefix: "/home/user/".into(),
        read_only: false,
    };

    assert!(cap.authorizes(&Operation::FileRead {
        path: "/home/user/file.txt".into()
    }));
    assert!(cap.authorizes(&Operation::FileWrite {
        path: "/home/user/file.txt".into()
    }));
    assert!(!cap.authorizes(&Operation::FileRead {
        path: "/etc/passwd".into()
    }));
}

// r[verify ucan.auth.read-only-blocks-write]
#[test]
fn test_capability_file_access_read_only() {
    let cap = Capability::FileAccess {
        prefix: "/home/user/".into(),
        read_only: true,
    };

    assert!(cap.authorizes(&Operation::FileRead {
        path: "/home/user/file.txt".into()
    }));
    assert!(!cap.authorizes(&Operation::FileWrite {
        path: "/home/user/file.txt".into()
    }));
}

#[test]
fn test_capability_bot_command_exact() {
    let cap = Capability::BotCommand {
        command_pattern: "status".into(),
    };
    assert!(cap.authorizes(&Operation::BotCommand {
        command: "status".into()
    }));
    assert!(!cap.authorizes(&Operation::BotCommand {
        command: "restart".into()
    }));
}

#[test]
fn test_capability_bot_command_comma_separated() {
    let cap = Capability::BotCommand {
        command_pattern: "status,help,info".into(),
    };
    assert!(cap.authorizes(&Operation::BotCommand {
        command: "status".into()
    }));
    assert!(cap.authorizes(&Operation::BotCommand { command: "help".into() }));
    assert!(!cap.authorizes(&Operation::BotCommand {
        command: "restart".into()
    }));
}

// r[verify ucan.auth.wildcard-matches-all]
#[test]
fn test_capability_bot_command_wildcard() {
    let cap = Capability::BotCommand {
        command_pattern: "*".into(),
    };
    assert!(cap.authorizes(&Operation::BotCommand {
        command: "anything".into()
    }));
}

#[test]
fn test_capability_session_manage() {
    let cap = Capability::SessionManage;
    assert!(cap.authorizes(&Operation::SessionManage {
        action: "restart".into()
    }));
    assert!(!cap.authorizes(&Operation::Prompt { text: "hello".into() }));
}

#[test]
fn test_capability_model_switch() {
    let cap = Capability::ModelSwitch;
    assert!(cap.authorizes(&Operation::ModelSwitch { model: "gpt-4".into() }));
    assert!(!cap.authorizes(&Operation::Prompt { text: "hello".into() }));
}

// ============================================================================
// Capability Containment Tests (for delegation)
// ============================================================================

#[test]
fn test_contains_simple_capabilities() {
    assert!(Capability::Prompt.contains(&Capability::Prompt));
    assert!(!Capability::Prompt.contains(&Capability::SessionManage));
    assert!(Capability::SessionManage.contains(&Capability::SessionManage));
    assert!(Capability::ModelSwitch.contains(&Capability::ModelSwitch));
    assert!(Capability::Delegate.contains(&Capability::Delegate));
}

#[test]
fn test_contains_tool_use_wildcard() {
    let parent = Capability::ToolUse {
        tool_pattern: "*".into(),
    };
    let child = Capability::ToolUse {
        tool_pattern: "read,grep".into(),
    };
    assert!(parent.contains(&child));
}

// r[verify ucan.auth.pattern-set-containment]
#[test]
fn test_contains_tool_use_subset() {
    let parent = Capability::ToolUse {
        tool_pattern: "read,grep,find,ls".into(),
    };
    let child = Capability::ToolUse {
        tool_pattern: "read,grep".into(),
    };
    assert!(parent.contains(&child));
}

// r[verify ucan.auth.pattern-set-containment]
#[test]
fn test_contains_tool_use_not_subset() {
    let parent = Capability::ToolUse {
        tool_pattern: "read,grep".into(),
    };
    let child = Capability::ToolUse {
        tool_pattern: "read,grep,bash".into(),
    };
    assert!(!parent.contains(&child)); // Child has bash, parent doesn't
}

// r[verify ucan.auth.pattern-set-containment]
#[test]
fn test_contains_tool_use_child_wildcard() {
    let parent = Capability::ToolUse {
        tool_pattern: "read,grep".into(),
    };
    let child = Capability::ToolUse {
        tool_pattern: "*".into(),
    };
    assert!(!parent.contains(&child)); // Can't escalate to wildcard
}

#[test]
fn test_contains_file_access_narrower_prefix() {
    let parent = Capability::FileAccess {
        prefix: "/home/".into(),
        read_only: false,
    };
    let child = Capability::FileAccess {
        prefix: "/home/user/".into(),
        read_only: false,
    };
    assert!(parent.contains(&child));
}

#[test]
fn test_contains_file_access_wider_prefix() {
    let parent = Capability::FileAccess {
        prefix: "/home/user/".into(),
        read_only: false,
    };
    let child = Capability::FileAccess {
        prefix: "/home/".into(),
        read_only: false,
    };
    assert!(!parent.contains(&child)); // Child wants wider access
}

// r[verify ucan.auth.no-escalation]
#[test]
fn test_contains_file_access_readonly_escalation() {
    let parent = Capability::FileAccess {
        prefix: "/home/".into(),
        read_only: true,
    };
    let child = Capability::FileAccess {
        prefix: "/home/user/".into(),
        read_only: false,
    };
    assert!(!parent.contains(&child)); // Can't escalate from read-only to read-write
}

#[test]
fn test_contains_file_access_readonly_to_readonly() {
    let parent = Capability::FileAccess {
        prefix: "/home/".into(),
        read_only: false,
    };
    let child = Capability::FileAccess {
        prefix: "/home/user/".into(),
        read_only: true,
    };
    assert!(parent.contains(&child)); // Can attenuate to read-only
}

#[test]
fn test_contains_bot_command_patterns() {
    let parent = Capability::BotCommand {
        command_pattern: "*".into(),
    };
    let child = Capability::BotCommand {
        command_pattern: "status,help".into(),
    };
    assert!(parent.contains(&child));
}

#[test]
fn test_contains_shell_execute_wildcard() {
    let parent = Capability::ShellExecute {
        command_pattern: "*".into(),
        working_dir: None,
    };
    let child = Capability::ShellExecute {
        command_pattern: "ls".into(),
        working_dir: Some("/home".into()),
    };
    assert!(parent.contains(&child));
}

#[test]
fn test_contains_shell_execute_glob_pattern() {
    let parent = Capability::ShellExecute {
        command_pattern: "pg_*".into(),
        working_dir: None,
    };
    let child = Capability::ShellExecute {
        command_pattern: "pg_dump".into(),
        working_dir: None,
    };
    assert!(parent.contains(&child));
}

#[test]
fn test_contains_shell_execute_working_dir() {
    let parent = Capability::ShellExecute {
        command_pattern: "*".into(),
        working_dir: Some("/home".into()),
    };
    let child = Capability::ShellExecute {
        command_pattern: "*".into(),
        working_dir: Some("/home/user".into()),
    };
    assert!(parent.contains(&child));
}

// ============================================================================
// Token Builder Tests
// ============================================================================

#[test]
fn test_token_builder_basic() {
    let key = test_secret_key();

    let token = TokenBuilder::new(key.clone())
        .with_capability(Capability::Prompt)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .expect("should build token");

    assert_eq!(token.version, 1);
    assert_eq!(token.issuer, key.public());
    assert_eq!(token.capabilities.len(), 1);
    assert!(token.expires_at > token.issued_at);
}

#[test]
fn test_token_builder_with_audience() {
    let issuer = test_secret_key();
    let audience_key = test_secret_key().public();

    let token = TokenBuilder::new(issuer)
        .for_key(audience_key)
        .with_capability(Capability::Prompt)
        .build()
        .expect("should build token");

    assert_eq!(token.audience, Audience::Key(audience_key));
}

#[test]
fn test_token_builder_with_nonce() {
    let key = test_secret_key();

    let token = TokenBuilder::new(key)
        .with_capability(Capability::Prompt)
        .with_random_nonce()
        .build()
        .expect("should build token");

    assert!(token.nonce.is_some());
    assert_ne!(token.nonce.unwrap(), [0u8; 16]);
}

#[test]
fn test_token_builder_too_many_capabilities() {
    let key = test_secret_key();

    let mut builder = TokenBuilder::new(key);
    for i in 0..40 {
        builder = builder.with_capability(Capability::FileAccess {
            prefix: format!("/prefix{i}/"),
            read_only: true,
        });
    }

    let result = builder.build();
    assert!(matches!(result, Err(AuthError::TooManyCapabilities { .. })));
}

#[test]
fn test_token_builder_delegation() {
    let root_key = test_secret_key();
    let child_key = test_secret_key();

    // Create root token with delegation capability
    let root = TokenBuilder::new(root_key)
        .with_capability(Capability::FileAccess {
            prefix: "/home/".into(),
            read_only: false,
        })
        .with_capability(Capability::Delegate)
        .build()
        .expect("should build root token");

    // Create child token with narrower scope
    let child = TokenBuilder::new(child_key)
        .delegated_from(root.clone())
        .with_capability(Capability::FileAccess {
            prefix: "/home/user/".into(),
            read_only: true,
        })
        .build()
        .expect("should build child token");

    assert!(child.proof.is_some());
    assert_eq!(child.proof.unwrap(), root.hash().unwrap());
}

#[test]
fn test_token_builder_delegation_without_delegate_cap() {
    let root_key = test_secret_key();
    let child_key = test_secret_key();

    // Create root token WITHOUT delegation capability
    let root = TokenBuilder::new(root_key)
        .with_capability(Capability::Prompt)
        .build()
        .expect("should build root token");

    // Attempt to delegate should fail
    let result = TokenBuilder::new(child_key).delegated_from(root).with_capability(Capability::Prompt).build();

    assert!(matches!(result, Err(AuthError::DelegationNotAllowed)));
}

// r[verify ucan.auth.no-escalation]
#[test]
fn test_token_builder_capability_escalation() {
    let root_key = test_secret_key();
    let child_key = test_secret_key();

    // Create root token with limited scope
    let root = TokenBuilder::new(root_key)
        .with_capability(Capability::FileAccess {
            prefix: "/home/user/".into(),
            read_only: true,
        })
        .with_capability(Capability::Delegate)
        .build()
        .expect("should build root token");

    // Attempt to escalate to read-write should fail
    let result = TokenBuilder::new(child_key)
        .delegated_from(root)
        .with_capability(Capability::FileAccess {
            prefix: "/home/user/".into(),
            read_only: false,
        })
        .build();

    assert!(matches!(result, Err(AuthError::CapabilityEscalation { .. })));
}

// ============================================================================
// Token Encoding Tests
// ============================================================================

#[test]
fn test_token_encode_decode_roundtrip() {
    let key = test_secret_key();

    let token = TokenBuilder::new(key)
        .with_capability(Capability::Prompt)
        .with_capability(Capability::ToolUse {
            tool_pattern: "read,write".into(),
        })
        .with_random_nonce()
        .build()
        .expect("should build token");

    let encoded = token.encode().expect("should encode");
    let decoded = CapabilityToken::decode(&encoded).expect("should decode");

    assert_eq!(token.issuer, decoded.issuer);
    assert_eq!(token.capabilities, decoded.capabilities);
    assert_eq!(token.nonce, decoded.nonce);
}

#[test]
fn test_token_base64_roundtrip() {
    let key = test_secret_key();

    let token = TokenBuilder::new(key).with_capability(Capability::Prompt).build().expect("should build token");

    let b64 = token.to_base64().expect("should encode to base64");
    let decoded = CapabilityToken::from_base64(&b64).expect("should decode from base64");

    assert_eq!(token.issuer, decoded.issuer);
}

// ============================================================================
// Token Verifier Tests
// ============================================================================

#[test]
fn test_verifier_accepts_valid_token() {
    let key = test_secret_key();

    let token = TokenBuilder::new(key)
        .with_capability(Capability::Prompt)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .expect("should build token");

    let verifier = TokenVerifier::new();
    verifier.verify(&token, None).expect("should verify");
}

#[test]
fn test_verifier_rejects_tampered_signature() {
    let key = test_secret_key();

    let mut token = TokenBuilder::new(key).with_capability(Capability::Prompt).build().expect("should build token");

    // Tamper with the signature
    token.signature[0] ^= 0xFF;

    let verifier = TokenVerifier::new();
    let result = verifier.verify(&token, None);
    assert!(matches!(result, Err(AuthError::InvalidSignature)));
}

#[test]
fn test_verifier_checks_audience() {
    let issuer = test_secret_key();
    let intended_audience = test_secret_key();
    let wrong_presenter = test_secret_key();

    let token = TokenBuilder::new(issuer)
        .for_key(intended_audience.public())
        .with_capability(Capability::Prompt)
        .build()
        .expect("should build token");

    let verifier = TokenVerifier::new();

    // Correct presenter should pass
    verifier.verify(&token, Some(&intended_audience.public())).expect("should verify");

    // Wrong presenter should fail
    let result = verifier.verify(&token, Some(&wrong_presenter.public()));
    assert!(matches!(result, Err(AuthError::WrongAudience { .. })));

    // No presenter should fail for Key audience
    let result = verifier.verify(&token, None);
    assert!(matches!(result, Err(AuthError::AudienceRequired)));
}

#[test]
fn test_verifier_authorize() {
    let key = test_secret_key();

    let token = TokenBuilder::new(key)
        .with_capability(Capability::Prompt)
        .with_capability(Capability::ToolUse {
            tool_pattern: "read,grep".into(),
        })
        .build()
        .expect("should build token");

    let verifier = TokenVerifier::new();

    // Should authorize matching operations
    verifier
        .authorize(&token, &Operation::Prompt { text: "hello".into() }, None)
        .expect("should authorize prompt");
    verifier
        .authorize(
            &token,
            &Operation::ToolUse {
                tool_name: "read".into(),
            },
            None,
        )
        .expect("should authorize tool use");

    // Should reject non-matching operations
    let result = verifier.authorize(
        &token,
        &Operation::ToolUse {
            tool_name: "bash".into(),
        },
        None,
    );
    assert!(matches!(result, Err(AuthError::Unauthorized { .. })));
}

#[test]
fn test_verifier_revocation() {
    let key = test_secret_key();

    let token = TokenBuilder::new(key)
        .with_capability(Capability::Prompt)
        .with_random_nonce()
        .build()
        .expect("should build token");

    let verifier = TokenVerifier::new();

    // Should verify before revocation
    verifier.verify(&token, None).expect("should verify before revocation");

    // Revoke the token
    verifier.revoke_token(&token).unwrap();

    // Should fail after revocation
    let result = verifier.verify(&token, None);
    assert!(matches!(result, Err(AuthError::TokenRevoked)));
}

#[test]
fn test_verifier_trusted_roots() {
    let trusted = test_secret_key();
    let untrusted = test_secret_key();

    let trusted_token = TokenBuilder::new(trusted.clone())
        .with_capability(Capability::Prompt)
        .build()
        .expect("should build token");

    let untrusted_token = TokenBuilder::new(untrusted)
        .with_capability(Capability::Prompt)
        .build()
        .expect("should build token");

    let verifier = TokenVerifier::new().with_trusted_root(trusted.public());

    // Trusted issuer should pass
    verifier.verify(&trusted_token, None).expect("should verify trusted");

    // Untrusted issuer should fail
    let result = verifier.verify(&untrusted_token, None);
    assert!(matches!(result, Err(AuthError::UntrustedRoot)));
}

// ============================================================================
// Root Token Generation Tests
// ============================================================================

#[test]
fn test_generate_root_token() {
    let secret = test_secret_key();
    let lifetime = Duration::from_secs(3600);

    let token = generate_root_token(&secret, lifetime).expect("should generate root token");

    // Should have correct issuer
    assert_eq!(token.issuer, secret.public());

    // Should be a bearer token
    assert!(matches!(token.audience, Audience::Bearer));

    // Should have all root capabilities
    assert!(token.capabilities.contains(&Capability::Prompt));
    assert!(token.capabilities.contains(&Capability::ToolUse {
        tool_pattern: "*".into()
    }));
    assert!(token.capabilities.contains(&Capability::ShellExecute {
        command_pattern: "*".into(),
        working_dir: None
    }));
    assert!(token.capabilities.contains(&Capability::FileAccess {
        prefix: "/".into(),
        read_only: false
    }));
    assert!(token.capabilities.contains(&Capability::BotCommand {
        command_pattern: "*".into()
    }));
    assert!(token.capabilities.contains(&Capability::SessionManage));
    assert!(token.capabilities.contains(&Capability::ModelSwitch));
    assert!(token.capabilities.contains(&Capability::Delegate));

    // Should have a nonce
    assert!(token.nonce.is_some());

    // Should verify correctly
    let verifier = TokenVerifier::new().with_trusted_root(secret.public());
    verifier.verify(&token, None).expect("root token should verify");

    // Should authorize all operations
    verifier
        .authorize(&token, &Operation::Prompt { text: "test".into() }, None)
        .expect("root token should authorize prompts");

    verifier
        .authorize(
            &token,
            &Operation::ToolUse {
                tool_name: "bash".into(),
            },
            None,
        )
        .expect("root token should authorize tool use");

    verifier
        .authorize(
            &token,
            &Operation::FileWrite {
                path: "/etc/config".into(),
            },
            None,
        )
        .expect("root token should authorize file writes");
}

// ============================================================================
// Delegation Chain Tests
// ============================================================================

#[test]
fn test_delegation_depth_tracking() {
    let root_key = test_secret_key();

    // Root token should have depth 0
    let root = TokenBuilder::new(root_key.clone())
        .with_capability(Capability::Prompt)
        .with_capability(Capability::Delegate)
        .build()
        .expect("should build root token");

    assert_eq!(root.delegation_depth, 0);
    assert!(root.proof.is_none());

    // First level delegation should have depth 1
    let child1_key = test_secret_key();
    let child1 = TokenBuilder::new(child1_key.clone())
        .delegated_from(root.clone())
        .with_capability(Capability::Prompt)
        .with_capability(Capability::Delegate)
        .build()
        .expect("should build child1 token");

    assert_eq!(child1.delegation_depth, 1);
    assert!(child1.proof.is_some());
    assert_eq!(child1.proof.unwrap(), root.hash().unwrap());

    // Second level delegation should have depth 2
    let child2_key = test_secret_key();
    let child2 = TokenBuilder::new(child2_key)
        .delegated_from(child1.clone())
        .with_capability(Capability::Prompt)
        .build()
        .expect("should build child2 token");

    assert_eq!(child2.delegation_depth, 2);
    assert_eq!(child2.proof.unwrap(), child1.hash().unwrap());
}

#[test]
fn test_delegation_too_deep() {
    use crate::constants::MAX_DELEGATION_DEPTH;

    // Build a chain at MAX_DELEGATION_DEPTH
    let mut current_token: Option<CapabilityToken> = None;

    for i in 0..=MAX_DELEGATION_DEPTH {
        let key = test_secret_key();
        let mut builder =
            TokenBuilder::new(key).with_capability(Capability::Prompt).with_capability(Capability::Delegate);

        if let Some(parent) = current_token.take() {
            builder = builder.delegated_from(parent);
        }

        current_token = Some(builder.build().expect(&format!("should build token at depth {}", i)));
    }

    // Attempt to delegate beyond MAX_DELEGATION_DEPTH should fail
    let final_token = current_token.unwrap();
    assert_eq!(final_token.delegation_depth, MAX_DELEGATION_DEPTH);

    let one_more_key = test_secret_key();
    let result = TokenBuilder::new(one_more_key)
        .delegated_from(final_token)
        .with_capability(Capability::Prompt)
        .build();

    assert!(
        matches!(result, Err(AuthError::DelegationTooDeep { depth, max }) if depth == MAX_DELEGATION_DEPTH + 1 && max == MAX_DELEGATION_DEPTH)
    );
}

#[test]
fn test_verify_with_chain() {
    let trusted_key = test_secret_key();
    let service_key = test_secret_key();
    let client_key = test_secret_key();

    // Create 3-level chain: trusted -> service -> client
    let root = TokenBuilder::new(trusted_key.clone())
        .with_capability(Capability::ToolUse {
            tool_pattern: "*".into(),
        })
        .with_capability(Capability::Delegate)
        .build()
        .expect("should build root token");

    let service = TokenBuilder::new(service_key.clone())
        .delegated_from(root.clone())
        .with_capability(Capability::ToolUse {
            tool_pattern: "read,grep,find".into(),
        })
        .with_capability(Capability::Delegate)
        .build()
        .expect("should build service token");

    let client = TokenBuilder::new(client_key.clone())
        .delegated_from(service.clone())
        .for_key(client_key.public())
        .with_capability(Capability::ToolUse {
            tool_pattern: "read,grep".into(),
        })
        .build()
        .expect("should build client token");

    let verifier = TokenVerifier::new().with_trusted_root(trusted_key.public());

    // Verify with explicit chain
    verifier
        .verify_with_chain(&client, &[service, root], Some(&client_key.public()))
        .expect("should verify with chain");
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[test]
fn test_concurrent_revocation() {
    use std::sync::Arc;
    use std::thread;

    let verifier = Arc::new(TokenVerifier::new());
    let mut handles = vec![];

    // Spawn multiple threads to revoke different tokens concurrently
    for i in 0..10 {
        let verifier_clone = Arc::clone(&verifier);
        handles.push(thread::spawn(move || {
            let mut hash = [0u8; 32];
            hash[0] = i;
            verifier_clone.revoke(hash).expect("should revoke");
        }));
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("thread should complete");
    }

    // Verify all revocations were recorded
    assert_eq!(verifier.revocation_count().unwrap(), 10);
}

#[test]
fn test_load_and_get_revoked() {
    let verifier = TokenVerifier::new();

    // Add some revocations
    let hash1 = [1u8; 32];
    let hash2 = [2u8; 32];
    let hash3 = [3u8; 32];

    verifier.revoke(hash1).unwrap();
    verifier.revoke(hash2).unwrap();
    verifier.revoke(hash3).unwrap();

    // Get all revoked hashes
    let all_revoked = verifier.get_all_revoked().unwrap();
    assert_eq!(all_revoked.len(), 3);
    assert!(all_revoked.contains(&hash1));
    assert!(all_revoked.contains(&hash2));
    assert!(all_revoked.contains(&hash3));

    // Create a new verifier and load from the first one
    let verifier2 = TokenVerifier::new();
    verifier2.load_revoked(&all_revoked).unwrap();

    // Both verifiers should see the same revocations
    assert_eq!(verifier2.revocation_count().unwrap(), 3);
    assert!(verifier2.is_revoked(&hash1).unwrap());
    assert!(verifier2.is_revoked(&hash2).unwrap());
    assert!(verifier2.is_revoked(&hash3).unwrap());
}
