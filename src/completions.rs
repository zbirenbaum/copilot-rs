use futures_util::stream::StreamExt;
use eventsource_stream::Eventsource;
use reqwest::RequestBuilder;
use chrono::Utc;
use uuid::Uuid;
use serde_derive::{Deserialize, Serialize};

pub struct CompletionFetcher { builder: RequestBuilder }

impl CompletionFetcher {
  pub async fn request(&self, data: CompletionRequest) {
    let body = serde_json::to_string(&data).unwrap();
    let request_builder = self.builder.try_clone().unwrap();
    let mut stream = request_builder
      .header("X-Request-Id", Uuid::new_v4().to_string())
      .header("VScode-SessionId", Uuid::new_v4().to_string() + &Utc::now().timestamp().to_string())
      .body(body)
      .send()
      .await.unwrap()
      .bytes_stream()
      .eventsource();
    while let Some(event) = stream.next().await {
      match event {
        Ok(event) => println!(
          "received event[type={}]: {}",
          event.event,
          event.data
        ),
        Err(e) => eprintln!("error occured: {}", e),
      }
    }
  }

  pub fn new(builder: RequestBuilder) -> Self {
    // let builder = get_request_builder(&copilot_token).unwrap();
    // let session_id = format!("{}-{}-{}", machine_id, session_id, timestamp);
    Self { builder }
  }
}

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

