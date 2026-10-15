use std::future::Future;

use tokio::sync::OnceCell;

use crate::settings::Settings;

/// One startup attempt per application process, including failed/skipped attempts.
/// Reloading the webview must never reconnect after the user has disconnected.
#[derive(Default)]
pub struct Startup {
    result: OnceCell<Result<(), String>>,
}

impl Startup {
    pub async fn run(
        &self,
        settings: &Settings,
        connect: impl Future<Output = Result<(), String>>,
    ) -> Result<(), String> {
        self.result
            .get_or_init(|| async {
                if settings.auto_connect && settings.server.is_some() {
                    connect.await
                } else {
                    Ok(())
                }
            })
            .await
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::Startup;
    use crate::settings::{Server, Settings};

    #[test]
    fn auto_connect_is_independent_of_autostart_and_requires_a_server() {
        tauri::async_runtime::block_on(async {
            for (autostart, auto_connect, configured, expected) in [
                (false, false, false, 0),
                (false, false, true, 0),
                (false, true, false, 0),
                (false, true, true, 1),
                (true, false, false, 0),
                (true, false, true, 0),
                (true, true, false, 0),
                (true, true, true, 1),
            ] {
                let startup = Startup::default();
                let calls = AtomicUsize::new(0);
                let settings = Settings {
                    autostart,
                    auto_connect,
                    server: configured.then(Server::default),
                    ..Settings::default()
                };
                for _ in 0..2 {
                    startup
                        .run(&settings, async {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Ok(())
                        })
                        .await
                        .unwrap();
                }
                assert_eq!(calls.load(Ordering::SeqCst), expected);
            }
        });
    }

    #[test]
    fn failure_is_reported_without_retrying_on_webview_reload() {
        tauri::async_runtime::block_on(async {
            let startup = Startup::default();
            let settings = Settings {
                auto_connect: true,
                server: Some(Server::default()),
                ..Settings::default()
            };
            let error = startup
                .run(&settings, async { Err("UAC cancelled".into()) })
                .await;
            assert_eq!(error, Err("UAC cancelled".into()));
            assert_eq!(
                startup
                    .run(&settings, async { panic!("must not retry") })
                    .await,
                error
            );
        });
    }

    #[test]
    fn enabling_auto_connect_later_does_not_start_a_connection_on_reload() {
        tauri::async_runtime::block_on(async {
            let startup = Startup::default();
            let mut settings = Settings {
                server: Some(Server::default()),
                ..Settings::default()
            };
            startup
                .run(&settings, async { panic!("auto-connect is disabled") })
                .await
                .unwrap();
            settings.auto_connect = true;
            startup
                .run(&settings, async { panic!("startup has already completed") })
                .await
                .unwrap();
        });
    }

    #[test]
    fn concurrent_initialization_connects_once() {
        tauri::async_runtime::block_on(async {
            let startup = Arc::new(Startup::default());
            let calls = Arc::new(AtomicUsize::new(0));
            let mut tasks = Vec::new();
            for _ in 0..2 {
                let startup = startup.clone();
                let calls = calls.clone();
                tasks.push(tauri::async_runtime::spawn(async move {
                    let settings = Settings {
                        auto_connect: true,
                        server: Some(Server::default()),
                        ..Settings::default()
                    };
                    startup
                        .run(&settings, async {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::task::yield_now().await;
                            Ok(())
                        })
                        .await
                }));
            }
            for task in tasks {
                task.await.unwrap().unwrap();
            }
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        });
    }
}
