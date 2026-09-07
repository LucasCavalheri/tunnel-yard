# Platform support

The UI, profile editor and `.conf` files are shared. Execution, privilege elevation, networking and autostart are platform specific.

| Platform / architecture | VPN engine | Current release artifact | Profile directory |
| --- | --- | --- | --- |
| Linux x86_64 | openfortivpn | `my-vpns-linux-x64` or `.deb` / `.rpm` | `/etc/openfortivpn` (existing location) |
| Linux ARM64 | openfortivpn | `my-vpns-linux-arm64` or `.deb` / `.rpm` | `/etc/openfortivpn` (existing location) |
| macOS Intel + Apple Silicon | openfortivpn from Homebrew | `my-vpns-macos` or `.dmg` (universal) | `~/Library/Application Support/My VPNs/profiles` |
| Windows x64 | official OpenConnect 9.21 with Wintun | `my-vpns-windows-x64.exe` | `%APPDATA%\My VPNs\profiles` |
| Windows ARM64 | native GUI; native OpenConnect + Wintun required | `my-vpns-windows-arm64.exe` | `%APPDATA%\My VPNs\profiles` |

The new platforms must pass the native acceptance checklist below before they are considered validated for production. The code and packaging being present do not establish compatibility with every FortiGate or authentication policy.

The Windows ARM64 executable is a native ARM64 GUI build. The pinned OpenConnect 9.21 installer in `packaging/windows-client.json` is currently a MinGW64/x64 package, so the app deliberately refuses to install it on ARM64 rather than silently mixing architectures. A native ARM64 OpenConnect + Wintun distribution is still required for VPN connections on that target.

## Configuration compatibility

Files remain **openfortivpn `.conf` files**, including on Windows. Do not feed these files directly to OpenConnect's `--config`: that is a different format. My VPNs translates them at connection time.

| Existing field | Windows behavior |
| --- | --- |
| `host`, `port` | Fortinet HTTPS gateway |
| `username` / `user`, `password` | User argument; password delivered to an isolated hidden Unicode console, never command-line arguments |
| `trusted-cert` | Verify the full SHA256 **certificate** fingerprint in a credential-free TLS probe, then derive an OpenConnect SPKI pin; a mismatch aborts before login |
| `set-dns` | Apply domain-specific NRPT rules for Fortinet split-DNS, or interface DNS/suffix/metric when no split domains are supplied; preserve unrelated policies and remove owned settings on disconnect |
| `set-routes` | Apply split routes, or a full IPv4 tunnel when the gateway sends no split routes; preserve the transport route to the public gateway |
| `realm` | URL-encoded Fortinet login realm |
| `persistent` | My VPNs retries after the configured interval; authentication/cookie failures, certificate errors and a server forbidding reconnect-after-drop disable automatic retry |
| `ca-file`, `user-cert`, `user-key` | OpenConnect CA / client certificate / private-key options; referenced files must exist on the destination computer |
| `otp` | Supply an additional one-time code as console input; interactive challenge variants still require gateway validation |

When `trusted-cert` is absent, OpenConnect uses normal CA and hostname verification. No option disables certificate verification. Multiple trusted fingerprints are retained during import and editing.

OpenConnect can log `Server certificate verify failed: signer not found` even when it subsequently accepts an explicitly configured public-key pin. My VPNs logs the successful full-fingerprint preflight separately. An unknown CA never becomes a blanket trust exception: with no pin, install the administrator-provided CA and reference it with `ca-file`; with a pin, verify the complete SHA256 fingerprint through a trusted channel. Do not copy a fingerprint from an unverified connection as a permanent workaround.

Some FortiGate appliances advertise DTLS but reject the Fortinet DTLS hello. For those profiles, enable **Disable DTLS** in the editor. My VPNs stores this as `# my-vpns-no-dtls = 1`, then passes OpenConnect's `--no-dtls` so the session stays on HTTPS, matching openfortivpn. This remains opt-in because other gateways use DTLS successfully.

Some hosted FortiGate gateways close the tunnel unless it follows openfortivpn's legacy hand-off: a fresh TLS connection after the XML request and `Host: sslvpn` on the tunnel request. Enable **Legacy FortiGate tunnel** for those profiles when using a My VPNs patched OpenConnect build. It stores `# my-vpns-legacy-tunnel = 1`; the patched client reads this only for that session, while the normal request sequence remains unchanged for other profiles. The standard Windows installer ships the official OpenConnect build, which ignores this optional marker; use **Disable DTLS** for a gateway that works with openfortivpn but rejects the standard DTLS negotiation.

Windows profiles can optionally specify an internal IPv4 service and TCP port in the editor. The metadata stays in comments so the file remains compatible with openfortivpn:

```ini
# my-vpns-health-host = 198.18.0.2
# my-vpns-health-port = 30015
```

