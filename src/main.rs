use uuid::Uuid;
use copilot_rs::{auth, copilot, parse};
use copilot_rs::copilot::{CopilotCompletionRequest, CopilotCompletionParams};
use dashmap::DashMap;
use ropey::Rope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tower_lsp::jsonrpc::{Error, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use chrono::Utc;
use reqwest::RequestBuilder;
use tokio::sync::Mutex;
use std::sync::Arc;


#[derive(Debug)]
struct CopilotLSP {
  client: Client,
  // state: Mutex<State>
  document_map: Arc<DashMap<String, Rope>>,
  language_map: Arc<DashMap<String, String>>,
  http_client: Arc<reqwest::Client>,
}
type CompletionCyclingResponse = Result<Option<CompletionResponse>>;

impl CopilotLSP {
  pub fn build_request(
    &self,
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
    self.http_client.post(completions_url)
      .header("X-Request-Id", Uuid::new_v4().to_string())
      .header("VScode-SessionId", Uuid::new_v4().to_string() + &Utc::now().timestamp().to_string())
      .body(body)
  }

  async fn on_change(&self, params: TextDocumentItem) {
    let rope = ropey::Rope::from_str(&params.text);
    self.document_map
      .insert(params.uri.to_string(), rope);
  }

  async fn get_completions_cycling(&self, params: CompletionParams) -> CompletionCyclingResponse {
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;
    let rope = self.document_map.get(&uri.to_string()).unwrap().clone();
    let language = self.language_map.get(&uri.to_string()).unwrap().clone();
    let doc_params = parse::DocumentCompletionParams::new(uri, position, rope);

    let req = self.build_request(language, doc_params.prompt, doc_params.suffix);
    let resp = req.send().await.unwrap();
    let status = resp.status();
    if status != 200 {
      self.client.log_message(MessageType::ERROR, status).await;
      let text = resp.text().await.unwrap();
      self.client.log_message(MessageType::ERROR, text).await;
      return Err(Error {
        code: tower_lsp::jsonrpc::ErrorCode::from(10),
        data: None,
        message: format!("http request failed with status: {}",status)
      })
    }
    // let s = format!("{:?}", &params.text_document_position.position.character);
    let resp = copilot::fetch_completions(resp, doc_params.line_before).await;
    let s = format!("{:?}", &resp);
    self.client.log_message(MessageType::ERROR, s).await;
    match resp {
      Ok(complete) => { Ok(Some(CompletionResponse::Array(complete))) }
      Err(e) => {
        Err(Error {
          code: tower_lsp::jsonrpc::ErrorCode::from(10),
          data: None,
          message: format!("completions failed with reason: {}", e)
        })
      }
    }
  }
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

#[tower_lsp::async_trait]
impl LanguageServer for CopilotLSP {
  async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
    Ok(InitializeResult {
      server_info: None,
      offset_encoding: None,
      capabilities: ServerCapabilities {
        inlay_hint_provider: Some(OneOf::Left(true)),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
          resolve_provider: Some(false),
          trigger_characters: Some(vec![".".to_string()]),
          work_done_progress_options: Default::default(),
          all_commit_characters: None,
          completion_item: None,
        }),
        execute_command_provider: Some(ExecuteCommandOptions {
          commands: vec!["dummy.do_something".to_string()],
          work_done_progress_options: Default::default(),
        }),
        workspace: Some(WorkspaceServerCapabilities {
          workspace_folders: Some(WorkspaceFoldersServerCapabilities {
            supported: Some(true),
            change_notifications: Some(OneOf::Left(true)),
          }),
          file_operations: None,
        }),
        semantic_tokens_provider: None,
        ..ServerCapabilities::default()
      },
    })
  }
  async fn initialized(&self, _: InitializedParams) {
    self.client
      .log_message(MessageType::INFO, "initialized!")
      .await;
  }

  async fn shutdown(&self) -> Result<()> {
    Ok(())
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    self.client
      .log_message(MessageType::INFO, "file opened!")
      .await;
    self.language_map.insert(params.text_document.uri.to_string(), params.text_document.language_id);
    self.on_change(TextDocumentItem {
      uri: params.text_document.uri,
      text: params.text_document.text,
      version: params.text_document.version,
    }).await
  }

  async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
    self.on_change(TextDocumentItem {
      uri: params.text_document.uri,
      text: std::mem::take(&mut params.content_changes[0].text),
      version: params.text_document.version
    }).await
  }

  async fn did_save(&self, _: DidSaveTextDocumentParams) {
    self.client
      .log_message(MessageType::ERROR, "file saved!")
      .await;
  }
  async fn did_close(&self, _: DidCloseTextDocumentParams) {
    self.client
      .log_message(MessageType::ERROR, "file closed!")
      .await;
  }

  async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
    self.client
      .log_message(MessageType::ERROR, "configuration changed!")
      .await;
  }

  async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
    self.client
      .log_message(MessageType::ERROR, "workspace folders changed!")
      .await;
  }

  async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
    self.client
      .log_message(MessageType::ERROR, "watched files have changed!")
      .await;
  }

  async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
    self.client
      .log_message(MessageType::ERROR, "command executed!")
      .await;

    match self.client.apply_edit(WorkspaceEdit::default()).await {
      Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
      Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
      Err(err) => self.client.log_message(MessageType::ERROR, err).await,
    }

    Ok(None)
  }
}

#[tokio::main]
async fn main() {
  env_logger::init();
  let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
  // let copilot_handler = CopilotHandler::new();

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

// fn build_request_headers(authenticator: CopilotAuthenticator) -> RequestBuilder {
// }

  let (service, socket) = LspService::build(|client| CopilotLSP {
    client,
    document_map: Arc::new(DashMap::new()),
    language_map: Arc::new(DashMap::new()),
    http_client: Arc::new(http_client),
  }).custom_method("getCompletionsCycling", CopilotLSP::get_completions_cycling)
    .finish();
  Server::new(stdin, stdout, socket)
    .serve(service)
    .await;
  // tracing_subscriber::fmt().init();
}
