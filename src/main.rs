use std::io::{BufWriter, Write};
use std::path::Path;
use serde_json::Value;
use warp::Filter;
use warp::ws::{Message, WebSocket};
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use futures::{SinkExt, StreamExt};

//定义一个常量用于计算注册的人数
static user_ID : AtomicUsize = AtomicUsize::new(1);

/// 定义一个 `add_user` 类型，它是一个线程安全的用户列表，
/// 用 `Arc` 包裹的 `tokio::sync::Mutex`，内部包含一个 `HashMap`，
/// 键是用户名，值是 `mpsc::UnboundedSender`，用于发送 WebSocket 消息。
type add_user = Arc<tokio::sync::Mutex<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

//类似的定义一个User Store类型的用户列表，用于注册时保存用户信息
type UserStore = Arc<tokio::sync::Mutex<HashMap<String,(String,usize, mpsc::UnboundedSender<Result<Message,warp::Error>>)>>>;


#[tokio::main]
async fn main() {
    // 初始化 `users`和'user_store'，这是一个共享的、线程安全的用户列表。
    let users = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let user_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    
    // 添加 "Group" 用户到已登录的用户列表
    let (group_tx, _group_rx) = mpsc::unbounded_channel();
    {
    let mut user_lock = users.lock().await; 
    user_lock.insert("Group".to_string(), group_tx.clone());
    }
    
    //添加"Group"用户到已注册的用户列表
    {
        //保证安全
        let mut store_lock = user_store.lock().await;

        let group_id = user_ID.fetch_add(1, Ordering::SeqCst); // 获取下一个 ID
        store_lock.insert(
            "Group".to_string(),
            ("default_password".to_string(), group_id, group_tx.clone()) // 使用默认密码
        );
    }
    
    // 创建一个 `users` 过滤器，用于将 `users` 传递给 Warp 处理函数。user_store同理
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
    //局部测试println!("你有进入到这个界面吗？？");
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
                    eprintln!("(users_connect)用户连接发送消息环节失败: {}", e);
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
                eprintln!("WebSocket 出错了！: {}", e);
                break;
            }
        };

        // 将消息转换为字符串
        if let Ok(msg_str) = message.to_str() {
            //进行消息的获取（解析获得）
            if let Ok(client_message) = serde_json::from_str::<Value>(msg_str) {
                //接收到客户端的消息，消息类型为add_user——用户登录进来
                if client_message["type"] == "add_user" {
                    if let Some(username) = client_message["username"].as_str() {  //从发来的消息获取用户名
                        //保证安全的获取信息
                        let mut user_lock = users.lock().await;
                        let store_lock = user_store.lock().await;
                        user_lock.insert(username.to_string(), tx.clone());   //将新的用户放入登录列表中
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
                        /*
                        以上获取到所有在线用户的用户名和对应的编号，用于广播消息
                        用于测试消息的传递
                        println!("用户名");
                        for i in user_ids.clone(){
                            println!(" 用户名{}，ID{}\n",i.0,i.1);
                        }*/
                        
                        // 设置广播信息
                        let add_msg = serde_json::json!({
                            "type": "add_user",
                            "users": user_ids     //类型为Vec<String, usize>
                        });


                        //实现广播消息给所有在线用户
                        for (user, user_tx) in user_lock.iter(){
                        if let Err(e) = user_tx.send(Ok(Message::text(add_msg.to_string()))){
                            eprintln!("Failed to broadcast user list to {}:{}",user,e);
                        }
                       }

                       // 处理用户历史记录
                       let file_path = format!("./chat_history/{}.json", username);
                       if Path::new(&file_path).exists() {
                           // 读取历史记录文件
                           match fs::read_to_string(&file_path) {
                               Ok(content) => {
                                   if let Ok(history) = serde_json::from_str::<Value>(&content) {
                                       let history_msg = serde_json::json!({
                                           "type": "history",
                                           "history": history
                                       });
                                       //将历史数据发送给客户端的用户
                                       if let Err(e) = tx.send(Ok(Message::text(history_msg.to_string()))) {
                                           eprintln!("发送历史数据失败 {}: {}", username, e);
                                       }
                                   } else {
                                       eprintln!("无法打开历史文件 {}", username);
                                   }
                               }
                               Err(e) => {
                                   eprintln!("无法正确读取历史文件 {}: {}", username, e);
                               }
                            }
                        }
                    }

                }else if client_message["type"] == "private_message" {  //私发消息
                    //得到消息发送对象以及具体消息
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

                } else if client_message["type"] == "public_message" {   //公聊消息传递
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
                }else if client_message["type"] == "logout" {     //下线处理
                    let user = client_message["user"].as_str().unwrap_or("");    //获取下线用户的用户名
                    let mut users_lock = users.lock().await;
                    users_lock.remove(user);       //将下线用户从在线用户列表删除
                    //用于局部测试println!("{}",user);
                    
                    //实现将消息放在同一个文件夹下，文件夹存在则放入，不存在则创建
                    let history_dir = "./chat_history";
                    fs::create_dir_all(history_dir).unwrap_or_else(|e| {
                        eprintln!("创建目录失败: {}", e);
                    });

                    // 文件路径
                    let file_path = format!("{}/{}.json", history_dir, user);

                    // 获取并保存历史消息
                    if let Some(history) = client_message.get("history") {
                        // 将历史消息序列化为字符串
                        //let history_str = serde_json::to_string_pretty(&history).unwrap_or_default();
                        
                        // 打开或创建文件
                        let file = OpenOptions::new().write(true).create(true).open(file_path).unwrap();
                        let mut writer = BufWriter::new(file);

                        // 写入历史消息
                        if let Err(e) = writeln!(writer, "{}", history) {
                            eprintln!("保存历史消息失败: {}", e);
                        }

                    }

                    //编辑删除消息用于广播给所有在线客户端
                    let user_left_msg = serde_json::json!({
                        "type": "user_remove",
                        "user": user
                    });

                    //广播消息
                    for (_, user_tx) in users_lock.iter() {
                        if let Err(e) = user_tx.send(Ok(Message::text(user_left_msg.to_string()))) {
                            eprintln!("Failed to notify user disconnected: {}", e);
                        }
                    }
                    //用于局部测试println!("有用户退出登录了：{}",user);
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
                    eprintln!("(login_handle)登录注册尝试连接服务器失败: {}", e);
                }
            }
        }
    });

    while let Some(result) = user_ws_rx.next().await {
        let message = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("登陆阶段 WebSocket 出错了！: {}", e);
                break;
            }
        };

        if let Ok(msg_str) = message.to_str() {
            if let Ok(client_message) = serde_json::from_str::<Value>(msg_str) {
                // 处理登录消息。
                if client_message["type"] == "login" {
                    if let (Some(username), Some(password)) = (     //获取客户端传来的用户名和密码
                        client_message["username"].as_str(),
                        client_message["password"].as_str()
                    ) {
                        let user_store_lock = user_store.lock().await;
                        let users_lock = users.lock().await;

                        // 首先检查用户名是否已经在 add_user（在线用户列表） 中
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
                                    "error": "账号或密码错误."
                                });

                                if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                    eprintln!("返回登录消息给客户端失败1: {}", e);
                                }
                            }
                        } else {
                            // 用户已经登录，不允许用户重复登录
                            let response = serde_json::json!({
                                "type": "login_response",
                                "success": false,
                                "error": "用户已经登录，不可重复登录."
                            });

                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("返回登录消息给客户端失败2: {}", e);
                            }
                        }
                    }
                
                } else if client_message["type"] == "register" {    //处理用户注册
                    //获取用户注册输入的账号和密码
                    if let (Some(username), Some(password)) = (client_message["username"].as_str(), client_message["password"].as_str()) {
                        let mut user_store_lock = user_store.lock().await;
                        let user_id = user_ID.fetch_add(1, Ordering::SeqCst);      //登录用户使得全局计数变量加一

                        //在已注册过的用户名中查找——不可重复的保证
                        if user_store_lock.contains_key(username) {
                            let response = serde_json::json!({
                                "type": "register_response",
                                "success": false,
                                "error": "用户已经存在，请重新设计用户名."
                            });
                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("返回注册消息给客户端失败1: {}", e);
                            }
                        } else {
                            //成功则将信息保存下来
                            user_store_lock.insert(username.to_string(), (password.to_string(), user_id, tx.clone()));
                            let response = serde_json::json!({
                                "type": "register_response",
                                "success": true
                            });
                            if let Err(e) = tx.send(Ok(Message::text(response.to_string()))) {
                                eprintln!("返回注册消息给客户端失败2: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }
}
