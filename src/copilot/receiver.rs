
use serde_derive::{Deserialize};


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
