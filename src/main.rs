use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use connectrpc::client::{ClientConfig, HttpClient};
use connectrpc::rustls;
use katara_ai_katara_anthropics_buffa::katara::agent::v1alpha1::{
    Agent, AgentState, AgentType, GetAgentRequest, ListAgentsRequest,
};
use katara_ai_katara_anthropics_buffa::katara::content_uploader::v1alpha1::{
    File as UploadFile, UploadDocumentRequest,
};
use katara_ai_katara_anthropics_buffa::katara::corpus::v1alpha1::{
    CreateDocumentRequest, Document, GetDocumentRequest, ListDocumentsRequest,
    UpdateDocumentRequest, document,
};
use katara_ai_katara_anthropics_connect_rust::katara::agent::v1alpha1::AgentServiceClient;
use katara_ai_katara_anthropics_connect_rust::katara::content_uploader::v1alpha1::ContentUploaderServiceClient;
use katara_ai_katara_anthropics_connect_rust::katara::corpus::v1alpha1::CorpusServiceClient;
use serde::{Deserialize, Serialize};

const DEFAULT_API_URL: &str = "https://api.katara.ai";
const AUTH_FILE_ENV: &str = "KATARA_AUTH_FILE";
const AUTH_KEY_NAME: &str = "default";
const MAX_UPLOAD_BYTES: u64 = 20 * 1024 * 1024;

type CliResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Parser, Debug)]
#[command(name = "katara-cli", about = "Command-line client for Katara API")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Authenticate with a Katara API key.
    Auth(AuthArgs),
    /// Read and write corpus documents.
    Document(DocumentArgs),
    /// Read agents.
    Agent(AgentArgs),
    /// List generated Katara API service clients exposed by this CLI.
    Services,
}

#[derive(Args, Debug)]
struct AuthArgs {
    #[command(subcommand)]
    command: AuthCommand,
}

#[derive(Subcommand, Debug)]
enum AuthCommand {
    /// Store a Katara API key locally.
    Login(LoginArgs),
    /// List locally stored authentication credentials without revealing secrets.
    List,
}

