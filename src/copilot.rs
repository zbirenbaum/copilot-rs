use dashmap::DashMap;
use futures_util::{StreamExt, FutureExt};
use eventsource_stream::{Eventsource, EventStreamError, EventStream};
use tower_lsp::lsp_types::*;
use serde_derive::{Deserialize, Serialize};
use std::time::Instant;
use cancellation::{CancellationToken, CancellationTokenSource, OperationCanceled};
use std::sync::Arc;

pub struct ResponseResult {
  pub ct: CancellationToken,
  pub result: Option<CopilotCompletionResponse>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotCyclingCompletion {
  display_text: String, // partial text
  text: String, // fulltext
  range: Range, // start char always 0
  position: Position,
}

#[derive(Deserialize, Debug)]
pub struct CopilotAnswer {
  pub id: Option<String>,
  pub model: String,
  pub created: u128,
  pub choices: Vec<Choices>
}

#[derive(Deserialize, Debug)]
pub enum CopilotResponse {
  Answer(CopilotAnswer),
  Done,
  Error(String)
}

#[derive(Deserialize, Debug)]
pub struct Choices {
  pub text: String,
  pub index: i16,
  pub finish_reason: Option<String>,
  pub logprobs: Option<String>,
}

pub async fn on_cancel() -> CopilotCompletionResponse {
  CopilotCompletionResponse {
    cancellation_reason: Some("RequestCancelled".to_string()),
    completions: vec![]
  }
}


#[derive(Debug, Serialize)]
pub struct CopilotCompletionResponse {
  completions: Vec<CopilotCyclingCompletion>,
  cancellation_reason: Option<String>,
}

fn handle_event(
  event: eventsource_stream::Event
) -> CopilotResponse {
  if event.data == "[DONE]" {
    return CopilotResponse::Done;
  }
  match serde_json::from_str(&event.data) {
    Ok(data) => { CopilotResponse::Answer(data) }
    Err(e) => { CopilotResponse::Error(e.to_string()) }
  }
}
fn create_item(
  text: String,
  line_before: &String,
  position: Position
) -> CopilotCyclingCompletion {
  let display_text = text.clone();
  let text = format!("{}{}", line_before, text);
  let end_char = text.find('\n').unwrap_or(text.len()) as u32;
  CopilotCyclingCompletion {
    display_text, // partial text
    text, // fulltext
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
  }
}

pub async fn fetch_completions(
  resp: reqwest::Response,
  line_before: String,
  position: Position,
  // pending: Arc<DashMap<i32, ResponseResult>>,
  // ct: CancellationToken
) -> Result<CopilotCompletionResponse, String> {
  let mut stream = resp.bytes_stream().eventsource();
  let mut completion_list = Vec::<CopilotCyclingCompletion>::with_capacity(4);
  let mut s = "".to_string();
  let mut cancellation_reason = None;
  while let Some(event) = stream.next().await {
    match handle_event(event.unwrap()) {
      CopilotResponse::Answer(ans) => {
        ans.choices.iter().for_each(|x| {
          s.push_str(&x.text);
          if x.finish_reason.is_some() {
            let item = create_item(s.clone(), &line_before, position);
            completion_list.push(item);
            s = "".to_string();
          }
        });
      },
      CopilotResponse::Done => { break; },
      CopilotResponse::Error(e) => { cancellation_reason = Some(e) }
    }
  }
  Ok(
    CopilotCompletionResponse {
      cancellation_reason,
      completions: completion_list
    }
  )
}
