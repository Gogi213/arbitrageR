use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main]
async fn main() {
    let url = "wss://fstream.binance.com/ws";
    println!("Connecting to {}...", url);
    
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    println!("Connected!");
    
    let subscribe = serde_json::json!({
        "method": "SUBSCRIBE",
        "params": ["btcusdt@bookTicker"],
        "id": 1
    });
    
    ws_stream.send(Message::Text(subscribe.to_string().into())).await.expect("Failed to send");
    println!("Subscribed!");
    
    while let Some(msg) = ws_stream.next().await {
        match msg {
            Ok(m) => println!("Received: {:?}", m),
            Err(e) => println!("Error: {}", e),
        }
    }
    
    println!("Connection closed");
}