#[derive(Args, Debug)]
struct LoginArgs {
    /// Katara API endpoint used by future commands.
    #[arg(long, env = "KATARA_API_URL", default_value = DEFAULT_API_URL)]
    api_url: String,
    /// API key to store. If omitted, the CLI prompts without echoing input.
    #[arg(long, env = "KATARA_API_KEY", hide_env_values = true)]
    api_key: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct AuthFile {
    #[serde(rename = "apiKeys", default)]
    api_keys: BTreeMap<String, String>,
}

#[derive(Args, Debug)]
struct DocumentArgs {
    #[command(subcommand)]
    command: DocumentCommand,
}

#[derive(Subcommand, Debug)]
enum DocumentCommand {
    /// Fetch a document by organization and document ID.
    Get(DocumentGetArgs),
    /// List documents in an organization.
    List(DocumentListArgs),
    /// Create a document.
    Create(DocumentCreateArgs),
    /// Update a document.
    Update(DocumentUpdateArgs),
    /// Upload a local file through a Brain or Content Uploader agent.
    Upload(DocumentUploadArgs),
}

#[derive(Args, Debug)]
struct ApiArgs {
    /// Katara API endpoint.
    #[arg(long, env = "KATARA_API_URL", default_value = DEFAULT_API_URL)]
    api_url: String,
}

#[derive(Args, Debug)]
struct DocumentGetArgs {
    #[command(flatten)]
    api: ApiArgs,
    /// Organization ID that owns the document.
    #[arg(long)]
    org_id: String,
    /// Document ID to fetch.
    #[arg(long)]
    id: String,
    /// Include document references in the response.
    #[arg(long)]
    include_references: bool,
}

#[derive(Args, Debug)]
struct DocumentListArgs {
    #[command(flatten)]
    api: ApiArgs,
    /// Organization ID to list documents from.
    #[arg(long)]
    org_id: String,
    /// Filter by document ID. May be repeated.
    #[arg(long)]
    id: Vec<String>,
    /// Maximum page size.
    #[arg(long)]
    page_size: Option<i32>,
    /// Page token returned by a previous list call.
    #[arg(long)]
    page_token: Option<String>,
    /// Include document references in the response.
    #[arg(long)]
    include_references: bool,
}

#[derive(Args, Debug)]
struct DocumentCreateArgs {
    #[command(flatten)]
    api: ApiArgs,
    /// Organization ID that will own the document.
    #[arg(long)]
    org_id: String,
    /// Parent document ID.
    #[arg(long)]
    parent_id: Option<String>,
    /// Document content.
    #[arg(long)]
    content: String,
    /// Content field to populate.
    #[arg(long, value_enum, default_value_t = ContentFormat::Text)]
    content_format: ContentFormat,
    /// BCP-47 language tag.
    #[arg(long)]
    language: Option<String>,
    /// Agent ID associated with the document.
    #[arg(long)]
    agent_id: Option<String>,
    /// External source ID for this document.
    #[arg(long)]
    source_id: Option<String>,
    /// Update an existing document with the same source ID if one exists.
    #[arg(long)]
    update_existing: bool,
}

#[derive(Args, Debug)]
struct DocumentUpdateArgs {
    #[command(flatten)]
    api: ApiArgs,
    /// Organization ID that owns the document.
    #[arg(long)]
    org_id: String,
    /// Document ID to update.
    #[arg(long)]
    id: String,
    /// Parent document ID.
    #[arg(long)]
    parent_id: Option<String>,
    /// Document content.
    #[arg(long)]
    content: String,
    /// Content field to populate.
    #[arg(long, value_enum, default_value_t = ContentFormat::Text)]
    content_format: ContentFormat,
    /// BCP-47 language tag.
    #[arg(long)]
    language: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ContentFormat {
    Text,
    Markdown,
    Json,
}

#[derive(Args, Debug)]
struct DocumentUploadArgs {
    #[command(flatten)]
    api: ApiArgs,
    /// Organization ID that will own the document.
    #[arg(long)]
    org_id: String,
    /// Brain or Content Uploader agent ID in this organization.
    #[arg(long)]
    agent_id: String,
    /// Local file to upload (nonempty, at most 20 MiB).
    #[arg(long)]
    file: PathBuf,
    /// MIME type override; otherwise detected by the API.
    #[arg(long)]
    content_type: Option<String>,
    /// External source ID; defaults to the filename on the API.
    #[arg(long)]
    source_id: Option<String>,
    /// BCP-47 language tag.
    #[arg(long)]
    language: Option<String>,
    /// Tag ID to associate with the document. May be repeated.
    #[arg(long)]
    tag_id: Vec<String>,
    /// Stable key for retrying the same Brain upload intent.
    #[arg(long)]
    idempotency_key: Option<String>,
}

#[derive(Args, Debug)]
struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Subcommand, Debug)]
enum AgentCommand {
    /// Fetch an agent by ID.
    Get(AgentGetArgs),
    /// List agents.
    List(AgentListArgs),
}

#[derive(Args, Debug)]
struct AgentGetArgs {
    #[command(flatten)]
    api: ApiArgs,
    /// Agent ID to fetch.
    #[arg(long)]
    id: String,
}

