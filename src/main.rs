mod reader;
mod completions;
mod auth;

#[tokio::main]
async fn main() {
  let user_token = reader::read_config();
  let copilot_token = auth::get_copilot_token(&user_token).await.unwrap();
  let builder = auth::get_request_builder(&copilot_token).unwrap();
  let fetcher = completions::CompletionFetcher::new(builder);
  fetcher.request(get_test_request()).await;

}

fn get_test_request() -> completions::CompletionRequest {
  let params = completions::CompletionParams {
    language: "javascript".to_string(),
    next_indent: 0,
    trim_by_indentation: true,
    prompt_tokens: 19,
    suffix_tokens: 1
  };

  completions::CompletionRequest {
    prompt: "// Path: app/my_file.js\nfunction fetch_tweet() {\nva".to_string(),
    suffix: "}".to_string(),
    max_tokens: 500,
    temperature: 0,
    top_p: 1,
    n: 1,
    stop: ["\n".to_string()].to_vec(),
    nwo: "my_org/my_repo".to_string(),
    stream: true,
    extra: params
  }
}
