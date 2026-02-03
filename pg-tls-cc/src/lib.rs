use deadpool_postgres::{Manager, Pool, Runtime};
use std::fmt;
use std::path::PathBuf;
use tokio_postgres::Client;
use tokio_postgres::NoTls;
use tokio_postgres_rustls::MakeRustlsConnect;

// ~を環境変数HOMEに置換
pub fn expand_home(path: &str) -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    PathBuf::from(path.replace("~", &home))
}

#[derive(Debug)]
struct PgConnectionError {
    msg: String,
}

impl fmt::Display for PgConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for PgConnectionError {}

pub async fn create_client(
    connector: Option<MakeRustlsConnect>,
    connection_str: &str,
) -> Result<Client, Box<dyn std::error::Error>> {
    // 接続
    let pg_config: tokio_postgres::Config = connection_str.parse()?;
    let client = match connector {
        // 2つのpg_config.connectは 、一見そっくりのメソッドだが、戻り値が異なる
        Some(c) => {
            let (client, connection) = pg_config.connect(c).await?;
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("Postgres connection error: {}", e);
                }
            });
            client
        }
        None => {
            let (client, connection) = match pg_config.connect(NoTls).await {
                Ok(t) => t,
                Err(e) => {
                    return Err((PgConnectionError {
                        msg: format!("{}", e),
                    })
                    .into());
                }
            };
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("Postgres connection error: {}", e);
                }
            });
            client
        }
    };

    Ok(client)
}

pub fn create_manager(
    connector: Option<MakeRustlsConnect>,
    connection_str: &str,
) -> Result<Manager, Box<dyn std::error::Error>> {
    // プールの設定
    let pg_config: tokio_postgres::Config = connection_str.parse()?;
    // Manager の作成 (これまでの connector を渡す)
    match connector {
        Some(c) => Ok(Manager::new(pg_config, c)),
        None => Ok(Manager::new(pg_config, NoTls)),
    }
}

pub async fn single_execute(client: Client) -> Result<(), Box<dyn std::error::Error>> {
    // テストクエリ
    let row = client.query_one("SELECT current_user", &[]).await?;
    let user: String = row.get(0);
    eprintln!("Logged in as: {}", user);
    Ok(())
}

pub async fn parallel_execute(manager: Manager) -> Result<(), Box<dyn std::error::Error>> {
    // プールの作成
    let pool = Pool::builder(manager)
        .max_size(10) // 最大10接続
        .runtime(Runtime::Tokio1)
        .wait_timeout(Some(std::time::Duration::from_secs(30))) // 接続取得のタイムアウト設定（例: 30秒）
        .build()?;

    eprintln!("Connection pool created!");

    // 並列実行するタスク数
    let num_tasks = 5;
    let mut handles = Vec::new();

    for i in 1..=num_tasks {
        // プールからコネクションを取得
        let p = pool.clone();
        // 各タスクを並列に起動
        let handle = tokio::spawn(async move {
            let client = p.get().await.map_err(|e| e.to_string())?;

            // クエリの実行
            let row = client
                .query_one("SELECT current_user, now()", &[])
                .await
                .map_err(|e| e.to_string())?;

            let user: String = row.get(0);
            let time: std::time::SystemTime = row.get(1);

            // タスクの結果として値を返す
            Ok::<(usize, String, std::time::SystemTime), String>((i, user, time))
        });
        handles.push(handle);
    }

    // すべてのタスクの終了を待ち、結果を統合
    let results = futures::future::join_all(handles).await;

    eprintln!("\nParallel Execution Results:");
    for res in results {
        match res {
            // JoinHandle の結果 (タスクはパニックしなかったか？)
            Ok(Ok((id, user, time))) => {
                println!("Task {}: User = {}, Time = {:?}", id, user, time);
            }
            Ok(Err(db_err)) => eprintln!("Database error: {}", db_err),
            Err(join_err) => eprintln!("Task panicked: {}", join_err),
        }
    }

    Ok(())
}
