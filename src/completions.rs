use futures_util::stream::StreamExt;
use eventsource_stream::Eventsource;
use reqwest::{Client, RequestBuilder, Url};
use chrono::Utc;
use uuid::{uuid, Uuid};
use github_device_flow::authorize;
use github_device_flow::{Credential, DeviceFlowError};
use serde_derive::{Deserialize, Serialize};
use interfaces::Interface;
use std::process;
use sha256::digest;

pub fn get_machine_id() -> String {
  fn try_get_mac_addr() -> Result<String,()> {
    fn validate_interface(itf: &Interface) -> bool {
      let exclude = vec!["00:00:00:00:00:00", "ff:ff:ff:ff:ff:ff", "ac:de:48:00:11:22"];
      return itf.is_running() &&
        !exclude.iter().any(|&s| s == itf.hardware_addr()
          .unwrap()
          .as_string()
        );
    }
    let ifaces = Interface::get_all().unwrap();
    let itf = ifaces.iter().find(|&itf| validate_interface(itf));
    match itf {
      Some(x) => Ok(
        x.hardware_addr()
          .unwrap()
          .to_string()),
      None => Err(())
    }
  }
  let mac_addr = try_get_mac_addr();
  if mac_addr.is_err() {
    eprintln!("Problem finding hardware address");
    process::exit(1);
  }
  digest(mac_addr.unwrap())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CopilotTokenGrant {
  chat_enabled: bool,
  code_quote_enabled: bool,
  copilotignore_enabled: bool,
  expires_at: i128,
  public_suggestions: String,
  refresh_in: f64,
  sku: String,
  telemetry: String,
  token: String,
  tracking_id: String
}

pub async fn get_copilot_token(user_token: &String) -> Result<String, reqwest::Error> {
  println!("Getting copilot token");
  let url = "https://api.github.com/copilot_internal/v2/token".to_string();
  let auth_data = format!("token {}", user_token);
  let client: reqwest::Client = reqwest::Client::new();
  let res = client.get(url)
    .bearer_auth(user_token)
    .header("editor-plugin-version", "copilot-intellij/1.2.8.2631")
    .header("editor-version", "JetBrains-IC/231.9011.34")
    .header("User-Agent", "Rust")
    .send().await?;
  let token_grant = res.json::<CopilotTokenGrant>().await.unwrap();
  Ok(token_grant.token)
}

pub fn device_auth() -> Result<Credential, DeviceFlowError> {
  authorize(
    "Iv1.b507a08c87ecfe98".to_string(),
    None
  )
}

pub fn build_headers(copilot_token: &String) -> Result<RequestBuilder, ()> {
  let completions_url = "https://copilot-proxy.githubusercontent.com/v1/engines/copilot-codex/completions";
  let body = r#"{
    "prompt": "// Path: app/my_file.js\nfunction fetch_tweet() {\nva",
    "suffix": "}",
    "max_tokens": 500,
    "temperature": 0,
    "top_p": 1,
    "n": 1,
    "stop": [
       "\n"
    ],
    "nwo": "my_org/my_repo",
    "stream": true,
    "extra": {
        "language": "javascript",
        "next_indent": 0,
        "trim_by_indentation": true,
        "prompt_tokens": 19,
        "suffix_tokens": 1
    }
  }"#;
  let client = reqwest::Client::new();
  let builder = client.post(completions_url)
    .bearer_auth(copilot_token)
    .header("X-Request-Id", Uuid::new_v4().to_string())
    .header("Openai-Organization", "github-copilot")
    .header("VScode-SessionId", Uuid::new_v4().to_string() + &Utc::now().timestamp().to_string())
    .header("VScode-MachineId", get_machine_id())
    .header("Editor-Version", "JetBrains-IC/231.9011.34")
    .header("Editor-Plugin-Version", "copilot-intellij/1.2.8.2631")
    .header("OpenAI-Intent", "copilot-ghost")
    .body(body);
  Ok(builder)
}
//
pub async fn stream_completions(builder: RequestBuilder) {
  let mut stream = builder.send()
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
