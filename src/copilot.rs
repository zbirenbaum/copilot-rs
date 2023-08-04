use reqwest::RequestBuilder;
use ropey::Rope;
use futures_util::StreamExt;
use eventsource_stream::Eventsource;
use tower_lsp::lsp_types::*;
use chrono::{Utc, TimeZone};
use uuid::{Uuid, timestamp, Timestamp};
use crate::parse::{get_line_before, get_text_after, get_text_before, position_to_offset};
use crate::auth::CopilotAuthenticator;
use tokio::time::timeout;
use serde_derive::{Deserialize, Serialize};
use std::process::exit;
use tokio::runtime::Handle;
use std::time::{Duration, Instant};

impl CopilotHandler {
  pub fn new(authenticator: CopilotAuthenticator) -> Self {
    Self {
      authenticator,
    }
  }

  fn build_request_headers(&self) -> RequestBuilder {
    let machine_id = self.authenticator.get_machine_id().to_string();
    let completions_url = "https://copilot-proxy.githubusercontent.com/v1/engines/copilot-codex/completions";
    let client = reqwest::Client::new();
    client.post(completions_url)
      .bearer_auth(self.authenticator.get_token())
      .header("Openai-Organization", "github-copilot")
      .header("VScode-MachineId", machine_id)
      .header("Editor-Version", "JetBrains-IC/231.9011.34")
      .header("Editor-Plugin-Version", "copilot-intellij/1.2.8.2631")
      .header("OpenAI-Intent", "copilot-ghost")
      .header("X-Request-Id", Uuid::new_v4().to_string())
      .header("VScode-SessionId", Uuid::new_v4().to_string() + &Utc::now().timestamp().to_string())
  }

  fn build_request_body(&self, language: &String, prompt: &String, suffix: &String) -> Option<CopilotCompletionRequest> {
    let extra = CopilotCompletionParams { language: language.to_string(),
      next_indent: 0,
      trim_by_indentation: true,
      prompt_tokens: prompt.len() as i32,
      suffix_tokens: suffix.len() as i32
    };
    Some(CopilotCompletionRequest {
      prompt: prompt.to_string(),
      suffix: suffix.to_string(),
      max_tokens: 500,
      temperature: 1.0,
      top_p: 1.0,
      n: 3,
      stop: ["unset".to_string()].to_vec(),
      nwo: "my_org/my_repo".to_string(),
      stream: true,
      extra
    })
  }

  pub fn build_request(
    &self,
    language: &String,
    prompt: &String,
    suffix: &String
  ) -> RequestBuilder {
    let builder = self.build_request_headers();
    let data = self.build_request_body(language, prompt, suffix).unwrap();
    let body = serde_json::to_string(&data).unwrap();
    builder.body(body)
  }


  pub async fn fetch_completions(
    &self,
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
      if timeout.elapsed().as_millis() >= 1000 {
        return Err("timeout".to_string());
      }
      let e = event.unwrap();
      let data = e.data;
      let event_type = e.event;
      if data.eq("[DONE]") {
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
}

#[derive(Debug)]
pub struct CopilotHandler {
  authenticator: CopilotAuthenticator,
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
struct CopilotCompletionRequest {
  prompt: String,
  suffix: String,
  max_tokens: i32,
  temperature: f32,
  top_p: f32,
  n: i16,
  stop: Vec<String>,
  nwo: String,
  stream: bool,
  extra: CopilotCompletionParams
}

#[derive(Serialize, Deserialize, Debug)]
struct CopilotCompletionParams {
  language: String,
  next_indent: i8,
  trim_by_indentation: bool,
  prompt_tokens: i32,
  suffix_tokens: i32
}

