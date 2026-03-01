use anyhow::{Context, Result, anyhow, bail};
use libsql::Builder;
use oauth2::{
    AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet,
    EndpointSet, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope,
    TokenResponse, TokenUrl,
    basic::BasicClient,
};
use reqwest::Client;
use serde::{Deserialize, de::DeserializeOwned};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use tracing::info;
use url::Url;

const DEFAULT_REDIRECT_URL: &str = "http://127.0.0.1:53682/callback";
const DEFAULT_DB_PATH: &str = ".mail/mail.db";
const CALLBACK_TIMEOUT_SECS: u64 = 180;
const MESSAGE_LIMIT: usize = 20;
pub const DEFAULT_GOOGLE_CLIENT_ID: &str = "";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Google,
    Outlook,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Provider::Google => "Google",
            Provider::Outlook => "Outlook",
        }
    }

    fn as_key(self) -> &'static str {
        match self {
            Provider::Google => "google",
            Provider::Outlook => "outlook",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "google" => Some(Provider::Google),
            "outlook" => Some(Provider::Outlook),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderCredentials {
    pub client_id: String,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SavedOAuthSettings {
    pub google: Option<ProviderCredentials>,
    pub outlook: Option<ProviderCredentials>,
}

#[derive(Debug, Clone)]
pub struct MailMessage {
    pub subject: String,
    pub from: String,
    pub date: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct LoginResult {
    pub provider: Provider,
    pub account: String,
    pub messages: Vec<MailMessage>,
}

pub struct Engine {
    app_name: String,
}

impl Engine {
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("engine start: {}", self.app_name);
        Ok(())
    }

    pub async fn load_oauth_settings(&self) -> Result<SavedOAuthSettings> {
        let conn = open_conn().await?;
        let mut rows = conn
            .query(
                "SELECT provider, client_id, COALESCE(client_secret, '') FROM oauth_settings",
                (),
            )
            .await?;

        let mut settings = SavedOAuthSettings::default();

        while let Some(row) = rows.next().await? {
            let provider_raw: String = row.get(0)?;
            let client_id: String = row.get(1)?;
            let client_secret_raw: String = row.get(2)?;

            let credentials = ProviderCredentials {
                client_id,
                client_secret: empty_to_none(client_secret_raw),
            };

            match Provider::from_key(&provider_raw) {
                Some(Provider::Google) => settings.google = Some(credentials),
                Some(Provider::Outlook) => settings.outlook = Some(credentials),
                None => {}
            }
        }

        Ok(settings)
    }

    pub async fn save_provider_credentials(
        &self,
        provider: Provider,
        credentials: ProviderCredentials,
    ) -> Result<()> {
        let client_id = credentials.client_id.trim();
        if client_id.is_empty() {
            bail!("client id mag niet leeg zijn");
        }

        let conn = open_conn().await?;

        conn.execute(
            "INSERT INTO oauth_settings (provider, client_id, client_secret)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(provider) DO UPDATE SET
                client_id = excluded.client_id,
                client_secret = excluded.client_secret",
            libsql::params![
                provider.as_key(),
                client_id.to_owned(),
                normalized_secret(credentials.client_secret)
            ],
        )
        .await?;

        Ok(())
    }

    pub async fn login_and_fetch(&self, provider: Provider) -> Result<LoginResult> {
        info!("starting OAuth for provider={}", provider.label());
        let credentials = self
            .load_provider_credentials(provider)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Geen OAuth-client ingesteld voor {}. Vul eerst Client ID in de app in.",
                    provider.label()
                )
            })?;

        validate_credentials(provider, &credentials)?;

        let config = ProviderConfig::from_credentials(provider, credentials);
        let redirect_url = redirect_url()?;
        let redirect_target = RedirectTarget::from_url(&redirect_url)?;
        let oauth = build_oauth_client(&config, redirect_url)?;
        let stored_refresh = self.load_refresh_token(provider).await?;

        if let Some(refresh_token) = stored_refresh.as_deref() {
            match exchange_refresh_token(&oauth, refresh_token.to_owned()).await {
                Ok(token_set) => {
                    if let Some(new_refresh_token) = token_set.refresh_token {
                        self.save_refresh_token(provider, &new_refresh_token).await?;
                    }
                    return fetch_inbox(&config, &token_set.access_token).await;
                }
                Err(error) => {
                    info!(
                        "stored refresh token rejected for provider={}: {error:#}",
                        provider.label()
                    );
                    self.clear_refresh_token(provider).await?;
                }
            }
        }

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let mut request = oauth
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        for scope in config.scopes {
            request = request.add_scope(Scope::new((*scope).to_owned()));
        }

        if provider == Provider::Google && stored_refresh.is_none() {
            request = request
                .add_extra_param("access_type", "offline")
                .add_extra_param("prompt", "consent");
        }

        let (auth_url, csrf_state) = request.url();
        webbrowser::open(auth_url.as_str())
            .map_err(|error| anyhow!("browser kon niet worden geopend: {error}"))?;

        let code = wait_for_oauth_code(&redirect_target, csrf_state.secret()).await?;
        let token_set = exchange_token(&oauth, code, pkce_verifier)
            .await
            .map_err(|error| with_token_exchange_hint(provider, error))?;

        if let Some(refresh_token) = token_set.refresh_token {
            self.save_refresh_token(provider, &refresh_token).await?;
        }

        fetch_inbox(&config, &token_set.access_token).await
    }

    pub async fn try_restore_session(&self, provider: Provider) -> Result<Option<LoginResult>> {
        let refresh_token = match self.load_refresh_token(provider).await? {
            Some(token) => token,
            None => return Ok(None),
        };

        let credentials = self
            .load_provider_credentials(provider)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Geen OAuth-client ingesteld voor {}. Vul eerst Client ID in de app in.",
                    provider.label()
                )
            })?;

        validate_credentials(provider, &credentials)?;

        let config = ProviderConfig::from_credentials(provider, credentials);
        let oauth = build_oauth_client(&config, redirect_url()?)?;
        let token_set = match exchange_refresh_token(&oauth, refresh_token).await {
            Ok(token_set) => token_set,
            Err(error) => {
                info!(
                    "session restore failed for provider={}: {error:#}",
                    provider.label()
                );
                self.clear_refresh_token(provider).await?;
                return Ok(None);
            }
        };

        if let Some(new_refresh_token) = token_set.refresh_token {
            self.save_refresh_token(provider, &new_refresh_token).await?;
        }

        let result = fetch_inbox(&config, &token_set.access_token).await?;
        Ok(Some(result))
    }

    async fn load_provider_credentials(
        &self,
        provider: Provider,
    ) -> Result<Option<ProviderCredentials>> {
        let settings = self.load_oauth_settings().await?;
        let creds = match provider {
            Provider::Google => settings.google.or_else(load_google_credentials_from_env),
            Provider::Outlook => settings.outlook,
        };

        Ok(creds)
    }

    async fn load_refresh_token(&self, provider: Provider) -> Result<Option<String>> {
        let conn = open_conn().await?;
        let mut rows = conn
            .query(
                "SELECT refresh_token FROM oauth_tokens WHERE provider = ?1",
                libsql::params![provider.as_key()],
            )
            .await?;

        let token = if let Some(row) = rows.next().await? {
            let refresh_token: String = row.get(0)?;
            if refresh_token.trim().is_empty() {
                None
            } else {
                Some(refresh_token)
            }
        } else {
            None
        };

        Ok(token)
    }

    async fn save_refresh_token(&self, provider: Provider, refresh_token: &str) -> Result<()> {
        let conn = open_conn().await?;
        conn.execute(
            "INSERT INTO oauth_tokens (provider, refresh_token)
             VALUES (?1, ?2)
             ON CONFLICT(provider) DO UPDATE SET
                refresh_token = excluded.refresh_token",
            libsql::params![provider.as_key(), refresh_token],
        )
        .await?;
        Ok(())
    }

    async fn clear_refresh_token(&self, provider: Provider) -> Result<()> {
        let conn = open_conn().await?;
        conn.execute(
            "DELETE FROM oauth_tokens WHERE provider = ?1",
            libsql::params![provider.as_key()],
        )
        .await?;
        Ok(())
    }
}

