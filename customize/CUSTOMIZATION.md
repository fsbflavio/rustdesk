# Custom fork / portable build

This fork ships a rebranded RustDesk client that runs **alongside** an official RustDesk
(installed or portable) on the same machine without conflict. It reuses RustDesk's existing
`custom.txt` overlay mechanism, but signed with **your** key instead of RustDesk's.

Design goal: keep the tracked diff tiny so you can rebase onto upstream with minimal conflicts.
Everything brand-specific lives either in **data** (`custom.txt`, not tracked) or in **two
one-line constants**.

## The whole change set

| What | File | Why |
|------|------|-----|
| Trust your own signing key | `src/common.rs` — `KEY` in `read_custom_client` | lets you sign `custom.txt` yourself |
| Unique portable extract dir | `libs/portable/src/main.rs` — `APP_PREFIX` | avoids `%LOCALAPPDATA%\rustdesk` collision |
| Signer helper (new, isolated) | `tools/custom-signer/` | generates key + signs `custom.txt` |
| This doc (new) | `CUSTOMIZATION.md` | the checklist |

The app **name/identity** (config dir, IPC pipe, service, install-detection, URL scheme) is set at
runtime from `custom.txt`'s `app-name` — **no code change** for it.

## One-time setup

1. **Pick your brand** (e.g. `MyApp`). Set the lowercase prefix in `libs/portable/src/main.rs`:
   ```rust
   const APP_PREFIX: &str = "myapp";
   ```
   The built binary **must** be named `<AppName>.exe` (e.g. `MyApp.exe`) — RustDesk derives the
   install path, service, and `is_installed()` from it. The CI build renames `rustdesk.exe`
   accordingly; if it stays `rustdesk.exe`, install/uninstall break (see `DISCOVERIES.md` §4).

2. **Generate your signing key** and paste the public key into `src/common.rs`:
   ```
   cd tools/custom-signer
   cargo run --release -- keygen           # prints PUBLIC key, writes custom-signer.key (PRIVATE)
   ```
   Put the printed public key into `read_custom_client`:
   ```rust
   const KEY: &str = "<your base64 public key>";
   ```
   Keep `custom-signer.key` **secret and out of git** (already git-ignored).

## Per-build: author + sign `custom.txt`

3. Write `config.json` (all fields optional):
   ```json
   {
     "app-name": "MyApp",
     "conn-type": "incoming",
     "override-settings": {
       "custom-rendezvous-server": "rs.mycompany.com",
       "relay-server": "rs.mycompany.com",
       "api-server": "https://rs.mycompany.com:21114",
       "key": "<server public key base64>"
     }
   }
   ```
   - `app-name` → gives your build its own `%APPDATA%\MyApp`, pipe `\\.\pipe\MyApp\query`, service,
     and makes `is_installed()` independent from official RustDesk. **This is what enables clean
     coexistence.**
   - `override-settings` = forced & never persisted (locks the field, ideal for pinning the server).
     Use `default-settings` instead if you want a changeable default.

### `conn-type` — receive-only / send-only (like the Pro custom client)

`conn-type` is a **top-level** key (NOT inside `override-settings`; `read_custom_client` puts any
top-level extra key into `HARD_SETTINGS`, which is what the gates below read):

| value | behavior |
|-------|----------|
| `"incoming"` | **Receive-only.** The device can only *be controlled*. Outgoing connections are refused at the connection layer (`client.rs` `is_incoming_only()` → `bail!("Incoming only mode")`) and the connect UI is hidden. The `--service`/CM path is kept so it can still accept sessions. |
| `"outgoing"` | **Send-only.** Can only control others; cannot be controlled. Upstream also skips creating the service in this mode. |
| unset | Normal bidirectional. |

Not a code change — since you control the signing key, just add `conn-type` to the JSON and re-sign.

### `disable-installation` — portable-only (hide the install button)

Top-level `"disable-installation": "Y"` hides the install banner **and** disables every install path
(the in-app button, the rename-to-`*install.exe` trick, and `--silent-install`). Use it for a pure
portable build so users can't accidentally install (which would otherwise create a portable-vs-installed
identity clash). To keep an install-capable build, ship a **separate** build *without* this flag.
(See `DISCOVERIES.md` §7.)

> Not settable via `custom.txt`: the **top logo and app icons** are compile-time Flutter/resource
> assets. Add `flutter/assets/logo.png` (optional `logo_light.png` / `logo_dark.png`, max 300x60);
> `flutter/assets/` is already globbed in `pubspec.yaml`, so no pubspec/code change. Other icons:
> `flutter/windows/runner/resources/app_icon.ico`, `res/tray-icon.ico`, `res/icon.*`.

4. Sign it:
   ```
   cargo run --release -- sign config.json custom-signer.key custom.txt
   ```

## Build & bundle

5. Build the client normally, then place `custom.txt` **beside the executable**.
   - Installed/plain: same folder as `myapp.exe`.
   - **Portable:** `custom.txt` is read from the *extracted* exe dir, so it must be part of the
     portable bundle (include it where the packer collects files) so it lands next to the exe.
   - macOS: `.../Resources/custom.txt`.

Until `KEY` is replaced and a valid `custom.txt` is present, the build behaves exactly like stock
RustDesk (the overlay is silently ignored) — so partial setups fail safe.

## Isolation checklist (verified against the code)

Set `app-name` and these are automatically separate from official RustDesk:

- Config dir `%APPDATA%\<app-name>\config`
- IPC pipe `\\.\pipe\<app-name>\query…` (this is the single-instance mechanism — no global mutex)
- Windows service name; registry install keys; `is_installed()` detection
- Log dir; tray shortcut; run keys; URL scheme `<app-name>://`

Set `APP_PREFIX` and the **portable extraction dir** is separate too.

Only-if-you-use-them (system-global, not name-scoped):
- **Direct-IP-access port** — if you enable it, give each build a distinct port.
- **Virtual-display / printer drivers** — machine-wide; only relevant if your build installs them
  (portable clients normally don't).

## Optional: self-update to your own release feed

Not part of this change set. If you want auto-update pointing at your infrastructure:
- Repoint the version endpoint `https://api.rustdesk.com/version/latest` in
  `libs/hbb_common/src/lib.rs` (`version_check_request`) to your API.
- Relax the `is_custom_client()` early-return in `check_software_update` (`src/common.rs`), since
  custom clients skip update checks by default.
- Host a version endpoint + your signed release binaries.

## Keeping in sync with upstream

- Fork **both** `rustdesk/rustdesk` and `rustdesk/hbb_common` (self-update lives in the submodule).
- Keep your changes as a small, ordered commit series on top of `upstream/master` and **rebase**
  (don't merge) each sync — your few one-line hunks re-apply cleanly.
- Enable `git rerere` so recurring conflict resolutions replay automatically.
- Don't reformat upstream code. Keep shared-file edits to single lines; put logic in new files.

## Licensing / trademark

RustDesk is AGPL-3.0: you may modify and distribute, but must publish your modified source and
preserve the license. "RustDesk" name/logo are trademarks — ship under **your own** brand, not as
"RustDesk". (Not legal advice.)
