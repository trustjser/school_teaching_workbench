//! 局域网与 P2P 网络层出口：mDNS 自发现、Axum 本地 API、验签中间件、心跳、客户端。

pub mod client;
pub mod discovery;
pub mod handlers;
pub mod heartbeat;
pub mod middleware;
pub mod server;
