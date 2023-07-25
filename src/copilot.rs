use reqwest::RequestBuilder;
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
use std::time::Duration;
use serde_derive::Deserialize;

#[derive(Debug)]
pub struct CopilotHandler {
  builder: CopilotRequestBuilder,
}

#[derive(Deserialize, Debug)]
pub struct CopilotResponse {
  pub id: Option<String>,
  pub model: String,
  pub created: u128,
  pub choices: Vec<Choices>
}

#[derive(Deserialize, Debug)]
pub struct Choices {
  pub text: String,
  pub index: i16,
  pub finish_reason: Option<String>,
  pub logprobs: Option<String>,
}

impl CopilotHandler {
  pub async fn new() -> Self {
    Self {
      builder: builder::CopilotRequestBuilder::new().await
    }
  }
  pub async fn stream_completions(&self, language: &String, params: &CompletionParams, rope: &Rope, _client: &tower_lsp::Client) -> Result<Vec<CompletionItem>, String> {
    let offset = position_to_offset(params.text_document_position.position, rope).unwrap();
    let prefix = parse::get_text_before(offset, rope).unwrap();
    let _prompt = format!(
      "// Path: {}\n{}",
      params.text_document_position.text_document.uri,
      prefix.to_string()
    );
    let suffix = parse::get_text_after(offset, rope).unwrap();
    let req = self.builder.build_request(language, &prefix, &suffix);
    let line_before = parse::get_line_before(params.text_document_position.position, rope).unwrap();

    let resp = req.send().await;
    match resp {
      Ok(r) => {
        _client.log_message(MessageType::ERROR, &r.status()).await;
        let mut stream = r
          .bytes_stream()
          .eventsource();
        let mut idx = 0;
        let mut v: Vec<String> = vec!["".to_string()];

        while let Some(e) = stream.next().await {
          let data = e.unwrap().data;
          if data == "[DONE]" { break }
          let copilot_resp_data: CopilotResponse = serde_json::from_str(&data).unwrap();
          let choices = copilot_resp_data.choices.as_slice();
          choices.iter().for_each(|x| {
            if v.len() <= idx { v.push("".to_string()); }
            v.get_mut(idx).unwrap().push_str(&x.text.to_string());
            if x.finish_reason.is_some() { idx += 1; }
          });
        }
        // _client.log_message(MessageType::ERROR, &s).await;
        let mut completion_list: Vec<CompletionItem> = Vec::with_capacity(v.len());
        v.iter().for_each(|s| {
          let preview = format!("{}{}", line_before.trim_start(), s);
          completion_list.push(CompletionItem {
            label: preview.to_string(),
            filter_text: Some(preview),
            insert_text: Some(s.to_string()),
            kind: Some(CompletionItemKind::TEXT),
            ..Default::default()
          })
        });
        Ok(completion_list)
      },
      Err(e) => {
        _client.log_message(MessageType::ERROR, e.to_string()).await;
        Err(e.to_string())
      }
    }
  }
}

