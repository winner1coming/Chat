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
let currentUser = localStorage.getItem('username');  // 自己的用户名
let currentChatUser = null;   // 当前聊天的用户

const chatlist = document.getElementById('chatlistbox');  // 好友列表
const chatBox = document.getElementById('chatBox_item');  // 右侧的聊天消息框
const messageInput = document.getElementById('chat_context_item');  // 输入框
// const chatUserName = document.getElementById('chat_user_name');  // 对方的用户名
// const chatUserStatus = document.getElementById('chat_user_status');  // 是否在线
// const chatUserImg = document.getElementById('chat_user_img');  // 对方的头像

let chatHistory = new Array() // 保存聊天记录，{usersname(String): html标签}

// todo test
let userBlock = chatlist.firstElementChild.firstElementChild;
userBlock.addEventListener('click', () => selectUser("user", userBlock));


ws_chat.onopen = function() {
    console.log('Connected to WebSocket');
};

// 收到服务器的消息
ws_chat.onmessage = function(event) {
    const data = JSON.parse(event.data);  // 使用json序列化与反序列化
    console.log('收到消息')  // todo: debug
    console.log(data)  // todo: debug
    if (data["type"] === 'private_message') {   // 接收到私聊
        boxAddMessage(data["from"], currentUser, data["message"], data["timestamp"]);
    }else if(data["type"] === 'add_user'){
        addUser(data["user"]);
    }
    // } else if (data["type"] === 'update_users') {  // 更新用户列表
    //     console.log('即将更新用户列表')  // todo: debug
    //     updateUsersList(data["users"]);
    // }
};

// 增加新用户
function addUser(user){
    console.log('有新用户上线') 
    if (user !== currentUser) {
        const userBlock = document.createElement('li');
        userBlock.innerHTML = `
            <div class="block active">
                <!-- 头像 -->
                <div class="imgbx">
                    <img src="img1.jpg" class="cover">
                </div>
                <div class="details">
                    <div class="listhead">
                        <!-- 显示上线人员的网名 -->
                        <h4>${user}</h4>
                        <!-- 显示消息时间 -->
                        <p class="time"></p>
                    </div>
                    <!-- 显示新收到的消息 -->
                    <div class="message_p">
                        <p></p>    <!--内容-->
                        <b></b>    <!--1表示为读-->
                    </div>
                </div>
            </div>
        `;
        userBlock.addEventListener('click', () => selectUser(user, userBlock));  // todo 冲突
        chatlist.firstElementChild.appendChild(userBlock);
    }
}

// 选择要聊天的用户
function selectUser(user, userBlock) {
    // 保存与上一个用户聊天的记录（deleted，因为boxAddMessage里会直接保存历史）
    // if (!currentChatUser){ 
    //     chatHistory[currentChatUser] = chatBox.innerHTML;
    // }
    if(!currentChatUser){  //清除未读消息
        userBlock.querySelector('.message_p').firstElementChild.nextElementSibling = ''
    }
    currentChatUser = user;
    // chatUserName.innerText = user;  
    // chatUserStatus.innerText = 'Online';
    // chatUserImg.src = 'default.jpg';
    if(!chatHistory && chatHistory.find(currentChatUser)){
        chatBox.innerHTML = chatHistory[currentChatUser];
    }
    else{
        chatBox.innerHTML = '';
    }
}

// 在聊天箱里增加新消息
function boxAddMessage(sendUser, receiveUser, message, timestamp) {  
    const messageDiv = document.createElement('div');
    messageDiv.className = `message ${currentUser === sendUser ? 'my_message' : 'friend_message'}`;
    messageDiv.innerHTML = `
        <p>${message}<br><span>${timestamp}</span></p>
    `;
    if(sendUser === currentUser){
        var peerUser = receiveUser;  // 对方
    }else{
        var peerUser = sendUser;  // 对方
    }
    // 增加到对应的聊天历史里
    chatHistory[peerUser].appendChild(messageDiv);
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
                userlist[i].nextElementSibling.firstElementChild.innerHTML = 1;
            }
        }
    }
    // 发送方即为当前的聊天方，在聊天箱里新增消息
    if(currentChatUser === sendUser){  
        chatBox.appendChild(messageDiv);
        chatBox.scrollTop = chatBox.scrollHeight;
    }
}

function sendMessage() {
    const message = messageInput.value;
    if (message && currentChatUser) {  // 判断message是否为空以及删去空格后是否为空，并且判断是否已经选了要发消息的对象
        console.log('发送消息'+message);  // todo debug
        const timestamp = new Date().toLocaleTimeString();  // 获取时间戳
        boxAddMessage(currentUser, currentChatUser, message, timestamp);   // 在己方的对话框显示消息
        // 发送消息
        ws_chat.send(JSON.stringify({ 
            "type": 'private_message',
            "from": currentUser, 
            "to": currentChatUser, 
            "message": message, 
            "timestamp": timestamp }));
        messageInput.value = '';  //清空输入框
    }
}

document.getElementById('button').addEventListener('click', sendMessage);

// 回车键发送消息（也可以删掉，让回车表换行）
messageInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
        e.preventDefault();
        sendMessage();
    }
});
