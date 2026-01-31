use axum::{
    Router,
    extract::{Json, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use axum_extra::{
    TypedHeader,
    extract::cookie::SameSite,
    extract::cookie::{Cookie, CookieJar},
    headers::{Authorization, authorization::Bearer},
};
use openidconnect::{
    AuthenticationFlow, ClaimsVerificationError, ClientId, ClientSecret, CsrfToken,
    EmptyAdditionalClaims, IdTokenClaims, IssuerUrl, Nonce, OAuth2TokenResponse, RedirectUrl,
    RefreshToken, Scope, TokenResponse,
    core::{CoreClient, CoreGenderClaim, CoreIdToken, CoreProviderMetadata, CoreResponseType},
    reqwest::async_http_client,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use tokio::{net::TcpListener, signal};
use tower_http::services::ServeDir;

const HOME_PATH: &str = "/";
const LOGIN_PATH: &str = "/login";
const REDIRECT_PATH: &str = "/callback";
const PROTECTED_PATH: &str = "/api";
const LOGOUT_PATH: &str = "/logout";

// Keycloakの設定など環境変数を反映する部分
#[derive(Clone)]
struct Info {
    client_hostname: String,
    client_id: String,
    client_secret: String,
    keycloak_url: String,
    keycloak_logout_url: String,
    redirect_url: String,
    nonce_salt: String,
    csrf_salt: String,
}

#[derive(Clone)]
struct AppState {
    oidc_client: CoreClient,
    info: Info,
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    code: String,
    state: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // デフォルトのCLIENT_SECRETは開発用のモノなので心配無用
    let info = Info {
        client_hostname: env::var("CLIENT_HOSTNAME").unwrap_or("localhost:18080".to_string()),
        client_id: env::var("CLIENT_ID").unwrap_or("rust-web-app".to_string()),
        client_secret: env::var("CLIENT_SECRET")
            .unwrap_or("V7WuCUs2FYUW45tDK6YPifKhCl4HKDkW".to_string()),
        keycloak_url: env::var("KEYCLOAK_URL")
            .unwrap_or("http://localhost:8080/realms/DevRealm".to_string()),
        keycloak_logout_url: env::var("KEYCLOAK_LOGOUT_URL").unwrap_or(
            "http://localhost:8080/realms/DevRealm/protocol/openid-connect/logout".to_string(),
        ),
        redirect_url: env::var("REDIRECT_URL")
            .unwrap_or("http://localhost:18080/callback".to_string()),
        nonce_salt: env::var("NONCE_SALT").unwrap_or("a secret phrase".to_string()),
        csrf_salt: env::var("CSRF_SALT").unwrap_or("another secret phrase".to_string()),
    };

    let provider_metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new(info.keycloak_url.clone())?,
        async_http_client,
    )
    .await?;

    let client = CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(info.client_id.clone()),
        Some(ClientSecret::new(info.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(info.redirect_url.clone())?);

    let app_state = AppState {
        oidc_client: client,
        info: info.clone(),
    };

    // ルーティング
    let app = Router::new()
        .nest_service(HOME_PATH, ServeDir::new("./htdocs"))
        .route(LOGIN_PATH, get(login_handler))
        .route(REDIRECT_PATH, get(callback_handler))
        .route(PROTECTED_PATH, get(protected_handler))
        .route(LOGOUT_PATH, get(logout_handler))
        .with_state(app_state);

    println!("Listening on http://{}", info.client_hostname);
    let listener = TcpListener::bind(info.client_hostname).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async { signal::ctrl_c().await.unwrap() })
        .await?;

    Ok(())
}

// /login Handler: KeyCloakにログインする前の準備を行う
async fn login_handler(State(state): State<AppState>, jar: CookieJar) -> Response {
    // OpenID Connectと通信するための一時コード等を作成
    let (auth_url, csrf_token, nonce) = state
        .oidc_client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_string()))
        .url();

    // セッションを使わない方針なので、検証用トークンはそのまま保存せず、シークレットキーを足してハッシュ化したものを保存する
    // CSRF対策のstateトークンのハッシュ値を保存
    let jar = jar.add(make_secret_cookie(
        "csrf_token",
        sha256text(csrf_token.secret(), &state.info.csrf_salt),
        String::from("/"),
        true,
    ));

    // コールバックがKeyCloakにログインできたブラウザーからのものか検証するために、nonceのハッシュ値を保存
    let jar = jar.add(make_secret_cookie(
        "nonce",
        sha256text(nonce.secret(), &state.info.nonce_salt),
        String::from("/"),
        true,
    ));

    eprintln!("{}", auth_url.as_str());

    // ブラウザーをKeyCloakにリダイレクトする
    (
        StatusCode::TEMPORARY_REDIRECT,
        jar,
        Redirect::to(auth_url.as_str()),
    )
        .into_response()
}

