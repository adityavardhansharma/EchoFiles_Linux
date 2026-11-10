//! The supervisor and Drive calls against a stand-in rclone (tests/fake-rclone.py): the
//! process starts on our socket with our config, answers, and stops; requests have the
//! shapes rclone's rc expects. Google itself is never involved.

use std::path::PathBuf;

use ef_cloud::drive::{self, Access, MountOptions};
use ef_cloud::supervisor::{Started, Supervisor};
use ef_cloud::{Rclone, Version};
use serde_json::Value;

fn calls(log: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(log).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn starts_answers_and_stops() {
    if std::process::Command::new("python3").arg("--version").output().is_err() {
        eprintln!("python3 missing; skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("calls.jsonl");
    // SAFETY: this test binary has one test, so nothing else reads the environment meanwhile.
    unsafe {
        std::env::set_var("XDG_RUNTIME_DIR", dir.path().join("run"));
        std::env::set_var("XDG_CONFIG_HOME", dir.path().join("config"));
        std::env::set_var("XDG_CACHE_HOME", dir.path().join("cache"));
        std::env::set_var("XDG_STATE_HOME", dir.path().join("state"));
        std::env::set_var("FAKE_RCLONE_LOG", &log);
    }
    std::fs::create_dir_all(dir.path().join("run")).unwrap();
    let fake = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fake-rclone.py");
    let sup = Supervisor::new(Rclone { path: fake, version: Version(1, 68, 2) });

    let (rc, started) = sup.ensure().expect("fake rcd starts");
    assert_eq!(started, Started::Fresh);
    assert_eq!(sup.ensure().unwrap().1, Started::Existing);

    drive::create(&rc, "k3f9q2", Access::Read, None).unwrap();
    assert_eq!(drive::about(&rc, "k3f9q2").unwrap(), (4_509_715_660, 16_106_127_360));
    assert_eq!(drive::uploads(&rc, "k3f9q2").unwrap().pending(), 2);
    let kinds = drive::google_kinds(&drive::list(&rc, "k3f9q2", "").unwrap());
    assert_eq!(kinds.get("Budget.link.html"), Some(&drive::GoogleKind::Sheet));
    assert_eq!(drive::delete_permanently(&rc, "k3f9q2", "missing.txt", false).unwrap_err().kind, ef_cloud::Kind::NotFound);
    // Mounting needs FUSE; only the request is checked here, via its error-free shape.
    let _ = drive::mount(&rc, "k3f9q2", MountOptions { cache_max_gb: 10, read_only: true });

    sup.stop();
    assert!(sup.running().is_none());

    let c = calls(&log);
    let rcd = c.iter().find(|e| e["argv"].as_array().is_some_and(|a| a.iter().any(|x| x == "rcd"))).expect("rcd started");
    let argv: Vec<&str> = rcd["argv"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    let after = |flag: &str| argv.iter().position(|a| *a == flag).map(|i| argv[i + 1]);
    assert_eq!(after("--config").unwrap(), dir.path().join("config/echofiles/rclone/rclone.conf").to_str().unwrap());
    assert_eq!(after("--rc-addr").unwrap(), format!("unix://{}", dir.path().join("run/echofiles/rclone.sock").display()));
    assert!(argv.contains(&"--rc-no-auth"));
    assert!(!argv.iter().any(|a| a.contains("secret")), "no secrets on the command line");

    let by = |m: &str| c.iter().find(|e| e["method"] == m).unwrap_or_else(|| panic!("{m} called"))["params"].clone();
    let create = by("config/create");
    assert_eq!(create["name"], "ef-k3f9q2");
    assert_eq!(create["type"], "drive");
    assert_eq!(create["parameters"]["scope"], "drive.readonly");
    assert_eq!(create["parameters"]["export_formats"], "link.html");
    assert_eq!(create["opt"]["nonInteractive"], true);
    let del = by("operations/deletefile");
    assert_eq!(del["fs"], "ef-k3f9q2,use_trash=false:");
    if let Some(m) = c.iter().find(|e| e["method"] == "mount/mount") {
        assert_eq!(m["params"]["fs"], "ef-k3f9q2:");
        assert_eq!(m["params"]["vfsOpt"]["CacheMode"], "full");
        assert_eq!(m["params"]["vfsOpt"]["ReadOnly"], true);
    }
    assert!(c.iter().any(|e| e["method"] == "core/quit"));
    // Our config is private.
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(dir.path().join("config/echofiles/rclone/rclone.conf")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
