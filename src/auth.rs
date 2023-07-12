use anyhow;
use oauth2::{
  AuthorizationCode,
  AuthUrl,
  ClientId,
  ClientSecret,
  CsrfToken,
  PkceCodeChallenge,
  RedirectUrl,
  Scope,
  TokenResponse,
  TokenUrl
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::http_client;
use url::Url;

pub fn device_auth() {
  let client = BasicClient::new(
      ClientId::new("client_id".to_string()),
      Some(ClientSecret::new("client_secret".to_string())),
      AuthUrl::new("http://authorize".to_string())?,
      Some(TokenUrl::new("http://token".to_string())?)
    ).set_redirect_url(RedirectUrl::new("http://redirect".to_string())?);

  // Generate a PKCE challenge.
  let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

  // Generate the full authorization URL.
  let (auth_url, csrf_token) = client
    .authorize_url(CsrfToken::new_random)
    // Set the desired scopes.
    .add_scope(Scope::new("read".to_string()))
    .add_scope(Scope::new("write".to_string()))
    // Set the PKCE code challenge.
    .set_pkce_challenge(pkce_challenge)
    .url();

  // This is the URL you should redirect the user to, in order to trigger the authorization
  // process.
  println!("Browse to: {}", auth_url);
  let token_result =
    client
    .exchange_code(AuthorizationCode::new("some authorization code".to_string()))
    // Set the PKCE code verifier.
    .set_pkce_verifier(pkce_verifier)
    .request(http_client)?;
}
