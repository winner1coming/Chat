
function $(id){
    return typeof id=="string"?document.getElementById(id):id;
}

window.onload = function(){
    var titleName = $("chatlistbox").getElementsByTagName("li");
    var tabcontent = $("rightSide_item").getElementsByTagName("div");
    var parentDiv = $("rightSide_item");
    var outerDivs = [];
    if(parentDiv)
    {
        var children = parentDiv.childNodes;
        var outerDiv = parentDiv.querySelectorAll(":scope > div");
        for (var i = 0; i < children.length; i++) {
            if (children[i].nodeType === 1 && children[i].tagName.toLowerCase() === "div") {
                outerDivs.push(children[i]);
            }
        }
    }
    // alert(outerDivs.length);
    //聊天界面切换
    for(var i = 0; i<titleName.length;i++)
    {
        titleName[i].id = i;
        // 内容的显示与隐藏
        titleName[i].onclick = function()
        {
            // 消除新消息气泡提示
            var b_label = titleName[this.id].getElementsByTagName("b")[0];
            b_label.style.display = "none";
            // 显示对应的页面
            for( var j=0 ;j<titleName.length;j++)
            {
                outerDivs[j].style.display = "none";
            }
            outerDivs[this.id].style.display = "block";

        }
    }
}