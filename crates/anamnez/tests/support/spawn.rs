//! Spawn the real `anamnez` binary against the bootstrapped state.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Daemon {
    child: Child,
    pub bind: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Daemon {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    pub fn wait_for_exit(&mut self, timeout: Duration) -> Option<std::process::ExitStatus> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(Some(s)) = self.child.try_wait() {
                return Some(s);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }
}

pub fn spawn_serve(config_path: &Path, pid_path: &Path, bind: &str, recovery_code: &str) -> Daemon {
    let bin = assert_cmd::cargo::cargo_bin("anamnez");
    let child = Command::new(bin)
        .arg("serve")
        .arg("--config")
        .arg(config_path)
        .arg("--pid-file")
        .arg(pid_path)
        .arg("--bind")
        .arg(bind)
        .env("ANAMNEZ_RECOVERY_CODE", recovery_code)
        .env("RUST_LOG", "anamnez=debug,info")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn anamnez serve");

    Daemon {
        child,
        bind: bind.to_owned(),
    }
}

pub fn pick_free_port() -> u16 {
    // Ephemeral OS-assigned port. Rebinds in tests are tolerated as long as we
    // pass the port through; reqwest connects right after `wait_for_ready`.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listen on ephemeral");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);
    port
}

pub async fn wait_for_ready(client: &reqwest::Client, base_url: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(r) = client
            .get(format!("{base_url}/v1/health"))
            .header("x-client-version", "1.0.0")
            .send()
            .await
        {
            if r.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}
