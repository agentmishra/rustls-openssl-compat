use std::io::Read;
use std::process::{Child, Command, Output, Stdio};
use std::{net, thread, time};

/* Note:
 *
 * In the tests below, we are relying on the fact that cargo sets
 * `LD_LIBRARY_PATH` (or equivalent) so that artifacts it built
 * are preferred to others.  This means processes we run from here
 * that depend on OpenSSL will use our libssl.so.
 *
 * We set LD_LIBRARY_PATH="" to disable this where we want
 * to actually use OpenSSL's libssl.
 *
 * The test programs called below should use streams according to these rules:
 *
 * - **stdout** should contain the same output whether this program is run against
 *   bona-fide openssl or rustls-libssl.
 *
 * - **stderr**: may contain log output for tests that see error conditions.  This output
 *   need not match between openssl and rustls-libssl and such tests should not assert
 *   equality between stderr.  Tests that do not expect errors _should_ assert `stderr`
 *   equality, to ensure there is no noisy log output or spurious error stack usage.
 *
 * Note that the content of openssl error stacks is _not_ a stable interface
 * (file names, line numbers, function names, messages can all change between versions
 * of upstream openssl).  However, we try to ensure that interesting errors
 * have the same error code (see `ERR_peek_error`).
 */

#[test]
#[ignore]
fn client_unauthenticated() {
    let _server = KillOnDrop(Some(
        Command::new("openssl")
            .args([
                "s_server",
                "-cert",
                "test-ca/rsa/end.cert",
                "-cert_chain",
                "test-ca/rsa/inter.cert",
                "-key",
                "test-ca/rsa/end.key",
                "-alpn",
                "hello,world",
                "-accept",
                "localhost:4443",
                "-rev",
            ])
            .env("LD_LIBRARY_PATH", "")
            .spawn()
            .expect("failed to start openssl s_server"),
    ));

    wait_for_port(4443);

    // server is unauthenticated
    let openssl_insecure_output = Command::new("tests/maybe-valgrind.sh")
        .env("LD_LIBRARY_PATH", "")
        .args(["target/client", "localhost", "4443", "insecure"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    let rustls_insecure_output = Command::new("tests/maybe-valgrind.sh")
        .args(["target/client", "localhost", "4443", "insecure"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    assert_eq!(openssl_insecure_output, rustls_insecure_output);

    // server is authenticated, client has no creds
    let openssl_secure_output = Command::new("tests/maybe-valgrind.sh")
        .env("LD_LIBRARY_PATH", "")
        .args(["target/client", "localhost", "4443", "test-ca/rsa/ca.cert"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    let rustls_secure_output = Command::new("tests/maybe-valgrind.sh")
        .args(["target/client", "localhost", "4443", "test-ca/rsa/ca.cert"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    assert_eq!(openssl_secure_output, rustls_secure_output);

    // server is authenticated, client has creds but server doesn't ask for them
    let openssl_offered_output = Command::new("tests/maybe-valgrind.sh")
        .env("LD_LIBRARY_PATH", "")
        .args([
            "target/client",
            "localhost",
            "4443",
            "test-ca/rsa/ca.cert",
            "test-ca/rsa/client.key",
            "test-ca/rsa/client.cert",
        ])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    let rustls_offered_output = Command::new("tests/maybe-valgrind.sh")
        .args([
            "target/client",
            "localhost",
            "4443",
            "test-ca/rsa/ca.cert",
            "test-ca/rsa/client.key",
            "test-ca/rsa/client.cert",
        ])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    assert_eq!(openssl_offered_output, rustls_offered_output);
}

#[test]
#[ignore]
fn client_auth() {
    let _server = KillOnDrop(Some(
        Command::new("openssl")
            .args([
                "s_server",
                "-cert",
                "test-ca/rsa/end.cert",
                "-cert_chain",
                "test-ca/rsa/inter.cert",
                "-key",
                "test-ca/rsa/end.key",
                "-alpn",
                "hello,world",
                "-Verify",
                "1",
                "-CAfile",
                "test-ca/rsa/ca.cert",
                "-accept",
                "localhost:4444",
                "-rev",
            ])
            .env("LD_LIBRARY_PATH", "")
            .spawn()
            .expect("failed to start openssl s_server"),
    ));

    wait_for_port(4444);

    // mutual auth
    let openssl_authed_output = Command::new("tests/maybe-valgrind.sh")
        .env("LD_LIBRARY_PATH", "")
        .args([
            "target/client",
            "localhost",
            "4444",
            "test-ca/rsa/ca.cert",
            "test-ca/rsa/client.key",
            "test-ca/rsa/client.cert",
        ])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    let rustls_authed_output = Command::new("tests/maybe-valgrind.sh")
        .args([
            "target/client",
            "localhost",
            "4444",
            "test-ca/rsa/ca.cert",
            "test-ca/rsa/client.key",
            "test-ca/rsa/client.cert",
        ])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    assert_eq!(openssl_authed_output, rustls_authed_output);

    // failed auth
    let openssl_failed_output = Command::new("tests/maybe-valgrind.sh")
        .env("LD_LIBRARY_PATH", "")
        .args(["target/client", "localhost", "4444", "test-ca/rsa/ca.cert"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    let rustls_failed_output = Command::new("tests/maybe-valgrind.sh")
        .args(["target/client", "localhost", "4444", "test-ca/rsa/ca.cert"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    // nb. only stdout need match; stderr contains full error details (filenames, line numbers, etc.)
    assert_eq!(openssl_failed_output.stdout, rustls_failed_output.stdout);
}

#[test]
#[ignore]
fn client_real_world() {
    let openssl_output = Command::new("tests/maybe-valgrind.sh")
        .env("LD_LIBRARY_PATH", "")
        .env("NO_ECHO", "1")
        .args(["target/client", "example.com", "443", "default"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    let rustls_output = Command::new("tests/maybe-valgrind.sh")
        .env("NO_ECHO", "1")
        .args(["target/client", "example.com", "443", "default"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    assert_eq!(openssl_output, rustls_output);
}

#[test]
#[ignore]
fn constants() {
    let openssl_output = Command::new("tests/maybe-valgrind.sh")
        .args(["target/constants"])
        .env("LD_LIBRARY_PATH", "")
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    let rustls_output = Command::new("tests/maybe-valgrind.sh")
        .args(["target/constants"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    assert_eq!(openssl_output, rustls_output);
}

#[test]
#[ignore]
fn ciphers() {
    let openssl_output = Command::new("tests/maybe-valgrind.sh")
        .args(["target/ciphers"])
        .env("LD_LIBRARY_PATH", "")
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    let rustls_output = Command::new("tests/maybe-valgrind.sh")
        .args(["target/ciphers"])
        .stdout(Stdio::piped())
        .output()
        .map(print_output)
        .unwrap();

    assert_eq!(openssl_output, rustls_output);
}

#[test]
#[ignore]
fn server() {
    fn curl() {
        Command::new("curl")
            .env("LD_LIBRARY_PATH", "")
            .args([
                "-v",
                "--cacert",
                "test-ca/rsa/ca.cert",
                "https://localhost:5555/",
            ])
            .stdout(Stdio::piped())
            .output()
            .map(print_output)
            .unwrap();
    }

    let mut openssl_server = KillOnDrop(Some(
        Command::new("tests/maybe-valgrind.sh")
            .env("LD_LIBRARY_PATH", "")
            .args([
                "target/server",
                "5555",
                "test-ca/rsa/server.key",
                "test-ca/rsa/server.cert",
                "unauth",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    wait_for_stdout(openssl_server.0.as_mut().unwrap(), b"listening\n");
    curl();

    let openssl_output = print_output(openssl_server.take_inner().wait_with_output().unwrap());

    let mut rustls_server = KillOnDrop(Some(
        Command::new("tests/maybe-valgrind.sh")
            .args([
                "target/server",
                "5555",
                "test-ca/rsa/server.key",
                "test-ca/rsa/server.cert",
                "unauth",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    wait_for_stdout(rustls_server.0.as_mut().unwrap(), b"listening\n");
    curl();

    let rustls_output = print_output(rustls_server.take_inner().wait_with_output().unwrap());
    assert_eq!(openssl_output, rustls_output);
}

struct KillOnDrop(Option<Child>);

impl KillOnDrop {
    fn take_inner(&mut self) -> Child {
        self.0.take().unwrap()
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            child.kill().expect("failed to kill subprocess");
            child.wait().expect("failed to wait for killed subprocess");
        }
    }
}

fn print_output(out: Output) -> Output {
    println!("status: {:?}\n", out.status);
    println!(
        "stdout:\n{}\n",
        String::from_utf8(out.stdout.clone()).unwrap()
    );
    println!(
        "stderr:\n{}\n",
        String::from_utf8(out.stderr.clone()).unwrap()
    );
    out
}

/// Wait until we can connect to localhost:port.
fn wait_for_port(port: u16) -> Option<()> {
    let mut count = 0;
    loop {
        thread::sleep(time::Duration::from_millis(500));
        if net::TcpStream::connect(("localhost", port)).is_ok() {
            return Some(());
        }
        println!("waiting for port {port}");
        count += 1;
        if count == 10 {
            return None;
        }
    }
}

/// Read from the `Child`'s `stdout` until the string `expected` is seen.
///
/// To ensure this function can be used several times in succession
/// on a given `Child`, this must not read bytes from its `stdout`
/// that appear after `expected`.
fn wait_for_stdout(stream: &mut Child, expected: &[u8]) {
    let stdout = stream.stdout.as_mut().unwrap();

    let mut buffer = Vec::with_capacity(1024);

    loop {
        let mut input = [0u8];
        let new = stdout.read(&mut input).unwrap();
        assert_eq!(new, 1);
        buffer.push(input[0]);

        if buffer.ends_with(expected) {
            return;
        }
    }
}
