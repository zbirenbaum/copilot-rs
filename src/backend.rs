use crate::{copilot, parse, request::build_request};
use crate::copilot::{CopilotCompletionResponse};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use tower_lsp::lsp_types::*;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::{Client, LanguageServer};


use std::sync::Arc;
use std::borrow::Cow;
use std::fmt::{Debug};


use std::str::FromStr;


#[derive(Debug)]
pub struct Backend {
  pub client: Client,
  pub document_map: Arc<DashMap<String, TextDocumentItem>>,
  pub language_map: Arc<DashMap<String, String>>,
  pub http_client: Arc<reqwest::Client>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
  pub uri: Url,
  pub text: String,
  pub version: i32,
}

impl Backend {
  // async fn on_completions_cycling(&mut self, params: Request) {}
  pub async fn get_completions_cycling(&mut self, params: CompletionParams) -> Result<CopilotCompletionResponse> {
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;
    let text_doc = self.document_map.get(&uri.to_string()).unwrap();
    let _version = text_doc.version;
    let rope = ropey::Rope::from_str(&text_doc.text.clone());
    let language = self.language_map.get(&uri.to_string()).unwrap().clone();
    let doc_params = parse::DocumentCompletionParams::new(uri, position, rope);
    drop(text_doc);
    let req = build_request(self.http_client.clone(), language, doc_params.prompt, doc_params.suffix);
    let resp = req.send().await.unwrap();
    let _status = resp.status();
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

  pub async fn on_change(&mut self, params: TextDocumentItem) {
    let _rope = ropey::Rope::from_str(&params.text);
    self.document_map
      .insert(params.uri.to_string(), params);
  }
}

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


