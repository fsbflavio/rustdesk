# RustDesk custom-client discoveries

What we learned building **ProDesk** (a rebranded, incoming-only, self-hosted fork of RustDesk)
without paying for the Pro custom-client generator. This is the "why it works" companion to
`CUSTOMIZATION.md` (how-to) and `BUILD-STEPS.md` (build runbook).

TL;DR: **behavior/identity/config = runtime `custom.txt`** (a signed data file beside the exe);
**branding images = compile-time assets**; **install/portable identity is keyed on the exe filename
matching the app-name**.

---

## 1. `custom.txt` — the whole config lever

RustDesk already reads a config file beside the executable; the Pro "custom client" is just this
file signed by RustDesk. We repurposed it with **our own signing key**.

- `load_custom_client()` — `src/common.rs:2083` — reads `<exe_dir>/custom.txt` (macOS:
  `../Resources/custom.txt`; debug: `./custom.txt`). Runs early at `core_main.rs:35`, before any
  config access.
- `read_custom_client()` — `src/common.rs:2181` — base64-decodes, **verifies the signature against
  the `KEY` constant** (`:2186`, `sign::verify` at `:2191`), then applies the JSON.
- We changed `KEY` to **our own ed25519 public key**; `tools/custom-signer` signs `custom.txt` with
  the matching private key. That one-line change is the only edit needed to trust our own config.

### How `custom.txt` JSON maps to internal stores

