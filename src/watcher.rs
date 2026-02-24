use crate::index::SyncManager;
use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Arc;
use std::time::Duration;

pub fn watch_vault(manager: Arc<SyncManager>) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(2)),
    )?;

    watcher.watch(&manager.vault_path, RecursiveMode::Recursive)?;

    println!("👀 Watching for changes in {:?}...", manager.vault_path);

    // Keep the watcher alive in a background thread
    std::thread::spawn(move || {
        // Hold the watcher to prevent it from being dropped
        let _watcher = watcher;
        
        for event in rx {
            handle_event(&manager, event);
        }
    });

    Ok(())
}

fn handle_event(manager: &SyncManager, event: notify::Event) {
    use notify::EventKind;

    for path in event.paths {
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        match event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
                println!("📝 File changed: {:?}", path);
                if let Err(e) = manager.index_file(&path) {
                    eprintln!("❌ Failed to index file {:?}: {}", path, e);
                }
            }
            EventKind::Remove(_) => {
                println!("🗑️ File removed: {:?}", path);
                if let Err(e) = manager.remove_file(&path) {
                    eprintln!("❌ Failed to remove file {:?}: {}", path, e);
                }
            }
            _ => {}
        }
    }
}
