use completions::CompletionFetcher;
use serde_json::Value;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
mod reader;
mod completions;
mod auth;
use tower_lsp::jsonrpc::Result;
use serde::{Deserialize, Serialize};
use futures_util::stream::StreamExt;
use eventsource_stream::{Eventsource};

struct Copilot {
  client: Client,
  fetcher: completions::CompletionFetcher,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TextDocument {
  relative_path: String,
  uri: String,
  version: i16,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Position {
  character: i16,
  line: i16,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Document {
  indent_size: i8,
  insert_spaces: bool,
  position: Position,
  relative_path: String,
  tab_size: i8,
  uri: String,
  version: i8,
}

#[derive(Deserialize, Serialize, Debug,)]
#[serde(rename_all = "camelCase")]
struct CopilotCompletionParams {
  doc: Document,
  position: Position,
  text_document: TextDocument,
}

impl Copilot {
  async fn get_completions_cycling(&self, _params: CopilotCompletionParams) -> Result<Option<CompletionResponse>> {
    self.client.log_message(MessageType::ERROR, "FUCKYES").await;
    let data = get_test_request();
    let mut stream = self.fetcher.request(data)
      .send()
      .await.unwrap()
      .bytes_stream()
      .eventsource();
    let mut choices = Vec::<CompletionItem>::new();
    while let Some(event) = stream.next().await {
      match event {
        Ok(event) => {
          if event.data == "[DONE]" { break; }
          let resp: completions::CopilotResponse = serde_json::from_str(&event.data).unwrap();
          let it = &resp.choices;
          for i in it.iter() {
            choices.push(CompletionItem::new_simple(i.text.to_string(), "More detail".to_string()))
          }
        },
        Err(e) => println!("error occured: {}", e),
      }
    }
    let resp = CompletionResponse::Array(choices);
    Ok(Some(resp))
  }
}

#[tower_lsp::async_trait]
impl LanguageServer for Copilot {
  async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
    Ok(InitializeResult {
      server_info: None,
      capabilities: ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
          TextDocumentSyncKind::INCREMENTAL,
        )),
        completion_provider: Some(CompletionOptions {
          resolve_provider: Some(false),
          trigger_characters: Some(vec![".".to_string()]),
          work_done_progress_options: Default::default(),
          all_commit_characters: None,
          ..Default::default()
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

  async fn did_change_workspace_folders(&self, _: DidChangeWorkspaceFoldersParams) {
    self.client
      .log_message(MessageType::INFO, "workspace folders changed!")
      .await;
  }

  async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
    self.client
      .log_message(MessageType::INFO, "configuration changed!")
      .await;
  }

  async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
    self.client
      .log_message(MessageType::INFO, "watched files have changed!")
      .await;
  }

  async fn execute_command(&self, _: ExecuteCommandParams) -> Result<Option<Value>> {
    self.client
      .log_message(MessageType::INFO, "command executed!")
      .await;

    match self.client.apply_edit(WorkspaceEdit::default()).await {
      Ok(res) if res.applied => self.client.log_message(MessageType::INFO, "applied").await,
      Ok(_) => self.client.log_message(MessageType::INFO, "rejected").await,
      Err(err) => self.client.log_message(MessageType::ERROR, err).await,
    }

    Ok(None)
  }

  async fn did_open(&self, _: DidOpenTextDocumentParams) {
    self.client
      .log_message(MessageType::INFO, "file opened!")
      .await;
  }

  async fn did_change(&self, _: DidChangeTextDocumentParams) {
    self.client
      .log_message(MessageType::INFO, "file changed!")
      .await;
  }

  async fn did_save(&self, _: DidSaveTextDocumentParams) {
    self.client
      .log_message(MessageType::INFO, "file saved!")
      .await;
  }

  async fn did_close(&self, _: DidCloseTextDocumentParams) {
    self.client
      .log_message(MessageType::INFO, "file closed!")
      .await;
  }
}

#[tokio::main]
async fn main() {
  async fn start_server(fetcher: CompletionFetcher) {
    tracing_subscriber::fmt().init();
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    #[cfg(feature = "runtime-agnostic")]
    let (stdin, stdout) = (stdin.compat(), stdout.compat_write());


    let (service, socket) = LspService::build(|client| Copilot { client, fetcher })
      .custom_method("getCompletionsCycling", Copilot::get_completions_cycling)
      .custom_method("getCompletions", Copilot::get_completions_cycling)
      .custom_method("TextDocument/completions", Copilot::get_completions_cycling)
      .finish();
    println!("Listening on stdin/stdout");
    Server::new(stdin, stdout, socket).serve(service).await;
  }

  let user_token = reader::read_config();
  let copilot_token = auth::get_copilot_token(&user_token).await.unwrap();
  let builder = auth::get_request_builder(&copilot_token).unwrap();
  let fetcher = completions::CompletionFetcher::new(builder);
  start_server(fetcher).await
}

fn get_test_request() -> completions::CompletionRequest {
  let params = completions::CompletionParams {
    language: "javascript".to_string(),
    next_indent: 0,
    trim_by_indentation: true,
    prompt_tokens: 19,
    suffix_tokens: 1
  };

  completions::CompletionRequest {
    prompt: "// Path: app/my_file.js\nfunction fetch_tweet() {\nva".to_string(),
    suffix: "}".to_string(),
    max_tokens: 500,
    temperature: 0,
    top_p: 1,
    n: 1,
    stop: ["\n".to_string()].to_vec(),
    nwo: "my_org/my_repo".to_string(),
    stream: true,
    extra: params
  }
}
