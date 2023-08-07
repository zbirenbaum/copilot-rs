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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]

pub struct CopilotCyclingCompletion {
  display_text: String, // partial text
  text: String, // fulltext
  doc_version: i32,
  range: Range, // start char always 0
  position: Position,
}

#[derive(Debug, Serialize)]
enum CancellationReason { RequestCancelled, }
impl CancellationReason {
  fn as_str(&self) -> &'static str { match self { CancellationReason::RequestCancelled => "RequestCancelled" } }
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotCompletionResponse {
  completions: Vec<CopilotCyclingCompletion>,
  cancellation_reason: Option<CancellationReason>,
}

pub async fn fetch_completions(
  resp: reqwest::Response,
  line_before: String,
  position: Position,
  doc_version: i32,
) -> Result<CopilotCompletionResponse, String> {
  let mut stream = resp
    .bytes_stream()
    .eventsource();

  // let mut completion_list: Vec<CompletionItem> = Vec::with_capacity(v.len());
  let mut v = Vec::<String>::new();
  v.push("".to_string());
  let mut completion_list = Vec::<CopilotCyclingCompletion>::new();

  let timeout = Instant::now();
  let mut idx = 0;

  while let Some(event) = stream.next().await {
    if timeout.elapsed().as_millis() >= 500 {
      return Err("timeout".to_string());
    }
    let e = event.unwrap();
    let data = e.data;
    let event_type = e.event;
    if data.eq("[DONE]") {
      v.iter().for_each(|s| {
        // let preview = format!("{}{}", line_before.to_string().trim_start(), s);
        let display_text = s.clone();
        let text = format!("{}{}", line_before.to_string(), s);
        let end_char = text.find('\n').unwrap_or(text.len()) as u32;
        let item = CopilotCyclingCompletion {
          display_text, // partial text
          text, // fulltext
          doc_version,
          range: Range {
            start: Position {
              character: 0,
              line: position.line,
            },
            end: Position {
              character: end_char,
              line: position.line,
            }
          }, // start char always 0
          position,
        };
        completion_list.push(item);
      });
      break;
    }
    let copilot_resp_data: Result<CopilotResponse, serde_json::error::Error> = serde_json::from_str(&data);
    match copilot_resp_data {
      Ok(r) => {
        let choices = r.choices;
        choices.iter().for_each(|x| {
          match v.get_mut(idx) {
            Some(val) => {
              val.push_str(&x.text.to_string());
              if x.finish_reason.is_some() { idx += 1; }
            }
            None => { v.push(x.text.to_string()); }
          }
        });
      },
      Err(e) => { return Err(e.to_string()); }
    }
  }
  Ok(CopilotCompletionResponse { cancellation_reason: None, completions: completion_list })
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

