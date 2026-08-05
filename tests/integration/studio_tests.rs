use std::net::TcpListener;

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn studio_start_port_conflict_is_reported_with_hint() {
    let project = ProjectBuilder::new("studio-port-conflict").build();
    let listener = TcpListener::bind("127.0.0.1:0").expect("Studio test port must be reserved");
    let port = listener
        .local_addr()
        .expect("Reserved Studio TCP port has no address")
        .port()
        .to_string();

    project
        .acton()
        .arg("studio")
        .arg("start")
        .arg("--port")
        .arg(&port)
        .arg("--no-open")
        .run()
        .failure()
        .assert_not_contains("Starting Acton Studio")
        .assert_stderr_contains("Failed to start Acton Studio on 127.0.0.1:")
        .assert_stderr_contains("Set another port with --port")
        .assert_stderr_contains("Or stop the process currently listening on that port")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/studio/studio_start_port_conflict.stderr.txt",
        );
}
