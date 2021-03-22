use serde_json::Value;
use serenity::{
    builder::CreateMessage,
    client::{Context, EventHandler},
    http::Http,
    model::id::GuildId,
    utils::hashmap_to_json_map,
};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

mod kingdom;
mod util;

struct Handler {
    db_pool: SqlitePool,
    channel_id: u64,
    is_initialized: tokio::sync::watch::Sender<bool>,
}

#[async_trait::async_trait]
impl EventHandler for Handler {
    // Connected to discrod & cache system is ready
    async fn cache_ready(&self, context: Context, _: Vec<GuildId>) {
        self.is_initialized.send(true).unwrap();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = std::env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN is mandatory");
    let guild_id: u64 = std::env::var("GUILD_ID")
        .expect("GUILD_ID is mandatory")
        .parse()?;
    let channel_id: u64 = std::env::var("CHANNEL_ID")
        .expect("CHANNEL_ID is mandatory")
        .parse()?;

    let db_pool = SqlitePoolOptions::new()
        .connect(&{
            let mut dir = std::env::current_dir().unwrap();
            dir.push("data.db");
            let path = format!("sqlite://{}?mode=rwc", dir.display());
            path
        })
        .await
        .unwrap();
    // run DB migration
    sqlx::migrate!().run(&db_pool).await?;

    let (tx, mut rx) = tokio::sync::watch::channel(false);
    let mut discord = serenity::Client::builder(token)
        .event_handler(Handler {
            db_pool: db_pool.clone(),
            channel_id,
            is_initialized: tx,
        })
        .await?;

    let mut scheduler = clokwerk::Scheduler::new();
    use clokwerk::TimeUnits;
    let http = discord.cache_and_http.http.clone();

    let schedule_rt = tokio::runtime::Runtime::new().unwrap();
    let (_, ret) = tokio::join!(
        schedule_rt.spawn_blocking(move || {
            tokio::runtime::Handle::current().block_on(rx.changed());

            let job = scheduler.every(10.minutes()).run(move || {
                let handle = tokio::runtime::Handle::current();
                let ret: anyhow::Result<()> = handle.block_on(async {
                    let last_id = sqlx::query!("SELECT id FROM last_id")
                        .fetch_optional(&db_pool)
                        .await
                        .unwrap()
                        .map(|i| i.id);
                    let items = kingdom::scrap_notice(last_id).await?;
                    println!("prev_id: {:?} - {} items", last_id, items.len());
                    let last_id = match items.get(0) {
                        Some(item) => item.number,
                        None => last_id.unwrap(),
                    };
                    for item in items.into_iter().rev() {
                        http.send_message(channel_id, &{
                            let mut msg = CreateMessage::default();
                            msg.embed(|e| {
                                e.title(item.title)
                                    .url(item.url)
                                    .author(|author| author.name("네이버 카페 공지사항"))
                                    .timestamp(&item.datetime)
                            });

                            Value::Object(hashmap_to_json_map(msg.0))
                        })
                        .await?;
                    }

                    sqlx::query!(
                        "INSERT OR REPLACE INTO last_id (pk, id) VALUES (?, ?)",
                        0,
                        last_id
                    )
                    .execute(&db_pool)
                    .await?;

                    Ok(())
                });
                if let Err(e) = ret {
                    eprintln!("{:?}", e);
                }
            });
            job.execute(&chrono::offset::Local::now());

            loop {
                scheduler.run_pending();
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }),
        discord.start()
    );
    ret?;

    Ok(())
}
