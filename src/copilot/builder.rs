use uuid::Uuid;
use reqwest::RequestBuilder;
use chrono::Utc;
use serde_derive::{Deserialize, Serialize};
use reqwest::{Result, Response};

use super::auth;

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


#[derive(Debug)]
pub struct CopilotRequestBuilder {
  authenticator: auth::CopilotAuthenticator,
}

impl CopilotRequestBuilder {
  pub async fn new() -> Self {
    let authenticator = auth::CopilotAuthenticator::new().await;
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
    let extra = CopilotCompletionParams {
      language: language.to_string(),
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
}
