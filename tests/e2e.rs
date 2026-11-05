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
    assert!(stdout.contains("upload"));
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
fn cli_document_upload_help_exposes_file_options() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["document", "upload", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    for option in [
        "--org-id",
        "--agent-id",
        "--file",
        "--content-type",
        "--source-id",
        "--language",
        "--tag-id",
        "--idempotency-key",
        "https://api.katara.ai",
    ] {
        assert!(stdout.contains(option), "missing option: {option}");
    }
    Ok(())
}

#[test]
fn cli_agent_filters_match_current_api_types() -> TestResult {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"))
        .args(["agent", "list", "--help"])
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("brain"));
    assert!(!stdout.contains("taxonomy"));
    assert!(!stdout.contains("classifier"));
    Ok(())
}

struct TestDirectory(std::path::PathBuf);

impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("katara-cli-e2e-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn cli(&self) -> std::process::Command {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_katara-cli"));
        command
            .env("KATARA_AUTH_FILE", self.0.join("auth.json"))
            .env_remove("KATARA_API_KEY")
            .env_remove("KATARA_API_URL");
        command
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_upload_rejects_empty_file_before_authentication() -> TestResult {
    let directory = TestDirectory::new()?;
    let file = directory.0.join("empty.txt");
    std::fs::write(&file, "")?;
    let output = directory
        .cli()
        .args([
            "document",
            "upload",
            "--org-id",
            "org",
            "--agent-id",
            "agent",
            "--file",
        ])
        .arg(&file)
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)?.contains("upload file cannot be empty"));
    assert!(!directory.0.join("auth.json").exists());
    Ok(())
}

