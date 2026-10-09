use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use super::*;

fn run_installer(install: Install, payload: &[u8], fail: bool) -> (bool, bool) {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let curl = bin.join("curl");
    std::fs::write(&curl, "#!/bin/sh\n[ \"$FAIL\" = 0 ] || exit 22\nwhile [ $# -gt 0 ]; do if [ \"$1\" = -o ]; then cp \"$PAYLOAD\" \"$2\"; exit; fi; shift; done\nexit 2\n").unwrap();
    std::fs::set_permissions(curl, std::fs::Permissions::from_mode(0o755)).unwrap();
    let payload_path = dir.path().join("payload");
    std::fs::write(&payload_path, payload).unwrap();
    let tool = crate::core::manifest::embedded()
        .unwrap()
        .tool("uv")
        .unwrap()
        .clone();
    let actions = executor_for(&install).install(&tool, &install).unwrap();
    let marker = dir.path().join("ran");
    let mut ok = true;
    for action in actions {
        let Action::Shell { command } = action else {
            panic!("expected shell")
        };
        ok &= Command::new("sh")
            .args(["-c", &command])
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("PAYLOAD", &payload_path)
            .env("HOME", dir.path())
            .env("FAIL", if fail { "1" } else { "0" })
            .env("MARKER", &marker)
            .status()
            .unwrap()
            .success();
        if !ok {
            break;
        }
    }
    (ok, marker.exists())
}

fn script(hash: Option<String>) -> Install {
    Install::CurlScript {
        url: "https://example.invalid/install".into(),
        sha256: hash,
        shell: "sh".into(),
        args: vec!["argument with spaces".into()],
        github_repo: None,
    }
}

#[test]
fn failed_download_never_executes_script_or_reports_success() {
    assert_eq!(
        run_installer(script(None), b"touch \"$MARKER\"", true),
        (false, false)
    );
}

#[test]
fn installer_failures_and_argument_boundaries_survive_staging() {
    assert_eq!(
        run_installer(
            script(None),
            b"[ \"$1\" = 'argument with spaces' ] || exit 2\ntouch \"$MARKER\"\n",
            false
        ),
        (true, true)
    );
    assert_eq!(
        run_installer(script(None), b"exit 42\n", false),
        (false, false)
    );
    assert_eq!(
        run_installer(script(Some("0".repeat(64))), b"touch \"$MARKER\"", false),
        (false, false)
    );
}

#[test]
fn corrupt_release_fails_before_destination_is_created() {
    let install = Install::GithubRelease {
        repo: "example/tool".into(),
        asset: "tool.tar.gz".into(),
        dest: "$HOME/destination".into(),
        version: Some("v1".into()),
        sha256: None,
        checksum_asset: None,
        confirm: false,
    };
    assert_eq!(
        run_installer(install, b"not a tarball", false),
        (false, false)
    );
}
