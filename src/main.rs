use serde_json::Value;
use warp::Filter;
use warp::ws::{Message, WebSocket};
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use futures::{SinkExt, StreamExt};

static user_ID : AtomicUsize = AtomicUsize::new(1);
/// 定义一个 `Users` 类型，它是一个线程安全的用户列表，
/// 用 `Arc` 包裹的 `tokio::sync::Mutex`，内部包含一个 `HashMap`，
/// 键是用户名，值是 `mpsc::UnboundedSender`，用于发送 WebSocket 消息。
type add_user = Arc<tokio::sync::Mutex<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;
type UserStore = Arc<tokio::sync::Mutex<HashMap<String,(String,usize, mpsc::UnboundedSender<Result<Message,warp::Error>>)>>>;


#[tokio::main]
async fn main() {
    // 初始化 `users`，这是一个共享的、线程安全的用户列表。
    let users = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let user_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    // 添加 "Group" 用户到用户列表
      
    let (group_tx, _group_rx) = mpsc::unbounded_channel();
    {
    let mut user_lock = users.lock().await; 
    user_lock.insert("Group".to_string(), group_tx.clone());
    }
    
    {
        let mut store_lock = user_store.lock().await;
        let group_id = user_ID.fetch_add(1, Ordering::SeqCst); // 获取下一个 ID
        store_lock.insert(
            "Group".to_string(),
            ("default_password".to_string(), group_id, group_tx.clone()) // 使用默认密码
        );
    }
    
    // 创建一个 `users` 过滤器，用于将 `users` 传递给 Warp 处理函数。
    let users_filter = warp::any().map(move || users.clone());
    let store_filter = warp::any().map(move || user_store.clone());

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
        .and(store_filter.clone())
        .map(|ws: warp::ws::Ws, users,user_store| {
            // 当 WebSocket 连接升级时，调用 `user_connected` 函数。
            ws.on_upgrade(move |socket| user_connected(socket, users, user_store))
        });
    println!("你有进入到这个界面吗？？");
    // 定义登录 WebSocket 路径的处理函数。
    let login = warp::path("login")
        .and(warp::ws())
        .and(users_filter.clone())
        .and(store_filter.clone())
        .map(|ws: warp::ws::Ws, users, user_store| {
            // 当 WebSocket 连接升级时，调用 `handle_login` 函数。
            ws.on_upgrade(move |socket| handle_login(socket, users,user_store))
        });
  
    let routes = login_route.or(wechat_route).or(chat).or(login).or(static_files);

    // 启动 Warp 服务器，监听 3030 端口。
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn user_connected(ws: WebSocket, users: add_user, user_store: UserStore) {
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
    //let mut Name ="".to_string();   

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
                if client_message["type"] == "add_user" {
                    if let Some(username) = client_message["username"].as_str() {
                        println!("目前的用户是{}", username);
                        let mut user_lock = users.lock().await;
                        let mut store_lock = user_store.lock().await;
                        user_lock.insert(username.to_string(), tx.clone());
                        // 获取所有用户的用户名和编号
                        let name: Vec<String> = user_lock.keys().cloned().collect();
                        let user_ids: Vec<(String, usize)> = store_lock.iter()
                            .filter_map(|(user, (_, id, _))| {
                                if name.contains(user) {
                                    Some((user.clone(), *id))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        
                        // 设置广播信息
                        let add_msg = serde_json::json!({
                            "type": "add_user",
                            "users": user_ids
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
                            "type": "public_message",
                            "from": client_message["from"].as_str(),
                            "message": msg,
                            "timestamp": client_message["timestamp"].as_str().unwrap_or("")
                        });

                        let users_lock = users.lock().await;  // 获取 users 的锁

                        // 将消息发送给所有在线用户
                        for (user, user_tx) in users_lock.iter(){
                            if let Err(e) = user_tx.send(Ok(Message::text(new_msg.to_string()))){
                                eprintln!("Failed to broadcast user list to {}:{}",user,e);
                            }
                        }
                        
                    }
                }
                else if client_message["type"] == "logout" {
                    let user = client_message["user"].as_str().unwrap_or("");
                    let mut users_lock = users.lock().await;
                    users_lock.remove(user);
                    println!("{}",user);

                    let user_left_msg = serde_json::json!({
                        "type": "user_remove",
                        "user": user
                    });

                    for (_, user_tx) in users_lock.iter() {
                        if let Err(e) = user_tx.send(Ok(Message::text(user_left_msg.to_string()))) {
                            eprintln!("Failed to notify user disconnected: {}", e);
                        }
                    }
                    println!("有用户退出登录了：{}",user);
                }
            }
        }
    }
}

async fn handle_login(ws: WebSocket, users: add_user, user_store: UserStore) {
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
                    if let (Some(username), Some(password)) = (
                        client_message["username"].as_str(),
                        client_message["password"].as_str()
                    ) {
                        let user_store_lock = user_store.lock().await;
                        let mut users_lock = users.lock().await;

                        // 首先检查用户名是否已经在 add_user 中
                        if !users_lock.contains_key(username) {
                            // 用户未登录，进行密码验证
                            if let Some((stored_password, usize_id, _)) = user_store_lock.get(username) {
                                if stored_password == password {
                                    // 密码正确，返回登录成功
                                    let response = serde_json::json!({
                                        "type": "login_response",
                                        "success": true,
                                        "username": username,
                                        "image_id": usize_id
                                    });

                                    if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                        eprintln!("发送登录回复失败: {}", e);
                                    }
                                } else {
                                    // 密码错误
                                    let response = serde_json::json!({
                                        "type": "login_response",
                                        "success": false,
                                        "error": "账号或密码错误."
                                    });

                                    if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                        eprintln!("返回错误信息出错: {}", e);
                                    }
                                }
                            } else {
                                // 用户存在，但没有密码记录
                                let response = serde_json::json!({
                                    "type": "login_response",
                                    "success": false,
                                    "error": "Password not set."
                                });

                                if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                    eprintln!("Failed to send login_response message: {}", e);
                                }
                            }
                        } else {
                            // 用户已经登录
                            let response = serde_json::json!({
                                "type": "login_response",
                                "success": false,
                                "error": "用户已经登录，不可重复登录."
                            });

                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("Failed to send login_response message: {}", e);
                            }
                        }
                    }
            } else if client_message["type"] == "register" {
                    if let (Some(username), Some(password)) = (client_message["username"].as_str(), client_message["password"].as_str()) {
                        let mut user_store_lock = user_store.lock().await;
                        let user_id = user_ID.fetch_add(1, Ordering::SeqCst);

                        if user_store_lock.contains_key(username) {
                            let response = serde_json::json!({
                                "type": "register_response",
                                "success": false,
                                "error": "用户已经存在，请重新设计用户名."
                            });
                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("Failed to send register_response message: {}", e);
                            }
                        } else {
                            user_store_lock.insert(username.to_string(), (password.to_string(), user_id, tx.clone()));
                            let response = serde_json::json!({
                                "type": "register_response",
                                "success": true
                            });
                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("Failed to send register_response message: {}", e);
                            }
                        }
                    }
            }
        }
    }
}
}