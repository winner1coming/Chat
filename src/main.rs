use warp::Filter;
use warp::ws::{Message, WebSocket};
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use futures::{SinkExt, StreamExt};
use futures_util::stream::StreamExt as FuturesStreamExt;
use futures_util::sink::SinkExt as FuturesSinkExt;

// 定义一个类型别名，用于存储所有连接用户的发送器。
// 这里使用了 Arc 和 Mutex 来允许多个线程安全地访问和修改用户列表。
// HashMap 中的键是用户名，值是 UnboundedSender，用于向该用户发送 WebSocket 消息。
type Users = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

#[tokio::main]
async fn main() {
    // 初始化用户列表，用户列表将会在程序运行期间被共享和修改。
    let users = Users::default();

    // 使用 warp 的 Filter 创建一个新的过滤器，用于将用户列表传递到 WebSocket 处理函数中。
    let users = warp::any().map(move || users.clone());

    // 定义 WebSocket 路径处理函数。
    // 当接收到 WebSocket 请求时，调用 user_connected 函数来处理新的 WebSocket 连接。
    let chat = warp::path("chat")
        .and(warp::ws())
        .and(users)
        .map(|ws: warp::ws::Ws, users| {
            ws.on_upgrade(move |socket| user_connected(socket, users))
        });
    // 启动 Warp 服务器，监听在本地地址 127.0.0.1:3030
    warp::serve(chat).run(([127, 0, 0, 1], 3030)).await;
}

// 处理新用户连接的异步函数。
async fn user_connected(ws: WebSocket, users: Users) {
    // 将 WebSocket 拆分为发送端和接收端
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();
    // 创建一个无界通道，用于向用户发送消息。
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // 启动一个新的任务，负责从通道中接收消息并将其发送到 WebSocket。
    tokio::task::spawn(async move {
        while let Some(result) = rx.recv().await {
            if let Ok(msg) = result {
                user_ws_tx.send(msg).await.unwrap();
            }
        }
    });
    //可以通过身份验证（登录）的方法获得用户的用户名，加以区分
    let username = "userID".to_string();
    // 将用户的发送器存储在用户列表中。
    users.lock().unwrap().insert(username.clone(), tx);

    // 处理用户发送的消息
    while let Some(result) = user_ws_rx.next().await {
        let message = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error: {}", e);
                break;
            }
        };
        // 将接收到的消息广播给所有用户。
        user_message(username.clone(), message, &users).await;
    }
    // 用户断开连接后，从用户列表中移除该用户。
    users.lock().unwrap().remove(&username);
}


// 处理用户消息的异步函数。
// 将接收到的消息格式化为 "用户名: 消息内容" 并广播给所有连接的用户。
async fn user_message(username: String, msg: Message, users: &Users) {
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    // 格式化消息。
    let new_msg = format!("{}: {}", username, msg);
    // 遍历所有用户，将格式化后的消息发送给每个用户。
    for (_user, tx) in users.lock().unwrap().iter() {
        let _ = tx.send(Ok(Message::text(new_msg.clone())));
    }
}
