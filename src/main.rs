use warp::Filter;
use warp::ws::{Message, WebSocket};
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use futures::{SinkExt, StreamExt};
use futures_util::stream::StreamExt as FuturesStreamExt;
use futures_util::sink::SinkExt as FuturesSinkExt;

type Users = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

#[tokio::main]
async fn main() {
    let users = Users::default();
    let users = warp::any().map(move || users.clone());

    let chat = warp::path("chat")
        .and(warp::ws())
        .and(users)
        .map(|ws: warp::ws::Ws, users| {
            ws.on_upgrade(move |socket| user_connected(socket, users))
        });

    warp::serve(chat).run(([127, 0, 0, 1], 3030)).await;
}

async fn user_connected(ws: WebSocket, users: Users) {
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    tokio::task::spawn(async move {
        while let Some(result) = rx.recv().await {
            if let Ok(msg) = result {
                user_ws_tx.send(msg).await.unwrap();
            }
        }
    });

    let username = "userID".to_string();
    users.lock().unwrap().insert(username.clone(), tx);

    while let Some(result) = user_ws_rx.next().await {
        let message = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error: {}", e);
                break;
            }
        };

        user_message(username.clone(), message, &users).await;
    }

    users.lock().unwrap().remove(&username);
}

async fn user_message(username: String, msg: Message, users: &Users) {
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    let new_msg = format!("{}: {}", username, msg);
    for (_user, tx) in users.lock().unwrap().iter() {
        let _ = tx.send(Ok(Message::text(new_msg.clone())));
    }
}
