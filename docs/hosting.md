# Hosting and networking

[Back to README](../README.md)

The desktop client starts a server when you host a table. No separate backend or
account is required. All players should use the same game version.

## Invites and addresses

An invite contains the host address, TCP port, and a random room token:

```text
uno://192.168.1.20:34197/0123456789abcdef0123456789abcdef
```

Invites are pasted into the native client, not opened as web pages. The game does
not register an OS-level `uno://` URL handler. You can prefill the join screen:

```sh
cargo run --release --locked --bin uno -- --name Morgan --join "uno://HOST:34197/ROOM_TOKEN"
```

Replace `HOST` and `ROOM_TOKEN` with the values from a real invite.

The advertised address and local listening port are separate so a router can map
an external port to a different internal one. **Use LAN address** refreshes the
detected interface address. On machines with multiple adapters, verify that it
is the one friends can reach.

The desktop host listens on IPv4. The console host and invite parser also support
bracketed IPv6 addresses; configure IPv6 reachability and firewall policy separately.

## Same network

Use the host's LAN address, such as `192.168.1.20:34197`. Do not share `127.0.0.1`
with other computers: it means their own computer. No internet connection is
needed. Wi-Fi client isolation and guest networks can block LAN connections.

## Different networks

A link alone **cannot bypass NAT, CGNAT, or a firewall**. Choose one:

- **Private mesh VPN (recommended):** put players on the same VPN, such as Tailscale, ZeroTier, or a self-managed WireGuard network. Advertise the host's reachable VPN address and port. Third-party VPNs may use coordination/relay infrastructure; the game itself does not provide it.
- **Direct internet connection:** forward the chosen TCP port on the router to the host's LAN address, allow it through the host firewall, and advertise the public IP or DNS name with the external port. Double NAT may require two mappings; ISP CGNAT usually prevents this. The game does not open router ports automatically.

**Security:** TCP traffic is not TLS-encrypted. Prefer a trusted LAN or an encrypted
VPN. Direct port-forwarded play is suitable only where you accept that risk.
Treat invite and rejoin tokens as secrets. The host owns the game state and can
inspect every hand; this is a trusted-friends game, not a cheat-resistant service.

## Firewall and manual port forwarding

Allow inbound TCP on the chosen port for the hosting executable (`uno.exe` or
`uno-server.exe` on Windows). Scope access to the networks you trust. Never
disable the firewall or enable router DMZ to make hosting work.

For direct internet hosting, reserve the host's LAN IP in the router and forward
the external TCP port to that address and the game's listen port (34197 by
default). In **Host a table**, advertise `PUBLIC_IP:34197`, adjusting the external
port if needed, then share the generated guest invite.

Port forwarding does not bypass upstream NAT/CGNAT. Test with a friend on another
connection while the game is hosting; testing from inside the same LAN may fail
without NAT loopback. Remove unneeded forwarding and firewall rules when you stop
hosting. Prefer a private VPN if you do not want to expose a port publicly.

## Disconnects and match lifetime

A broken connection pauses the table without discarding hands or advancing the
turn. Use **Rejoin** in the disconnected player's still-open client. The private
rejoin credential is kept only in memory and is distinct from the room invite.
An old half-open socket is replaced after successful reconnect. Controls remain
disabled until a fresh snapshot arrives.

There is no disk persistence, host migration, spectator mode, kicking, or seat
replacement during a match. New seats cannot join a running match. Disconnected
lobby seats are also reserved.

Closing a guest client loses its rejoin credential; the host must reopen the table
if that player cannot return. Closing the host terminates the room and loses the
match. **Leave table** ends participation; it is not a reconnect mechanism.

## Optional console host

```sh
cargo run --release --locked --bin uno-server -- 0.0.0.0:34197 192.168.1.20:34197
```

Arguments are the bind address and advertised `HOST:PORT`; use your own reachable
address. The program prints a **private owner invite** and a **guest invite**.
Connect the owner first using a desktop client, then share only the guest invite.
The owner controls dealing. Press Enter or Ctrl+C to stop the console host; if
standard input is closed, use Ctrl+C.
