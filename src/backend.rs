use crate::parse::DocumentCompletionParams;
use crate::{copilot, parse, request::build_request};
use crate::copilot::{CopilotCompletionResponse, CopilotResponse, CopilotCyclingCompletion};
use futures_util::stream::PollNext;
use ropey::Rope;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::{jsonrpc::{Error, Result},lsp_types::*,  {Client, LanguageServer}};
use dashmap::DashMap;
use std::{
  borrow::Cow,
  str::FromStr,
  fmt::Debug,
  thread,
  collections::HashMap,
  time::{Duration,
  Instant},
  sync::{
    mpsc::channel, RwLock, Arc, Condvar, Mutex, atomic::{
      Ordering,
      AtomicBool,
      AtomicU16
    }
  }
};
use tower_lsp::{LspService, Server};
use reqwest::header::{HeaderMap, HeaderValue};
use futures::future::{Abortable, AbortHandle, Aborted};
use tokio::time;
use futures_util::{StreamExt, FutureExt};
use eventsource_stream::{Eventsource};
use futures::{future, task::Poll};

type Handle = tokio::task::JoinHandle<Result<CopilotCompletionResponse>>;
type ProtectedPair = (Mutex<u32>, Condvar);

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[derive(Clone)]
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
  pub pending: Arc<AtomicBool>,
  pub started: Arc<ProtectedPair>,
  pub finished: Arc<ProtectedPair>,
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
fn create_item(
  text: String,
  line_before: &String,
  position: Position
) -> CopilotCyclingCompletion {
  let display_text = text.clone();
  let text = format!("{}{}", line_before, text);
  let end_char = text.find('\n').unwrap_or(text.len()) as u32;
  CopilotCyclingCompletion {
    display_text, // partial text
    text, // fulltext
    range: Range {
      start: Position {
        character: 0,
        line: position.line,
      },
      end: Position {
        character: position.character,
        line: position.line,
      }
    }, // start char always 0
    position,
  }
}

struct DocParams {
  rope: Rope,
  uri: String,
  language: String,
  line_before: String,
  prefix: String,
  suffix: String
}

pub async fn await_stream(req: RequestBuilder, line_before: String, pos: Position) -> Vec<CopilotCyclingCompletion> {
  let resp = req.send().await.unwrap();

  let mut stream = resp.bytes_stream().eventsource();
  let mut completion_list = Vec::<CopilotCyclingCompletion>::with_capacity(4);
  let mut s = "".to_string();
  let mut cancellation_reason = None;

  while let Some(event) = stream.next().await {
    match handle_event(event.unwrap()) {
      CopilotResponse::Answer(ans) => {
        ans.choices.iter().for_each(|x| {
          s.push_str(&x.text);
          if x.finish_reason.is_some() {
            let item = create_item(s.clone(), &line_before, pos);
            completion_list.push(item);
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

impl Backend {
  fn get_doc_info(&self, uri: &String) -> Result<Box<TextDocumentItem>> {
    let data = Arc::clone(&self.documents);
    let map = data.read().expect("RwLock poisoned");
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
  fn get_doc_params(&self, uri: &String, pos: Position) -> Result<DocParams> {
    let doc = self.get_doc_info(uri)?;
    let rope = ropey::Rope::from_str(&doc.text);
    let offset = parse::position_to_offset(pos, &rope).unwrap();

    Ok(DocParams {
      uri: uri.to_string(),
      language: doc.language_id.to_string(),
      prefix: parse::get_text_before(offset, &rope).unwrap(),
      suffix: parse::get_text_after(offset, &rope).unwrap(),
      line_before: parse::get_line_before(pos, &rope).unwrap().to_string(),
      rope,
    })
  }

  fn get_completions_cycling_request(&self, params: DocParams) -> Result<reqwest::RequestBuilder> {
    let http_client = Arc::clone(&self.http_client);
    let _prompt = format!(
      "// Path: {}\n{}",
      params.uri,
      params.prefix.to_string()
    );
    Ok(build_request(http_client, params.language, params.prefix, params.suffix))
  }
  // async fn on_completions_cycling(&mut self, params: Request) {}
  pub async fn get_completions_cycling(&self, params: CompletionParams) -> Result<CopilotCompletionResponse> {
    let pos = params.text_document_position.position.clone();
    let uri = params.text_document_position.text_document.uri.to_string();
    let doc_params = self.get_doc_params(&uri, pos)?;
    let line_before = doc_params.line_before.to_string();
    let rope = doc_params.rope.clone();
    let req = self.get_completions_cycling_request(doc_params)?;
    let completion_list = await_stream(req, line_before, params.text_document_position.position.clone()).await.clone();
    // self.client.log_message(MessageType::ERROR, completions.get(0).unwrap().text.to_string()).await;
    Ok(CopilotCompletionResponse {
      cancellation_reason: None,
      completions: completion_list
    })
  }
  //   let pair2 = Arc::clone(&self.pair);
  //
  //   self.pending.store(false, Ordering::Release);
  //
  //   let pending = Arc::clone(&self.pending);
  //   let handle = tokio::task::spawn(async move {
  //     if !pending.load(Ordering::Acquire) {
  //     }
  //     time::sleep(Duration::from_millis(10)).await;
  //     return on_completions_cycling(
  //       req,
  //       position,
  //       doc_params.line_before
  //     ).await;
  //   });
  //   self.pending.store(true, Ordering::Release);
  //
  //   match handle.await {
  //     Ok(res) => { return res }
  //     Err(_) => {
  //       self.client.log_message(MessageType::ERROR, "ABORTED".to_string()).await;
  //       return Ok(CopilotCompletionResponse {
  //         completions: vec![],
  //         cancellation_reason: Some("RequestCancelled".to_string()),
  //       })
  //     }
  //   }
  // }

  // pub async fn on_change(&self, id: Url, params: TextDocumentItem) {
}
// self.document_map.insert(params.uri.to_string(), params);

#[tower_lsp::async_trait(?Send)]
impl LanguageServer for Backend {
  async fn initialize(&mut self, _: InitializeParams) -> Result<InitializeResult> {
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
  async fn initialized(&mut self, _: InitializedParams) {
    self.client
      .log_message(MessageType::INFO, "initialized!")
      .await;
  }

  async fn shutdown(&mut self) -> Result<()> {
    Ok(())
  }

  async fn did_open(&mut self, params: DidOpenTextDocumentParams) {
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

  async fn did_change(&mut self, mut params: DidChangeTextDocumentParams) {
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