With a target configured, the supervisor verifies that the selected route uses this VPN, binds the probe to its assigned IP and requires a successful TCP handshake before showing connected. It repeats this service check about every 15 seconds. Isolated service failures are retried in the background while the adapter and routes remain healthy, so they do not produce a false disconnect notification or reauthentication. The probe never tears down a healthy tunnel; adapter, route, MTU and OpenConnect terminal events remain the authoritative disconnect signals. A success resets the failure counter. A TCP handshake establishes reachability, not database authentication or query correctness. Without a target, connected means the adapter, address, routes and MTU have passed validation. These service probes currently apply only to Windows.

Options not exposed by the form are preserved in `extraOptions`, rather than silently discarded. Windows rejects unrecognized options with their names before elevation. macOS also rejects options that can load executable pppd plugins, run arbitrary pinentry programs or redirect privileged logs. An imported Linux profile containing these options remains intact but requires a platform-specific edit.

The initial Windows backend configures IPv4 tunnels (`--disable-ipv6`). Fortinet support in OpenConnect is described as experimental upstream. This is FortiGate **SSL VPN**, not IPsec. SAML/browser login, all MFA challenge variants, IPv6 tunnels and arbitrary openfortivpn/pppd settings are not claimed as verified Windows features.

## Process and network lifecycle

The GUI process never runs as administrator/root. Each native connection starts a supervisor after an OS authorization prompt. Windows uses PowerShell and macOS uses `osascript` plus a Bash supervisor. Profiles and temporary credentials are restricted to the current user and privileged accounts. The Linux PolicyKit flow remains in place.

Each Windows connection uses a distinct Wintun interface. The network script records only changes it owns, restores previous DNS, suffix and interface metric, and removes its own IP/routes. A global mutex serializes changes; shared transport routes are retained while another My VPNs session needs them. Network setup failure triggers rollback instead of reporting a successful tunnel.

Starting with 1.1.1, the helper applies OpenConnect's **per-session `INTERNAL_IP4_MTU`** to the IPv4 interface in ActiveStore before assigning its IP or installing routes. It reads the effective MTU back and refuses setup if it exceeds the negotiated value. This prevents Wintun's large default MTU from causing OpenConnect to discard packets. The value is not hardcoded and already excludes tunnel overhead. Wintun accepted and enforced the negotiated MTU on the tested Windows host, so no TAP driver installation or automatic driver switch was necessary. An adapter that rejects the MTU fails closed; unverified TAP fallback is not advertised. Version 1.1.3 also gives the callback-to-supervisor mutex hand-off enough time to finish flushing `NETWORK_READY` before the first health check.

The connection callback has a short execution budget in OpenConnect. DNS policy and bound service probes therefore run in the supervisor after the callback returns. Only a successful validation produces a connected status. Topology checks run approximately every three seconds; a missing/down adapter, unusable IP, missing route or incompatible MTU marks an established session disconnected and stops the client immediately. Confirmed service failure uses the bounded verification policy above. The UI also rejects stale supervisor reports. OpenConnect dead-peer detection is requested at ten-second intervals. A rejected cookie is terminal; when the server forbids reconnect-after-drop, manual reconnection performs a new authentication instead of retrying the expired session.

OpenConnect 9.21 exposes Fortinet split-DNS only as diagnostic lines. The supervisor collects those domain/server pairs, validates them and applies exact-domain plus suffix NRPT rules when `set-dns=1`. It checks existing rules for conflicts and does not overwrite a different VPN's DNS policy. `set-dns=0` continues to leave DNS untouched.

macOS registers a per-session DNS entry in SystemConfiguration via `scutil`, instead of editing `/etc/resolv.conf`. openfortivpn manages PPP and routing. The entry is removed on exit. This path needs validation on actual supported macOS releases, including multiple simultaneous VPNs.

Supervisors observe a heartbeat. Closing the window keeps the app in the tray; quitting the app requests graceful VPN shutdown. If the app crashes, a supervisor detects the missing heartbeat after about 15 seconds and disconnects. Windows sends Ctrl+C so OpenConnect can log out and run cleanup; after a timeout it forces termination and retries network cleanup. No process is stopped by a broad executable-name match on the new platforms.

## Distribution

The mark is a coral squircle with two white tunnel arches meeting at a hub. The
concept was explored with OpenAI ImageGen and rebuilt as precise geometry in
`scripts/generate-icon.py`, which writes `public/icon.svg`, `public/icon.png`,
`public/icon-32.png`, `public/icon-64.png`, `public/icon.ico`, and
`build/icon.png`. The binary embeds the PNG/ICO (`include_bytes`) and copies
them into the user cache so the window, tray and notifications all show the
same icon. On Linux it also writes a per-user launcher named after
`dev.cavallheri.myvpns`, matching the Wayland app id used by the window and
preventing GNOME from falling back to its generic gear icon. Re-run the script
after changing the mark.

