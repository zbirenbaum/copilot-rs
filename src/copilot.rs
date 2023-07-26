use reqwest::RequestBuilder;
use ropey::Rope;
use futures_util::StreamExt;
use eventsource_stream::Eventsource;
use tower_lsp::lsp_types::*;
use chrono::Utc;
use uuid::Uuid;
use crate::parse::{get_line_before, get_text_after, get_text_before, position_to_offset};
use crate::auth::CopilotAuthenticator;
use tokio::time::timeout;
use std::time::Duration;
use serde_derive::{Deserialize, Serialize};

impl CopilotHandler {
  pub fn new(authenticator: CopilotAuthenticator) -> Self {
    Self {
      authenticator
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

  pub async fn stream_completions(
    &self,
    language: &String,
    params: &CompletionParams,
    rope: &Rope,
    _client: &tower_lsp::Client
  ) -> Result<Vec<CompletionItem>, String> {
    let offset = position_to_offset(params.text_document_position.position, rope).unwrap();
    let prefix = get_text_before(offset, rope).unwrap();
    let _prompt = format!(
      "// Path: {}\n{}",
      params.text_document_position.text_document.uri,
      prefix.to_string()
    );
    let suffix = get_text_after(offset, rope).unwrap();
    let req = self.build_request(language, &prefix, &suffix);
    let line_before = get_line_before(params.text_document_position.position, rope).unwrap();

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

