use reqwest::RequestBuilder;
use chrono::Utc;
use uuid::Uuid;
use serde_derive::{Deserialize, Serialize};

pub struct CompletionFetcher { builder: RequestBuilder }

impl CompletionFetcher {
  pub fn request(&self, data: CompletionRequest) -> RequestBuilder {
    let body = serde_json::to_string(&data).unwrap();
    let request_builder = self.builder.try_clone().unwrap();
    request_builder
      .header("X-Request-Id", Uuid::new_v4().to_string())
      .header("VScode-SessionId", Uuid::new_v4().to_string() + &Utc::now().timestamp().to_string())
      .body(body)
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

#[derive(Deserialize, Debug, PartialEq)]
struct Foo {
    #[serde(deserialize_with = "object_empty_as_none")]
    bar: Option<Bar>,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Bar {
    inner: u32,
}

pub fn object_empty_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
  D: serde::Deserializer<'de>, for<'a> T: serde::Deserialize<'a>, {
  #[derive(Deserialize, Debug)]
  #[serde(deny_unknown_fields)]
  struct Empty {}

  #[derive(Deserialize, Debug)]
  #[serde(untagged)]
  enum Aux<T> {
    T(T),
    Empty(Empty),
    Null,
  }

  match serde::Deserialize::deserialize(deserializer)? {
    Aux::T(t) => Ok(Some(t)),
    Aux::Empty(_) | Aux::Null => Ok(None),
  }
}
//
#[derive(Deserialize, Debug)]
pub struct CopilotResponse {
  pub id: String,
  pub model: String,
  pub created: u128,
  pub choices: Vec<Choices>
}

#[derive(Deserialize, Debug)]
pub struct Choices {
  pub text: String,
  pub index: i16,
  #[serde(deserialize_with = "object_empty_as_none")]
  pub finish_reason: Option<String>,
  #[serde(deserialize_with = "object_empty_as_none")]
  pub logprobs: Option<String>,
}
// {"id":"cmpl-7d9WF6gEkeqXnbmMaATVx1EZxLX7h","model":"cushman-ml","created":1689565739,"choices":[{"text":"r","index":0,"finish_reason":null,"logprobs":null}]}

