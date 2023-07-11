use std::net::TcpListener;

pub struct TcpHandler {
    url: String,
    listener: TcpListener,
}

impl TcpHandler {
    pub fn new(url: String, listener: TcpListener) -> Self {
        TcpHandler { url, listener }
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }
}
