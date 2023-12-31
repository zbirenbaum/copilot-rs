use crate::{parse, debounce, cache, request::build_request};
use crate::copilot::{CopilotCompletionResponse, CopilotResponse, CopilotCyclingCompletion, CopilotEditorInfo, DocParams};
use futures_util::stream::PollNext;
use ropey::Rope;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::{jsonrpc::{Error, Result}, lsp_types::*, Client, LanguageServer, LspService, Server};
use dashmap::DashMap;
use std::{
  borrow::Cow,
  str::FromStr,
  fmt::Debug,
  collections::HashMap,
  time::{Duration, Instant},
  sync::{
    mpsc::channel, RwLock, Arc, Mutex
  }
};
use tower_lsp::{};
use reqwest::header::{HeaderMap, HeaderValue};
use futures::future::{Abortable, AbortHandle, Aborted};
use tokio::time;
use futures_util::{StreamExt, FutureExt};
use eventsource_stream::Eventsource;
use futures::{future, task::Poll};

#[derive(Deserialize, Serialize, Debug)]
pub struct Success {
  success: bool
}
impl Success {
  pub fn new(success: bool) -> Self { Self { success } }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
  pub uri: String,
  pub text: String,
  pub version: i32,
  pub language_id: String
}

type SafeMap = Arc<RwLock<HashMap<String, Mutex<TextDocumentItem>>>>;

#[derive(Debug)]
pub struct Backend {
  pub client: Client,
  pub documents: SafeMap,
  pub http_client: Arc<reqwest::Client>,
  pub current_dispatch: Option<AbortHandle>,
  pub runner: debounce::Runner,
  pub editor_info: Arc<RwLock<CopilotEditorInfo>>,
  pub cache: cache::CopilotCache
}

fn handle_event(
  event: eventsource_stream::Event
) -> CopilotResponse {
  if event.data == "[DONE]" {
    return CopilotResponse::Done;
  }
  match serde_json::from_str(&event.data) {
    Ok(data) => { CopilotResponse::Answer(data) }
    Err(e) => { CopilotResponse::Error(e.to_string()) }
  }
}

pub async fn await_stream(req: RequestBuilder, line_before: String, pos: Position) -> Vec<String> {
  let resp = req.send().await.unwrap();
  let mut stream = resp.bytes_stream().eventsource();
  let mut completion_list = Vec::<String>::with_capacity(4);
  let mut s = "".to_string();
  let mut cancellation_reason = None;

  while let Some(event) = stream.next().await {
    match handle_event(event.unwrap()) {
      CopilotResponse::Answer(ans) => {
        ans.choices.iter().for_each(|x| {
          s.push_str(&x.text);
          if x.finish_reason.is_some() {
            completion_list.push(s.to_string());
            s = "".to_string();
          }
        });
      },
      CopilotResponse::Done => {
        return completion_list;
      },
      CopilotResponse::Error(e) => {
        cancellation_reason = Some(e);
      }
    }
  };
  return completion_list;
}

struct CompletionStreamingParams {
  req: RequestBuilder,
  line_before: String,
  position: Position
}

impl Backend {
  fn get_doc_info(&self, uri: &String) -> Result<Box<TextDocumentItem>> {
    let data = Arc::clone(&self.documents);
    let map = data.read().unwrap();
    match map.get(uri) {
      Some(e) => {
        let element = e.lock().expect("RwLock poisoned");
        Ok(Box::new(element.clone()))
      },
      None => {
        Err(Error {
          code: tower_lsp::jsonrpc::ErrorCode::from(69),
          data: None,
          message: Cow::from(format!("Failed to get doc info"))
        })
      }
    }
  }

  pub async fn set_editor_info(&self, params: CopilotEditorInfo) -> Result<Success> {
    self.client.log_message(MessageType::INFO, "setEditorInfo").await;
    let copy = Arc::clone(&self.editor_info);
    let mut lock = copy.write().unwrap();
    *lock = params;
    return Ok(Success::new(true));
  }
  pub fn get_doc_params(&self, params: &CompletionParams) -> DocParams {
    let pos = params.text_document_position.position.clone();
    let uri = params.text_document_position.text_document.uri.to_string();
    let doc = self.get_doc_info(&uri).unwrap();
    let rope = ropey::Rope::from_str(&doc.text);
    let offset = parse::position_to_offset(pos, &rope).unwrap();

    DocParams {
      uri: uri.to_string(),
      pos: pos.clone(),
      language: doc.language_id.to_string(),
      prefix: parse::get_text_before(offset, &rope).unwrap(),
      suffix: parse::get_text_after(offset, &rope).unwrap(),
      line_before: parse::get_line_before(pos, &rope).unwrap().to_string(),
      rope,
    }
  }

  pub async fn get_completions_cycling(&self, params: CompletionParams) -> Result<CopilotCompletionResponse> {
    let doc_params = self.get_doc_params(&params);
    let cached_result = self.cache.get_cached_result(&doc_params.uri, doc_params.pos.line);
    if cached_result.is_some() {
      return Ok(cached_result.unwrap());
    }

    let valid = self.runner.increment_and_do_stuff().await;
    if !valid {
      return Ok(CopilotCompletionResponse {
        cancellation_reason: Some("More Recent".to_string()),
        completions: vec![]
      });
    }

    let doc_params = self.get_doc_params(&params);
    let line_before = doc_params.line_before.to_string();
    let http_client = Arc::clone(&self.http_client);
    let _prompt = format!(
      "// Path: {}\n{}",
      doc_params.uri,
      doc_params.prefix.to_string()
    );

    let req = build_request(http_client, doc_params.language, doc_params.prefix, doc_params.suffix);

    let completion_list = await_stream(
      req,
      line_before.to_string(),
      params.text_document_position.position.clone()
    );
    let response = CopilotCompletionResponse::from_str_vec(
      completion_list.await,
      line_before,
      doc_params.pos
    );
    self.cache.set_cached_result(&doc_params.uri, &doc_params.pos.line, &response);
    Ok(response)
  }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
  async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
    Ok(InitializeResult {
      server_info: None,
      offset_encoding: None,
      capabilities: ServerCapabilities {
        inlay_hint_provider: None,
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
    self.client.log_message(MessageType::INFO, "file opened!").await;
    let id = params.text_document.uri.to_string();
    let doc = Mutex::new(TextDocumentItem {
      uri: params.text_document.uri.to_string(),
      text: params.text_document.text,
      version: params.text_document.version,
      language_id: params.text_document.language_id
    });
    let mut map = self.documents.write().expect("RwLock poisoned");
    map.entry(id).or_insert_with(|| doc);
  }

  async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
    let data = Arc::clone(&self.documents);
    let mut map = data.write().expect("RwLock poisoned");
    if let Some(element) = map.get(&params.text_document.uri.to_string()) {
      let mut element = element.lock().expect("Mutex poisoned");
      let doc = TextDocumentItem {
        uri: element.uri.to_string(),
        text: std::mem::take(&mut params.content_changes[0].text),
        version: params.text_document.version,
        language_id: element.language_id.to_string()
      };
      *element = doc
    }
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