CI runs `cargo fmt --check` (Linux x64), `cargo test` and `cargo build` on Linux x64/ARM64, Windows x64/ARM64 and macOS Intel/Apple Silicon. Release tags build each native target, create Debian/RPM packages for Linux, and combine the two macOS slices with `lipo` into the universal binary and `.dmg` installer. Unsigned builds may trigger Gatekeeper/SmartScreen; do not disable those protections globally.

The OpenConnect download URL and SHA256 are pinned in `packaging/windows-client.json`. If the official artifact expires or changes, download/verification fails closed; update the reviewed metadata rather than removing the hash check.

## Verification

Automated checks include configuration translation and preservation; path traversal and newline injection rejection; an actual local TLS handshake for certificate pin conversion; and Windows network command tests covering every DNS/routes toggle combination, partial setup rollback, repeated cleanup and shared transport routes.

Regression coverage also includes multiple negotiated MTUs, rejected/invalid MTU before any IP/routes, adapter/address/route/MTU loss, cleanup after the IP interface disappears, split-DNS restoration/conflicts, stale connection status, server reconnect restrictions and cookie rejection. Real-client localhost tests verify that an untrusted CA or incorrect pin transmits no HTTP credentials.

On 2026-08-31, a real Windows/FortiGate session reproduced the oversized-packet issue with interface MTU 65535 and negotiated MTU 1351. Applying 1351 restored an internal application response of 21,819 bytes (HTTP 200 in approximately 0.86 seconds, versus no response in a bounded 20-second diagnostic beforehand). A new connection through the corrected application applied the MTU automatically and validated the internal service before reporting connected; the received internal DNS domain resolved through its NRPT policy. These observations validate that environment, not every FortiGate policy or database workload. macOS gateway acceptance remains separate.

Disabling that session's adapter made the application report disconnected, stop the client and remove its DNS policy. No automatic retry occurred under the gateway's no-reconnect policy; a subsequent manual connection succeeded. After restarting the internal application to clear its connection pool/cache, the first request returned the same 21,819-byte response with HTTP 200 in about 4.16 seconds. No internal application code, environment files or timeouts were changed.

An additional read-only test with the application's existing native `@sap/hana-client` connection settings returned a small query in 128 ms and 256 synthetic rows totaling 1 MiB in 1.03 seconds; every byte of the larger result was checked. Prolonged monitoring exposed isolated three-second service-probe failures, which motivated the bounded retry policy in 1.1.2 instead of terminating on the first failed probe. No database or API timeout was increased.

A controlled Windows firewall test blocked only the supervisor's service probe on its VPN interface. The UI switched to verification in progress, then recovered to connected after the temporary rule was removed, keeping the same adapter/session and without reauthenticating. The test rule was removed in a `finally` block. A five-minute native-client query loop also completed without losing its database connection; temporary network delays were observed, so individual query latency is not guaranteed by the MTU fix.

On Windows, `scripts/prepare-windows-test.ps1` extracts the pinned official OpenConnect binary without installing it. The runtime tests execute the real client and supervisor against a localhost HTTPS server to verify username/password/realm handling (including non-ASCII passwords), authentication failure and graceful cancellation. They **do not** prove PPP negotiation or real network connectivity.

`my-vpns --smoke` prints engine, platform, profile directory, client status and locale from the same binary. It does not install dependencies or touch system networking.

Native acceptance checklist (requires a test FortiGate and a Mac/Windows host):

- Install the appropriate package and complete dependency setup, including cancellation of the OS authorization prompt.
- Import a copy of a real `.conf`; verify host, port, username, realm and all retained extra options. Use test credentials; never commit private profiles.
- Connect and reach an internal IP and an internal DNS name; confirm the app reports connected only after network setup.
- Test `set-dns` and `set-routes` both on and off; compare routes/DNS before connection and after disconnect.
- Check an invalid password and an incorrect certificate fingerprint: neither may establish a tunnel or trigger repeated authentication attempts.
- Test two simultaneous VPNs, then disconnect in both orders. Check shared gateway routes and DNS restoration.
- Test reconnect after a network interruption, cancel during startup, quit from the tray and terminate the UI to exercise the heartbeat cleanup.
- Test native login items and the appropriate macOS architecture. Verify signing/notarization on the actual distribution artifacts.

## Upstream references

- [openfortivpn](https://github.com/adrienverge/openfortivpn) and [Homebrew package](https://formulae.brew.sh/formula/openfortivpn).
- [OpenConnect Windows packages](https://www.infradead.org/openconnect/packages.html#windows), [Fortinet protocol](https://www.infradead.org/openconnect/fortinet.html), and [CLI manual](https://www.infradead.org/openconnect/manual.html).
- [OpenConnect v9.21 source](https://gitlab.com/openconnect/openconnect/-/tree/v9.21), used to verify the Windows console, Wintun and Fortinet login behavior.
