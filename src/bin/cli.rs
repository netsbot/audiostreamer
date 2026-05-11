#[tokio::main]
async fn main() {
    env_logger::init();

    tokio::select! {
        result = audiostreamer::app::run() => {
            if let Err(err) = result {
                eprintln!("error: {err}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\n[!] Ctrl+C received, exiting...");
        }
    }
    
    // Always try to cleanup wrapper
    let _ = audiostreamer::am_wrapper::shutdown().await;
}
