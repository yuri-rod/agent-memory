use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn cli_and_mcp_report_the_package_version() {
    let binary = env!("CARGO_BIN_EXE_agent-memory");
    let version = env!("CARGO_PKG_VERSION");
    let cli = Command::new(binary).arg("--version").output().unwrap();
    assert!(cli.status.success());
    assert_eq!(
        String::from_utf8(cli.stdout).unwrap().trim(),
        format!("agent-memory {version}")
    );

    let workspace = tempfile::tempdir().unwrap();
    let mut child = Command::new(binary)
        .arg("mcp")
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"version-test\",\"version\":\"1\"}}}\n",
    ).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["result"]["serverInfo"]["version"], version);
}