| JSON | Goes to | Effect |
|------|---------|--------|
| `app-name` (top-level) | `APP_NAME` (`common.rs:2204` — the only place it's set) | changes identity (see §2) |
| `default-settings` {}   | `DEFAULT_SETTINGS` | soft defaults (user can change) |
| `override-settings` {}  | `OVERWRITE_SETTINGS` | **forced, never persisted** (locks the field) |
| any other top-level key | `HARD_SETTINGS` (`common.rs:2247-2254`) | hard flags like `conn-type`, `disable-installation` |

`get_option()` resolution order (`libs/hbb_common/src/config.rs:1245`):
**`OVERWRITE_SETTINGS` → `Config2.options` (RustDesk2.toml in %APPDATA%) → `DEFAULT_SETTINGS`**.
Server keys (`custom-rendezvous-server`, `relay-server`, `api-server`, `key`) live in RustDesk2.toml,
so putting them in `override-settings` forces them without writing to the user's config.

---

## 2. Identity = `APP_NAME` (this is what makes coexistence work)

Almost every "which instance am I" resource is derived from `APP_NAME`. Set a distinct app-name and
you get a fully independent client that coexists with stock RustDesk (or any other build):

| Resource | Derived from | Code |
|----------|--------------|------|
| Config dir `%APPDATA%\<name>\config` | app-name | `config.rs:783,742` (`Config::path` / `file_`) |
| IPC pipe `\\.\pipe\<name>\query` (this IS the single-instance mechanism; **no global mutex**) | app-name | `config.rs:851` |
| Windows service name | app-name | `windows.rs:3703` (`get_create_service`) |
| `is_installed()` (registry lookup) | app-name | `windows.rs:1949 → get_install_info → get_valid_subkey` |
| Install exe path `…\<name>.exe` | app-name | `get_install_info_with_subkey` (see §5) |
| Log dir, tray shortcut, run keys | app-name | `config.rs:819` etc. |
| URL scheme `<name>://` | app-name | `common.rs:1014` |

**Same app-name = same identity** (portable and installed of the same name are NOT isolated — see §6).
**Different app-name = independent** (own ID, config, pipe, service; `is_installed()` false for it).

---

## 3. Portable packaging & self-updating

- The portable exe (`libs/portable`) is a self-extractor: it brotli-unpacks the app into
  `%LOCALAPPDATA%\<APP_PREFIX>` and runs the inner exe. `APP_PREFIX` is a **build-time constant**
  (`libs/portable/src/main.rs:20`), *not* app-name — must be changed too, or two portable builds
  collide in `%LOCALAPPDATA%\rustdesk`.
- **Self-refresh:** the launcher compares an embedded build timestamp to `meta.toml` in the
  extraction dir (`is_timestamp_matches`). Ship a newer portable exe → next run wipes and re-extracts
  it. `%APPDATA%\<name>` (ID/settings) is a separate dir and is preserved. So "update the portable"
  = just distribute the new exe.
- `custom.txt` must be **embedded in the bundle** (staged into the app folder before packing), because
  the portable reads it from the *extraction dir*, not beside the launcher.

---

## 4. The exe filename must equal `<app-name>.exe`

The single biggest gotcha. RustDesk derives the installed binary path as
`format!("{}\\{}.exe", path, get_app_name())` (`get_install_info_with_subkey`). The Pro generator
renames the binary to `<AppName>.exe` at build time; setting app-name only via `custom.txt` does NOT
rename the binary. If the file stays `rustdesk.exe`:

- `is_installed()` looks for `ProDesk.exe` → false even though files are installed → the app shows
  the **install prompt**, the **service binpath**/shortcuts point at a missing exe, and **uninstall
  breaks**.

**Fix:** rename the built binary to `<AppName>.exe` (we do it in CI before packing, and point the
portable packer's `-e` at it). This makes it a "real" custom client for install/update purposes.
`update_me` and process-matching also key off `<app-name>.exe`.

---

## 5. Install behavior (`install_me`, `windows.rs:1555`)

Runs elevated (UAC via `runas`, `windows.rs:1900`). It:
1. `XCOPY "<exe folder>" "<InstallLocation>" /Y /E …` (`copy_raw_cmd`) — copies the **entire folder,
   including `custom.txt`**, so the installed copy keeps its identity.
2. Writes the HKLM uninstall registry key (DisplayName = app-name, InstallLocation, version).
3. Creates the `<app-name>` **service** (`get_create_service` → `sc create <name> binpath "…\<name>.exe --service"`).
4. Creates Start-Menu / Desktop / Uninstall / Tray shortcuts.

`install_me` begins by running `get_uninstall(...)` (deletes old files/registry/shortcuts) — so
**re-running the installer is an in-place upgrade**. Uninstall/reinstall never touch `%APPDATA%\<name>`,
so **ID/settings survive upgrades**.

---

## 6. Portable vs installed of the SAME name (the piggyback trap)

Because identity is app-name, running the portable `ProDesk` on a machine where `ProDesk` is
installed does NOT create a second instance:

- `is_installed()` returns **true** (registry check, independent of which exe runs).
- The portable process shares `%APPDATA%\ProDesk`, the same IPC pipe, and **does not** start its own
  portable service (that path is gated on `!is_installed()`, `core_main.rs:170`). It attaches to the
  installed instance instead.
- Running both "as servers" would fight over the same ID registration on the server.

**Rule:** don't distribute a same-named portable + installed to the same machine. Installed = the
one on managed boxes; portable = machines without an install. For genuine side-by-side, use a
different app-name.

---

## 7. Feature flags we can set purely via `custom.txt`

All are `HARD_SETTINGS` (top-level keys), read as `== "Y"` / exact string (`is_some_hard_opton`,
`config.rs:2793`):

- **`conn-type`** — `"incoming"` = receive-only (outgoing refused at `client.rs:255`
  `bail!("Incoming only mode")`, connect UI hidden); `"outgoing"` = send-only (also skips the
  service). Same as the Pro incoming/outgoing custom client. (`is_incoming_only`/`is_outgoing_only`,
  `config.rs:2775/2784`.)
- **`disable-installation`** — `"Y"` hides the install banner (`desktop_home_page.dart:463` shows it
  only `if !isDisableInstallation()`) **and** disables the install flow, including the
  rename-to-`*install.exe` trick (`core_main.rs:123` gates `click_setup` on `!is_disable_installation`)
  and `--silent-install` (`core_main.rs:262`). Same switch controls all install paths — you can't
  hide only the banner via config.
- Also available: `hide-*-settings` keys (`config.rs:2982+`) to hide settings panels.

---

## 8. Elevation model (why portable can still get admin)

- **Portable (unprivileged):** can't drive UAC prompts / secure desktop / elevated windows
  (`elevated_foreground_window_tip`), and has no pre-login/unattended access. During a session the
  controller can **Request Elevation** → a **UAC prompt on the controlled machine** (`ShellExecuteW`
  `runas`, `windows.rs:2381`, via `elevate_or_run_as_system`, `core_main.rs:183`). If the local user
  accepts (or admin creds are given), a **temporary elevated helper** runs for that session only.
  Consent-gated and session-scoped — a portable client can't silently gain admin.
- **Installed:** a SYSTEM service handles UAC/secure-desktop automatically and persistently (unattended,
  pre-login). That's the sole reason to install.

---

## 9. Updates for a custom client

- **No auto-update:** `check_software_update()` returns early if `is_custom_client()`
  (`common.rs:941`). The version endpoint is hardcoded `https://api.rustdesk.com/version/latest`
  (`libs/hbb_common/src/lib.rs:496`).
- **Portable:** ship a new exe; it self-refreshes on run (§3).
- **Installed:** redistribute the installer (in-place upgrade, §5), keeping app-name + `<name>.exe` +
  embedded `custom.txt` stable and bumping the version.
- **Self-update (optional, not done):** repoint the endpoint in `hbb_common` + relax the
  `is_custom_client()` gate + host your own version feed/binaries. `updater.rs` drives download/apply.

---

## 10. Other findings

- **Exe-name server trick** (`get_license_from_exe_name`, `windows.rs:2126`): a plaintext
  `host=…,key=…` in the exe filename sets the server as a live override — but Windows-only and can't
  carry URLs (`:`/`/` are illegal in filenames). `custom.txt` is the better mechanism.
- **Direct IP Access** (`direct-server` + `direct-access-port`, default **21118** =
  `RENDEZVOUS_PORT+2`, `rendezvous_mediator.rs:837/847`): the controlled machine opens a TCP listener
  so a controller can connect by IP, bypassing the rendezvous/relay. Off by default; the port is not
  app-name-scoped, so two instances that both enable it collide on the port.
- **Logo/branding are compile-time assets, not `custom.txt`:** `flutter/assets/logo.png`
  (`logo_light.png`/`logo_dark.png`, ≤300×60; `common.dart:3763`). OSS ships none, so adding one is
  purely additive; `flutter/assets/` is globbed in `pubspec.yaml` (no pubspec/code change). Icons:
  `flutter/windows/runner/resources/app_icon.ico`, `res/tray-icon.ico`, `res/icon.*`.
- **Server licensing:** RustDesk Server **OSS** (`hbbs`+`hbbr`) is free/AGPL (what we self-host at
  `rd.pronet.app.br`). **Server Pro** is licensed per device (web console, user mgmt). Managed hosting
  exists (e.g. Elestio, Cloudzy) but hosts the free OSS server — you pay the host, not RustDesk.
- **Licensing of the client:** AGPL-3.0 — may fork/ship but must publish modified source; "RustDesk"
  name/logo are trademarks, so ship under your own brand.

---

## 11. Fork-maintenance strategy (low merge conflict)

The persistent diff is tiny and rebases cleanly:

- `src/common.rs` — 1 line: the `KEY` public-key constant.
- `libs/portable/src/main.rs` — 1 line: `APP_PREFIX`.
- CI build renames the exe to `<AppName>.exe` (in `.github/workflows/prodesk-windows.yml`, a new file).
- Everything else is **data** (`custom.txt`, our signing key, the config JSON) or **new files**
  (`tools/custom-signer`, docs) that upstream never touches.
- Keep changes as a small commit series on top of `upstream/master`, **rebase** (don't merge), enable
  `git rerere`, and don't reformat upstream code. Fork both `rustdesk/rustdesk` and
  `rustdesk/hbb_common` (config/version live in the submodule).

---

## Key code locations (quick reference)

| What | File:line |
|------|-----------|
| custom.txt reader / signature key | `src/common.rs:2083` / `:2181` / `:2186` |
| app-name set (only place) | `src/common.rs:2204` |
| get_option resolution order | `libs/hbb_common/src/config.rs:1245` |
| config dir / file path | `libs/hbb_common/src/config.rs:783` / `:742` |
| IPC pipe name | `libs/hbb_common/src/config.rs:851` |
| is_installed → install exe path | `src/platform/windows.rs:1949` / `get_install_info_with_subkey` |
| install / upgrade / uninstall | `install_me` `windows.rs:1555` / `update_me` / `get_uninstall` |
| service create | `windows.rs:3703` |
| portable extract dir / self-refresh | `libs/portable/src/main.rs:20` / `is_timestamp_matches` |
| conn-type (incoming/outgoing) | `config.rs:2775/2784`, enforced `src/client.rs:255` |
| disable-installation gates | `config.rs:2822`, `core_main.rs:123/262`, `desktop_home_page.dart:463` |
| elevation | `core_main.rs:183`, `windows.rs:2381` |
| update check (skipped for custom) / endpoint | `common.rs:941` / `hbb_common/src/lib.rs:496` |
| exe-name server trick | `windows.rs:2126` |
| direct IP access | `rendezvous_mediator.rs:837/847`, `config.rs:2918/2919` |
| logo asset | `flutter/lib/common.dart:3763` |
