use std::{io, thread, time::Duration};
use uno::net::{local_address, ServerHandle, DEFAULT_PORT};

fn main() {
    let mut args = std::env::args().skip(1);
    let bind = args
        .next()
        .unwrap_or_else(|| format!("0.0.0.0:{DEFAULT_PORT}"));
    let server = match ServerHandle::start(&bind) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let advertised = args
        .next()
        .unwrap_or_else(|| local_address(server.address.port()));
    println!("UNO server listening on {}", server.address);
    println!("Host connects first with: {}", server.owner_invite().link());
    println!(
        "Share with guests:       {}",
        server.invite(&advertised).link()
    );
    println!(
        "Plain TCP; use a trusted LAN or private VPN. Forward TCP {} for direct internet access.",
        server.address.port()
    );
    println!("Press Enter to shut down. If stdin is closed, use Ctrl+C.");
    let mut line = String::new();
    if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
}
