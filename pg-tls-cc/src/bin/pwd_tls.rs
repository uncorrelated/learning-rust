use pg_tls_cc::{create_client, create_manager, expand_home, parallel_execute, single_execute};
use rustls::pki_types::CertificateDer;
use rustls::{ClientConfig, RootCertStore};
use std::fs::File;
use std::io::BufReader;
use tokio_postgres_rustls::MakeRustlsConnect;

const HOST: &str = "localhost";
const USER: &str = "pgusr";
const DBNAME: &str = "empty_db";
const PASSWORD: &str = "u0b2u1n9t2s3u";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let connector = create_connector().unwrap();
    let connection_string = format!(
        "host={} dbname={} user={} password={}",
        HOST, DBNAME, USER, PASSWORD
    );
    let client = create_client(Some(connector.clone()), connection_string.as_str())
        .await
        .unwrap();
    let _ = single_execute(client).await;
    let manager = create_manager(Some(connector), connection_string.as_str()).unwrap();
    parallel_execute(manager).await
}

fn create_connector() -> Result<MakeRustlsConnect, Box<dyn std::error::Error>> {
    // 認証局証明書のファイルパス
    let ca_path = expand_home("~/.ssl/ca_DevRealm-crt.pem");

    // CA証明書のロード
    let mut root_store = RootCertStore::empty();
    let mut ca_reader = BufReader::new(File::open(&ca_path)?);

    // rustls-pemfile 2.0 は Iterator<Item = Result<CertificateDer, ...>> を返す
    let ca_certs: Vec<CertificateDer> =
        rustls_pemfile::certs(&mut ca_reader).collect::<Result<_, _>>()?;

    root_store.add_parsable_certificates(ca_certs);

    // Configの構築
    let config = ClientConfig::builder()
        //        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(MakeRustlsConnect::new(config))
}
