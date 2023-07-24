use serde_derive::{Deserialize, Serialize};

use interfaces::Interface;
use std::{process, ops::Sub};
use sha256::digest;
use github_device_flow::authorize;
use std::fs;
use chrono::{Utc, DateTime, Duration};

#[derive(Serialize, Deserialize, Debug)]
pub struct CopilotTokenGrant {
  chat_enabled: bool,
  code_quote_enabled: bool,
  copilotignore_enabled: bool,
  expires_at: String,
  public_suggestions: String,
  refresh_in: String,
  sku: String,
  telemetry: String,
  token: String,
  tracking_id: String
}


#[derive(Serialize, Deserialize, Debug)]
pub struct CopilotAuthenticator {
  token_grant: CopilotTokenGrant,
  machine_id: String,
  timestamp: DateTime<Utc>
}

impl CopilotAuthenticator {
  pub fn get_token(&self) -> &String { &self.token_grant.token }
  pub fn get_refresh(&self) -> &String { &self.token_grant.refresh_in }

  pub fn get_machine_id(&self) -> &String { &self.machine_id }

  pub async fn new() -> Self {
    let user_token = read_config();
    let token_grant = get_copilot_token(&user_token).await.unwrap();
    let machine_id = get_machine_id();
    Self { token_grant, machine_id, timestamp: Utc::now() }
  }

}
#[derive(Deserialize, Serialize, Debug)]
struct HostsFile {
  github_com: UserCredentials,
}

#[derive(Deserialize, Serialize, Debug)]
struct UserCredentials {
  user: String,
  oauth_token: String,
}

pub fn create_hosts_file(path: &String) {
  let auth_data = authorize("Iv1.b507a08c87ecfe98".to_string(), None);
  println!("Creating hosts file at {}", path);
  println!("Auth data: {:?}", auth_data);
}

pub fn read_config() -> String {
  let fp = format!("{}/.config/github-copilot/hosts.json", std::env::var("HOME").unwrap());
  let exists = std::path::Path::new(&fp).exists();
  if !exists { create_hosts_file(&fp); }
  println!("Exists: {}", exists);

  // if !exists { create_hosts_file(&fp); }
  serde_json::from_str::<HostsFile>(
    &fs::read_to_string(fp)
    .unwrap()
    .replace('.', "_")
  ).unwrap().github_com.oauth_token
}
pub async fn get_copilot_token(user_token: &String) -> Result<CopilotTokenGrant, reqwest::Error> {
  let url = "https://api.github.com/copilot_internal/v2/token".to_string();
  let client: reqwest::Client = reqwest::Client::new();
  let res = client.get(url)
    .bearer_auth(user_token)
    .header("editor-plugin-version", "copilot-intellij/1.2.8.2631")
    .header("editor-version", "JetBrains-IC/231.9011.34")
    .header("User-Agent", "Rust")
    .send().await?;
  let token_grant = res.json::<CopilotTokenGrant>().await.unwrap();
  Ok(token_grant)
}

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

