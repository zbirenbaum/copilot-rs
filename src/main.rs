use copilot_rs::{auth, copilot, parse, request::build_request};
use copilot_rs::pending;
use copilot_rs::copilot::{CopilotCompletionResponse, CopilotResponse};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use reqwest::header::{HeaderMap, HeaderValue};
use std::ops::Deref;
use std::sync::Arc;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;
use futures::future::{self, Either};
use tower_lsp::ExitedError;
use tower_lsp::jsonrpc::{Request, RequestBuilder};
use tower_lsp::lsp_types::notification::Cancel;
use std::sync::atomic::{Ordering, AtomicBool};
use std::str::FromStr;
use tower_lsp::jsonrpc::{Error, Id, Result, Response};
use std::borrow::Cow;
// pub struct ResponseWrapper<V>
// where
// for <'a> V: Serialize + Deserialize<'static> + Debug + Clone + Send + Sync + 'static {
//   data: V,
// }
#[derive(Debug)]
struct Backend {
  client: Client,
  document_map: Arc<DashMap<String, TextDocumentItem>>,
  language_map: Arc<DashMap<String, String>>,
  http_client: Arc<reqwest::Client>,
}

impl Backend {
  // async fn on_completions_cycling(&mut self, params: Request) {}
  async fn get_completions_cycling(&mut self, params: CompletionParams) -> Result<CopilotCompletionResponse> {
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;
    let text_doc = self.document_map.get(&uri.to_string()).unwrap();
    let version = text_doc.version;
    let rope = ropey::Rope::from_str(&text_doc.text.clone());
    let language = self.language_map.get(&uri.to_string()).unwrap().clone();
    let doc_params = parse::DocumentCompletionParams::new(uri, position, rope);
    drop(text_doc);
    let req = build_request(self.http_client.clone(), language, doc_params.prompt, doc_params.suffix);
    let resp = req.send().await.unwrap();
    let status = resp.status();
    let resp = copilot::fetch_completions(resp, doc_params.line_before, position).await;
    self.client.next_request_id();
    match resp {
      Ok(complete) => { Ok(complete) }
      Err(e) => {
        Err(Error {
          code: tower_lsp::jsonrpc::ErrorCode::from(10),
          data: None,
          message: Cow::from(format!("completions failed with reason: {}", e))
        })
      }
    }
  }

  async fn on_change(&mut self, params: TextDocumentItem) {
    let _rope = ropey::Rope::from_str(&params.text);
    self.document_map
      .insert(params.uri.to_string(), params);
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

#[tower_lsp::async_trait(?Send)]
impl LanguageServer for Backend {
  async fn initialize(&mut self, _: InitializeParams) -> Result<InitializeResult> {
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
  async fn initialized(&mut self, _: InitializedParams) {
    self.client
      .log_message(MessageType::INFO, "initialized!")
      .await;
  }

  async fn shutdown(&mut self) -> Result<()> {
    Ok(())
  }

  async fn did_open(&mut self, params: DidOpenTextDocumentParams) {
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

  async fn did_change(&mut self, mut params: DidChangeTextDocumentParams) {
    self.on_change(TextDocumentItem {
      uri: params.text_document.uri,
      text: std::mem::take(&mut params.content_changes[0].text),
      version: params.text_document.version
    }).await
  }

  async fn did_save(&mut self, _: DidSaveTextDocumentParams) {
    self.client
      .log_message(MessageType::ERROR, "file saved!")
      .await;
  }
  async fn did_close(&mut self, _: DidCloseTextDocumentParams) {
    self.client
      .log_message(MessageType::ERROR, "file closed!")
      .await;
  }

  async fn did_change_configuration(&mut self, _: DidChangeConfigurationParams) {
    self.client
      .log_message(MessageType::ERROR, "configuration changed!")
      .await;
  }

  async fn did_change_workspace_folders(&mut self, _: DidChangeWorkspaceFoldersParams) {
    self.client
      .log_message(MessageType::ERROR, "workspace folders changed!")
      .await;
  }

  async fn did_change_watched_files(&mut self, _: DidChangeWatchedFilesParams) {
    self.client
      .log_message(MessageType::ERROR, "watched files have changed!")
      .await;
  }

  async fn execute_command(&mut self, _: ExecuteCommandParams) -> Result<Option<Value>> {
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
  #[cfg(feature = "runtime-agnostic")]
  use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

  tracing_subscriber::fmt().init();

  let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
  #[cfg(feature = "runtime-agnostic")]
  let (stdin, stdout) = (stdin.compat(), stdout.compat_write());
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

  let (service, socket) = LspService::build(|client| Backend {
    client,
    document_map: Arc::new(DashMap::new()),
    language_map: Arc::new(DashMap::new()),
    http_client: Arc::new(http_client),
  }).custom_method("getCompletionsCycling", Backend::get_completions_cycling)
    .finish();
  Server::new(stdin, stdout, socket)
    .serve(service)
    .await;
  // tracing_subscriber::fmt().init();
}
