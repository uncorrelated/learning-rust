# Rustの練習用リポジトリー
## axum-oidc/
[axum](https://crates.io/crates/axum)で[OpenID Connect Library for Rust](https://crates.io/crates/openidconnect)/[Keycloak](https://www.keycloak.org/)を用いるコード例のCargoリポジトリです。
## pg-tls-cc/
[tokio_postgres](https://crates.io/crates/tokio-postgres)でのログイン処理の紹介用コードです。非TLSパスワード認証（src/bin/no_tls.rs）、TLSパスワード認証（src/bin/pwd_tls.rs）、TLSクライアント証明書認証（src/main.rs）の3パターンがあります。接続後の[deadpool_postgres](https://crates.io/crates/deadpool-postgres)によるコネクションプーリングの例もつけました。
