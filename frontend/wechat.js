// 客户端发给服务端的消息
// request={ 
//     "type": 'private_message',
//     "from": currentUser, 
//     "to": currentChatUser, 
//     "message": message, 
//     "timestamp": timestamp 
// };
// 服务端返回的响应
// response={
//     "type": 'private_message',
//     "from": currentUser, 
//     "message": message, 
//     "timestamp": timestamp 
// }
// response={
//     "type": 'private_message',
//     "users": users, 
// }
// 增加用户时={
//      "type": 'add_user',
//      "user": String
// }
const ws_chat = new WebSocket('ws://127.0.0.1:3030/chat');
const currentUser = localStorage.getItem('username');  // 自己的用户名
let imageId = localStorage.getItem('imageId');  // 自己的头像编号
let currentChatUser = null;   // 当前聊天的用户

const chatlist = document.getElementById('chatlistbox');  // 好友列表
const chatBox = document.getElementById('chatBox_item');  // 右侧的聊天消息框
const messageInput = document.getElementById('chat_context_item');  // 输入框

let chatHistory = new Array(); // 保存聊天记录，{usersname(String): html标签}
let user_list = new Array();  // 用户名列表
let user_img = new Array();  // 用户名与图片的映射

// 判断头像和名字
if(currentUser == "undefined"){  // 还未登录
    alert("请先登录!");
    var pathname = window.location.pathname;
    if(pathname.search('html')==-1){
        window.location.href = 'login';
    }else{
        window.location.href = 'login.html';  
    }
} 
// 为左上方添加自己的头像和名字
let header = document.querySelector(".leftSide .header"); 
header.innerHTML = `<div class="userimg">
                        <img src="img/img${imageId}.jpg" class="cover">
                    </div>
                    <h4>${currentUser}</h4>
                    <ul class="nav_icons">
                        <li>
                            <ion-icon name="chatbubble-ellipses"></ion-icon>
                        </li>
                    </ul>`;

//websocket打开时的事件
ws_chat.onopen = function() {
    console.log('Connected to WebSocket');
    // 发送用户名给服务端，以便让它记住
    ws_chat.send(JSON.stringify({ 
        "type": 'add_user',
        "username": currentUser}));
};

// 收到服务器的消息
ws_chat.onmessage = function(event) {
    const data = JSON.parse(event.data);  // 使用json序列化与反序列化
    if (data["type"] === 'private_message') {   // 接收到私聊
        boxAddMessage(data["from"], currentUser, data["message"], data["timestamp"]);
    }else if( data["type"] === 'public_message'){  // 公聊
        boxAddMessage(data["from"], "Group", data["message"], data["timestamp"]);
    }else if(data["type"] === 'add_user'){  // 增加用户
        addUser(data["users"]);
    }else if (data["type"] === 'user_remove'){  // 删除用户
        removeUser(data["user"]);
    }else if(data["type"] === 'history'){  // 接收历史记录
        // 接收上次的文件历史
        let history = JSON.parse(data.history);  // history由json序列化和反序列化
        imageId = history.imageId;  // 自己的头像
        // 获取聊天记录，将二维数组转为字典
        
        chatHistory = Object.fromEntries(history.chatHistory);  // 聊天记录
    }
};

// 增加新用户
function addUser(users){
    console.log('有新用户上线') 
    // 对于每个之前没有记录过的用户，在左侧用户列表增加用户块
    users.forEach(function (user) {  // user 1号是name，2号是图像编号
        if (user[0] !== currentUser)
            if(!user_list.length || (user_list.length && user_list.indexOf(user[0])==-1)) {  // 判断是否记录过该用户
                const userBlock = document.createElement('li');
                userBlock.innerHTML = `
                    <div class="block active">
                        <!-- 头像 -->
                        <div class="imgbx">
                            <img src="img/img${user[1]}.jpg" class="cover">
                        </div>
                        <div class="details">
                            <div class="listhead">
                                <!-- 显示上线人员的网名 -->
                                <h4>${user[0]}</h4>
                                <!-- 显示消息时间 -->
                                <p class="time"></p>
                            </div>
                            <!-- 显示新收到的消息 -->
                            <div class="message_p">
                                <p></p>    <!--内容-->
                            </div>
                        </div>
                    </div>
                `;
                // 为用户块增加点击事件，以便选择用户
                userBlock.addEventListener('click', () => selectUser(user[0], userBlock));
                chatlist.firstElementChild.appendChild(userBlock);   // 放入用户列表中
                user_list.push(user[0]);   // 记录用户名
                user_img[user[0]] = user[1];    // 记录用户名与头像的映射
            }
    });
}

// 删除用户
function removeUser(user) {
    console.log('有用户下线');
    // 删除映射
    user_list.splice(user_list.indexOf(user));
    // 删除用户列表中的用户
    let chatList = document.querySelector('.chatlist').firstElementChild;
    for (let i = 0; i < chatList.children.length; i++) {
        if (chatList.children[i].querySelector('.listhead h4').innerText === user) {
            chatList.removeChild(chatList.children[i]);
            break;
        }
    }
    //可调用更新列表的函数
}

