use crate::{logs::LogReceiver, Server};
use sqlx::{MySql, Pool};
use srcds_log_parser::MessageType;
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, RwLock},
    time::Instant,
};

mod tracker;

/// launch seeder time tracking thread
pub async fn spawn_tracker(
    log_receiver: LogReceiver,
    servers: HashMap<SocketAddr, Server>,
    pool: Pool<MySql>,
) {
    // we have a sender and receiver pair for each server that is later passed into each server's event loop thread.
    let senders = Arc::new(RwLock::new(HashMap::new()));
    let mut receivers: HashMap<SocketAddr, mpsc::Receiver<MessageType>> = HashMap::new();
    for (addr, _) in &servers {
        let (s, r) = mpsc::channel(100);
        senders.write().await.insert(addr.clone(), s);
        receivers.insert(addr.clone(), r);
    }

    // pass all senders into the log receiver callback
    {
        let senders = senders.clone();
        log_receiver
            .subscribe(Box::new(move |addr, _, parsed| {
                let senders = senders.clone();
                let parsed = parsed.clone();
                let addr = addr.clone();
                tokio::spawn(async move {
                    let s = senders.read().await;
                    if let Some(sender) = s.get(&addr) {
                        let _ = sender.send(parsed.clone()).await;
                    }
                });
            }))
            .await;
    }

    for (addr, server) in servers.into_iter() {
        // fetch initial game state
        let istate = server
            .controller
            .write()
            .await
            .status()
            .await
            .expect("Could not fetch server state");

        let mut stracker = tracker::Tracker::new(istate, pool.clone());
        let mut receiver = receivers.remove(&addr).unwrap();

        // set up event listener for this server
        tokio::spawn(async move {
            // primary event loop
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut last_sync = Instant::now();
            let mut last_flush = Instant::now();
            loop {
                interval.tick().await;
                let mut events = vec![];
                receiver.recv_many(&mut events, 100).await;

                // update online players
                for event in events {
                    match event {
                        MessageType::Connected { user, .. } => stracker.on_join(user.steamid).await,
                        MessageType::Disconnected { user, .. } => {
                            stracker.on_leave(user.steamid).await
                        }
                        _ => (),
                    }
                }

                // try flushing to db
                if last_flush.elapsed().as_secs() >= 10 {
                    last_flush = Instant::now();
                    let _ = stracker
                        .flush_cache_to_db()
                        .await
                        .inspect_err(|e| eprintln!("Could not flush seeder cache to db: {e}"));
                }

                // try resynchronizing
                if last_sync.elapsed().as_secs() >= 60 {
                    last_sync = Instant::now();
                    if let Ok(state) = server.controller.write().await.status().await {
                        stracker.synchronize(state);
                    }
                }
            }
        });
    }
}