// /callback Handler: KeyCloakから認可コードと一緒にリダイレクトされてくる
async fn callback_handler(
    Query(query): Query<AuthRequest>,
    State(state): State<AppState>,
    jar: CookieJar,
) -> Response {
    // CSRF対策のstateの検証
    let csrf_token_in_cookie = match jar.get("csrf_token").map(|c| c.value().to_string()) {
        Some(value) => value,
        None => {
            return (StatusCode::UNAUTHORIZED, "No State / CSRF Token").into_response();
        }
    };
    if csrf_token_in_cookie != sha256text(&query.state, &state.info.csrf_salt) {
        return (StatusCode::UNAUTHORIZED, "Invalid State / CSRF Token").into_response();
    }

    // 認可コードからトークンを取得
    let token_response = match state
        .oidc_client
        .exchange_code(openidconnect::AuthorizationCode::new(query.code))
        .request_async(async_http_client)
        .await
    {
        Ok(token) => token,
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                format!("Failed to exchange token: {}", e),
            )
                .into_response();
        }
    };

    // IDトークンを取得
    let id_token = match token_response.id_token() {
        Some(token) => token,
        None => {
            return (StatusCode::UNAUTHORIZED, "Error: No ID Token received").into_response();
        }
    };

    // nonce検証
    match id_token.claims(&state.oidc_client.id_token_verifier(), &|nonce: Option<
        &Nonce,
    >| {
        return check_nonce(nonce, &jar, &state.info.nonce_salt);
    }) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                format!("Verification failed: {}", e),
            )
                .into_response();
        }
    };

    // Cookieからnonceとstateを削除
    let jar = jar.remove(Cookie::from("csrf_token"));
    let jar = jar.remove(Cookie::from("nonce"));

    // リフレッシュトークンをCookieに保存する
    let jar = jar.add(make_secret_cookie(
        "refresh_token",
        match token_response.refresh_token() {
            Some(refresh_token) => refresh_token.secret(),
            None => "",
        }
        .to_string(),
        String::from("/"),
        true,
    ));

    // ID TokenをCookieに保存
    let jar = jar.add(make_secret_cookie(
        "id_token",
        id_token.to_string(),
        String::from("/"),
        true,
    ));
    // check digitsをつくってCookieに保存
    let jar = jar.add(make_secret_cookie(
        "check_digits",
        sha256text(id_token.to_string().as_str(), state.info.csrf_salt.as_str()).to_string(),
        String::from("/"),
        false,
    ));

    // HOMEにリダイレクト
    (StatusCode::TEMPORARY_REDIRECT, jar, Redirect::to(HOME_PATH)).into_response()
}

// Web APIなどの保護された領域のhandler: ID Tokenによる認証を行う
async fn protected_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>, // TypedHeaderでAuthorization: Bearer <token> をあれば取得
) -> Response {
    let (jar, map) = match auth_by_oidc_token(State(state), jar, auth_header).await {
        Ok(t) => t,
        Err(e) => {
            return (StatusCode::UNAUTHORIZED, e).into_response();
        }
    };

    return (jar, Json(map)).into_response();
}

// Keycloakからバックチャンネル・ログアウト (POST)を行う
async fn logout_handler(
    State(state): State<AppState>,
    jar: CookieJar,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Response {
    // ID Tokenの存在とCheck Digitsの整合性を確認
    let _token_str = match check_digits(State(state.clone()), jar.clone(), auth_header) {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::UNAUTHORIZED, e.to_string()).into_response();
        }
    };

    // Cookie内のID Tokenの文字列を引っ張ってくる
    if let Some(cookie) = jar.get("refresh_token") {
        let refresh_token_str = cookie.value();
        let client = reqwest::Client::new();
        let logout_url = state.info.keycloak_logout_url.as_str();
        let params = [
            ("client_id", state.info.client_id.clone()),
            ("client_secret", state.info.client_secret.clone()),
            ("refresh_token", refresh_token_str.to_string()),
        ];

        match client
            .post(logout_url)
            .form(&params) // Content-Type: application/x-www-form-urlencoded
            .send()
            .await
        {
            Ok(res) => {
                if res.status().is_success() {
                    let jar = jar.remove(Cookie::from("id_token"));
                    let jar = jar.remove(Cookie::from("refresh_token"));
                    return (StatusCode::OK, jar).into_response();
                } else {
                    return (
                        StatusCode::UNAUTHORIZED,
                        format!("Keycloak logout failed. Status: {}", res.status()),
                    )
                        .into_response();
                    // レスポンスボディを確認したい場合
                    // eprintln!("Body: {:?}", res.text().await);
                }
            }
            Err(e) => {
                return (StatusCode::UNAUTHORIZED, e.to_string()).into_response();
            }
        }
    } else {
        return (
            StatusCode::UNAUTHORIZED,
            "No refresh token found in session. Skipping Keycloak logout.",
        )
            .into_response();
    }
}

