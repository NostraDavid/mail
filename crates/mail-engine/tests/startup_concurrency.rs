use anyhow::Result;
use mail_engine::{Engine, Provider, ProviderCredentials};
use std::{
    path::PathBuf,
    process::Command,
    sync::{Arc, Barrier},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn concurrent_startup_calls_use_fresh_database() -> Result<()> {
    let directory = std::env::temp_dir().join(format!(
        "mail-startup-race-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let mut failures = Vec::new();
    for attempt in 0..16 {
        // Each child gets its own immutable environment; parallel tests share no DB or env mutation.
        let output = Command::new(std::env::current_exe()?)
            .args(["--exact", "startup_worker", "--ignored", "--nocapture"])
            .env(
                "MAIL_DB_PATH",
                directory.join(format!("fresh-{attempt}.sqlite")),
            )
            .env("RUST_BACKTRACE", "0")
            .output()?;
        if !output.status.success() {
            failures.push(String::from_utf8_lossy(&output.stderr).into_owned());
        }
    }
    std::fs::remove_dir_all(directory)?;
    assert!(
        failures.is_empty(),
        "concurrent startup failed: {failures:?}"
    );
    Ok(())
}

#[test]
#[ignore = "invoked by concurrent_startup_calls_use_fresh_database in an isolated child"]
fn startup_worker() -> Result<()> {
    let path = PathBuf::from(std::env::var("MAIL_DB_PATH")?);
    assert!(path.starts_with(std::env::temp_dir()));
    assert!(
        path.parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("mail-startup-race-")
    );
    assert!(!path.exists());
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|task| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || -> Result<()> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let engine = Engine::new("synthetic startup");
                barrier.wait();
                runtime.block_on(async {
                    if task == 0 {
                        let settings = engine.load_oauth_settings().await?;
                        assert!(settings.google.is_none() && settings.outlook.is_none());
                    } else {
                        // No token exists, so restore returns before any OAuth/network code.
                        assert!(
                            engine
                                .try_restore_session(Provider::Google)
                                .await?
                                .is_none()
                        );
                    }
                    Ok(())
                })
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("startup thread panicked")?;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let engine = Engine::new("synthetic settings roundtrip");
        engine
            .save_provider_credentials(
                Provider::Google,
                ProviderCredentials {
                    client_id: "synthetic.apps.googleusercontent.com".to_owned(),
                    client_secret: None,
                },
            )
            .await?;
        let settings = engine.load_oauth_settings().await?;
        assert_eq!(
            settings.google.unwrap().client_id,
            "synthetic.apps.googleusercontent.com"
        );
        assert!(
            engine
                .try_restore_session(Provider::Google)
                .await?
                .is_none()
        );
        Ok(())
    })
}
