mod reader;
mod completions;

#[tokio::main]
async fn main() {
  let (user, user_token) = reader::read_config();
  let copilot_token = completions::get_copilot_token(&user_token).await.unwrap();
  let builder = completions::build_headers(&copilot_token).unwrap();
  completions::stream_completions(builder).await;
}
