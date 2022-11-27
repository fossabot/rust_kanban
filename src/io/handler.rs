use std::{sync::Arc, path::PathBuf};
use crate::app::kanban::Board;
use std::env;
use crate::constants::{
    CONFIG_DIR_NAME,
    CONFIG_FILE_NAME, SAVE_DIR_NAME
};
use crate::app::AppConfig;
use crate::io::data_handler::{reset_config, save_kanban_state_locally};
use eyre::Result;
use log::{error, info, debug};

use super::IoEvent;
use super::data_handler::{get_available_local_savefiles, get_local_kanban_state};
use crate::app::App;

/// In the IO thread, we handle IO event without blocking the UI thread
pub struct IoAsyncHandler {
    app: Arc<tokio::sync::Mutex<App>>,
}

impl IoAsyncHandler {
    pub fn new(app: Arc<tokio::sync::Mutex<App>>) -> Self {
        Self { app }
    }

    /// We could be async here
    pub async fn handle_io_event(&mut self, io_event: IoEvent) {
        let result = match io_event {
            IoEvent::Initialize => self.do_initialize().await,
            IoEvent::GetLocalData => self.get_local_save().await,
            IoEvent::GetCloudData => self.get_cloud_save().await,
            IoEvent::Reset => self.reset_config().await,
            IoEvent::SaveLocalData => self.save_local_data().await,
        };

        if let Err(err) = result {
            error!("Oops, something wrong happen: {:?}", err);
        }

        let mut app = self.app.lock().await;
        app.loaded();
    }

    /// We use dummy implementation here, just wait 1s
    async fn do_initialize(&mut self) -> Result<()> {
        info!("🚀 Initialize the application");
        let mut app = self.app.lock().await;
        if !prepare_config_dir() {
            error!("Cannot create config directory");
        }
        if !prepare_save_dir() {
            error!("Cannot create save directory");
        }
        app.boards = prepare_boards();
        debug!("Boards: {:?}", app.boards);
        app.initialized(); // we could update the app state
        info!("👍 Application initialized");
        Ok(())
    }

    async fn get_local_save(&mut self) -> Result<()> {
        info!("🚀 Getting local save");
        let mut app = self.app.lock().await;
        app.set_boards(vec![]);
        info!("👍 Local save loaded");
        Ok(())
    }

    async fn get_cloud_save(&mut self) -> Result<()> {
        info!("🚀 Getting cloud save");
        let mut app = self.app.lock().await;
        app.set_boards(vec![]);
        info!("👍 Cloud save loaded");
        Ok(())
    }

    async fn reset_config(&mut self) -> Result<()> {
        info!("🚀 Resetting config");
        reset_config();
        info!("👍 Config reset");
        Ok(())
    }

    async fn save_local_data(&mut self) -> Result<()> {
        info!("🚀 Saving local data");
        let app = self.app.lock().await;
        let board_data = &app.boards;
        let status = save_kanban_state_locally(board_data.to_vec());
        match status {
            Ok(_) => info!("👍 Local data saved"),
            Err(err) => error!("Cannot save local data: {:?}", err),
        }
        Ok(())
    }
}

pub(crate) fn get_config_dir() -> PathBuf {
    let mut config_dir = home::home_dir().unwrap();
    config_dir.push(".config");
    config_dir.push(CONFIG_DIR_NAME);
    config_dir
}

pub(crate) fn get_save_dir() -> PathBuf {
    let mut save_dir = env::temp_dir();
    save_dir.push(SAVE_DIR_NAME);
    save_dir
}

fn prepare_config_dir() -> bool {
    let config_dir = get_config_dir();
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).unwrap();
    }
    // make config file if it doesn't exist and write default config to it
    let mut config_file = config_dir.clone();
    config_file.push(CONFIG_FILE_NAME);
    if !config_file.exists() {
        let default_config = AppConfig::default();
        let config_json = serde_json::to_string_pretty(&default_config).unwrap();
        std::fs::write(&config_file, config_json).unwrap();
    }
    true
}

fn prepare_save_dir() -> bool {
    let save_dir = get_save_dir();
    if !save_dir.exists() {
        std::fs::create_dir_all(&save_dir).unwrap();
    }
    true
}

fn prepare_boards () -> Vec<Board> {
    let local_save_files = get_available_local_savefiles();
    let fall_back_version = "1".to_string();
    let latest_version = local_save_files.iter().max().unwrap_or(&fall_back_version);
    // get v1, v2 version number from latest_version
    let mut version_number = latest_version.split("v").collect::<Vec<&str>>();
    // get last version number
    let last_version_number = version_number.pop().unwrap_or("1");
    // convert to u32
    let last_version_number = last_version_number.parse::<u32>().unwrap_or(1);
    let local_data = get_local_kanban_state(last_version_number);
    match local_data {
        Ok(data) => {
            info!("👍 Local data loaded from {}", latest_version);
            data
        },
        Err(err) => {
            error!("Cannot get local data: {:?}", err);
            info!("👍 Local data loaded from default");
            vec![Board::default()]
        },
    }
}