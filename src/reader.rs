use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct Host {
  github_com: UserCredentials,
}

#[derive(Deserialize)]
struct UserCredentials {
  user: String,
  oauth_token: String,
}

pub fn read_config() -> (String, String) {
  let relpath = "/.config/github-copilot/hosts.json";
  let filepath = std::env::var("HOME").unwrap() + relpath;
  let config_string = fs::read_to_string(filepath)
    .expect("Failed to read hosts.json");
  let parsed =  config_string.replace('.', "_");
  let user_credentials = serde_json::from_str::<Host>(&parsed)
    .unwrap().github_com;
  (user_credentials.user, user_credentials.oauth_token)
}
