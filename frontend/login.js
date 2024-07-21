// 客户端发给服务端的消息：
        // request={
        //      "type":"login", 
        //      "username": string 
        //      "password": string
        // }
        //服务端返回的响应消息为：
        // response={
        //      "type": "login_response",
        //      "success": true,
        //      "username": str
        //      "image_id": int
        // }
        // response={
        //      "type": "login_response",
        //      "success": false,
        //      "error": str
        // }
        //
        // 客户端发给服务端的消息：
        // request={
        //      "type":"register", 
        //      "username": string 
        //      "password": string
        // }
        //服务端返回的响应消息为：
        // response={
        //      "type": "register_response",
        //      "success": bool
        // }
        const ws = new WebSocket('ws://127.0.0.1:3030/login');
        const usernameInput = document.getElementById('username');
        const passwordInput = document.getElementById('password');
        ws.onopen = function() {
            console.log('Connected to WebSocket');
        };
        ws.onmessage = function(event){
            const data = JSON.parse(event.data);  // data中有success(bool)和username(string)
            if(data.type === 'login_response'){
                if (data.success) {
                    localStorage.setItem('username', data.username);   // 设置用户名
                    localStorage.setItem('imageId', data.image_id);   // 设置密码
                    var pathname = window.location.pathname;
                    if(pathname.search('html')==-1){
                        window.location.href = 'wechat';
                    }else{
                        window.location.href = 'wechat.html';  
                    }
                }else {
                    alert(data.error);
                }
            }else{
                if (data.success) {
                    alert('注册成功，请登录');
                } else {
                    alert('用户名已被占用，请更换用户名');
                }
            }
            
        };

        // 登录部分
        function loginEvent(){
            const username = usernameInput.value;
            const password = passwordInput.value;
            ws.send(JSON.stringify({ "type":"login", "username": username, "password": password }));
        }
        document.getElementById('loginButton').addEventListener('click', loginEvent);

        // 注册部分
        document.getElementById('registerButton').addEventListener('click', () => {
            const username = usernameInput.value;
            const password = passwordInput.value;
            if(username == "undefined" || username == ""){
                alert("请输入合法的用户名")
            }else if(password == "" || password == "undefined"){
                alert("请输入合法的密码")
            }
            else{
                ws.send(JSON.stringify({ "type":"register", "username": username, "password": password }));
            }
        });

        // 键盘监听事件
        // 当在账户名处输入回车时，跳转到输入密码处
        // 当在密码处输入回车时，默认等效于点击登录
        passwordInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                loginEvent();
            }
        });
        