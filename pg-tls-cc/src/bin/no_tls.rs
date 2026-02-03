use pg_tls_cc::{create_client, create_manager, parallel_execute, single_execute};

const HOST: &str = "localhost";
const USER: &str = "pgusr";
const DBNAME: &str = "empty_db";
const PASSWORD: &str = "u0b2u1n9t2s3u";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection_string = format!(
        "host={} dbname={} user={} password={}",
        HOST, DBNAME, USER, PASSWORD
    );
    let client = create_client(Option::None, connection_string.as_str())
        .await
        .unwrap();
    let _ = single_execute(client).await;
    let manager = create_manager(Option::None, connection_string.as_str()).unwrap();
    parallel_execute(manager).await
}
