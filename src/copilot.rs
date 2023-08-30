use futures_util::{StreamExt, FutureExt};
use eventsource_stream::{Eventsource};
use tower_lsp::lsp_types::*;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CopilotCyclingCompletion {
  pub display_text: String, // partial text
  pub text: String, // fulltext
  pub range: Range, // start char always 0
  pub position: Position,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CopilotAnswer {
  pub id: Option<String>,
  pub model: String,
  pub created: u128,
  pub choices: Vec<Choices>
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CopilotResponse {
  Answer(CopilotAnswer),
  Done,
  Error(String)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Choices {
  pub text: String,
  pub index: i16,
  pub finish_reason: Option<String>,
  pub logprobs: Option<String>,
}

pub async fn on_cancel() -> CopilotCompletionResponse {
  CopilotCompletionResponse {
    cancellation_reason: Some("RequestCancelled".to_string()),
    completions: vec![]
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CopilotCompletionResponse {
  pub completions: Vec<CopilotCyclingCompletion>,
  pub cancellation_reason: Option<String>,
}

impl CopilotCompletionResponse {
  pub fn from_str_vec(str_vec: Vec<String>, line_before: String, pos: Position) -> Self {
    let completions = str_vec.iter().map(|x| {
      return CopilotCyclingCompletion::new(x.to_string(), line_before.to_string(), pos.clone());
    }).collect();
    Self { completions, cancellation_reason: None }
  }
}

fn handle_event(
  event: eventsource_stream::Event
) -> CopilotResponse {
  if event.data == "[DONE]" {
    return CopilotResponse::Done;
  }
  match serde_json::from_str(&event.data) {
    Ok(data) => { CopilotResponse::Answer(data) }
    Err(e) => { CopilotResponse::Error(e.to_string()) }
  }
}

impl CopilotCyclingCompletion {
  pub fn new(text: String, line_before: String, position: Position) -> Self {
    let display_text = text.clone();
    let text = format!("{}{}", line_before, text);
    let end_char = text.find('\n').unwrap_or(text.len()) as u32;
    Self {
      display_text, // partial text
      text, // fulltext
      range: Range {
        start: Position { character: 0, line: position.line },
        end: Position { character: end_char, line: position.line }
      }, // start char always 0
      position,
    }
  }
}

fn create_item(
  text: String,
  line_before: &String,
  position: Position
) -> CopilotCyclingCompletion {
  let display_text = text.clone();
  let text = format!("{}{}", line_before, text);
  let end_char = text.find('\n').unwrap_or(text.len()) as u32;
  CopilotCyclingCompletion {
    display_text, // partial text
    text, // fulltext
    range: Range {
      start: Position {
        character: 0,
        line: position.line,
      },
      end: Position {
        character: end_char,
        line: position.line,
      }
    }, // start char always 0
    position,
  }
}

pub async fn fetch_completions(
  resp: reqwest::Response,
  line_before: String,
  position: Position,
) -> Result<CopilotCompletionResponse, String> {
  let mut stream = resp.bytes_stream().eventsource();
  let mut completion_list = Vec::<CopilotCyclingCompletion>::with_capacity(4);
  let mut s = "".to_string();
  let mut cancellation_reason = None;
  while let Some(event) = stream.next().await {
    match handle_event(event.unwrap()) {
      CopilotResponse::Answer(ans) => {
        ans.choices.iter().for_each(|x| {
          s.push_str(&x.text);
          if x.finish_reason.is_some() {
            let item = create_item(s.clone(), &line_before, position);
            completion_list.push(item);
            s = "".to_string();
          }
        });
      },
      CopilotResponse::Done => { break; },
      CopilotResponse::Error(e) => { cancellation_reason = Some(e) }
    }
  }
  Ok(
    CopilotCompletionResponse {
      cancellation_reason,
      completions: completion_list
    }
  )
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LanguageEntry {
  language_id: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EditorConfiguration {
  disabled_languages: Vec<LanguageEntry>,
  enable_auto_completions: bool,
}

impl EditorConfiguration {
  pub fn default() -> Self {
    Self {
      disabled_languages: vec![], 
      enable_auto_completions: true,
    }
  }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EditorInfo {
  name: String,
  version: String
}

impl EditorInfo {
  pub fn default() -> Self {
    Self { name: "".to_string(), version: "".to_string() }
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CopilotEditorInfo {
  editor_configuration: EditorConfiguration,
  editor_info: EditorInfo,
  editor_plugin_info: EditorInfo,
}

impl CopilotEditorInfo {
  pub fn default() -> Self {
    Self {
      editor_configuration: EditorConfiguration::default(),
      editor_info: EditorInfo::default(),
      editor_plugin_info: EditorInfo::default()
    }
  }
}

  // editorConfiguration = {
  //   disabledLanguages = { {
  //       languageId = "."
  //     }, {
  //       languageId = "cvs"
  //     }, {
  //       languageId = "gitcommit"
  //     }, {
  //       languageId = "gitrebase"
  //     }, {
  //       languageId = "help"
  //     }, {
  //       languageId = "hgcommit"
  //     }, {
  //       languageId = "markdown"
  //     }, {
  //       languageId = "svn"
  //     }, {
  //       languageId = "yaml"
  //     } },
  //   enableAutoCompletions = true
  // },
  // editorInfo = {
  //   name = "Neovim",
  //   version = "0.10.0-dev-481+g0329f5c2f + Node.js 18.15.0"
  // },
  // editorPluginInfo = {
  //   name = "copilot.lua",
  //   version = "1.10.1"
  // }
