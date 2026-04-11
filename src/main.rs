#[tokio::main]
async fn main() {
    env_logger::init();

    audiostreamer::app::run().await.unwrap();
}
