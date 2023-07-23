use serde::{Deserialize, Serialize};
use ropey::Rope;
use futures_util::StreamExt;
use eventsource_stream::Eventsource;
use tower_lsp::lsp_types::*;
use builder::CopilotRequestBuilder;
mod receiver;
use tower_lsp::lsp_types::CompletionItem;
use self::builder::CompletionRequest;
mod builder;
mod auth;

// mod auth;
// mod util;
// use serde_json::Value;
// use serde_derive::{Deserialize, Serialize};
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TextDocument {
  relative_path: String,
  language_id: String,
  uri: String,
  version: i16,
}

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

// fn offset_to_position(offset: usize, rope: &Rope) -> Option<Position> {
//   let line = rope.try_char_to_line(offset).ok()?;
//   let first_char_of_line = rope.try_line_to_char(line).ok()?;
//   let column = offset - first_char_of_line;
//   Some(Position::new(line as u32, column as u32))
// }

fn position_to_offset(position: Position, rope: &Rope) -> Option<usize> {
  Some(rope.try_line_to_char(position.line as usize).ok()? + position.character as usize)
}

fn on_receive_cb (data: &str) -> String {
  let resp: receiver::CopilotResponse = serde_json::from_str(data).unwrap();
  resp.choices.iter().map(|x| {
    x.text.to_string()
  }).collect::<Vec<_>>().join("")
}

fn get_prompt(pos: usize, rope: &Rope) -> String {
  if pos == 0 { return "".to_string() }
  rope.slice(0..pos).to_string()
}

fn get_suffix(pos: usize, rope: &Rope) -> String {
  let end_idx = rope.len_chars();
  if pos == end_idx { return "".to_string() }
  rope.slice(pos..end_idx).to_string()
}

fn get_params(language: &String, prompt: &String, suffix: &String) -> builder::CompletionParams {
  builder::CompletionParams {
    language: language.to_string(),
    next_indent: 0,
    trim_by_indentation: true,
    prompt_tokens: prompt.len() as i32,
    suffix_tokens: suffix.len() as i32
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


  pub async fn stream_completions(&self, language: String, params: CompletionParams, rope: &Rope, client: &tower_lsp::Client) -> Option<Vec<CompletionItem>> {
    let pos = position_to_offset(params.text_document_position.position, rope).unwrap();
    client.log_message(MessageType::ERROR, "pos").await;
    let prompt = format!(
      "// Path: {}\n{}",
      params.text_document_position.text_document.uri,
      get_prompt(pos, rope)
    );
    client.log_message(MessageType::ERROR, "prompt").await;
    let suffix = get_suffix(pos, rope);
    client.log_message(MessageType::ERROR, "suffix").await;
    let _params = get_params(&language, &prompt, &suffix);
    client.log_message(MessageType::ERROR, "params").await;
    let text_prefix = (|| {
      let char_offset = params.text_document_position.position.character as usize;
      if char_offset == 0 { return "".to_string(); }
      let line_start = pos-char_offset;
      rope.slice(line_start..pos).to_string()
    })();
    client.log_message(MessageType::ERROR, "prefix").await;
    let data = CompletionRequest {
      prompt,
      suffix,
      max_tokens: 500,
      temperature: 1.0,
      top_p: 1.0,
      n: 1,
      stop: ["unset".to_string()].to_vec(),
      nwo: "my_org/my_repo".to_string(),
      stream: true,
      extra: _params
    };
    client.log_message(MessageType::ERROR, "data").await;
    let req = self.builder.build_request(&data).send().await;
    if req.is_err() {
      client.log_message(MessageType::ERROR, "request error").await;
    }
    match req {
      Ok(req) => {
        let status = format!("Status: {}", req.status());
        client.log_message(MessageType::ERROR, status).await;
        let err = req.error_for_status_ref();
        match err {
          Ok(_res) => {},
          Err(e) => {
            client.log_message(MessageType::ERROR, e).await;
          }
        }
        client.log_message(MessageType::ERROR, req.status()).await;
        let mut stream = req
          .bytes_stream()
          .eventsource();
        let mut responses: Vec<String> = vec![];
        while let Some(event) = stream.next().await {
          match event {
            Ok(event) => {
              if event.data == "[DONE]" { break };
              client.log_message(MessageType::ERROR, &event.data).await;
              let resp: receiver::CopilotResponse = serde_json::from_str(&event.data).unwrap();
              resp.choices.iter().map(|x| {
                x.text.to_string()
              }).collect::<Vec<_>>().join("");
              responses.push(on_receive_cb(&event.data));
            },
            Err(e) =>{
              let err_str = format!("{}{:?}", "Request Error: ", &e);
              client.log_message(MessageType::ERROR, err_str).await;
            }
          }
        }

        let _prompt = data.prompt.to_string();
        let res = responses.join("");
        // let mut full = text_prefix.to_string();
        // full.push_str(&res.to_string());
        let prefix_without_space = text_prefix.trim_start().to_string();
        let num_spaces = text_prefix.len() - prefix_without_space.len();
        let formatted: Vec<String>;
        if res.find('\n').is_some() {
          if num_spaces > 0 {
            let indent = " ".to_string().repeat(num_spaces);
            formatted = res.split('\n').map(|x| {
              x.replacen(&indent, "", 1)
            }).collect();
          }
          else {
            formatted = res.split('\n').map(|x| x.to_string()).collect()
          }
        } else {
          formatted = vec![res];
        }
        // let filter_text = line_splits.join("");
        let res = responses.join("\n");
        let insert_text = formatted.join("\n");
        let label =  format!("{}{}", &prefix_without_space, &insert_text);
        client.log_message(MessageType::ERROR, res.to_string()).await;
        let filter_text = format!("{}{}", text_prefix, res);

        Some(vec![
          CompletionItem {
            label,
            filter_text: Some(filter_text),
            insert_text: Some(insert_text),
            kind: Some(CompletionItemKind::SNIPPET),
            ..Default::default()
          }
        ])
        // let resp = CompletionResponse::Array(completions.completions);
      },
      Err(req) => {
        println!("{:?}", req);
        client.log_message(MessageType::ERROR, req).await;
        None
      }
    }
  }
}