fn empty_to_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn normalized_secret(secret: Option<String>) -> Option<String> {
    secret.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

fn load_google_credentials_from_env() -> Option<ProviderCredentials> {
    let client_id = std::env::var("MAIL_GOOGLE_CLIENT_ID").ok()?;
    let client_id = client_id.trim().to_owned();
    if client_id.is_empty() {
        return None;
    }

    let client_secret = std::env::var("MAIL_GOOGLE_CLIENT_SECRET")
        .ok()
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        });

    Some(ProviderCredentials {
        client_id,
        client_secret,
    })
}

fn validate_credentials(provider: Provider, credentials: &ProviderCredentials) -> Result<()> {
    let client_id = credentials.client_id.trim();
    if client_id.is_empty() {
        bail!("Client ID is leeg.");
    }

    if provider == Provider::Google && !client_id.ends_with(".apps.googleusercontent.com") {
        bail!(
            "Google Client ID lijkt ongeldig. Gebruik de volledige OAuth Client ID uit Google Cloud (eindigt op .apps.googleusercontent.com)."
        );
    }

    Ok(())
}

fn with_token_exchange_hint(provider: Provider, error: anyhow::Error) -> anyhow::Error {
    let rendered = format!("{error:#}");
    let lowered = rendered.to_ascii_lowercase();

    if lowered.contains("client_secret is missing")
        || lowered.contains("client secret is missing")
        || lowered.contains("aadsts7000218")
    {
        let hint = match provider {
            Provider::Google => {
                "Deze Google OAuth client verwacht een Client Secret. Vul de juiste secret in, of \
                 gebruik een Desktop app OAuth client in Google Cloud."
            }
            Provider::Outlook => {
                "Deze Microsoft App Registration verwacht een Client Secret. Voeg een secret toe \
                 (Certificates & secrets), of zet de app als public client (Allow public client \
                 flows) en log in zonder secret."
            }
        };

        return anyhow!("{rendered}\nTip: {hint}");
    }

    if lowered.contains("invalid_client") {
        let hint = match provider {
            Provider::Google => {
                "Controleer Google OAuth: juiste Client ID/Secret uit hetzelfde project en \
                 app-type Desktop app. Gebruik exact dezelfde credentials als in Google Cloud \
                 Console."
            }
            Provider::Outlook => {
                "Controleer Microsoft OAuth: Client ID/Secret moeten uit dezelfde App \
                 Registration komen. Als je geen secret gebruikt, zet de app als public client \
                 (mobile/desktop) en gebruik de loopback redirect URI."
            }
        };

        return anyhow!("{rendered}\nTip: {hint}");
    }

    error
}

