use dashmap::DashMap;
use std::{sync::{Arc, atomic::{Ordering, AtomicBool, AtomicU16}, RwLock}, collections::HashMap};
use copilot_rs::{backend::Backend, auth};
use tower_lsp::{LspService, Server};
use reqwest::{header::{HeaderMap, HeaderValue}, ClientBuilder};
use tower::{Layer, Service};
use std::sync::{Mutex, Condvar};

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


  let (service, socket) = LspService::build(|client|
   Backend {
    client,
    documents: Arc::new(RwLock::new(HashMap::new())),
    http_client: Arc::new(http_client),
    current_dispatch: None,
    }
  ).custom_method("getCompletionsCycling", Backend::get_completions_cycling)
    .finish();
  Server::new(stdin, stdout, socket)
    .serve(service)
    .await;
  // tracing_subscriber::fmt().init();
}
