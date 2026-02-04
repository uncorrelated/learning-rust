use pg_tls_cc::{create_client, create_manager, expand_home, parallel_execute, single_execute};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ClientConfig, RootCertStore};
use std::fs::File;
use std::io::BufReader;
use tokio_postgres_rustls::MakeRustlsConnect;

const HOST: &str = "localhost";
const USER: &str = "pgusr";
const DBNAME: &str = "empty_db";

const CA_CRT: &str = "~/.ssl/ca_DevRealm-crt.pem";
const CLIENT_CRT: &str = "~/.ssl/pgclient-cert.pem";
const CLIENT_KEY: &str = "~/.ssl/pgclient-key.pem";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 暗号化処理のエンジンとして ring クレートを使用する
    let _ = rustls::crypto::ring::default_provider().install_default();

    let connector = create_connector().unwrap();
    // sslmode は connector 側で制御されるため指定不要
    let connection_string = format!("host={} dbname={} user={}", HOST, DBNAME, USER);

    // 単発実行
    let client = create_client(Some(connector.clone()), connection_string.as_str())
        .await
        .unwrap();
    let _ = single_execute(client).await;

    // プール並列実行
    // 元のコードの型不一致を避けるため、connectorは必須とします
    let manager = create_manager(Some(connector), connection_string.as_str()).unwrap();
    parallel_execute(manager).await
}

fn create_connector() -> Result<MakeRustlsConnect, Box<dyn std::error::Error>> {
    // 認証局証明書、クライアント証明書、クライアント秘密鍵のファイルパス
    let ca_path = expand_home(CA_CRT);
    let cert_path = expand_home(CLIENT_CRT);
    let key_path = expand_home(CLIENT_KEY);

    // CA証明書のロード
    let mut root_store = RootCertStore::empty();
    let mut ca_reader = BufReader::new(File::open(&ca_path)?);

    // rustls-pemfile 2.0 は Iterator<Item = Result<CertificateDer, ...>> を返す
    let ca_certs: Vec<CertificateDer> =
        rustls_pemfile::certs(&mut ca_reader).collect::<Result<_, _>>()?;

    root_store.add_parsable_certificates(ca_certs);

    // クライアント証明書チェーンのロード
    let mut cert_reader = BufReader::new(File::open(&cert_path)?);
    let cert_chain: Vec<CertificateDer> =
        rustls_pemfile::certs(&mut cert_reader).collect::<Result<_, _>>()?;

    // クライアント秘密鍵のロード
    let mut key_reader = BufReader::new(File::open(&key_path)?);
    // private_key() は Result<Option<PrivateKeyDer>> を返す
    let key: PrivateKeyDer =
        rustls_pemfile::private_key(&mut key_reader)?.ok_or("No private key found")?;

    // Configの構築 (Rustls 0.23 スタイル)
    // with_safe_defaults() は削除され、builder() がデフォルトプロバイダ(ring/aws-lc)を使用します
    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(cert_chain, key)?;

    Ok(MakeRustlsConnect::new(config))
}