#[derive(Args, Debug)]
struct AgentListArgs {
    #[command(flatten)]
    api: ApiArgs,
    /// Organization ID filter.
    #[arg(long)]
    org_id: Option<String>,
    /// Agent ID filter. May be repeated.
    #[arg(long)]
    id: Vec<String>,
    /// Agent type filter.
    #[arg(long, value_enum)]
    agent_type: Option<AgentTypeArg>,
    /// Agent state filter.
    #[arg(long, value_enum)]
    state: Option<AgentStateArg>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AgentStateArg {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AgentTypeArg {
    Website,
    Qna,
    Discord,
    Github,
    Synchronizer,
    Telegram,
    Playground,
    ContentUploader,
    WebWidget,
    Slack,
    Notion,
    Brain,
    All,
}

fn main() -> CliResult {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Auth(args)) => run_auth(args.command),
        Some(Command::Document(args)) => run_document(args.command),
        Some(Command::Agent(args)) => run_agent(args.command),
        Some(Command::Services) => {
            print_services();
            Ok(())
        }
        None => {
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
    }
}

fn run_auth(command: AuthCommand) -> CliResult {
    match command {
        AuthCommand::Login(args) => login(args),
        AuthCommand::List => list_auth(),
    }
}

fn run_document(command: DocumentCommand) -> CliResult {
    match command {
        DocumentCommand::Get(args) => block_on(async move {
            let client = corpus_client(&args.api.api_url)?;
            let response = client
                .get_document(GetDocumentRequest {
                    id: args.id,
                    org_id: args.org_id,
                    include_references: args.include_references,
                    ..Default::default()
                })
                .await?
                .into_owned();
            let document = response
                .document
                .into_option()
                .ok_or("document not found in response")?;
            print_document(&document);
            Ok(())
        }),
        DocumentCommand::List(args) => block_on(async move {
            let client = corpus_client(&args.api.api_url)?;
            let response = client
                .list_documents(ListDocumentsRequest {
                    org_id: args.org_id,
                    ids: args.id,
                    page_size: args.page_size,
                    page_token: args.page_token,
                    include_references: args.include_references,
                    ..Default::default()
                })
                .await?
                .into_owned();
            for document in response.documents {
                print_document(&document);
            }
            if !response.next_page_token.is_empty() {
                println!("next_page_token\t{}", response.next_page_token);
            }
            Ok(())
        }),
        DocumentCommand::Create(args) => block_on(async move {
            let client = corpus_client(&args.api.api_url)?;
            let response = client
                .create_document(CreateDocumentRequest {
                    org_id: args.org_id,
                    parent_id: args.parent_id,
                    r#type: document::Type::TYPE_TEXT.into(),
                    language: args.language,
                    content: document_content(args.content_format, args.content).into(),
                    agent_id: args.agent_id.unwrap_or_default(),
                    source_id: args.source_id.unwrap_or_default(),
                    update_existing: args.update_existing,
                    ..Default::default()
                })
                .await?
                .into_owned();
            let document = response
                .document
                .into_option()
                .ok_or("document not found in response")?;
            print_document(&document);
            Ok(())
        }),
        DocumentCommand::Update(args) => block_on(async move {
            let client = corpus_client(&args.api.api_url)?;
            let response = client
                .update_document(UpdateDocumentRequest {
                    id: args.id,
                    org_id: args.org_id,
                    parent_id: args.parent_id,
                    language: args.language,
                    content: document_content(args.content_format, args.content).into(),
                    ..Default::default()
                })
                .await?
                .into_owned();
            let document = response
                .document
                .into_option()
                .ok_or("document not found in response")?;
            print_document(&document);
            Ok(())
        }),
        DocumentCommand::Upload(args) => {
            let request = upload_request(&args)?;
            block_on(async move {
                let client = content_uploader_client(&args.api.api_url)?;
                let response = client.upload_document(request).await?.into_owned();
                if response.document_id.is_empty() {
                    return Err("document ID not found in upload response".into());
                }
                println!("{}", response.document_id);
                Ok(())
            })
        }
    }
}

fn upload_request(args: &DocumentUploadArgs) -> CliResult<UploadDocumentRequest> {
    let filename = args
        .file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("upload file must have a UTF-8 filename")?;
    if filename.is_empty() || filename.len() > 255 || filename.contains(['/', '\\']) {
        return Err("upload filename must be at most 255 bytes with no path separators".into());
    }
    if args
        .content_type
        .as_ref()
        .is_some_and(|value| value.len() > 255)
    {
        return Err("content type must be at most 255 bytes".into());
    }
    if !fs::metadata(&args.file)?.is_file() {
        return Err("upload path must be a regular file".into());
    }
    let file = fs::File::open(&args.file)?;
    if !file.metadata()?.is_file() {
        return Err("upload path must be a regular file".into());
    }
    let mut data = Vec::new();
    file.take(MAX_UPLOAD_BYTES + 1).read_to_end(&mut data)?;
    if data.is_empty() {
        return Err("upload file cannot be empty".into());
    }
    if data.len() as u64 > MAX_UPLOAD_BYTES {
        return Err("upload file must be at most 20 MiB".into());
    }
    Ok(UploadDocumentRequest {
        org_id: args.org_id.clone(),
        agent_id: args.agent_id.clone(),
        source_id: args.source_id.clone().unwrap_or_default(),
        language: args.language.clone(),
        tag_ids: args.tag_id.clone(),
        idempotency_key: args.idempotency_key.clone(),
        file: UploadFile {
            filename: filename.to_string(),
            content_type: args.content_type.clone(),
            data,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    })
}

fn run_agent(command: AgentCommand) -> CliResult {
    match command {
        AgentCommand::Get(args) => block_on(async move {
            let client = agent_client(&args.api.api_url)?;
            let response = client
                .get_agent(GetAgentRequest {
                    id: args.id,
                    ..Default::default()
                })
                .await?
                .into_owned();
            let agent = response
                .agent
                .into_option()
                .ok_or("agent not found in response")?;
            print_agent(&agent);
            Ok(())
        }),
        AgentCommand::List(args) => block_on(async move {
            let client = agent_client(&args.api.api_url)?;
            let response = client
                .list_agents(ListAgentsRequest {
                    org_id: args.org_id,
                    ids: args.id,
                    r#type: args.agent_type.map(|agent_type| agent_type.to_sdk().into()),
                    state: args.state.map(|state| state.to_sdk().into()),
                    ..Default::default()
                })
                .await?
                .into_owned();
            for agent in response.agents {
                print_agent(&agent);
            }
            Ok(())
        }),
    }
}

fn block_on<F>(future: F) -> CliResult
where
    F: std::future::Future<Output = CliResult>,
{
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(future)
}

fn login(args: LoginArgs) -> CliResult {
    let api_key = match args.api_key {
        Some(api_key) => api_key,
        None => rpassword::prompt_password("Katara API key: ")?,
    };
    let api_key = api_key.trim().to_string();
    if api_key.is_empty() {
        return Err("API key cannot be empty".into());
    }

    store_api_key(&api_key)?;
    println!("Katara API key stored for {}.", args.api_url);
    println!(
        "Future API commands will read it from {}.",
        auth_file_path()?.display()
    );
    Ok(())
}

fn list_auth() -> CliResult {
    let auth_path = auth_file_path()?;
    let auth_file = read_auth_file_or_default(&auth_path)?;
    let has_keys = !auth_file.api_keys.is_empty();
    for (name, api_key) in auth_file.api_keys {
        println!(
            "{}\t{}\tconfigured\t{}",
            auth_path.display(),
            name,
            mask_api_key(&api_key)
        );
    }
    if !has_keys {
        println!("{}\t{}\tunavailable", auth_path.display(), AUTH_KEY_NAME);
    }
    Ok(())
}

fn store_api_key(api_key: &str) -> CliResult {
    let path = auth_file_path()?;
    let mut auth_file = read_auth_file_or_default(&path)?;
    auth_file
        .api_keys
        .insert(AUTH_KEY_NAME.to_string(), api_key.to_string());
    write_auth_file(&path, &auth_file)?;
    Ok(())
}

fn load_api_key() -> CliResult<String> {
    read_auth_file(&auth_file_path()?)?
        .api_keys
        .get(AUTH_KEY_NAME)
        .cloned()
        .ok_or_else(|| "No Katara API key stored. Run `katara-cli auth login`.".into())
}

fn read_auth_file(path: &Path) -> CliResult<AuthFile> {
    let contents = fs::read_to_string(path)?;
    if contents.trim().is_empty() {
        return Ok(AuthFile::default());
    }
    Ok(serde_json::from_str(&contents)?)
}

fn read_auth_file_or_default(path: &Path) -> CliResult<AuthFile> {
    match read_auth_file(path) {
        Ok(auth_file) => Ok(auth_file),
        Err(error) => {
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            {
                return Ok(AuthFile::default());
            }
            Err(error)
        }
    }
}

fn write_auth_file(path: &Path, auth_file: &AuthFile) -> CliResult {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(auth_file)?;
    write_secret_file(path, &contents)
}

#[cfg(unix)]
fn write_secret_file(path: &Path, contents: &str) -> CliResult {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::fs::PermissionsExt;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("auth file path must include a valid file name")?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let temp_path = parent.join(format!(".{file_name}.{}.{nonce}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .create_new(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temp_path)?;
    let write_result = (|| -> CliResult {
        file.write_all(contents.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() {
            return Err("auth file path is not a regular file".into());
        }
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err("auth file permissions must be 0600".into());
        }
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result?;
    Ok(())
}

#[cfg(not(unix))]
fn write_secret_file(path: &Path, contents: &str) -> CliResult {
    fs::write(path, format!("{contents}\n"))?;
    Ok(())
}

fn auth_file_path() -> CliResult<PathBuf> {
    if let Ok(path) = std::env::var(AUTH_FILE_ENV) {
        return Ok(PathBuf::from(path));
    }

    #[cfg(windows)]
    {
        let appdata = std::env::var_os("APPDATA").ok_or("APPDATA is not set")?;
        return Ok(PathBuf::from(appdata).join("katara-cli").join("auth.json"));
    }

    #[cfg(not(windows))]
    {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("katara-cli")
            .join("auth.json"))
    }
}

fn mask_api_key(api_key: &str) -> String {
    let suffix_chars: Vec<char> = api_key.chars().rev().take(4).collect();
    if suffix_chars.len() < 4 {
        "****".to_string()
    } else {
        let suffix: String = suffix_chars.into_iter().rev().collect();
        format!("****{suffix}")
    }
}

fn authenticated_config(api_url: &str) -> CliResult<ClientConfig> {
    let uri: http::Uri = api_url.parse()?;
    validate_api_uri(&uri)?;
    let mut bearer = http::HeaderValue::from_str(&format!("Bearer {}", load_api_key()?))?;
    bearer.set_sensitive(true);
    Ok(ClientConfig::new(uri).with_default_header("authorization", bearer))
}

fn corpus_client(api_url: &str) -> CliResult<CorpusServiceClient<HttpClient>> {
    Ok(CorpusServiceClient::new(
        http_client(api_url)?,
        authenticated_config(api_url)?,
    ))
}

fn agent_client(api_url: &str) -> CliResult<AgentServiceClient<HttpClient>> {
    Ok(AgentServiceClient::new(
        http_client(api_url)?,
        authenticated_config(api_url)?,
    ))
}

fn content_uploader_client(api_url: &str) -> CliResult<ContentUploaderServiceClient<HttpClient>> {
    Ok(ContentUploaderServiceClient::new(
        http_client(api_url)?,
        authenticated_config(api_url)?,
    ))
}

fn validate_api_uri(uri: &http::Uri) -> CliResult {
    match uri.scheme_str() {
        Some("https") => Ok(()),
        Some("http") if is_loopback_host(uri.host().unwrap_or_default()) => Ok(()),
        Some("http") => {
            Err("API URL must use https unless the host is localhost or loopback".into())
        }
        _ => Err("API URL must use http or https".into()),
    }
}

fn http_client(api_url: &str) -> CliResult<HttpClient> {
    let uri: http::Uri = api_url.parse()?;
    validate_api_uri(&uri)?;
    if uri.scheme_str() == Some("http") {
        Ok(HttpClient::plaintext())
    } else {
        Ok(tls_http_client())
    }
}

fn is_loopback_host(host: &str) -> bool {
    host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]"
}

fn tls_http_client() -> HttpClient {
    let roots: rustls::RootCertStore = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    HttpClient::with_tls(Arc::new(tls_config))
}

fn document_content(format: ContentFormat, value: String) -> document::Content {
    let mut content = document::Content::default();
    match format {
        ContentFormat::Text => content.text = value,
        ContentFormat::Markdown => content.markdown = value,
        ContentFormat::Json => content.json = value,
    }
    content
}

fn print_document(document: &Document) {
    let content = document.content.as_option();
    let preview = content
        .map(|content| {
            if !content.text.is_empty() {
                content.text.as_str()
            } else if !content.markdown.is_empty() {
                content.markdown.as_str()
            } else {
                content.json.as_str()
            }
        })
        .unwrap_or("");
    println!(
        "{}\t{}\t{}",
        document.id,
        document.org_id,
        single_line_preview(preview)
    );
}

fn print_agent(agent: &Agent) {
    println!(
        "{}\t{}\t{}\t{:?}\t{:?}",
        agent.id, agent.org_id, agent.name, agent.r#type, agent.state
    );
}

fn single_line_preview(value: &str) -> String {
    let value = value.replace(['\r', '\n'], " ");
    let mut chars = value.chars();
    let preview: String = chars.by_ref().take(120).collect();
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

impl AgentStateArg {
    fn to_sdk(self) -> AgentState {
        match self {
            AgentStateArg::Enabled => AgentState::AGENT_STATE_ENABLED,
            AgentStateArg::Disabled => AgentState::AGENT_STATE_DISABLED,
        }
    }
}

impl AgentTypeArg {
    fn to_sdk(self) -> AgentType {
        match self {
            AgentTypeArg::Website => AgentType::AGENT_TYPE_WEBSITE,
            AgentTypeArg::Qna => AgentType::AGENT_TYPE_QNA,
            AgentTypeArg::Discord => AgentType::AGENT_TYPE_DISCORD,
            AgentTypeArg::Github => AgentType::AGENT_TYPE_GITHUB,
            AgentTypeArg::Synchronizer => AgentType::AGENT_TYPE_SYNCHRONIZER,
            AgentTypeArg::Telegram => AgentType::AGENT_TYPE_TELEGRAM,
            AgentTypeArg::Playground => AgentType::AGENT_TYPE_PLAYGROUND,
            AgentTypeArg::ContentUploader => AgentType::AGENT_TYPE_CONTENT_UPLOADER,
            AgentTypeArg::WebWidget => AgentType::AGENT_TYPE_WEB_WIDGET,
            AgentTypeArg::Slack => AgentType::AGENT_TYPE_SLACK,
            AgentTypeArg::Notion => AgentType::AGENT_TYPE_NOTION,
            AgentTypeArg::Brain => AgentType::AGENT_TYPE_BRAIN,
            AgentTypeArg::All => AgentType::AGENT_TYPE_ALL,
        }
    }
}

fn print_services() {
    println!("auth    AuthServiceClient   katara::auth::v1alpha1");
    println!("agent   AgentServiceClient  katara::agent::v1alpha1");
    println!("corpus  CorpusServiceClient katara::corpus::v1alpha1");
    println!("hello   HelloServiceClient  katara::hello::v1alpha1");
    println!("upload  ContentUploaderServiceClient katara::content_uploader::v1alpha1");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_defaults_to_production_api() {
        let args = LoginArgs {
            api_url: DEFAULT_API_URL.to_string(),
            api_key: Some("katara_test_key".to_string()),
        };
        assert_eq!(args.api_url, "https://api.katara.ai");
    }

    #[test]
    fn keyring_identity_is_stable() {
        assert_eq!(AUTH_FILE_ENV, "KATARA_AUTH_FILE");
        assert_eq!(AUTH_KEY_NAME, "default");
    }

    #[test]
    fn api_uri_validation_allows_https_and_local_http() {
        for uri in [
            "https://api.katara.ai",
            "http://localhost:8080",
            "http://127.0.0.1:8080",
            "http://[::1]:8080",
        ] {
            let uri: http::Uri = uri.parse().unwrap();
            validate_api_uri(&uri).unwrap();
        }
    }

    #[test]
    fn api_uri_validation_rejects_remote_http() {
        let uri: http::Uri = "http://api.katara.ai".parse().unwrap();
        let error = validate_api_uri(&uri).unwrap_err();
        assert_eq!(
            error.to_string(),
            "API URL must use https unless the host is localhost or loopback"
        );
    }

    #[test]
    fn mask_api_key_only_exposes_last_four_characters() {
        assert_eq!(mask_api_key("katara_secret_1234"), "****1234");
        assert_eq!(mask_api_key("abc"), "****");
    }

    fn upload_args(path: PathBuf) -> DocumentUploadArgs {
        DocumentUploadArgs {
            api: ApiArgs {
                api_url: DEFAULT_API_URL.to_string(),
            },
            org_id: "org".to_string(),
            agent_id: "agent".to_string(),
            file: path,
            content_type: None,
            source_id: None,
            language: None,
            tag_id: Vec::new(),
            idempotency_key: None,
        }
    }

    #[test]
    fn upload_request_preserves_file_bytes_and_metadata() {
        let path = unique_temp_auth_path().with_extension("md");
        let bytes = b"# Upload\nUTF-8: \xc3\xa9\n\x00\xff";
        fs::write(&path, bytes).unwrap();
        let mut args = upload_args(path.clone());
        args.content_type = Some("text/markdown".to_string());
        args.source_id = Some("source".to_string());
        args.language = Some("en".to_string());
        args.tag_id = vec!["tag-1".to_string(), "tag-2".to_string()];
        args.idempotency_key = Some("retry-key".to_string());
        let request = upload_request(&args).unwrap();
        assert_eq!(request.org_id, "org");
        assert_eq!(request.agent_id, "agent");
        assert_eq!(request.source_id, "source");
        assert_eq!(request.language.as_deref(), Some("en"));
        assert_eq!(request.tag_ids, args.tag_id);
        assert_eq!(request.idempotency_key.as_deref(), Some("retry-key"));
        let file = request.file.as_option().unwrap();
        assert_eq!(file.filename, path.file_name().unwrap().to_str().unwrap());
        assert_eq!(file.content_type.as_deref(), Some("text/markdown"));
        assert_eq!(file.data, bytes);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn upload_request_keeps_optional_metadata_absent() {
        let path = unique_temp_auth_path();
        fs::write(&path, "test").unwrap();
        let request = upload_request(&upload_args(path.clone())).unwrap();
        assert!(request.source_id.is_empty());
        assert!(request.language.is_none());
        assert!(request.idempotency_key.is_none());
        assert!(request.file.as_option().unwrap().content_type.is_none());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn upload_request_rejects_missing_empty_and_oversized_files() {
        let path = unique_temp_auth_path();
        let args = upload_args(path.clone());
        assert!(upload_request(&args).is_err());
        fs::write(&path, "").unwrap();
        assert_eq!(
            upload_request(&args).unwrap_err().to_string(),
            "upload file cannot be empty"
        );
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(MAX_UPLOAD_BYTES).unwrap();
        assert_eq!(
            upload_request(&args)
                .unwrap()
                .file
                .as_option()
                .unwrap()
                .data
                .len() as u64,
            MAX_UPLOAD_BYTES
        );
        file.set_len(MAX_UPLOAD_BYTES + 1).unwrap();
        assert_eq!(
            upload_request(&args).unwrap_err().to_string(),
            "upload file must be at most 20 MiB"
        );
        drop(file);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn upload_request_rejects_directories_and_invalid_metadata() {
        let path = unique_temp_auth_path();
        fs::create_dir(&path).unwrap();
        assert_eq!(
            upload_request(&upload_args(path.clone()))
                .unwrap_err()
                .to_string(),
            "upload path must be a regular file"
        );
        fs::remove_dir(&path).unwrap();
        let mut args = upload_args(path);
        args.content_type = Some("x".repeat(256));
        assert_eq!(
            upload_request(&args).unwrap_err().to_string(),
            "content type must be at most 255 bytes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn upload_request_rejects_backslashes_in_unix_filenames() {
        // On Windows a backslash separates directories, rather than being part of the basename.
        let args = upload_args(PathBuf::from("bad\\filename.txt"));
        assert!(
            upload_request(&args)
                .unwrap_err()
                .to_string()
                .contains("no path separators")
        );
    }

    #[test]
    fn auth_file_round_trips_api_key_json() {
        let path = std::env::temp_dir().join(format!(
            "katara-cli-auth-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let mut auth_file = AuthFile::default();
        auth_file
            .api_keys
            .insert(AUTH_KEY_NAME.to_string(), "katara_test_key".to_string());
        write_auth_file(&path, &auth_file).unwrap();

        let stored = read_auth_file(&path).unwrap();
        assert_eq!(
            stored.api_keys.get(AUTH_KEY_NAME).map(String::as_str),
            Some("katara_test_key")
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn malformed_auth_file_returns_error() {
        let path = unique_temp_auth_path();
        fs::write(&path, "not-json").unwrap();

        let error = read_auth_file_or_default(&path).unwrap_err();
        assert!(error.to_string().contains("expected ident"));

        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn auth_file_write_tightens_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = unique_temp_auth_path();
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let mut auth_file = AuthFile::default();
        auth_file
            .api_keys
            .insert(AUTH_KEY_NAME.to_string(), "katara_test_key".to_string());
        write_auth_file(&path, &auth_file).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        fs::remove_file(path).unwrap();
    }

    fn unique_temp_auth_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "katara-cli-auth-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
