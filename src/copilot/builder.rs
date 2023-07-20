use uuid::Uuid;
use reqwest::RequestBuilder;
use chrono::Utc;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct CompletionRequest {
  pub prompt: String,
  pub suffix: String,
  pub max_tokens: i32,
  pub temperature: i8,
  pub top_p: i8,
  pub n: i16,
  pub stop: Vec<String>,
  pub nwo: String,
  pub stream: bool,
  pub extra: CompletionParams
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CompletionParams {
  pub language: String,
  pub next_indent: i8,
  pub trim_by_indentation: bool,
  pub prompt_tokens: i32,
  pub suffix_tokens: i32
}

#[derive(Debug)]
pub struct CopilotRequestBuilder { builder: RequestBuilder }

impl CopilotRequestBuilder {
  pub fn test_request(&self) -> CompletionRequest {
    let params = CompletionParams {
      language: "".to_string(),
      next_indent: 0,
      trim_by_indentation: true,
      prompt_tokens: 19,
      suffix_tokens: 1
    };

    CompletionRequest {
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

  pub fn build_request(&self, data: CompletionRequest) -> RequestBuilder {
    let body = serde_json::to_string(&data).unwrap();
    let request_builder = self.builder.try_clone().unwrap();
    request_builder
      .header("X-Request-Id", Uuid::new_v4().to_string())
      .header("VScode-SessionId", Uuid::new_v4().to_string() + &Utc::now().timestamp().to_string())
      .body(body)
  }

  pub fn new(copilot_token: &String, machine_id: &String) -> Self {
    let completions_url = "https://copilot-proxy.githubusercontent.com/v1/engines/copilot-codex/completions";
    let client = reqwest::Client::new();
    let builder = client.post(completions_url)
      .bearer_auth(copilot_token)
      .header("Openai-Organization", "github-copilot")
      .header("VScode-MachineId", machine_id)
      .header("Editor-Version", "JetBrains-IC/231.9011.34")
      .header("Editor-Plugin-Version", "copilot-intellij/1.2.8.2631")
      .header("OpenAI-Intent", "copilot-ghost");
    Self { builder }
  }
}
