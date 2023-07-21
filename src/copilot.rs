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
  language_id: String,
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

fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
  let line = rope.try_char_to_line(offset).ok()?;
  let first_char_of_line = rope.try_line_to_char(line).ok()?;
  let column = offset - first_char_of_line;
  Some(Position::new(line as u32, column as u32))
}

fn position_to_offset(position: Position, rope: &Rope) -> Option<usize> {
  Some(rope.try_line_to_char(position.line as usize).ok()? + position.character as usize)
}

fn on_receive_cb (data: &str) -> String {
  let resp: receiver::CopilotResponse = serde_json::from_str(data).unwrap();
  resp.choices.iter().map(|x| {
    x.text.to_string()
  }).collect::<Vec<_>>().join("")
}

impl CopilotHandler {
  pub async fn new () -> Self {
    let user_token = auth::read_config();
    let copilot_token = auth::get_copilot_token(&user_token).await.unwrap();
    let machine_id = auth::get_machine_id();
    let builder = builder::CopilotRequestBuilder::new(&copilot_token, &machine_id);
    Self { builder }
  }


  pub async fn stream_completions(&self, language: String, params: CompletionParams, rope: &Rope, client: &tower_lsp::Client) -> Vec<CompletionItem> {
    let pos = position_to_offset(params.text_document_position.position, rope).unwrap();
    let prompt_text = (|| {
      if pos == 0 { return "".to_string() }
      return rope.slice(0..pos).to_string()
    })();

    let suffix = (|| {
      let end_idx = rope.len_chars();
      if pos == end_idx { return "".to_string() }
      return rope.slice(pos..end_idx).to_string()
    })();
    let _params = builder::CompletionParams {
      language,
      next_indent: 0,
      trim_by_indentation: true,
      prompt_tokens: prompt_text.len() as i32,
      suffix_tokens: suffix.len() as i32
    };
    // let prompt = params.context; // THIS MAY HAVE LANGUAGE
    let prompt = format!("// Path: {}\n{}", params.text_document_position.text_document.uri, prompt_text);

    let text_prefix = (|| {
      let char_offset = params.text_document_position.position.character as usize;
      if char_offset == 0 { return "".to_string(); }
      let line_start = pos-char_offset;
      return rope.slice(line_start..pos).to_string()
    })();

    let data = CompletionRequest {
      prompt,
      suffix,
      max_tokens: 500,
      temperature: 1.0,
      top_p: 1.0,
      n: 1,
      stop: ["\n".to_string()].to_vec(),
      nwo: "my_org/my_repo".to_string(),
      stream: true,
      extra: _params
    };
    let mut stream = self.builder.build_request(&data)
      .send()
      .await.unwrap()
      .bytes_stream()
      .eventsource();
    let mut responses: Vec<String> = vec![];
    while let Some(event) = stream.next().await{
      match event {
        Ok(event) => {
          if event.data == "[DONE]" { break };
          responses.push(on_receive_cb(&event.data));
        },
        Err(e) => println!("error occured: {}", e),
      }
    }
    let result = responses.join("");
    client.log_message(MessageType::ERROR, &result).await;
    let prompt = data.prompt.to_string();
    client.log_message(MessageType::ERROR, &result).await;

    let full = format!("{}{}", text_prefix, result.clone());
    client.log_message(MessageType::ERROR, &full).await;
    vec![
      CompletionItem {
        label: full,
        insert_text: Some(result.clone()),
        kind: Some(CompletionItemKind::TEXT),
        ..Default::default()
      }
    ]
    // let resp = CompletionResponse::Array(completions.completions);
  }
}