fn local_db_path() -> String {
    std::env::var("MAIL_DB_PATH").unwrap_or_else(|_| DEFAULT_DB_PATH.to_owned())
}

async fn open_conn() -> Result<libsql::Connection> {
    let path = local_db_path();
    let path_ref = Path::new(&path);

    if let Some(parent) = path_ref.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("kan DB map niet maken: {}", parent.display()))?;
    }

    let db = Builder::new_local(path).build().await?;
    let conn = db.connect()?;
    ensure_schema(&conn).await?;
    Ok(conn)
}

async fn ensure_schema(conn: &libsql::Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS oauth_settings (
            provider TEXT PRIMARY KEY NOT NULL,
            client_id TEXT NOT NULL,
            client_secret TEXT
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS oauth_tokens (
            provider TEXT PRIMARY KEY NOT NULL,
            refresh_token TEXT NOT NULL
        )",
        (),
    )
    .await?;

    Ok(())
}

#[derive(Debug)]
struct ProviderConfig {
    provider: Provider,
    credentials: ProviderCredentials,
    auth_url: &'static str,
    token_url: &'static str,
    scopes: &'static [&'static str],
}

impl ProviderConfig {
    fn from_credentials(provider: Provider, credentials: ProviderCredentials) -> Self {
        match provider {
            Provider::Google => Self {
                provider,
                credentials,
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
                token_url: "https://oauth2.googleapis.com/token",
                scopes: &[
                    "openid",
                    "email",
                    "profile",
                    "https://www.googleapis.com/auth/gmail.readonly",
                ],
            },
            Provider::Outlook => Self {
                provider,
                credentials,
                auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
                token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
                scopes: &["openid", "email", "profile", "offline_access", "Mail.Read"],
            },
        }
    }
}

type OAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

#[derive(Debug, Clone)]
struct TokenSet {
    access_token: String,
    refresh_token: Option<String>,
}

fn build_oauth_client(config: &ProviderConfig, redirect_url: Url) -> Result<OAuthClient> {
    let mut client = BasicClient::new(ClientId::new(config.credentials.client_id.clone()))
        .set_auth_uri(AuthUrl::new(config.auth_url.to_owned())?)
        .set_token_uri(TokenUrl::new(config.token_url.to_owned())?)
        .set_redirect_uri(RedirectUrl::new(redirect_url.to_string())?)
        .set_auth_type(AuthType::RequestBody);

    if let Some(secret) = &config.credentials.client_secret {
        client = client.set_client_secret(ClientSecret::new(secret.clone()));
    }

    Ok(client)
}

fn redirect_url() -> Result<Url> {
    let raw = std::env::var("MAIL_OAUTH_REDIRECT_URI")
        .unwrap_or_else(|_| DEFAULT_REDIRECT_URL.to_owned());
    Url::parse(&raw).with_context(|| format!("ongeldige MAIL_OAUTH_REDIRECT_URI: {raw}"))
}

#[derive(Debug)]
struct RedirectTarget {
    host: String,
    port: u16,
    path: String,
}

impl RedirectTarget {
    fn from_url(url: &Url) -> Result<Self> {
        if url.scheme() != "http" {
            bail!("redirect URI moet http zijn (localhost callback)");
        }

        let host = url
            .host_str()
            .ok_or_else(|| anyhow!("redirect URI mist host"))?
            .to_owned();

        if host != "127.0.0.1" && host != "localhost" {
            bail!("redirect URI host moet localhost of 127.0.0.1 zijn");
        }

        let port = url
            .port_or_known_default()
            .ok_or_else(|| anyhow!("redirect URI mist poort"))?;

        let path = url.path().to_owned();
        if path.is_empty() || path == "/" {
            bail!("redirect URI pad moet specifiek zijn (bijv. /callback)");
        }

        Ok(Self { host, port, path })
    }
}

async fn wait_for_oauth_code(target: &RedirectTarget, expected_state: &str) -> Result<String> {
    let listener = TcpListener::bind((target.host.as_str(), target.port))
        .await
        .with_context(|| {
            format!(
                "kan callback server niet starten op {}:{}",
                target.host, target.port
            )
        })?;

    let (mut stream, _) = timeout(
        Duration::from_secs(CALLBACK_TIMEOUT_SECS),
        listener.accept(),
    )
    .await
    .context("timeout wachtend op OAuth callback")??;

    let mut buf = [0_u8; 8192];
    let n = timeout(Duration::from_secs(20), stream.read(&mut buf))
        .await
        .context("timeout bij lezen callback request")??;
    if n == 0 {
        bail!("lege callback request ontvangen");
    }

    let request = String::from_utf8_lossy(&buf[..n]);
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| anyhow!("onleesbare callback request"))?;
    let path_with_query = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("onleesbare callback request line"))?;

    let callback_url = Url::parse(&format!("http://localhost{path_with_query}"))
        .context("ongeldige callback URL")?;
    let (status, body, code) = parse_callback_params(&callback_url, target, expected_state)?;

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;

    code
}

fn parse_callback_params(
    callback_url: &Url,
    target: &RedirectTarget,
    expected_state: &str,
) -> Result<(&'static str, String, Result<String>)> {
    if callback_url.path() != target.path {
        let msg = format!("Ongeldige callback path: {}", callback_url.path());
        return Ok(("400 Bad Request", msg.clone(), Err(anyhow!(msg))));
    }

    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    let mut error: Option<String> = None;

    for (key, value) in callback_url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            _ => {}
        }
    }

    if let Some(error) = error {
        let msg = format!("OAuth login mislukt: {error}");
        return Ok(("400 Bad Request", msg.clone(), Err(anyhow!(msg))));
    }

    if state.as_deref() != Some(expected_state) {
        let msg = "OAuth state mismatch".to_owned();
        return Ok(("400 Bad Request", msg.clone(), Err(anyhow!(msg))));
    }

    match code {
        Some(code) => Ok((
            "200 OK",
            "Login geslaagd. Je kunt dit tabblad sluiten.".to_owned(),
            Ok(code),
        )),
        None => {
            let msg = "OAuth callback bevat geen code".to_owned();
            Ok(("400 Bad Request", msg.clone(), Err(anyhow!(msg))))
        }
    }
}

