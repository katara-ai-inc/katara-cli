use std::env;
use std::error::Error;

use connectrpc::client::{ClientConfig, HttpClient};
use katara_ai_katara_anthropics_buffa::katara::hello::v1alpha1::CreateHelloRequest;
use katara_ai_katara_anthropics_connect_rust::katara::hello::v1alpha1::HelloServiceClient;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[test]
fn cli_auth_help_exposes_only_login() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["auth", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("login"));
    assert!(stdout.contains("list"));
    assert!(!stdout.contains("oauth"));
    assert!(!stdout.contains("passwordless"));
    assert!(!stdout.contains("refresh"));
    Ok(())
}

#[test]
fn cli_auth_list_help_describes_safe_listing() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["auth", "list", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("without revealing secrets"));
    Ok(())
}

#[test]
fn cli_login_help_describes_json_storage() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["auth", "login", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Store a Katara API key locally"));
    Ok(())
}

#[test]
fn cli_login_help_defaults_to_production_api_and_hides_env_value() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["auth", "login", "--help"])
        .env("KATARA_API_KEY", "secret-value-that-must-not-print")
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("https://api.katara.ai"));
    assert!(stdout.contains("KATARA_API_KEY"));
    assert!(!stdout.contains("secret-value-that-must-not-print"));
    Ok(())
}

#[test]
fn cli_document_help_exposes_read_write_commands() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["document", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("get"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("create"));
    assert!(stdout.contains("update"));
    Ok(())
}

#[test]
fn cli_document_create_help_defaults_to_production_api() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["document", "create", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("--org-id"));
    assert!(stdout.contains("--content"));
    assert!(stdout.contains("--content-format"));
    assert!(stdout.contains("https://api.katara.ai"));
    Ok(())
}

#[test]
fn cli_agent_help_exposes_read_commands() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["agent", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("get"));
    assert!(stdout.contains("list"));
    assert!(!stdout.contains("create"));
    assert!(!stdout.contains("update"));
    Ok(())
}

#[test]
fn cli_agent_list_help_exposes_filters() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["agent", "list", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("--org-id"));
    assert!(stdout.contains("--id"));
    assert!(stdout.contains("--agent-type"));
    assert!(stdout.contains("--state"));
    assert!(stdout.contains("https://api.katara.ai"));
    Ok(())
}

#[test]
#[ignore = "requires KATARA_E2E_API_KEY and a reachable Katara API"]
fn api_key_can_call_hello_service() -> TestResult {
    let api_key = match env::var("KATARA_E2E_API_KEY") {
        Ok(api_key) => api_key,
        Err(_) => {
            eprintln!("skipping e2e: set KATARA_E2E_API_KEY");
            return Ok(());
        }
    };
    let api_url =
        env::var("KATARA_E2E_API_URL").unwrap_or_else(|_| "https://api.katara.ai".to_string());
    let bearer = format!("Bearer {api_key}");
    let config = ClientConfig::new(api_url.parse()?).default_header("authorization", bearer);
    let client = HelloServiceClient::new(HttpClient::plaintext(), config);
    let response = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            client
                .create_hello(CreateHelloRequest {
                    message: "katara-cli e2e hello".to_string(),
                    ..Default::default()
                })
                .await
        })?
        .into_owned();
    let hello = response
        .hello
        .into_option()
        .ok_or("hello response did not include hello")?;
    assert!(!hello.id.is_empty(), "hello response must include id");
    assert_eq!(hello.message, "katara-cli e2e hello");
    Ok(())
}
