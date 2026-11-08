//! Watcher de lembretes: notifica tasks cujo `remind`/due chegou.

use std::path::PathBuf;
use std::time::Duration;

use tauri_plugin_notification::NotificationExt;

pub fn start(app: tauri::AppHandle, db_path: PathBuf) {
    std::thread::spawn(move || {
        let conn = match crate::db::open(&db_path) {
            Ok(conn) => conn,
            Err(err) => {
                eprintln!("[betastart] reminder watcher: {err}");
                return;
            }
        };

        loop {
            if let Ok(due) = crate::db::tasks::due_reminders(&conn, crate::db::now_secs()) {
                for (reminder_id, title) in due {
                    let _ = app
                        .notification()
                        .builder()
                        .title("Beta Start — lembrete")
                        .body(title.clone())
                        .show();
                    let _ = crate::db::tasks::mark_notified(&conn, reminder_id);
                }
            }

            std::thread::sleep(Duration::from_secs(30));
        }
    });
}
