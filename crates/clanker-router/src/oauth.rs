//! Anthropic OAuth PKCE flow
//!
//! Implements the full OAuth 2.0 Authorization Code flow with PKCE (Proof Key
//! for Code Exchange) for authenticating with Anthropic's API via Claude Max.
//!
//! ## Flow
//!
//! 1. Call [`build_auth_url()`] to generate the authorization URL + PKCE verifier
//! 2. User visits the URL in their browser and authorizes
//! 3. Call [`exchange_code()`] with the code from the callback to get tokens
//! 4. On expiry, call [`refresh_token()`] to get fresh tokens

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::error::Error;
use crate::error::Result;

const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";
const SCOPES: &str = "org:create_api_key user:profile user:inference";

/// OAuth credentials with access token, refresh token, and expiration time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    /// Access token for API requests
    pub access: String,
    /// Refresh token for obtaining new access tokens
    pub refresh: String,
    /// Expiration timestamp in milliseconds since epoch
    pub expires: i64,
}

impl OAuthCredentials {
    /// Check if the access token is expired or about to expire.
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp_millis() >= self.expires
    }

    /// Convert to a [`StoredCredential::OAuth`](crate::auth::StoredCredential).
    pub fn to_stored(&self) -> crate::auth::StoredCredential {
        crate::auth::StoredCredential::OAuth {
            access_token: self.access.clone(),
            refresh_token: self.refresh.clone(),
            expires_at_ms: self.expires,
            label: None,
        }
    }

    /// Convert from a [`StoredCredential::OAuth`](crate::auth::StoredCredential).
    ///
    /// Returns `None` if the credential is an API key.
    pub fn from_stored(cred: &crate::auth::StoredCredential) -> Option<Self> {
        match cred {
            crate::auth::StoredCredential::OAuth {
                access_token,
                refresh_token,
                expires_at_ms,
                ..
            } => Some(Self {
                access: access_token.clone(),
                refresh: refresh_token.clone(),
                expires: *expires_at_ms,
            }),
            crate::auth::StoredCredential::ApiKey { .. } => None,
        }
    }
}

/// PKCE challenge pair (verifier and challenge).
struct PkceChallenge {
    verifier: String,
    challenge: String,
}

/// Generate a PKCE verifier and challenge.
fn generate_pkce() -> PkceChallenge {
    let mut verifier_bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut verifier_bytes);

    let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();

    let challenge = URL_SAFE_NO_PAD.encode(hash);

    PkceChallenge { verifier, challenge }
}

/// Build the OAuth authorization URL.
///
/// Returns `(authorization_url, verifier)` where the verifier must be saved
/// for the [`exchange_code()`] step.
pub fn build_auth_url() -> (String, String) {
    let pkce = generate_pkce();

    let url = url::Url::parse_with_params(AUTHORIZE_URL, &[
        ("code", "true"),
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", REDIRECT_URI),
        ("scope", SCOPES),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", &pkce.verifier),
    ])
    .expect("valid authorization URL");

    (url.to_string(), pkce.verifier)
}

/// Token exchange response from Anthropic.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

/// Calculate expiration timestamp with a 5-minute safety buffer.
fn expiration_from_expires_in(expires_in: i64) -> i64 {
    chrono::Utc::now().timestamp_millis() + (expires_in * 1000) - (5 * 60 * 1000)
}

/// Exchange an authorization code for OAuth credentials.
///
/// # Arguments
/// * `code` — The authorization code from the OAuth callback
/// * `state` — The state parameter from the OAuth callback
/// * `verifier` — The PKCE verifier from [`build_auth_url()`]
pub async fn exchange_code(code: &str, state: &str, verifier: &str) -> Result<OAuthCredentials> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "grant_type": "authorization_code",
        "client_id": CLIENT_ID,
        "code": code,
        "state": state,
        "redirect_uri": REDIRECT_URI,
        "code_verifier": verifier,
    });

    let response = client.post(TOKEN_URL).json(&body).send().await.map_err(|e| Error::Auth {
        message: format!("failed to send token exchange request: {}", e),
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "unknown error".to_string());
        return Err(Error::Auth {
            message: format!("token exchange failed ({}): {}", status, error_text),
        });
    }

    let token_response: TokenResponse = response.json().await.map_err(|e| Error::Auth {
        message: format!("failed to parse token response: {}", e),
    })?;

    Ok(OAuthCredentials {
        access: token_response.access_token,
        refresh: token_response.refresh_token,
        expires: expiration_from_expires_in(token_response.expires_in),
    })
}

/// Refresh an expired OAuth token.
///
/// # Arguments
/// * `refresh_token` — The refresh token from previous [`OAuthCredentials`]
pub async fn refresh_token(refresh_token: &str) -> Result<OAuthCredentials> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "client_id": CLIENT_ID,
        "refresh_token": refresh_token,
    });

    let response = client.post(TOKEN_URL).json(&body).send().await.map_err(|e| Error::Auth {
        message: format!("failed to send token refresh request: {}", e),
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "unknown error".to_string());
        return Err(Error::Auth {
            message: format!("token refresh failed ({}): {}", status, error_text),
        });
    }

    let token_response: TokenResponse = response.json().await.map_err(|e| Error::Auth {
        message: format!("failed to parse refresh token response: {}", e),
    })?;

    Ok(OAuthCredentials {
        access: token_response.access_token,
        refresh: token_response.refresh_token,
        expires: expiration_from_expires_in(token_response.expires_in),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_auth_url() {
        let (url, verifier) = build_auth_url();

        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("client_id="));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("scope="));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state="));

        assert!(!verifier.is_empty());
        assert!(verifier.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_pkce_generation() {
        let pkce1 = generate_pkce();
        let pkce2 = generate_pkce();

        assert_ne!(pkce1.verifier, pkce2.verifier);
        assert_ne!(pkce1.challenge, pkce2.challenge);
        assert_ne!(pkce1.verifier, pkce1.challenge);
    }

    #[test]
    fn test_credentials_expiry() {
        let expired = OAuthCredentials {
            access: "token".to_string(),
            refresh: "refresh".to_string(),
            expires: chrono::Utc::now().timestamp_millis() - 1000,
        };
        assert!(expired.is_expired());

        let valid = OAuthCredentials {
            access: "token".to_string(),
            refresh: "refresh".to_string(),
            expires: chrono::Utc::now().timestamp_millis() + 3600000,
        };
        assert!(!valid.is_expired());
    }

    #[test]
    fn test_to_stored_roundtrip() {
        let creds = OAuthCredentials {
            access: "acc".to_string(),
            refresh: "ref".to_string(),
            expires: 12345,
        };
        let stored = creds.to_stored();
        let back = OAuthCredentials::from_stored(&stored).unwrap();
        assert_eq!(back.access, "acc");
        assert_eq!(back.refresh, "ref");
        assert_eq!(back.expires, 12345);
    }

    #[test]
    fn test_from_stored_api_key_returns_none() {
        let stored = crate::auth::StoredCredential::ApiKey {
            api_key: "sk-test".into(),
            label: None,
        };
        assert!(OAuthCredentials::from_stored(&stored).is_none());
    }
}
