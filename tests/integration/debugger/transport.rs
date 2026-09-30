use dap_client::DapClient;
use std::io::Write;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

#[test]
fn receives_stopped_after_long_idle() -> anyhow::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?.to_string();
    let server = thread::spawn(move || -> anyhow::Result<()> {
        let (mut stream, _) = listener.accept()?;
        // Debug sessions can remain idle while a step executes or the user pauses.
        thread::sleep(Duration::from_secs(32));
        let event =
            r#"{"seq":1,"type":"event","event":"stopped","body":{"reason":"step","threadId":1}}"#;
        write!(stream, "Content-Length: {}\r\n\r\n{event}", event.len())?;
        Ok(())
    });

    let mut client = DapClient::connect(&address)?;
    client.start()?;
    let event = client.try_receive_event(Duration::from_secs(40))?;
    server.join().expect("DAP server thread panicked")?;

    let reason = match event {
        Some(dap::events::Event::Stopped(body)) => Some(body.reason),
        _ => None,
    };
    expect_test::expect![["Some(Step)"]].assert_eq(&format!("{reason:?}"));
    Ok(())
}
