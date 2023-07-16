mod reader;
mod completions;
mod auth;
use std::fs::File;
use tower_lsp::jsonrpc::Result;
use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use futures_util::stream::StreamExt;
use reqwest::RequestBuilder;
use eventsource_stream::{Eventsource, EventStream};

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
struct CompletionParams {
  doc: Document,
  position: Position,
  text_document: TextDocument,
}

impl Copilot {
  async fn get_completions_cycling(&self, params: CompletionParams) {
    let data = get_test_request();
    let mut stream = self.fetcher.request(data)
      .send()
      .await.unwrap()
      .bytes_stream()
      .eventsource();
    while let Some(event) = stream.next().await {
      match event {
        Ok(event) => println!(
          "received event[type={}]: {}",
          event.event,
          event.data
        ),
        Err(e) => eprintln!("error occured: {}", e),
      }
    }
  }

}

#[tower_lsp::async_trait]
impl LanguageServer for Copilot {
  async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
    println!("initialize");
    Ok(InitializeResult {
      server_info: None,
      capabilities: ServerCapabilities {
        text_document_sync: None,
        completion_provider: None,
        execute_command_provider: None,
        workspace: None,
        ..ServerCapabilities::default()
      },
    })
  }

  async fn initialized(&self, _: InitializedParams) {
    println!("initialize");
    self.client
      .log_message(MessageType::INFO, "initialized!")
      .await;
  }

  async fn shutdown(&self) -> Result<()> {
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  let user_token = reader::read_config();
  let copilot_token = auth::get_copilot_token(&user_token).await.unwrap();
  let builder = auth::get_request_builder(&copilot_token).unwrap();
  let fetcher = completions::CompletionFetcher::new(builder);

  // #[cfg(feature = "runtime-agnostic")]
  // use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

  // tracing_subscriber::fmt().init();

  let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
  // #[cfg(feature = "runtime-agnostic")]
  // let (stdin, stdout) = (stdin.compat(), stdout.compat_write());

  let (service, socket) = LspService::build(|client| Copilot { client, fetcher })
    .custom_method("getCompletionsCycling", Copilot::get_completions_cycling)
    .finish();
  println!("Listening on stdin/stdout");
  Server::new(stdin, stdout, socket).serve(service).await;

  // let stdin = tokio::io::stdin();
  // let stdout = tokio::io::stdout();

  // let (service, messages) = LspService::builder(|client| Backend { client })
  //   .with_method("custom/request", Backend::custom_request)
  //   .with_method("custom/notification", Backend::custom_notification)
  //   .with_method("custom/noParamsWorksToo", Backend::no_params_works_too)
  //   .finish();
  //
  // Server::new(stdin, stdout)
  //   .interleave(messages)
  //   .serve(service)
  //   .await;
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
