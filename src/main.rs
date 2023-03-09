use clap::Parser;
use crossterm::event::DisableMouseCapture;
use crossterm::{execute, terminal};
use std::io::stdout;
use std::sync::Arc;
use tui::backend::CrosstermBackend;
use tui::Terminal;

use eyre::Result;
use log::LevelFilter;
use rust_kanban::start_ui;
use rust_kanban::{
    app::App,
    io::{handler::IoAsyncHandler, IoEvent},
};

extern crate savefile_derive;
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    // optional argument to reset config
    #[arg(short, long)]
    reset: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Handling Panic when terminal is in raw mode
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        _ = terminal::disable_raw_mode();
        let execute_result = execute!(stdout(), DisableMouseCapture);
        if let Err(e) = execute_result {
            println!("Error while disabling mouse capture: {}", e);
        }
        println!();
        let stdout = stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend);
        if terminal.is_ok() {
            let _ = terminal.unwrap().clear();
        }
        default_panic(info);
    }));

    // parse cli args
    let args = CliArgs::parse();

    let (sync_io_tx, mut sync_io_rx) = tokio::sync::mpsc::channel::<IoEvent>(100);

    // We need to share the App between thread
    let main_app_instance = Arc::new(tokio::sync::Mutex::new(App::new(sync_io_tx.clone())));
    let app_widget_manager_instance = Arc::clone(&main_app_instance);
    let app_ui_instance = Arc::clone(&main_app_instance);
    // Configure log
    tui_logger::init_logger(LevelFilter::Debug).unwrap();
    tui_logger::set_default_level(log::LevelFilter::Debug);

    // Handle IO in a specifc thread
    tokio::spawn(async move {
        let mut handler = IoAsyncHandler::new(main_app_instance);
        while let Some(io_event) = sync_io_rx.recv().await {
            handler.handle_io_event(io_event).await;
        }
    });

    tokio::spawn(async move {
        let mut widget_manager =
            rust_kanban::ui::widgets::WidgetManager::new(app_widget_manager_instance);
        loop {
            widget_manager.update().await;
        }
    });

    // TODO: get term bg color
    // let term_bg = get_term_bg_color();

    // check if we need to reset config
    if args.reset.is_some() {
        sync_io_tx.send(IoEvent::Reset).await.unwrap();
    }

    start_ui(&app_ui_instance).await?;

    Ok(())
}
