#[tokio::main]
async fn main() {
    env_logger::init();

    if let Err(err) = audiostreamer::app::run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