async fn exchange_token(
    client: &OAuthClient,
    code: String,
    pkce_verifier: PkceCodeVerifier,
) -> Result<TokenSet> {
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let response = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .await
        .context("token exchange mislukt")?;

    Ok(TokenSet {
        access_token: response.access_token().secret().to_owned(),
        refresh_token: response.refresh_token().map(|token| token.secret().to_owned()),
    })
}

async fn exchange_refresh_token(client: &OAuthClient, refresh_token: String) -> Result<TokenSet> {
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let response = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token))
        .request_async(&http_client)
        .await
        .context("refresh token exchange mislukt")?;

    Ok(TokenSet {
        access_token: response.access_token().secret().to_owned(),
        refresh_token: response.refresh_token().map(|token| token.secret().to_owned()),
    })
}

async fn fetch_inbox(config: &ProviderConfig, access_token: &str) -> Result<LoginResult> {
    let http = Client::new();

    match config.provider {
        Provider::Google => fetch_google_inbox(http, access_token).await,
        Provider::Outlook => fetch_outlook_inbox(http, access_token).await,
    }
}

async fn fetch_google_inbox(http: Client, access_token: &str) -> Result<LoginResult> {
    let me: GoogleUserInfo = send_google_json(
        http.get("https://www.googleapis.com/oauth2/v3/userinfo")
            .bearer_auth(access_token),
        "Google userinfo",
    )
    .await?;

    let list: GoogleListResponse = send_google_json(
        http.get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
            .query(&[("maxResults", MESSAGE_LIMIT)])
            .bearer_auth(access_token),
        "Gmail messages list",
    )
    .await?;

    let mut messages = Vec::new();
    for message in list.messages.unwrap_or_default() {
        let detail: GoogleMessageResponse = send_google_json(
            http.get(format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}",
                message.id
            ))
            .query(&[
                ("format", "metadata"),
                ("metadataHeaders", "Subject"),
                ("metadataHeaders", "From"),
                ("metadataHeaders", "Date"),
            ])
            .bearer_auth(access_token),
            "Gmail message detail",
        )
        .await?;

        let GoogleMessageResponse { payload, snippet } = detail;
        let (subject, from, date) = extract_google_headers(payload);
        messages.push(MailMessage {
            subject,
            from,
            date,
            body: snippet
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "(geen inhoud)".to_owned()),
        });
    }

    Ok(LoginResult {
        provider: Provider::Google,
        account: me.email.unwrap_or_else(|| "(onbekend account)".to_owned()),
        messages,
    })
}

async fn send_google_json<T>(request: reqwest::RequestBuilder, endpoint: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let response = request
        .send()
        .await
        .with_context(|| format!("{endpoint} request mislukt"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("{endpoint} response kon niet gelezen worden"))?;

    if !status.is_success() {
        let detail = google_error_detail(&body);
        let mut msg = format!("{endpoint} gaf HTTP {status}");
        if let Some(detail) = detail {
            msg.push_str(&format!(": {detail}"));
            if let Some(hint) = google_error_hint(&detail) {
                msg.push_str(&format!("\nTip: {hint}"));
            }
        } else {
            let raw = body.lines().next().unwrap_or("").trim();
            if !raw.is_empty() {
                msg.push_str(&format!(": {raw}"));
            }
        }
        bail!(msg);
    }

    serde_json::from_str(&body).with_context(|| format!("{endpoint} response heeft ongeldige JSON"))
}

fn google_error_detail(body: &str) -> Option<String> {
    let parsed: GoogleErrorEnvelope = serde_json::from_str(body).ok()?;
    let error = parsed.error?;
    let mut detail = error.message?;

    if let Some(first_reason) = error
        .errors
        .and_then(|items| items.into_iter().find_map(|item| item.reason))
    {
        detail.push_str(&format!(" (reason: {first_reason})"));
    }

    Some(detail)
}

fn google_error_hint(detail: &str) -> Option<&'static str> {
    let lowered = detail.to_ascii_lowercase();

    if lowered.contains("has not been used in project") || lowered.contains("is disabled") {
        return Some(
            "Enable de Gmail API in hetzelfde Google Cloud project als deze OAuth client.",
        );
    }

    if lowered.contains("insufficient authentication scopes")
        || lowered.contains("insufficientpermissions")
        || lowered.contains("insufficient permissions")
    {
        return Some(
            "Verwijder app-toegang in je Google account en log opnieuw in zodat gmail.readonly opnieuw wordt toegekend.",
        );
    }

    if lowered.contains("access blocked") || lowered.contains("access_not_configured") {
        return Some(
            "Controleer OAuth consent screen + test users en bevestig dat deze account toegang heeft tot de app.",
        );
    }

    None
}

