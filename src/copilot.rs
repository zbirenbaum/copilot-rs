use reqwest::RequestBuilder;
use ropey::Rope;
use futures_util::StreamExt;
use eventsource_stream::Eventsource;
use tower_lsp::lsp_types::*;
use chrono::{Utc, TimeZone};
use uuid::{Uuid, timestamp, Timestamp};
use crate::parse::{get_line_before, get_text_after, get_text_before, position_to_offset};
use tokio::time::timeout;
use serde_derive::{Deserialize, Serialize};
use std::process::exit;
use tokio::runtime::Handle;
use std::time::{Duration, Instant};

pub async fn fetch_completions(
  resp: reqwest::Response,
  line_before: String
) -> Result<Vec<CompletionItem>, String> {
  let mut idx = 0;
  let mut v: Vec<String> = vec!["".to_string()];
  let mut stream = resp
    .bytes_stream()
    .eventsource();

  let mut completion_list: Vec<CompletionItem> = Vec::with_capacity(v.len());

  let timeout = Instant::now();
  while let Some(event) = stream.next().await {
    let e = event.unwrap();
    let data = e.data;
    if data.eq("[DONE]") {
      v.iter().for_each(|s| {
        let preview = format!("{}{}", line_before.to_string().trim_start(), s);
        let filter = format!("{}{}", line_before.to_string(), s);
        completion_list.push(CompletionItem {
          label: preview.to_string(),
          filter_text: Some(filter),
          insert_text: Some(s.to_string()),
          kind: Some(CompletionItemKind::TEXT),
          ..Default::default()
        })
      });
      break;
    }
    let copilot_resp_data: Result<CopilotResponse, serde_json::error::Error> = serde_json::from_str(&data);
    match copilot_resp_data {
      Ok(r) => {
        let choices = r.choices;
        choices.iter().for_each(|x| {
          if v.len() <= idx { v.push("".to_string()); }
          v.get_mut(idx).unwrap().push_str(&x.text.to_string());
          if x.finish_reason.is_some() { idx += 1; }
        });
      },
      Err(e) => { return Err(e.to_string()); }
    }
  }
  Ok(completion_list)
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


#[derive(Serialize, Deserialize, Debug)]
pub struct CopilotCompletionRequest {
  pub prompt: String,
  pub suffix: String,
  pub max_tokens: i32,
  pub temperature: f32,
  pub top_p: f32,
  pub n: i16,
  pub stop: Vec<String>,
  pub nwo: String,
  pub stream: bool,
  pub extra: CopilotCompletionParams
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CopilotCompletionParams {
  pub language: String,
  pub next_indent: i8,
  pub trim_by_indentation: bool,
  pub prompt_tokens: i32,
  pub suffix_tokens: i32
}