// 認証の途中まで（logout処理に必要な分）
fn check_digits(
    State(state): State<AppState>,
    jar: CookieJar,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<String, String> {
    // Authorization HeaderからID Tokenの文字列を取り出す
    // CookieにあるID Tokenを参照するのはCSRFに脆弱なため
    let bearer = match auth_header {
        Some(TypedHeader(Authorization(bearer))) => bearer,
        None => {
            // Authorization: bearerがない
            return Err(String::from("No Authorization Header"));
        }
    };
    // Authorization: Bearerに続く文字列からcheck digitsを作成
    let check_digits = bearer.token();
    let token_str = match jar.get("id_token") {
        Some(cookie) => cookie.value(),
        None => {
            return Err(String::from("No ID Token"));
        }
    };
    // check_digitsがあわない場合は、CSRFの可能性があるのでエラー
    if check_digits != sha256text(token_str, state.info.csrf_salt.as_str()) {
        return Err(String::from("Illegal Check Digits"));
    }
    return Ok(token_str.to_string());
}

// 認証（参照や操作に必要な分）
async fn auth_by_oidc_token(
    State(state): State<AppState>,
    jar: CookieJar,
    auth_header: Option<TypedHeader<Authorization<Bearer>>>,
) -> Result<(CookieJar, HashMap<String, String>), String> {
    let token_str = match check_digits(State(state.clone()), jar.clone(), auth_header) {
        Ok(s) => s,
        Err(e) => {
            return Err(e.to_string());
        }
    };
    let id_token = match CoreIdToken::from_str(token_str.as_str()) {
        Ok(i) => i.clone(),
        Err(e) => {
            return Err(e.to_string());
        }
    };
    let verifier = state.oidc_client.id_token_verifier();
    let claims = match id_token.claims(&verifier, &|_nonce: Option<&Nonce>| {
        // 更新トークンにはnonceが入らないので、ログイン後は検証できない
        return Ok(());
    }) {
        Ok(c) => c,
        Err(e) => {
            let refresh_token_str = match e {
                ClaimsVerificationError::Expired(_msg) => {
                    // 期限切れ
                    match jar.get("refresh_token") {
                        Some(s) => s.value().to_string(),
                        None => {
                            return Err(String::from("Error, No reflesh token"));
                        }
                    }
                }
                _ => {
                    // 期限切れ以外
                    return Err(e.to_string());
                }
            };
            // リフレッシュトークンあり
            let refresh_token = RefreshToken::new(refresh_token_str);
            let token_response = state
                .oidc_client
                .exchange_refresh_token(&refresh_token)
                .request_async(async_http_client)
                .await;
            let res = match token_response {
                Ok(r) => r,
                Err(e) => {
                    // リフレッシュトークンによる認証失敗
                    return Err(e.to_string());
                }
            };
            // リフレッシュトークンをCookieに保存する
            let jar = jar.add(make_secret_cookie(
                "refresh_token",
                match res.refresh_token() {
                    Some(refresh_token) => refresh_token.secret(),
                    None => "",
                }
                .to_string(),
                String::from("/"),
                true,
            ));
            // ID Tokenの再取得
            let id_token = res.id_token().expect("No ID Token");
            let claims = match id_token.claims(&verifier, &|_nonce: Option<&Nonce>| {
                return Ok(()); // 更新トークンにはnonceが入らないので、ログイン後は検証できない
            }) {
                Ok(c) => c,
                Err(e) => {
                    return Err(e.to_string());
                }
            };
            // ID TokenをCookieに保存
            let jar = jar.add(make_secret_cookie(
                "id_token",
                id_token.to_string(),
                String::from("/"),
                true,
            ));
            // check digitsをつくってCookieに保存
            let jar = jar.add(make_secret_cookie(
                "check_digits",
                sha256text(id_token.to_string().as_str(), state.info.csrf_salt.as_str())
                    .to_string(),
                String::from("/"),
                false,
            ));
            return Ok((jar, claims_to_map(claims)));
        }
    };
    return Ok((jar, claims_to_map(claims)));
}

fn check_nonce(nonce: Option<&Nonce>, jar: &CookieJar, nonce_salt: &str) -> Result<(), String> {
    let nonce_in_jwt: String = match nonce {
        Some(_nonce) => sha256text(_nonce.secret(), nonce_salt),
        None => String::from(""),
    };
    return match jar.get("nonce").map(|c| c.value().to_string()) {
        Some(nonce_in_cookie) => {
            if nonce_in_jwt == nonce_in_cookie {
                Ok(())
            } else {
                Err(String::from("two nonce are different!"))
            }
        }
        None => Err(String::from("no nonce in the cookies!")),
    };
}

fn claims_to_map(
    claims: &IdTokenClaims<EmptyAdditionalClaims, CoreGenderClaim>,
) -> HashMap<String, String> {
    let subject = claims.subject().as_str();
    let email = claims.email().map(|e| e.as_str()).unwrap_or("");
    let name = claims
        .name()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str())
        .unwrap_or("");
    let mut map: HashMap<String, String> = HashMap::new();
    map.insert(String::from("subject"), subject.to_string());
    map.insert(String::from("name"), name.to_string());
    map.insert(String::from("email"), email.to_string());
    return map;
}

fn make_secret_cookie<'a>(
    name: &'a str,
    value: String,
    path: String,
    is_http_only: bool,
) -> Cookie<'a> {
    Cookie::build((name.to_string(), value))
        .path(path)
        .http_only(is_http_only)
        .secure(false)
        .same_site(SameSite::Lax)
        .build()
}

fn sha256text(s: &str, b: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b.as_bytes());
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}