async fn fetch_outlook_inbox(http: Client, access_token: &str) -> Result<LoginResult> {
    let me: GraphMeResponse = http
        .get("https://graph.microsoft.com/v1.0/me?$select=mail,userPrincipalName")
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let inbox: GraphInboxResponse = http
        .get("https://graph.microsoft.com/v1.0/me/messages")
        .query(&[
            ("$top", MESSAGE_LIMIT.to_string()),
            (
                "$select",
                "subject,from,receivedDateTime,bodyPreview".to_owned(),
            ),
            ("$orderby", "receivedDateTime desc".to_owned()),
        ])
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let messages = inbox
        .value
        .into_iter()
        .map(|entry| MailMessage {
            subject: entry
                .subject
                .unwrap_or_else(|| "(geen onderwerp)".to_owned()),
            from: entry
                .from
                .and_then(|f| f.email_address)
                .and_then(|a| a.address)
                .unwrap_or_else(|| "(onbekend)".to_owned()),
            date: entry
                .received_date_time
                .unwrap_or_else(|| "(onbekend)".to_owned()),
            body: entry
                .body_preview
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "(geen inhoud)".to_owned()),
        })
        .collect();

    Ok(LoginResult {
        provider: Provider::Outlook,
        account: me
            .mail
            .or(me.user_principal_name)
            .unwrap_or_else(|| "(onbekend account)".to_owned()),
        messages,
    })
}

fn extract_google_headers(payload: Option<GooglePayload>) -> (String, String, String) {
    let mut subject = "(geen onderwerp)".to_owned();
    let mut from = "(onbekend)".to_owned();
    let mut date = "(onbekend)".to_owned();

    if let Some(payload) = payload {
        for header in payload.headers.unwrap_or_default() {
            match header.name.as_str() {
                "Subject" => subject = header.value,
                "From" => from = header.value,
                "Date" => date = header.value,
                _ => {}
            }
        }
    }

    (subject, from, date)
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleListResponse {
    messages: Option<Vec<GoogleMessageRef>>,
}

#[derive(Debug, Deserialize)]
struct GoogleMessageRef {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GoogleMessageResponse {
    payload: Option<GooglePayload>,
    snippet: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GooglePayload {
    headers: Option<Vec<GoogleHeader>>,
}

#[derive(Debug, Deserialize)]
struct GoogleHeader {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorEnvelope {
    error: Option<GoogleErrorResponse>,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorResponse {
    message: Option<String>,
    errors: Option<Vec<GoogleErrorItem>>,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorItem {
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphMeResponse {
    mail: Option<String>,
    #[serde(rename = "userPrincipalName")]
    user_principal_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphInboxResponse {
    value: Vec<GraphMessage>,
}

#[derive(Debug, Deserialize)]
struct GraphMessage {
    subject: Option<String>,
    from: Option<GraphFrom>,
    #[serde(rename = "receivedDateTime")]
    received_date_time: Option<String>,
    #[serde(rename = "bodyPreview")]
    body_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphFrom {
    #[serde(rename = "emailAddress")]
    email_address: Option<GraphEmailAddress>,
}

#[derive(Debug, Deserialize)]
struct GraphEmailAddress {
    address: Option<String>,
}
