use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;

use rocket::tokio::sync::Mutex;
use rocket::State;
use rocket::futures::{stream::SplitSink, SinkExt, StreamExt};
use rocket_ws::{WebSocket, Channel, Message, stream::DuplexStream};

static ID_COUNT:AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
struct Room {
    connections: Mutex<HashMap<usize, SplitSink<DuplexStream, Message>>>
}

impl Room {
    pub async fn connect(&self, id:usize, tx:SplitSink<DuplexStream, Message>) {
        let _ = self.connections.lock().await.insert(id, tx);
    }
    pub async fn disconnect(&self, id:usize) {
        let _ = self.connections.lock().await.remove(&id);
    }
    pub async fn broadcast(&self, msg:Message) {
        let mut connections = self.connections.lock().await;
        for (_, tx) in connections.iter_mut() {
            let _ = tx.send(msg.clone()).await;
        }
    }
}

#[rocket::get("/")]
fn chat<'r>(ws:WebSocket, room:&'r State<Room>) ->Channel<'r> {
    ws.channel(move | mut stream| Box::pin(async move {
        let id = ID_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (mut ws_tx, mut ws_rx) = stream.split();
        
        let _ = room.connect(id, ws_tx).await;
        
        while let Some(msg) = ws_rx.next().await {
            room.broadcast(msg?).await;
        }

        let _ = room.disconnect(id).await;
        Ok(())
    }))
}

#[rocket::main]
async fn main() {
    let _ = rocket::build()
        .mount("/", rocket::routes![
            chat
        ])
        .manage(Room::default())
        .launch()
        .await;
}
