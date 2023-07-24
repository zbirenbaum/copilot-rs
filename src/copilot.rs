use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use ropey::Rope;
use futures_util::StreamExt;
use eventsource_stream::Eventsource;
use tower_lsp::lsp_types::*;
use builder::CopilotRequestBuilder;
mod receiver;
mod builder;
mod auth;
mod parse;
use parse::position_to_offset;
use tokio::time::timeout;
use tokio::sync::oneshot;
use std::time::Duration;
// mod auth;
// mod util;
// use serde_json::Value;
#[derive(Debug)]
pub struct CopilotHandler {
  builder: CopilotRequestBuilder,
}

async fn stream_completion_text(req: RequestBuilder) -> Option<String> {
  let resp = req.send().await.unwrap();
  let mut stream = resp
    .bytes_stream()
    .eventsource();
  let mut s = "".to_string();
  while let Some(e) = stream.next().await {
    let data = e.unwrap().data;
    if data == "[DONE]" {
      return Some(s);
    }
    s.push_str(&data)
  }
  None
}

impl CopilotHandler {
  pub async fn new() -> Self {
    Self {
      builder: builder::CopilotRequestBuilder::new().await
    }
  }
  pub async fn stream_completions(&self, language: String, params: CompletionParams, rope: &Rope, client: &tower_lsp::Client) -> Option<Vec<CompletionItem>> {
    let offset = position_to_offset(params.text_document_position.position, rope).unwrap();
    client.log_message(MessageType::ERROR, "pos").await;

    let prefix = parse::get_text_before(offset, rope).unwrap();
    let prompt = format!(
      "// Path: {}\n{}",
      params.text_document_position.text_document.uri,
      prefix.to_string()
    );
    let suffix = parse::get_text_after(offset, rope).unwrap();
    client.log_message(MessageType::ERROR, "prefix").await;
    client.log_message(MessageType::ERROR, "data").await;
    let req = self.builder.build_request(&language, &prefix, &suffix).unwrap();
    let line_before = parse::get_line_before(params.text_document_position.position, rope).unwrap();

    let fut = stream_completion_text(req);
    let res = timeout(Duration::from_millis(500), fut).await;
    match res {
      Ok(s) => {
        let comp = s.unwrap();
        let preview = format!("{}{}", line_before, comp);
        Some(vec![
          CompletionItem {
            label: preview.to_string(),
            filter_text: Some(preview),
            insert_text: Some(comp),
            kind: Some(CompletionItemKind::TEXT),
            ..Default::default()
          }
        ])
      },
      Err(e) => {
        let err_str = format!("Req Timeout: {}", e.to_string());
        client.log_message(MessageType::ERROR, err_str).await;
        None
      }
    }
  }
}

