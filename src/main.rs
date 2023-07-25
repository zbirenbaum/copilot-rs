mod copilot;
use copilot::CopilotHandler;
use dashmap::DashMap;
use ropey::Rope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct CopilotLSP {
  client: Client,
  document_map: DashMap<String, Rope>,
  language_map: DashMap<String, String>,
  copilot_handler: copilot::CopilotHandler
}

impl CopilotLSP {
  async fn on_change(&self, params: TextDocumentItem) {
    let rope = ropey::Rope::from_str(&params.text);
    self.document_map
      .insert(params.uri.to_string(), rope);
  }
  async fn get_completions_cycling(&self, params: CompletionParams) -> std::result::Result<Option<CompletionResponse>, tower_lsp::jsonrpc::Error> {
    let uri = &params.text_document_position.text_document.uri;
    let _position = &params.text_document_position.position;
    let rope = self.document_map.get(&uri.to_string()).unwrap();
    let language = self.language_map.get(&uri.to_string()).unwrap().to_string();
    // self.client
    //   .log_message(MessageType::ERROR, &language)
    //   .await;
    let s = format!("{:?}", &params.text_document_position.position.character);
    println!("{}", s);
    // self.client
    //   .log_message(MessageType::ERROR, s)
    //   .await;
    let completions = self.copilot_handler.stream_completions(&language, &params, &rope, &self.client).await;
    match completions {
      Ok(complete) => {
        let _s = format!("{:?}", complete);
        Ok(Some(CompletionResponse::Array(complete)))
      },
      Err(e) => {
        self.client
          .log_message(MessageType::ERROR, "No Completions")
          .await;
        Err(tower_lsp::jsonrpc::Error {
          code: tower_lsp::jsonrpc::ErrorCode::ServerError(69),
          message: e,
          data: None
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
    })
    .await
  }

  async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
    self.on_change(TextDocumentItem {
      uri: params.text_document.uri,
      text: std::mem::take(&mut params.content_changes[0].text),
      version: params.text_document.version,
    })
    .await
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

async fn start_lsp<'handler>(copilot_handler: copilot::CopilotHandler) {
  env_logger::init();
  let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());

  let (service, socket) = LspService::build(|client| CopilotLSP {
    client, document_map: DashMap::new(),
    language_map: DashMap::new(),
    copilot_handler
  })
  .custom_method("getCompletionsCycling", CopilotLSP::get_completions_cycling)
  .finish();
  Server::new(stdin, stdout, socket).serve(service).await;
}

#[tokio::main]
async fn main() {
  #[cfg(feature = "runtime-agnostic")]
  let (stdin, stdout) = (stdin.compat(), stdout.compat_write());
  let copilot_handler = copilot::CopilotHandler::new().await;
  start_lsp(copilot_handler).await;
  // tracing_subscriber::fmt().init();
}
