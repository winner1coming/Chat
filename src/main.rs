use serde_json::Value;
use warp::Filter;
use warp::ws::{Message, WebSocket};
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::Arc;
use futures::{SinkExt, StreamExt};


/// 定义一个 `Users` 类型，它是一个线程安全的用户列表，
/// 用 `Arc` 包裹的 `tokio::sync::Mutex`，内部包含一个 `HashMap`，
/// 键是用户名，值是 `mpsc::UnboundedSender`，用于发送 WebSocket 消息。
type Users = Arc<tokio::sync::Mutex<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

#[tokio::main]
async fn main() {
    // 初始化 `users`，这是一个共享的、线程安全的用户列表。
    let users = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    
    // 创建一个 `users` 过滤器，用于将 `users` 传递给 Warp 处理函数。
    let users_filter = warp::any().map(move || users.clone());

    // 定义聊天 WebSocket 路径的处理函数。

    let chat = warp::path("chat")
        .and(warp::ws())
        .and(users_filter.clone())
        .map(|ws: warp::ws::Ws, users| {
            // 当 WebSocket 连接升级时，调用 `user_connected` 函数。
            ws.on_upgrade(move |socket| user_connected(socket, users))
        });


    // 定义登录 WebSocket 路径的处理函数。
    let login = warp::path("login")
        .and(warp::ws())
        .and(users_filter)
        .map(|ws: warp::ws::Ws, users| {
            // 当 WebSocket 连接升级时，调用 `handle_login` 函数。
            ws.on_upgrade(move |socket| handle_login(socket, users))
        });

    // 启动 Warp 服务器，监听 3030 端口。
    warp::serve(login.or(chat)).run(([127, 0, 0, 1], 3030)).await;

}

// 处理新用户连接的异步函数。
async fn user_connected(ws: WebSocket, users: Users) {
    // 将 WebSocket 拆分成发送和接收部分。
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();
    // 创建一个不带缓冲区的通道，用于在任务间传递消息。
    let (tx, mut rx) = mpsc::unbounded_channel();

    // 启动一个异步任务，用于将消息从通道发送到 WebSocket。
    tokio::task::spawn(async move {
        while let Some(result) = rx.recv().await {
            if let Ok(msg) = result {
                // 尝试发送消息，如果失败则记录错误。
                if let Err(e) = user_ws_tx.send(msg).await {
                    eprintln!("Failed to send message: {}", e);
                }
            }
        }
    });

    // 为新连接的用户生成一个用户名（在实际应用中，这应该从客户端获取）。
    let username = "userID".to_string();
    // 将新用户及其 `UnboundedSender` 添加到用户列表中。
    users.lock().await.insert(username.clone(), tx.clone());

    // 处理来自 WebSocket 的消息。
    while let Some(result) = user_ws_rx.next().await {
        let message = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
        };

        // 将消息转换为字符串并解析为 JSON。
        if let Ok(msg_str) = message.to_str() {
            if let Ok(client_message) = serde_json::from_str::<Value>(msg_str) {
                // 处理私聊消息。
                if client_message["type"] == "private_message" {
                    if let (Some(to), Some(msg)) = (client_message["to"].as_str(), client_message["message"].as_str()) {
                        let new_msg = serde_json::json!({
                            "type": "private_message",
                            "from": username,
                            "message": msg,
                            "timestamp": client_message["timestamp"].as_str().unwrap_or("")
                        });

                        // 查找接收方并发送消息。
                        if let Some(tx) = users.lock().await.get(&to.to_string()) {
                            let _ = tx.send(Ok(Message::text(new_msg.to_string())));
                        }
                    }
                } else if client_message["type"] == "update_users" {
                    // 处理用户列表更新消息。
                    let users_list: Vec<String> = users.lock().await.keys().cloned().collect();
                    let update_msg = serde_json::json!({
                        "type" : "update_users",
                        "users": users_list
                    });
                    let _ = tx.send(Ok(Message::text(update_msg.to_string())));
                }
            }
        }
    }

    // 用户断开连接时，从用户列表中移除该用户。
    users.lock().await.remove(&username);
}

async fn handle_login(ws: WebSocket, users: Users) {
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // 启动一个异步任务，用于将消息从通道发送到 WebSocket。
    tokio::task::spawn(async move {
        while let Some(result) = rx.recv().await {
            if let Ok(msg) = result {
                // 尝试发送消息，如果失败则记录错误。
                if let Err(e) = user_ws_tx.send(msg).await {
                    eprintln!("Failed to send message: {}", e);
                }
            }
        }
    });

    while let Some(result) = user_ws_rx.next().await {
        let message = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
        };

        if let Ok(msg_str) = message.to_str() {
            if let Ok(client_message) = serde_json::from_str::<Value>(msg_str) {
                // 处理登录消息。
                if client_message["type"] == "login" {
                    if let Some(username) = client_message["username"].as_str() {
                        let mut users_lock = users.lock().await;
                        // 检查用户名是否已存在。
                        if users_lock.contains_key(username) {
                            let response = serde_json::json!({
                                "type": "login_response",
                                "success": false,
                            });
                            // 发送登录失败响应。
                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("Failed to send login_response message: {}", e);
                            }
                        } else {
                            // 用户名不存在，允许登录并更新用户列表。
                            users_lock.insert(username.to_string(), tx.clone());
                            let response = serde_json::json!({
                                "type": "login_response",
                                "success": true,
                                "username": username
                            });
                            // 发送登录成功响应。
                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("Failed to send login_response message: {}", e);
                            }

                            // 处理用户发送的消息。
                            while let Some(result) = user_ws_rx.next().await {
                                let message = match result {
                                    Ok(msg) => msg,
                                    Err(e) => {
                                        eprintln!("WebSocket error: {}", e);
                                        break;
                                    }
                                };

                                user_message(username.to_string(), message, &users).await;
                            }

                            // 用户退出时，从用户列表中移除该用户。
                            users_lock.remove(username);
                        }
                    }
                }
            }
        }
    }
}


// 处理用户消息的异步函数。
// 将接收到的消息格式化为 "用户名: 消息内容" 并广播给所有连接的用户。
async fn user_message(username: String, msg: Message, users: &Users) {

    // 将消息转换为字符串并解析为 JSON。
    if let Ok(msg_str) = msg.to_str() {
        if let Ok(client_message) = serde_json::from_str::<Value>(msg_str) {
            // 处理私聊消息。
            if client_message["type"] == "private_message" {
                if let Some(to) = client_message["to"].as_str() {
                    if let Some(tx) = users.lock().await.get(&to.to_string()) {
                        let new_msg = serde_json::json!({
                            "type": "private_message",
                            "from": username,
                            "message": client_message["message"].as_str().unwrap_or(""),
                            "timestamp": client_message["timestamp"].as_str().unwrap_or("")
                        });
                        // 发送私聊消息。
                        let _ = tx.send(Ok(Message::text(new_msg.to_string())));
                    }
                }
            }
        }
    }
}
