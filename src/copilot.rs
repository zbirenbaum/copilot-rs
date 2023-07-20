use std::fmt::format;

use serde::{Deserialize, Serialize};
use ropey::Rope;
use futures_util::StreamExt;
use eventsource_stream::Eventsource;
use tower_lsp::lsp_types::*;
use builder::CopilotRequestBuilder;
mod receiver;
use tower_lsp::lsp_types::{CompletionList, CompletionItem};

use self::{receiver::CopilotResponse, builder::CompletionRequest};
mod builder;
mod auth;


// mod auth;
// mod util;
// use serde_json::Value;
// use serde_derive::{Deserialize, Serialize};
//
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TextDocument {
  relative_path: String,
  uri: String,
  version: i16,
}
//
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Document {
  indent_size: i8,
  insert_spaces: bool,
  position: tower_lsp::lsp_types::Position,
  relative_path: String,
  tab_size: i8,
  uri: String,
  version: i8,
}
//
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CopilotCompletionParams {
  doc: Document,
  position: Position,
  text_document: TextDocument,
}

#[derive(Debug)]
pub struct CopilotHandler {
  builder: CopilotRequestBuilder,
}

fn on_receive_cb (vec: &mut Vec<String>, data: &str) {
  let resp: receiver::CopilotResponse = serde_json::from_str(data).unwrap();
  let v = &resp.choices;
  for i in v {
    let item = i.text.to_string();
    // let item = CompletionItem::new_simple(i.text.to_string(), "".to_string());
    vec.push(item);
  }
}

impl CopilotHandler {
  pub async fn new () -> Self {
    let user_token = auth::read_config();
    let copilot_token = auth::get_copilot_token(&user_token).await.unwrap();
    let machine_id = auth::get_machine_id();
    let builder = builder::CopilotRequestBuilder::new(&copilot_token, &machine_id);
    Self { builder }
  }

  pub fn completion_params_to_request(&self, params: CompletionParams, rope: &Rope) -> CompletionRequest {
    let prefix = (|| {
      let start_idx = rope.line_to_char(0);
      let char_pos = rope.line_to_char(params.text_document_position.position.character as usize);
      let end_idx = rope.line_to_char(params.text_document_position.position.line as usize);
      return rope.slice(start_idx..end_idx).to_string()
    })();
    let suffix = (|| {
      let start_char = rope.line_to_char(params.text_document_position.position.character as usize);
      let start_idx = rope.line_to_char(params.text_document_position.position.line as usize);
      let end_idx = rope.len_chars();
      return rope.slice(start_idx+start_char..end_idx).to_string()
    })();
    let _params = builder::CompletionParams {
      language: "lua".to_string(),
      next_indent: 0,
      trim_by_indentation: true,
      prompt_tokens: prefix.len() as i32,
      suffix_tokens: suffix.len() as i32
    };
    // let prompt = params.context; // THIS MAY HAVE LANGUAGE
    let prompt = format!("// Path: {}\n{}", params.text_document_position.text_document.uri, prefix);

    CompletionRequest {
      prompt,
      suffix,
      max_tokens: 500,
      temperature: 0,
      top_p: 1,
      n: 1,
      stop: ["\n".to_string()].to_vec(),
      nwo: "my_org/my_repo".to_string(),
      stream: true,
      extra: _params
    }
  }
  pub async fn stream_completions(&self, data: CompletionRequest) -> Result<Option<Vec<String>>, ()> {
    let mut stream = self.builder.build_request(data)
      .send()
      .await.unwrap()
      .bytes_stream()
      .eventsource();
    let mut choices: Vec<String> = vec![];
    while let Some(event) = stream.next().await{
      match event {
        Ok(event) => {
          if event.data == "[DONE]" { break };
          on_receive_cb(&mut choices, &event.data);
        },
        Err(e) => println!("error occured: {}", e),
      }
    }
    println!("choices: {:?}", choices);
    // let resp = CompletionResponse::Array(completions.completions);
    Ok(Some(choices))
  }
}

