use github_device_flow::authorize;
use serde::{Deserialize, Serialize};
use std::fs;

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
