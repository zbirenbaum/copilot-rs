// use crate::{auth, copilot, parse};
// use crate::copilot::{CopilotCompletionRequest, CopilotCompletionParams};
use dashmap::DashMap;
use ropey::Rope;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::*;
use tower_lsp::jsonrpc::{Error, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use chrono::Utc;
use reqwest::RequestBuilder;
use serde_json::Value;
use std::borrow::Cow;
use std::rc::Rc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use std::sync::Arc;

#[derive(Debug,Default)]
struct CopilotState {
  document_map: Arc<DashMap<String, Rope>>,
  language_map: Arc<DashMap<String, String>>,
  http_client: Arc<reqwest::Client>,
}

/// These are for notifications coming from our thread back to the editor.
#[derive(Debug)]
enum FromCopilot {
  Info(std::borrow::Cow<'static, str>),
}

/// Messages coming from the editor via Copilot to our thread.
#[derive(Debug)]
enum ToCopilot {
  DidOpen {
    params: DidOpenTextDocumentParams,
  },
  DidChange {
    params: DidChangeTextDocumentParams,
  },
}

struct CopilotThread {
  output: mpsc::UnboundedSender<FromCopilot>,
  input: mpsc::UnboundedReceiver<ToCopilot>,
  shutdown: tokio::sync::broadcast::Receiver<()>,
}

impl CopilotThread {
  fn init(mut self: CopilotThread) -> impl FnOnce() {
    move || {
      let mut state: CopilotState = Default::default();
      while let Err(tokio::sync::broadcast::error::TryRecvError::Empty) = self.shutdown.try_recv() {
        if let Ok(input) = self.input.try_recv() {
          use ToCopilot::*;
          match input {
            DidOpen { params } => {
              self.output.send(FromCopilot::Info(Cow::from(format!(
                "did open {}",
                params.text_document.uri
              )))).unwrap();
              state.language_map.insert(params.text_document.uri.to_string(), params.text_document.language_id);
              let rope = ropey::Rope::from_str(&params.text_document.text.to_string());
              state.document_map.insert(params.text_document.uri.to_string(), rope);
            }
            DidChange { params } => {
              let _ = params;
              let text = params.content_changes[0].text.to_string().to_owned();
              let rope = ropey::Rope::from_str(&text);
              state.document_map.insert(params.text_document.uri.to_string(), rope);
            }
          }
        }
      }
    }
  }
}

#[derive(Debug)]
struct BackendState {
  shutdown: broadcast::Sender<()>,
  to_copilot: Option<mpsc::UnboundedSender<ToCopilot>>,
  // uncomment for:
  // Rc<usize> cannot be sent between threads safely
  // bar: Rc<usize>,
}

#[derive(Debug)]
struct Backend {
  client: Client,
  state: Mutex<BackendState>,
}


async fn process_copilot_notifications<'a>(
  client: tower_lsp::Client,
  mut from_copilot: mpsc::UnboundedReceiver<FromCopilot>,
  mut shutdown: broadcast::Receiver<()>,
  ) {
  while let Err(broadcast::error::TryRecvError::Empty) = shutdown.try_recv() {
    if let Some(notif) = from_copilot.recv().await {
      match notif {
        FromCopilot::Info(msg) => {
          client.log_message(MessageType::INFO, msg).await;
        }
      }
    }
  }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
  async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
    let mut state = self.state.lock().await;
    let (to_copilot, copilot_input) = mpsc::unbounded_channel();
    let (copilot_output, from_copilot) = mpsc::unbounded_channel();

    state.to_copilot = Some(to_copilot);
    tokio::task::spawn(process_copilot_notifications(
      self.client.clone(),
      from_copilot,
      state.shutdown.subscribe(),
    ));
    std::thread::spawn(CopilotThread::init(CopilotThread {
      output: copilot_output,
      input: copilot_input,
      shutdown: state.shutdown.subscribe(),
    }));

    Ok(InitializeResult {
      server_info: None,
      capabilities: ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        execute_command_provider: None,
        ..ServerCapabilities::default()
      },
      ..Default::default()
    })
  }

  async fn initialized(&self, _: InitializedParams) {
    self.client
      .log_message(MessageType::INFO, "initialized!")
      .await;
  }

  async fn shutdown(&self) -> Result<()> {
    let state = self.state.lock().await;
    state.shutdown.send(()).unwrap();
    Ok(())
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    let state = self.state.lock().await;
    // We can do this:
    let _text_len = Rc::new(params.text_document.text.len());
    state
      .to_copilot
      .as_ref()
      .unwrap()
      .send(ToCopilot::DidOpen { params })
      .unwrap();
    // uncomment for:
    // future cannot be sent safely between threads
    // self.client.log_message(MessageType::INFO, "However we can't hold _text_len across this await point").await;
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let state = self.state.lock().await;
    state
      .to_copilot
      .as_ref()
      .unwrap()
      .send(ToCopilot::DidChange { params })
      .unwrap();
  }
}

#[tokio::main]
async fn main() {
  env_logger::init();
  let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
  let (shutdown, _) = broadcast::channel(1);
  let state = Mutex::new(BackendState {
    to_copilot: None,
    shutdown,
  });
  let (service, socket) = LspService::new(|client| Backend { client, state });
  Server::new(stdin, stdout, socket).serve(service).await;
}
