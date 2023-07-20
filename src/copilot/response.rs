use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug,)]
pub struct CopilotCompletionItem {
  display_text: String,
  doc_version: u32,
  position: tower_lsp::lsp_types::Position,
  range: tower_lsp::lsp_types::Range,
  text: String,
  uuid: String,
}

impl CopilotCompletionItem {
  pub fn new(completion_text: &String, prefix: &String, suffix: &String, pos: tower_lsp::lsp_types::Position) -> Self {
    let display_text = format!("{}{}{}", prefix, completion_text, suffix);
    let text = format!("{}{}", prefix, completion_text);
    let line_contents: Vec<String> = text.split(&['\n', '\r']).map(str::to_string).collect();

    let end_line_offset = line_contents.len() - 1;
    let end_char_offset = if end_line_offset != 0 { line_contents.get(0).unwrap().len() } else {0};
    let end = tower_lsp::lsp_types::Position::new(end_line_offset as u32, end_char_offset as u32);
    let range = tower_lsp::lsp_types::Range {
      start: pos,
      end
    };
    let uuid = Uuid::new_v4().to_string();
    let doc_version = 9;
    Self { display_text, doc_version, position: pos, range, text, uuid }
  }
}
