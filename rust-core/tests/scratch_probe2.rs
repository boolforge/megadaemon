use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SHM: &str = "/data/user/0/com.agy/files/snatcher_vla_shm.bin";

fn child_env_key() -> String {
    "SCOUT_CHILD".to_string()
}

#[test]
fn probe_shm_alloc_abort() {
    if std::env::var(child_env_key()).is_ok() {
        // Child: analyse the (hostile) SHM header.
        let rep = megadaemon_core::npu_cv::analyze_vla_frame_and_accel();
        println!("CHILD_OK w={} h={}", rep.frame_width, rep.frame_height);
        return;
    }
    // Parent: write a hostile header (width=height=65535) and run the child
    // under a 1 GiB address-space limit.
    let orig = std::fs::read(SHM).unwrap();
    let mut buf = vec![0u8; 1048576];
    buf[0..4].copy_from_slice(&0x534E4154u32.to_le_bytes());
    buf[4..8].copy_from_slice(&13u32.to_le_bytes());
    buf[20..22].copy_from_slice(&65535u16.to_le_bytes());
    buf[22..24].copy_from_slice(&65535u16.to_le_bytes());
    buf[24..26].copy_from_slice(&24u16.to_le_bytes());
    buf[32824] = 0xAB;
    std::fs::write(SHM, &buf).unwrap();

    let exe = std::env::current_exe().unwrap();
    let name = "probe_shm_alloc_abort";
    let cmd = format!(
        "ulimit -v 1000000; exec '{}' --exact {} --nocapture",
        exe.display(),
        name
    );
    let out = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .env(child_env_key(), "1")
        .output()
        .unwrap();
    std::fs::write(SHM, &orig).unwrap();
    println!(
        "child status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.status.success(),
        "el proceso murió por señal (abort/OOM) en vez de fallar limpiamente: {:?}",
        out.status
    );
}

#[test]
fn probe_su_no_timeout() {
    if std::env::var(child_env_key()).is_ok() {
        let t = Instant::now();
        let _ = megadaemon_core::npu_bridge::probe_exynos_npu();
        println!("PROBE_DONE elapsed_ms={}", t.elapsed().as_millis());
        return;
    }
    // Fake `su` that never returns (simulates the interactive root prompt).
    let dir = std::env::temp_dir().join("scout_fake_su");
    std::fs::create_dir_all(&dir).unwrap();
    let su = dir.join("su");
    let mut f = std::fs::File::create(&su).unwrap();
    f.write_all(b"#!/bin/sh\nsleep 300\n").unwrap();
    drop(f);
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&su, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let exe = std::env::current_exe().unwrap();
    let name = "probe_su_no_timeout";
    let path = format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = Command::new(&exe);
    cmd.arg("--exact")
        .arg(name)
        .arg("--nocapture")
        .env(child_env_key(), "1")
        .env("PATH", path)
        .env_remove("MEGADAEMON_NPU_RAW")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(|| {
            libc::setgid(65534);
            libc::setuid(65534);
            Ok(())
        });
    }
    let mut child = cmd.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut status = None;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(s)) => {
                status = Some(s);
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => panic!("try_wait: {}", e),
        }
    }
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = std::fs::remove_file(&su);
    assert!(
        status.is_some(),
        "probe_exynos_npu() NO volvió nunca: `su -c` se queda colgado sin timeout"
    );
}

#[test]
fn probe_shm_frame_bmp_panic() {
    let orig = std::fs::read(SHM).unwrap();
    let mut buf = vec![0u8; 1048576];
    buf[0..4].copy_from_slice(&0x534E4154u32.to_le_bytes());
    buf[4..8].copy_from_slice(&13u32.to_le_bytes());
    buf[20..22].copy_from_slice(&1000u16.to_le_bytes());
    buf[22..24].copy_from_slice(&1000u16.to_le_bytes());
    buf[24..26].copy_from_slice(&24u16.to_le_bytes());
    buf[32824] = 0xAB;
    std::fs::write(SHM, &buf).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_megadaemon-core"))
        .arg("shm-frame-bmp")
        .output()
        .unwrap();
    std::fs::write(SHM, &orig).unwrap();
    println!(
        "status={:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "el comando panicó");
}

#[test]
fn probe_max_turns_leak() {
    let a = Command::new(env!("CARGO_BIN_EXE_megadaemon-core"))
        .args(["seere", "$ echo SCOUT_PROOF"])
        .output()
        .unwrap();
    let b = Command::new(env!("CARGO_BIN_EXE_megadaemon-core"))
        .args(["seere", "--max-turns", "8", "$ echo SCOUT_PROOF"])
        .output()
        .unwrap();
    println!("A={}", String::from_utf8_lossy(&a.stdout));
    println!("B={}", String::from_utf8_lossy(&b.stdout));
    assert_eq!(
        String::from_utf8_lossy(&a.stdout),
        String::from_utf8_lossy(&b.stdout),
        "--max-turns N alteró la consulta"
    );
}