#[test]
fn cli_upload_uses_current_wire_contract_and_handles_responses() -> TestResult {
    use buffa::Message;
    use katara_ai_katara_anthropics_buffa::katara::content_uploader::v1alpha1::{
        UploadDocumentRequest, UploadDocumentResponse,
    };
    use std::io::{Read, Write};

    let directory = TestDirectory::new()?;
    let file = directory.0.join("sample.bin");
    let data = b"upload bytes\x00\xff";
    std::fs::write(&file, data)?;
    let login = directory
        .cli()
        .args(["auth", "login"])
        .env("KATARA_API_KEY", "synthetic-upload-key")
        .output()?;
    assert!(login.status.success());
    let id = "11111111-1111-4111-8111-111111111111";

    for outcome in ["success", "denied", "missing-id"] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let api_url = format!("http://{}", listener.local_addr()?);
        listener.set_nonblocking(true)?;
        let server = std::thread::spawn(move || -> TestResult {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                if headers.len() >= 16 * 1024 {
                    return Err("mock request headers too large".into());
                }
                let mut byte = [0];
                stream.read_exact(&mut byte)?;
                headers.push(byte[0]);
            }
            let headers = String::from_utf8(headers)?.to_ascii_lowercase();
            assert!(headers.starts_with("post /katara.content_uploader.v1alpha1.contentuploaderservice/uploaddocument http/1.1\r\n"));
            assert!(headers.contains("authorization: bearer synthetic-upload-key\r\n"));
            assert!(headers.contains("content-type: application/proto\r\n"));
            let size: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .ok_or("missing content-length")?
                .parse()?;
            if size > 4096 {
                return Err("mock request body too large".into());
            }
            let mut body = vec![0; size];
            stream.read_exact(&mut body)?;
            let request = UploadDocumentRequest::decode_from_slice(&body)?;
            assert_eq!(request.org_id, "org");
            assert_eq!(request.agent_id, "agent");
            assert_eq!(request.source_id, "source");
            assert_eq!(request.language.as_deref(), Some("en"));
            assert_eq!(request.tag_ids, ["tag-1", "tag-2"]);
            assert_eq!(request.idempotency_key.as_deref(), Some("retry"));
            let file = request.file.as_option().ok_or("missing file")?;
            assert_eq!(file.filename, "sample.bin");
            assert_eq!(
                file.content_type.as_deref(),
                Some("application/octet-stream")
            );
            assert_eq!(file.data, data);
            let (status, content_type, response) = match outcome {
                "denied" => (
                    403,
                    "application/json",
                    br#"{"code":"permission_denied","message":"access denied"}"#.to_vec(),
                ),
                "missing-id" => (200, "application/proto", Vec::new()),
                _ => (
                    200,
                    "application/proto",
                    UploadDocumentResponse {
                        document_id: id.to_string(),
                        ..Default::default()
                    }
                    .encode_to_vec(),
                ),
            };
            write!(
                stream,
                "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                if status == 200 { "OK" } else { "Forbidden" },
                response.len()
            )?;
            stream.write_all(&response)?;
            Ok(())
        });
        let output = directory
            .cli()
            .args([
                "document",
                "upload",
                "--api-url",
                &api_url,
                "--org-id",
                "org",
                "--agent-id",
                "agent",
                "--source-id",
                "source",
                "--language",
                "en",
                "--tag-id",
                "tag-1",
                "--tag-id",
                "tag-2",
                "--idempotency-key",
                "retry",
                "--content-type",
                "application/octet-stream",
                "--file",
            ])
            .arg(&file)
            .output()?;
        server.join().map_err(|_| "mock server panicked")??;
        let stdout = String::from_utf8(output.stdout)?;
        let stderr = String::from_utf8(output.stderr)?;
        assert!(!stdout.contains("synthetic-upload-key"));
        assert!(!stderr.contains("synthetic-upload-key"));
        match outcome {
            "success" => {
                assert!(output.status.success());
                assert_eq!(stdout, format!("{id}\n"));
            }
            "denied" => {
                assert!(!output.status.success());
                assert!(stderr.contains("access denied"));
            }
            _ => {
                assert!(!output.status.success());
                assert!(stderr.contains("document ID not found"));
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "writes a document; requires KATARA_E2E_API_KEY, KATARA_E2E_API_URL, KATARA_E2E_ORG_ID and KATARA_E2E_AGENT_ID"]
fn cli_file_upload_round_trips() -> TestResult {
    let api_key = env::var("KATARA_E2E_API_KEY")?;
    let api_url = env::var("KATARA_E2E_API_URL")?;
    let org_id = env::var("KATARA_E2E_ORG_ID")?;
    let agent_id = env::var("KATARA_E2E_AGENT_ID")?;
    let directory = TestDirectory::new()?;
    let source = directory.0.file_name().unwrap().to_str().unwrap();
    let content = format!("Katara CLI staging file upload verification: {source}");
    let file = directory.0.join("katara-cli-upload-smoke.txt");
    std::fs::write(&file, &content)?;
    let login = directory
        .cli()
        .args(["auth", "login", "--api-url", &api_url])
        .env("KATARA_API_KEY", api_key)
        .output()?;
    assert!(login.status.success(), "CLI login failed");
    let upload = directory
        .cli()
        .args([
            "document",
            "upload",
            "--api-url",
            &api_url,
            "--org-id",
            &org_id,
            "--agent-id",
            &agent_id,
            "--source-id",
            source,
            "--content-type",
            "text/plain",
            "--idempotency-key",
            source,
            "--file",
        ])
        .arg(&file)
        .output()?;
    assert!(
        upload.status.success(),
        "upload failed: {}",
        String::from_utf8_lossy(&upload.stderr)
    );
    let id = String::from_utf8(upload.stdout)?.trim().to_string();
    assert!(!id.is_empty(), "upload must return a document ID");
    let readback = directory
        .cli()
        .args([
            "document",
            "get",
            "--api-url",
            &api_url,
            "--org-id",
            &org_id,
            "--id",
            &id,
        ])
        .output()?;
    assert!(
        readback.status.success(),
        "readback failed: {}",
        String::from_utf8_lossy(&readback.stderr)
    );
    assert!(
        content.chars().count() <= 120,
        "fixture must fit the CLI preview"
    );
    let stdout = String::from_utf8(readback.stdout)?;
    assert!(stdout.starts_with(&format!("{id}\t{org_id}\t")));
    assert!(stdout.contains(&content));
    eprintln!(
        "verified uploaded document {id} in organization {org_id} (left in API for inspection)"
    );
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
    let uri: http::Uri = api_url.parse()?;
    let config = ClientConfig::new(uri.clone()).with_default_header("authorization", bearer);
    let transport = match uri.scheme_str() {
        Some("https") => {
            let roots: connectrpc::rustls::RootCertStore =
                webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
            let tls = connectrpc::rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            HttpClient::with_tls(std::sync::Arc::new(tls))
        }
        Some("http") if matches!(uri.host(), Some("localhost" | "127.0.0.1" | "[::1]")) => {
            HttpClient::plaintext()
        }
        _ => return Err("live test requires HTTPS or loopback HTTP".into()),
    };
    let client = HelloServiceClient::new(transport, config);
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
