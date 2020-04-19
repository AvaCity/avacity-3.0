mod client;
mod encoder;
mod decoder;
mod server;
mod common;
mod base_messages;
use server::Server;

fn main() {
    let srv = Server::new();
    srv.listen();
}
