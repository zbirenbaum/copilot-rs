use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;
use std::sync::Arc;
use reqwest::{RequestBuilder, Client};

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

pub fn build_request(
  http_client: Arc<Client>,
  language: String,
  prompt: String,
  suffix: String
) -> RequestBuilder {
  let extra = CopilotCompletionParams { language: language.to_string(),
    next_indent: 0,
    trim_by_indentation: true,
    prompt_tokens: prompt.len() as i32,
    suffix_tokens: suffix.len() as i32
  };
  let body = Some(CopilotCompletionRequest {
    prompt,
    suffix,
    max_tokens: 500,
    temperature: 1.0,
    top_p: 1.0,
    n: 3,
    stop: ["unset".to_string()].to_vec(),
    nwo: "my_org/my_repo".to_string(),
    stream: true,
    extra
  });
  let body = serde_json::to_string(&body).unwrap();
  let completions_url = "https://copilot-proxy.githubusercontent.com/v1/engines/copilot-codex/completions";
  http_client.post(completions_url)
    .header("X-Request-Id", Uuid::new_v4().to_string())
    .header("VScode-SessionId", Uuid::new_v4().to_string() + &Utc::now().timestamp().to_string())
    .body(body)
}
