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
type add_user = Arc<tokio::sync::Mutex<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

#[tokio::main]
async fn main() {
    // 初始化 `users`，这是一个共享的、线程安全的用户列表。
    let users = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    // 添加 "Group" 用户到用户列表
    let mut user_lock = users.lock().await;
    let (group_tx, _group_rx) = mpsc::unbounded_channel();
    user_lock.insert("Group".to_string(), group_tx);
    drop(user_lock); // 确保释放锁
    
    // 创建一个 `users` 过滤器，用于将 `users` 传递给 Warp 处理函数。
    let users_filter = warp::any().map(move || users.clone());


    // 定义host:port/login可以导航到login.html
    let login_route = warp::path("logining")
        .and(warp::fs::file("./frontend/login.html"));
    // 定义host:port/wechat可以导航到wechat.html
    let wechat_route = warp::path("wechat")
        .and(warp::fs::file("./frontend/wechat.html"));
    // 静态文件
    let static_files = warp::fs::dir("./frontend");

    // 定义聊天 WebSocket 路径的处理函数。
    let chat = warp::path("chat")
        .and(warp::ws())
        .and(users_filter.clone())
        .map(|ws: warp::ws::Ws, users| {
            // 当 WebSocket 连接升级时，调用 `user_connected` 函数。
            ws.on_upgrade(move |socket| user_connected(socket, users))
        });
    println!("你有进入到这个界面吗？？");
    // 定义登录 WebSocket 路径的处理函数。
    let login = warp::path("login")
        .and(warp::ws())
        .and(users_filter.clone())
        .map(|ws: warp::ws::Ws, users| {
            // 当 WebSocket 连接升级时，调用 `handle_login` 函数。
            ws.on_upgrade(move |socket| handle_login(socket, users))
        });
  
    let routes = login_route.or(wechat_route).or(chat).or(login).or(static_files);

    // 启动 Warp 服务器，监听 3030 端口。
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn user_connected(ws: WebSocket, users: add_user) {
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
                    eprintln!("(users_connect)Failed to send message: {}", e);
                }
            }
        }
    });

    // 为新连接的用户生成一个用户名（在实际应用中，这应该从客户端获取）。
    let mut Name ="".to_string();   

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
                /*let username = client_message["from"].to_string();
                Name = username.clone();
                println!("目前的用户是：{}",Name);
                // 将新用户及其 `UnboundedSender` 添加到用户列表中。
                users.lock().await.insert(username.clone(), tx.clone());
                for (user,tx) in users.lock().await.iter(){
                    println!("所有的用户名：{}",user);
                }*/
                if client_message["type"] == "add_user" {
                    if let Some(username) = client_message["username"].as_str() {
                        println!("目前的用户是{}", username);
                        let mut user_lock = users.lock().await;
                        user_lock.insert(username.to_string(), tx.clone());
                        let name : Vec<String>= user_lock.keys().cloned().collect();
                        // 设置广播信息
                        let add_msg = serde_json::json!({
                            "type": "add_user",
                            "users": name
                        });
                        for (user, user_tx) in user_lock.iter(){
                        if let Err(e) = user_tx.send(Ok(Message::text(add_msg.to_string()))){
                            eprintln!("Failed to broadcast user list to {}:{}",user,e);
                        }
                    }
                    }

                    
                }else if client_message["type"] == "private_message" {
                    if let (Some(to), Some(msg)) = (client_message["to"].as_str(), client_message["message"].as_str()) {
                        let new_msg = serde_json::json!({
                            "type": "private_message",
                            "from": client_message["from"].as_str(),
                            "message": msg,
                            "timestamp": client_message["timestamp"].as_str().unwrap_or("")
                        });

                        // 查找接收方并发送消息。
                        if let Some(tx) = users.lock().await.get(&to.to_string()) {
                            let _ = tx.send(Ok(Message::text(new_msg.to_string())));
                        }
                    }
                } else if client_message["type"] == "public_message" {
                    if let (Some(to), Some(msg)) = (client_message["to"].as_str(), client_message["message"].as_str()) {
                        let new_msg = serde_json::json!({
                            "type": "p_message",
                            "from": client_message["from"].as_str(),
                            "message": msg,
                            "timestamp": client_message["timestamp"].as_str().unwrap_or("")
                        });

                        let users_lock = users.lock().await;  // 获取 users 的锁

                        // 将消息发送给指定的接收者
                        if let Some(user_tx) = users_lock.get(to) {
                            if let Err(e) = user_tx.send(Ok(Message::text(new_msg.to_string()))) {
                                eprintln!("Failed to send public message to {}: {}", to, e);
                            }
                        }
                        //所有用户都要收到
                        for (user, user_tx) in users_lock.iter(){
                            if let Err(e) = user_tx.send(Ok(Message::text(new_msg.to_string()))){
                                eprintln!("Failed to broadcast user list to {}:{}",user,e);
                            }
                        }
                    }
                }
            }
        }
    }
}

async fn handle_login(ws: WebSocket, users: add_user) {
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Result<Message, warp::Error>>();

    // 启动一个异步任务，用于将消息从通道发送到 WebSocket。
    tokio::task::spawn(async move {
        while let Some(result) = rx.recv().await {
            if let Ok(msg) = result {
                // 尝试发送消息，如果失败则记录错误。
                if let Err(e) = user_ws_tx.send(msg).await {
                    eprintln!("(login_handle)Failed to send message: {}", e);
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
                            println!("重名了，你登陆不了的！我会返回一个false,此时的用户列表：");
                            for (user, tx) in users_lock.iter(){
                                println!("用户：{}",user);
                            }
                            let response = serde_json::json!({
                                "type": "login_response",
                                "success": false,
                                "username": username
                            });
                            // 发送登录失败响应。
                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("Failed to send login_response message: {}", e);
                            }
                        } else {
                            // 用户名不存在，允许登录并更新用户列表。
                            //users_lock.insert(username.to_string(), tx.clone());
                            println!("你可以输出了，输出参数列表：");
                            for (user, tx) in users_lock.iter(){
                                println!("用户：{}",user);
                            }
                            let response = serde_json::json!({
                                "type": "login_response",
                                "success": true,
                                "username": username
                            });

                            // 发送登录成功响应。
                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("Failed to send login_response message: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }
}