// 选择要聊天的用户
function selectUser(user, userBlock) {
    if(currentChatUser!=user){  
        //清除未读消息
        var messageBox = userBlock.querySelector('.message_p');
        if(messageBox){
            var messageCount = messageBox.firstElementChild.nextElementSibling;
            if(messageCount){
                messageBox.removeChild(messageCount);
            }
        }
        // 切换用户
        currentChatUser = user;
        if (chatHistory[currentChatUser]) {
            chatBox.innerHTML = '';
            chatBox.innerHTML = chatHistory[currentChatUser];
        }else{
            chatBox.innerHTML = '';
        }
        //切换图像与名字
        let top_header = document.getElementById("imgText");
        top_header.innerHTML = `<div class = "userimg">
                                    <img src = "img/img${user_img[currentChatUser]}.jpg" class="cover">
                                </div>
                                <h4>${currentChatUser}<br><span></span></h4>`;
    }
}

// 在聊天箱里增加新消息
function boxAddMessage(sendUser, receiveUser, message, timestamp) {  
    const messageDiv = document.createElement('div');
    if(receiveUser=="Group"){
        messageDiv.innerHTML = `
            <div class = "message ${currentUser === sendUser ? 'my_message' : 'friend_message'}">
                <div class="${currentUser === sendUser ? 'righimg' : 'leftimg'}">
                    <img src="img/img${currentUser === sendUser ? imageId : user_img[sendUser]}.jpg" class="cover">
                </div>
                <p>${message}<br><span>${timestamp}</span></p>
                <h4>${sendUser}</h4>
            </div>
            `;
    }else{
        messageDiv.innerHTML = `
            <div class = "message ${currentUser === sendUser ? 'my_message' : 'friend_message'}">
                <p>${message}<br><span>${timestamp}</span></p>
            </div>
        `;
    }
    if(receiveUser === currentUser){
        var peerUser = sendUser;  // 对方
    }else{
        var peerUser = receiveUser;  // 对方
    }
    // 增加到对应的聊天历史里
    if (!chatHistory[peerUser]) {
        chatHistory[peerUser] = "";
    }
    chatHistory[peerUser]+=messageDiv.outerHTML;
    
    // 在用户列表里显示新消息，
    // 拿到所有的用户块（用户块下有子节点用户名和消息时间
    //  ，其下一个兄弟节点的第一个子节点为消息，第二个表示是否未读，1表示为读）
    let userlist = document.querySelectorAll(".listhead");  
    var i=0;
    for(;i<userlist.length;i++){
        if(userlist[i].firstElementChild.innerText == peerUser){  // 要判断等于对方
            userlist[i].firstElementChild.nextElementSibling.innerText = timestamp;  // 设置消息时间
            userlist[i].nextElementSibling.firstElementChild.innerText = message;  // 设置消息内容
            // 判断是否要显示未读
            if(peerUser !== currentChatUser){
                if(!userlist[i].nextElementSibling.firstElementChild.nextElementSibling){
                    const messageCount = document.createElement('b');
                    messageCount.innerHTML = 1;
                    userlist[i].nextElementSibling.appendChild(messageCount);
                }else{
                    userlist[i].nextElementSibling.firstElementChild.nextElementSibling.innerHTML++;
                }
            }
        }
    }
    // 发送方即为当前的聊天方，在聊天箱里新增消息
    if(currentChatUser === peerUser){  
        chatBox.appendChild(messageDiv);
        chatBox.scrollTop = chatBox.scrollHeight;
    }
}

// 发送消息
function sendMessage() {
    // 如果没有选择聊天用户
    if(!currentChatUser){
        alert("请选择您要聊天的好友");
    }
    const message = messageInput.value;
    if (message && currentChatUser) {  // 判断message是否为空以及删去空格后是否为空，并且判断是否已经选了要发消息的对象
        const timestamp = new Date().toLocaleTimeString();  // 获取时间戳
        if(currentChatUser !="Group"){
            boxAddMessage(currentUser, currentChatUser, message, timestamp);   // 在己方的对话框显示消息
        }
        if(currentChatUser === "Group"){
            var message_type = 'public_message';
        }else{
            var message_type = 'private_message';
        }

        // 发送消息
        ws_chat.send(JSON.stringify({ 
            "type": message_type,
            "from": currentUser, 
            "to": currentChatUser, 
            "message": message, 
            "timestamp": timestamp }));
        messageInput.value = '';  //清空输入框
    }
}

document.getElementById('button').addEventListener('click', sendMessage);

// 关闭页面
// 用于判断界面是否关闭
let isPageClosing = false;

// 监听用户关闭页面
window.addEventListener('beforeunload', function(event) {
    if (ws_chat.readyState === WebSocket.OPEN && !isPageClosing) {
        // 将原来的chatHistory字典转为2维数组，方便由json进行序列化
        const chat_entries = Object.entries(chatHistory);
        let history = JSON.stringify({
            "imageId": imageId,
            "chatHistory": chat_entries
        })
        ws_chat.send(JSON.stringify({
            "type": "logout",
            "user": currentUser,
            "history": history
        }));
    }
    // // 让浏览器显示确认离开的对话框  todo
    // event.preventDefault(); // 现代浏览器可能需要这行来触发提示
    // event.returnValue = ''; // 兼容老旧浏览器
});

// 监听用户实际关闭页面事件
window.addEventListener('unload', function(event) {
    isPageClosing = true; // 仅在用户关闭页面时标记
});

// // 回车键发送消息（也可以删掉，让回车表换行）
// messageInput.addEventListener('keypress', (e) => {
//     if (e.key === 'Enter') {
//         e.preventDefault();
//         sendMessage();
//     }
// });
