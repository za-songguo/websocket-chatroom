use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::channel::mpsc::{self, UnboundedSender};
use futures::{future, StreamExt, TryStreamExt};

use tokio_tungstenite::tungstenite::Message;

/// 存储所有在线的客户端，sender 是用来方便其他 peer 广播消息给当前 peer 的
type PeerMap = Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Message>>>>;

const SERVER_ADDR: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind(SERVER_ADDR).await.unwrap();
    println!("服务器地址：{}", SERVER_ADDR);

    let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));

    // 为每个新连接分配一个 Tokio 任务
    while let Ok((stream, peer_addr)) = listener.accept().await {
        let peers = Arc::clone(&peers);
        tokio::spawn(handle_connection(stream, peers, peer_addr));
    }
}

async fn handle_connection(
    tcp_stream: tokio::net::TcpStream,
    peers: PeerMap,
    peer_addr: SocketAddr,
) {
    // WebSocket 握手
    let ws_stream = tokio_tungstenite::accept_async(tcp_stream)
        .await
        .expect("握手时发生错误");
    println!("成功建立 WebSocket 连接: {}", peer_addr);

    // sender 用来提供给其他 peer，用于给当前 peer 广播消息
    // receiver 用来接收其他 peer 给当前 peer 广播的消息
    // 有界（bounded）通道对通道可以存储的消息数量有限制，如果达到此限制，尝试发送另一条消息将需要等待，直到接收端从通道接收积压（未被接收）的消息为止。无界（unbounded）通道具有无限容量，积压的消息过多可能会耗尽系统内存
    let (msg_sender, msg_receiver) = mpsc::unbounded();

    let (write, read) = ws_stream.split();

    // 把新的客户端加入到 peers 中
    peers.lock().unwrap().insert(peer_addr, msg_sender);

    let receive_and_broadcast_messages = read.try_for_each(|message| {
        match message.clone() {
            // 这里只广播文字类型的消息
            Message::Text(text_message) => {
                let text_message = format!("{}：{}", peer_addr, text_message);
                println!("{text_message}");
                // 向其他客户端广播收到的消息
                let peers = peers.lock().unwrap();

                let broadcast_peers = peers.iter().filter(|(addr, _)| addr != &&peer_addr);

                for (broadcast_addr, broadcast_peer) in broadcast_peers {
                    // 如果这个 peer 的 channel 没有关闭（这个 peer 没有断开连接）
                    // 如果 peer 断开了连接，peer 就会从 peers 中移除，channel 自然也就关闭了，其他 peer 无法再给其广播消息
                    if !broadcast_peer.is_closed() {
                        // 向这个 peer 发送消息
                        if let Err(err) = broadcast_peer.unbounded_send(text_message.clone().into())
                        {
                            eprintln!("无法广播消息: {:?}（{}）", err, broadcast_addr);
                        }
                    }
                }
            }
            Message::Close(_) => {
                // 客户端断开了连接
                peers.lock().unwrap().remove(&peer_addr);
                println!("{} 断开了连接", peer_addr);
            }
            _ => {
                eprintln!("不支持的数据类型");
            }
        }

        future::ok(())
    });

    // 监听当前 peer 的 receiver，在接收到其他 peer 广播的消息的时候通过 WebSocket 转发给当前的 peer
    let forward_messages = msg_receiver.map(Ok).forward(write);

    if let Err(err) = tokio::try_join!(receive_and_broadcast_messages, forward_messages) {
        // 只有 forward_messages 会产生错误
        // 发生错误时，连接会断开
        eprintln!("广播消息时发生错误: {:?}", err);
        // 移除这个 peer
        peers.lock().unwrap().remove(&peer_addr);
        println!("{} 断开了连接", peer_addr);
    }
}
