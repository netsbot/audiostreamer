use log::LevelFilter;

#[tauri::command]
async fn run_stream() -> Result<(), String> {
    crate::app::run().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_apple_music_token() -> Result<String, String> {
    crate::client::AppleMusicClient::fetch_token().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_apple_music_user_token() -> Result<String, String> {
    crate::am_wrapper::get_music_token()
        .await
        .map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(LevelFilter::Info)
                        .build(),
                )?;
            }
            app.handle().plugin(tauri_plugin_http::init())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_stream,
            get_apple_music_token,
            get_apple_music_user_token
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
