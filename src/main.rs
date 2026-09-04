#![windows_subsystem = "windows"]

use std::sync::Arc;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;
use tray_icon::menu::MenuEvent;
use windows_sys::Win32::System::Console::FreeConsole;
use winit::event_loop::{ControlFlow, EventLoopBuilder};

use wincallnotifier::app::{AppCommand, AppService};
use wincallnotifier::config::AppConfig;
use wincallnotifier::ui::{StatusWindow, SystemTrayManager};

fn init_logging(log_level_str: &str) {
    let level = match log_level_str.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}

fn main() -> anyhow::Result<()> {
    // 1. Immediately detach and close any calling console / CLI prompt
    unsafe {
        FreeConsole();
    }

    // 2. Load persisted configuration
    let config = AppConfig::load();
    init_logging(&config.log_level);

    // 3. Build multi-threaded Tokio runtime for background ADB operations
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
    );

    // 4. Create core application service
    let service = AppService::new(config.clone());
    let cmd_tx = service.sender();
    let status = service.status();

    // 5. Spawn background service onto Tokio runtime
    let _service_handle = rt.spawn(async move {
        service.run().await;
    });

    // 6. Open the GUI window immediately upon launch
    StatusWindow::show(status.clone());

    // 7. Build Winit event loop for tray icon and Windows message handling
    let event_loop = EventLoopBuilder::new().build()?;

    // 8. Initialize System Tray Icon
    let tray_manager = match SystemTrayManager::new(status.clone(), config.enable_monitoring) {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to create system tray: {e}");
            return Err(e);
        }
    };

    let menu_channel = MenuEvent::receiver();

    // 9. Run Windows event loop
    event_loop.run(move |_event, target| {
        target.set_control_flow(ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(200),
        ));

        // Update status text on tray menu asynchronously via runtime handle
        let tray_ref = &tray_manager;
        rt.block_on(async {
            tray_ref.update_status().await;
        });

        // Handle menu events
        while let Ok(menu_event) = menu_channel.try_recv() {
            let id = menu_event.id;

            if id == tray_manager.menu_ids.toggle_monitoring {
                let current_checked = tray_manager.is_monitoring_checked();
                let new_val = !current_checked;
                tray_manager.set_monitoring_checked(new_val);
                let _ = cmd_tx.try_send(AppCommand::ToggleMonitoring(new_val));
                info!("Toggled monitoring via tray: {}", new_val);
            } else if id == tray_manager.menu_ids.test_notification {
                let _ = cmd_tx.try_send(AppCommand::TestNotification);
            } else if id == tray_manager.menu_ids.show_status {
                StatusWindow::show(status.clone());
            } else if id == tray_manager.menu_ids.restart_adb {
                let _ = cmd_tx.try_send(AppCommand::RestartAdb);
            } else if id == tray_manager.menu_ids.about {
                StatusWindow::show(status.clone());
            } else if id == tray_manager.menu_ids.exit {
                info!("Exiting WinCallNotifier application...");
                let _ = cmd_tx.try_send(AppCommand::Quit);
                target.exit();
            }
        }
    })?;

    Ok(())
}
