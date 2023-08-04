use crate::{auth, copilot, parse};
use crate::copilot::{CopilotCompletionRequest, CopilotCompletionParams};
use dashmap::DashMap;
use ropey::Rope;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tower_lsp::jsonrpc::{Error, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use chrono::Utc;
use reqwest::RequestBuilder;
use tokio::sync::Mutex;

type CompletionCyclingResponse = Result<Option<CompletionResponse>>;

#[derive(Debug)]
pub struct State {
  pub document_map: DashMap<String, Rope>,
  pub language_map: DashMap<String, String>,
  pub http_client: reqwest::Client,
}

impl State {
  pub async fn new() -> Self {
    env_logger::init();
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let auth_grant = auth::get_copilot_token().await.unwrap();
    let auth_token = auth_grant.token.to_string();

    let auth_string = format!("Bearer {}", &auth_token.as_str());
    let mut auth_value = HeaderValue::from_str(&auth_string).unwrap();
    auth_value.set_sensitive(true);
    let machine_id = auth::get_machine_id();

    let mut header_map = HeaderMap::new();
    header_map.insert("Authorization", auth_value);
    header_map.insert("Openai-Organization", HeaderValue::from_static("github-copilot"));
    header_map.insert("VScode-MachineId", HeaderValue::from_str(&machine_id).unwrap());
    header_map.insert("Editor-Version", HeaderValue::from_static("JetBrains-IC/231.9011.34"));
    header_map.insert("Editor-Plugin-Version", HeaderValue::from_static("copilot-intellij/1.2.8.2631"));
    header_map.insert("OpenAI-Intent", HeaderValue::from_static("copilot-ghost"));
    header_map.insert("Connection", HeaderValue::from_static("Keep-Alive"));
    let client_builder = reqwest::Client::builder()
      .default_headers(header_map);
    let http_client = client_builder.build().unwrap();

    Self {
      document_map: DashMap::new(),
      language_map: DashMap::new(),
      http_client
    }
  }
}
pub fn build_request(
  http_client: reqwest::Client,
  language: String,
  prompt: String,
  suffix: String
) -> RequestBuilder {
  let extra = CopilotCompletionParams { language: language.to_string(),
    next_indent: 0,
    trim_by_indentation: true,
    prompt_tokens: prompt.len() as i32,
    suffix_tokens: suffix.len() as i32
  };
  let body = Some(CopilotCompletionRequest {
    prompt,
    suffix,
    max_tokens: 500,
    temperature: 1.0,
    top_p: 1.0,
    n: 3,
    stop: ["unset".to_string()].to_vec(),
    nwo: "my_org/my_repo".to_string(),
    stream: true,
    extra
  });
  let body = serde_json::to_string(&body).unwrap();
  let completions_url = "https://copilot-proxy.githubusercontent.com/v1/engines/copilot-codex/completions";
  http_client.post(completions_url)
    .header("X-Request-Id", Uuid::new_v4().to_string())
    .header("VScode-SessionId", Uuid::new_v4().to_string() + &Utc::now().timestamp().to_string())
    .body(body)
}

async fn on_change(state: State, uri: String, rope: Rope) {
  state.document_map.insert(uri, rope);
}


#[derive(Debug, Deserialize, Serialize)]
struct InlayHintParams {
path: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TextDocumentItem {
uri: Url,
text: String,
version: i32,
}

// async fn initialized(state: &mut state, _: InitializedParams) {
//   state.client
//     .log_message(MessageType::INFO, "initialized!")
//     .await;
// }
//
// async fn shutdown(state: &mut state) -> Result<()> {
//   Ok(())
// }
//
// async fn did_open(state: &mut state, params: DidOpenTextDocumentParams) {
//   state.client
//     .log_message(MessageType::INFO, "file opened!")
//     .await;
//   state.language_map.insert(params.text_document.uri.to_string(), params.text_document.language_id);
//   state.on_change(TextDocumentItem {
//     uri: params.text_document.uri,
//     text: params.text_document.text,
//     version: params.text_document.version,
//   })
//   .await
// }
//
// async fn did_change(state: &mut state, mut params: DidChangeTextDocumentParams) {
//   state.on_change(TextDocumentItem {
//     uri: params.text_document.uri,
//     text: std::mem::take(&mut params.content_changes[0].text),
//     version: params.text_document.version,
//   }).await
// }
//
// async fn did_save(state: &mut state, _: DidSaveTextDocumentParams) {
//   state.client
//     .log_message(MessageType::ERROR, "file saved!")
//     .await;
// }
// async fn did_close(state: &mut state, _: DidCloseTextDocumentParams) {
//   state.client
//     .log_message(MessageType::ERROR, "file closed!")
//     .await;
// }
//
// async fn did_change_configuration(state: &mut state, _: DidChangeConfigurationParams) {
//   state.client
//     .log_message(MessageType::ERROR, "configuration changed!")
//     .await;
// }
//
// async fn did_change_workspace_folders(state: &mut state, _: DidChangeWorkspaceFoldersParams) {
//   state.client
//     .log_message(MessageType::ERROR, "workspace folders changed!")
//     .await;
// }
//
// async fn did_change_watched_files(state: &mut state, _: DidChangeWatchedFilesParams) {
//   state.client
//     .log_message(MessageType::ERROR, "watched files have changed!")
//     .await;
// }
