# Multiplayer, servidor dedicado e plugins desativados (cliente)

> Removidos do repositório em 29/10/2026: ver [MULTIPLAYER_REMOVED.md](MULTIPLAYER_REMOVED.md). Este registro fica como histórico.

Data: 2026-10-10. Decisão de Leo (OmsiModsBR Team). Nenhum commit feito junto com esta alteração.
Esta alteração vem depois da remoção do contador Worker ([PRESENCE_WORKER.md](PRESENCE_WORKER.md)); o diff abaixo é só o desta etapa.

## O que mudou

**Multiplayer inativado no cliente.** O código de rede (`omsi-net`, `lan.rs`, `lan_mods.rs`, `admin.rs`, etc.) ficou no repositório,
mas nada o aciona:
- `lan::start` devolve sempre `None`: `--lan-host`/`--lan-join` são ignorados (aviso no log); não se hospeda nem se entra em sessão, nada escuta nem envia.
- Launcher: a página Multiplayer e a aba Online mostram só um aviso; `ask_server` e `check_join` não fazem mais nenhuma consulta de rede; a duty nunca leva `--lan-*`.

**Servidor dedicado: código intocado, mas não é mais compilado/empacotado.** O modo servidor é o próprio binário `depot --server`; `--server` agora termina com erro.
`scripts/build-server.sh`, `scripts/server/start.sh|start.cmd` saem com erro, `docs/SERVER.md` e README avisam que está descontinuado.
O pacote `depot-*-server-*` não é mais gerado em nenhuma arquitetura. (O fonte do servidor, `server.rs`, continua no binário do jogo, inalcançável.)

**Página Multiplayer e aba Online ocultas.** Saíram das listas `PAGES` (launcher) e `TABS` (modo telefone); o aviso interno continua só como rede de segurança. `build-server.sh` e os scripts de start do servidor saem com erro logo na primeira linha: o build do servidor nem começa.

**Plugins (nativos e Lua) inativados.** `plugins::load()` devolve sempre vazio; nada em `Plugins/` é carregado. `docs/PLUGINS.md` avisa.
Desde a trilha 2 da refatoração, o código de plugins (`plugins.rs`, `plugin_ui.rs` e o crate `omsi-plugin`) fica atrás da feature `plugins` do `omsi-app`, desligada por padrão, e não entra mais no binário do jogo. Para compilar com ele: `cargo build -p omsi-app --features plugins`. O `omsi-plugin` continua membro do workspace e é testado por conta própria.
Do mesmo jeito, o código de LAN (`lan.rs`, `lan_world.rs`, `lan_mods.rs`, `admin.rs`, `voice.rs`, `online.rs` e o crate `omsi-net`) fica atrás da feature `lan`, e o do servidor dedicado (`server.rs` e a administração do servidor) atrás da feature `server`, que liga a `lan`. As duas ficam desligadas por padrão, e o `omsi-net` não entra mais no binário do jogo. Sem `lan`, `lan_off.rs` responde "nenhuma sessão" nos pontos do jogo que perguntavam pela LAN. Para compilar tudo: `cargo build -p omsi-app --features lan,server,plugins` (ou `--all-features`).

**Links corrigidos para o projeto certo.** `SECURITY.md`, templates de issue (`.github/ISSUE_TEMPLATE`), `site/` (`REPO` e links), `docs/USER_GUIDE.md`, `docs/BUILDING.md`
passam de `openOMSI-Project/openOMSI` para `OmsiModsBR/depot`. O endereço do site (`docs/VERSIONING.md`, `SECURITY.md`) é https://omsimods.com.br
(site ainda não existe). Créditos ao upstream (README, UPSTREAM.md, CHANGELOG histórico, `[patch]` do `gpu-allocator`) foram mantidos.

## Pendências

- Não compilei nem rodei testes (rustc 1.97.0 abaixo do exigido por `reedsolomon-rs`; faltam libs do wayland). Rodar `cargo build` e `cargo test` antes do commit.
  Podem aparecer avisos de código morto; a CI não usa `-D warnings`.
- O CI continua compilando o workspace inteiro (inclusive `omsi-net`/`omsi-plugin`); nada de servidor é publicado por ele.
- Muita UI de multiplayer (chat, voz, name tags, opções no launcher) segue existindo nos arquivos, só sem efeito; remover de verdade fica para a refatoração.
- `docs/GITHUB Pages`: `.github/workflows/pages.yml` não existe aqui; o site em https://omsimods.com.br ainda precisa ser publicado.

## Cópia das alterações (diff desta etapa)

Para restaurar: salvar o bloco como `multiplayer-disabled.patch` e aplicar `git apply -R multiplayer-disabled.patch`.

```diff
diff --git a/.github/ISSUE_TEMPLATE/bug_report.yml b/.github/ISSUE_TEMPLATE/bug_report.yml
index ee3f942..85b3a14 100644
--- a/.github/ISSUE_TEMPLATE/bug_report.yml
+++ b/.github/ISSUE_TEMPLATE/bug_report.yml
@@ -19,7 +19,7 @@ body:
       options:
         - label: I wrote this report in English.
           required: true
-        - label: I searched the [open and closed issues](https://github.com/openOMSI-Project/openOMSI/issues?q=is%3Aissue) and this is not a duplicate.
+        - label: I searched the [open and closed issues](https://github.com/OmsiModsBR/depot/issues?q=is%3Aissue) and this is not a duplicate.
           required: true
         - label: It still happens on the latest release.
           required: true
diff --git a/.github/ISSUE_TEMPLATE/config.yml b/.github/ISSUE_TEMPLATE/config.yml
index 42ad3de..269d9d3 100644
--- a/.github/ISSUE_TEMPLATE/config.yml
+++ b/.github/ISSUE_TEMPLATE/config.yml
@@ -4,8 +4,8 @@ contact_links:
     url: https://discord.gg/VG2EKVafYG
     about: How do I...? Setting up, mods, or a chat with other players - ask on our Discord server.
   - name: Security problem
-    url: https://github.com/openOMSI-Project/openOMSI/security/advisories/new
+    url: https://github.com/OmsiModsBR/depot/security/advisories/new
     about: A vulnerability in the game, the server or the updater - report it privately, not as an issue (see SECURITY.md).
   - name: User guide
-    url: https://github.com/openOMSI-Project/openOMSI/blob/main/docs/USER_GUIDE.md
+    url: https://github.com/OmsiModsBR/depot/blob/main/docs/USER_GUIDE.md
     about: Settings, controls, multiplayer and common problems explained.
diff --git a/.github/ISSUE_TEMPLATE/feature_request.yml b/.github/ISSUE_TEMPLATE/feature_request.yml
index 41c40db..cdb2f76 100644
--- a/.github/ISSUE_TEMPLATE/feature_request.yml
+++ b/.github/ISSUE_TEMPLATE/feature_request.yml
@@ -14,7 +14,7 @@ body:
       options:
         - label: I wrote this request in English.
           required: true
-        - label: I searched the [open and closed issues](https://github.com/openOMSI-Project/openOMSI/issues?q=is%3Aissue) and nobody has asked for this yet.
+        - label: I searched the [open and closed issues](https://github.com/OmsiModsBR/depot/issues?q=is%3Aissue) and nobody has asked for this yet.
           required: true
 
   - type: dropdown
diff --git a/README.md b/README.md
index 57df6c9..f78aa79 100644
--- a/README.md
+++ b/README.md
@@ -46,7 +46,7 @@ The first rebranding release is available on the
 | Platform | File |
 | --- | --- |
 | Windows x64 | `depot-0.2.17-windows-x64.zip` - run `depot.exe`; also includes `depot-launcher.exe` |
-| Dedicated server, Windows x64 | `depot-0.2.17-server-windows-x64.zip` - see [docs/SERVER.md](docs/SERVER.md) |
+| Dedicated server | **discontinued** - no longer built on any architecture, see [docs/SERVER.md](docs/SERVER.md) |
 
 ARM64, Linux, macOS and Android packages are not included in Depot 0.2.17.
 Inherited platform support and build instructions do not imply published Depot packages.
@@ -120,7 +120,7 @@ to.
 2. **No original code or assets.** Nothing from the original is copied; the formats are
    described in [docs/FORMATS.md](docs/FORMATS.md).
 3. **A better engine.** 64-bit address space, streaming and texture loading on worker threads,
-   no 2 GB limit, no single-thread stalls, LAN multiplayer and a dedicated server.
+   no 2 GB limit, no single-thread stalls (multiplayer and the dedicated server are disabled for now).
 
 ## Documentation
 
@@ -138,7 +138,7 @@ Documentation is available locally. The pages live in [`docs/`](docs):
 | [Architecture](docs/ARCHITECTURE.md) | crates, threading, renderer, roadmap |
 | [Routes](docs/ROUTES.md) | how the original runs timetables, chrono, HOF, IBIS |
 | [Plugins](docs/PLUGINS.md) | Lua plugins (API and examples), OMSI plugin DLLs and the 32-bit plugin host |
-| [Dedicated server](docs/SERVER.md) | hosting a session without a window |
+| [Dedicated server](docs/SERVER.md) | disabled (historical) |
 | [Versioning & releases](docs/VERSIONING.md) | the `MAJOR.MINOR.COMMIT` scheme and the CI |
 | [Changelog](CHANGELOG.md) | what changed in each version |
 
@@ -150,7 +150,7 @@ scripts/build-macos.sh        # macOS   → dist/macos/depot.app
 scripts\build-windows.cmd     # Windows → dist\windows\depot.exe
 scripts/build-linux.sh        # Linux   → dist/linux/depot
 scripts/build-android.sh      # Android → dist/android/depot-<version>.apk
-scripts/build-server.sh       # server  → dist/server
+scripts/build-server.sh       # server  → disabled (exits with an error)
 ```
 
 Needs the latest [Rust stable](https://rustup.rs) and the platform's C toolchain; details in
diff --git a/SECURITY.md b/SECURITY.md
index 65e6aa6..1e96ef7 100644
--- a/SECURITY.md
+++ b/SECURITY.md
@@ -3,8 +3,8 @@
 ## Reporting a vulnerability
 
 Please do not open a public issue for a security problem. Report it privately through
-GitHub instead: **[Report a vulnerability](https://github.com/openOMSI-Project/openOMSI/security/advisories/new)**
-(the *Security* tab of the repository). Only the maintainers see it.
+GitHub instead: **[Report a vulnerability](https://github.com/OmsiModsBR/depot/security/advisories/new)**
+(the *Security* tab of the repository). Only the maintainers see it. Project site: <https://omsimods.com.br>.
 
 Say what is affected, how to reproduce it, and what an attacker could do with it. A small
 example (a file, a request, a log) helps a lot. The maintainers are volunteers: we aim to
@@ -20,7 +20,7 @@ builds.
 
 Examples of what we want to hear about:
 
-* the **multiplayer and dedicated server**: a client that can crash or take over a server
+* the **multiplayer and dedicated server** (disabled in this version and being redone; reports about the inherited code are still welcome): a client that can crash or take over a server
   or another player's game, get past the admin password or the vehicle list, or read files
   from the host;
 * **content files**: a map, vehicle, script or archive that makes the game read or write
@@ -32,4 +32,4 @@ Examples of what we want to hear about:
 
 Not a vulnerability: a plugin (Lua, or an OMSI plugin's DLL) doing harm - plugins run with the game's rights
 by design, so install only plugins you trust - and a content file that only makes the game
-crash, which is a normal bug and goes into an [issue](https://github.com/openOMSI-Project/openOMSI/issues/new/choose).
+crash, which is a normal bug and goes into an [issue](https://github.com/OmsiModsBR/depot/issues/new/choose).
diff --git a/crates/omsi-app/src/lan.rs b/crates/omsi-app/src/lan.rs
index 9f4f93f..f3108ab 100644
--- a/crates/omsi-app/src/lan.rs
+++ b/crates/omsi-app/src/lan.rs
@@ -1036,6 +1036,14 @@ fn ws_join_target(url: &str) -> Result<String, String> {
 /// cannot be started is reported and the game runs on alone. A joining player waits
 /// briefly for the host's welcome, so that its map is loaded with the host's world.
 pub fn start(args: &Args) -> Option<LanSession> {
+    // Multiplayer is switched off in this client (see docs/removed/MULTIPLAYER_DISABLED.md):
+    // no session is hosted or joined, nothing is sent, nothing listens.
+    if args.lan_host.is_some() || args.lan_join.is_some() {
+        log::warn!("multiplayer is disabled in this build: --lan-host / --lan-join are ignored");
+    }
+    if true {
+        return None;
+    }
     // (every bus offered until the server joined now says otherwise, see `ServerOffers`)
     let offers_session = server_offers_state().reset();
     let world = world_info(args);
diff --git a/crates/omsi-app/src/launcher/mod.rs b/crates/omsi-app/src/launcher/mod.rs
index 398eb83..9b2c8eb 100644
--- a/crates/omsi-app/src/launcher/mod.rs
+++ b/crates/omsi-app/src/launcher/mod.rs
@@ -52,9 +52,9 @@ pub enum Page {
     Setup,
 }
 
-const PAGES: [(Page, &str, &str); 10] = [
+// (the Multiplayer page is hidden: multiplayer is disabled in this version)
+const PAGES: [(Page, &str, &str); 9] = [
     (Page::Drive, "Drive", "directions_bus"),
-    (Page::Multiplayer, "Multiplayer", "groups"),
     (Page::Profile, "Profile", "badge"),
     (Page::Settings, "Settings", "tune"),
     (Page::Controls, "Controls", "keyboard"),
diff --git a/crates/omsi-app/src/launcher/multiplayer.rs b/crates/omsi-app/src/launcher/multiplayer.rs
index deceb18..b723e6d 100644
--- a/crates/omsi-app/src/launcher/multiplayer.rs
+++ b/crates/omsi-app/src/launcher/multiplayer.rs
@@ -31,7 +31,11 @@ pub struct MultiplayerView {
 }
 
 pub fn draw(l: &mut Launcher, area: Rect) {
-    let body = l.page_title(area, "Multiplayer", "Drive with friends by a code, or on servers that are always on.");
+    let body = l.page_title(area, "Multiplayer", "Switched off in this version.");
+    if true {
+        l.ui.paragraph("Multiplayer is disabled while its structure is redone. Single-player is unaffected.", Vec2::new(body.x, body.y), body.w.min(520.0), 14.0, Weight::Regular, TEXT_DIM);
+        return;
+    }
     let tabs = Rect::new(body.x, body.y, 360.0, 36.0);
     let mut tab = l.mp.tab;
     if l.ui.segmented("mp-tab", tabs, &mut tab, &["Connect by Code", "Servers"]) {
diff --git a/crates/omsi-app/src/launcher/phone.rs b/crates/omsi-app/src/launcher/phone.rs
index c65c654..c998747 100644
--- a/crates/omsi-app/src/launcher/phone.rs
+++ b/crates/omsi-app/src/launcher/phone.rs
@@ -28,7 +28,8 @@ pub enum Tab {
     More,
 }
 
-const TABS: [(Tab, &str, &str); 4] = [(Tab::Play, "Play", "directions_bus"), (Tab::Online, "Online", "groups"), (Tab::Mods, "Mods", "extension"), (Tab::More, "More", "menu")];
+// (the Online tab is hidden: multiplayer is disabled in this version)
+const TABS: [(Tab, &str, &str); 3] = [(Tab::Play, "Play", "directions_bus"), (Tab::Mods, "Mods", "extension"), (Tab::More, "More", "menu")];
 
 /// A choice made on a sheet of its own.
 #[derive(Clone, Copy, PartialEq, Eq, Debug)]
@@ -1016,6 +1017,11 @@ fn time_sheet(l: &mut Launcher, r: Rect) -> bool {
 
 fn online(l: &mut Launcher, body: Rect) {
     let inner = body.pad(12.0, 12.0);
+    // (multiplayer is disabled in this version: nothing to join, nothing is asked)
+    if true {
+        l.ui.paragraph("Multiplayer is disabled while its structure is redone. Single-player is unaffected.", Vec2::new(inner.x, inner.y), inner.w, 14.0, Weight::Regular, TEXT);
+        return;
+    }
     let official = omsi_net::official::ALIAS;
     l.state.ask_server(official, 15.0);
     // the official server, first and large
diff --git a/crates/omsi-app/src/launcher/state.rs b/crates/omsi-app/src/launcher/state.rs
index 8f4e1cb..2349b5e 100644
--- a/crates/omsi-app/src/launcher/state.rs
+++ b/crates/omsi-app/src/launcher/state.rs
@@ -479,6 +479,10 @@ impl State {
     }
 
     pub fn check_join(&mut self) {
+        // (multiplayer is disabled: nothing is looked up)
+        if true {
+            return;
+        }
         if self.join_checked == self.choice.lan_addr {
             return;
         }
@@ -510,6 +514,10 @@ impl State {
 
     /// Ask a server about itself (its status and icon), at most every `every` seconds.
     pub fn ask_server(&mut self, address: &str, every: f32) {
+        // (multiplayer is disabled: no server is asked, nothing goes on the network)
+        if true {
+            return;
+        }
         if self.server_asked.get(address).map(|t| t.elapsed().as_secs_f32() < every).unwrap_or(false) {
             return;
         }
@@ -616,11 +624,8 @@ impl State {
     /// The duty as the backend takes it.
     pub fn duty(&self) -> core::Duty {
         let c = &self.choice;
-        let lan = match c.lan_mode.as_str() {
-            "host" => "host".to_string(),
-            "join" => format!("join:{}", c.lan_addr.trim()),
-            _ => "off".to_string(),
-        };
+        // (multiplayer is disabled: a duty never hosts or joins)
+        let lan = "off".to_string();
         // Joining: the host's map, as its status gives it (a code's host is asked when the
         // code is typed). The game takes it from the host's welcome too, but only when that
         // comes before the map is loaded: through a tunnel it came later, the game started
diff --git a/crates/omsi-app/src/lib.rs b/crates/omsi-app/src/lib.rs
index aa31d61..fdfbd5e 100644
--- a/crates/omsi-app/src/lib.rs
+++ b/crates/omsi-app/src/lib.rs
@@ -231,6 +231,10 @@ pub(crate) fn prepare(mut args: Args, bare: bool) -> Result<Option<(Args, Option
     // (a server has no interface to translate)
     mt::enable(settings::Settings::load().machine_translation && args.server.is_none());
     // the dedicated server: server.cfg decides the world, the rest is a host without a window
+    if args.server.is_some() {
+        eprintln!("server: the dedicated server is no longer built or supported (see docs/removed/MULTIPLAYER_DISABLED.md)");
+        std::process::exit(2);
+    }
     let server_cfg = match args.server.clone() {
         Some(p) => match server::prepare(&mut args, &p) {
             Ok(c) => Some(c),
diff --git a/crates/omsi-app/src/plugins.rs b/crates/omsi-app/src/plugins.rs
index 869b5bd..a17eb46 100644
--- a/crates/omsi-app/src/plugins.rs
+++ b/crates/omsi-app/src/plugins.rs
@@ -8,6 +8,10 @@ use omsi_script::SysVar;
 
 /// Load every plugin of every content root (`OMSI_NO_PLUGINS=1` leaves them out).
 pub(crate) fn load() -> Plugins {
+    // Plugins (native and Lua) are disabled in this client (docs/removed/MULTIPLAYER_DISABLED.md).
+    if true {
+        return Plugins::default();
+    }
     if omsi_cfg::env::var_os("OMSI_NO_PLUGINS").is_some() {
         return Plugins::default();
     }
diff --git a/docs/BUILDING.md b/docs/BUILDING.md
index 9893a7e..9e1d0e7 100644
--- a/docs/BUILDING.md
+++ b/docs/BUILDING.md
@@ -1,6 +1,6 @@
 # Building from source
 
-Releases for every commit are on the [Releases](https://github.com/openOMSI-Project/openOMSI/releases)
+Releases for every commit are on the [Releases](https://github.com/OmsiModsBR/depot/releases)
 page; build from source only to work on openOMSI itself.
 
 All scripts live in `scripts/`, run from any folder (paths with spaces are fine) and put the
diff --git a/docs/PLUGINS.md b/docs/PLUGINS.md
index dac8b4b..4366db0 100644
--- a/docs/PLUGINS.md
+++ b/docs/PLUGINS.md
@@ -1,3 +1,6 @@
+> **DISABLED.** Plugin loading (native and Lua) is switched off in this client; nothing under
+> `Plugins/` is loaded. See [removed/MULTIPLAYER_DISABLED.md](removed/MULTIPLAYER_DISABLED.md).
+
 # Plugins
 
 openOMSI runs two kinds of plugins from the `plugins` folder of the game (and of every
diff --git a/docs/SERVER.md b/docs/SERVER.md
index 2da10f3..bc0a601 100644
--- a/docs/SERVER.md
+++ b/docs/SERVER.md
@@ -1,3 +1,8 @@
+> **DISABLED.** The dedicated server is no longer built, packaged or supported, on any
+> architecture, and `--server` now exits with an error. The source is kept untouched for the future
+> multiplayer redo. See [docs/removed/MULTIPLAYER_DISABLED.md](removed/MULTIPLAYER_DISABLED.md).
+> Everything below is historical.
+
 # Dedicated server
 
 Depot 0.2.17 is a rebranding-only release of the openOMSI 0.2.16 baseline,
diff --git a/docs/USER_GUIDE.md b/docs/USER_GUIDE.md
index b3fb5a1..439c32d 100644
--- a/docs/USER_GUIDE.md
+++ b/docs/USER_GUIDE.md
@@ -66,7 +66,7 @@ whole start-up by itself (main switch, ignition, starter, gearbox to neutral); `
 is the same thing for an offscreen run.
 
 **Updates.** When the launcher starts it asks
-[github.com/openOMSI-Project/openOMSI](https://github.com/openOMSI-Project/openOMSI) for the latest release
+[github.com/OmsiModsBR/depot](https://github.com/OmsiModsBR/depot) for the latest release
 and, when there is a newer one, offers it: **Update now** downloads it (checked against the
 SHA-256 GitHub lists), puts the new program in place of the old one and starts the launcher
 again - on Windows `openomsi.exe` and `openomsi-launcher.exe`, on macOS the `openOMSI.app`
diff --git a/docs/VERSIONING.md b/docs/VERSIONING.md
index 9de8cfa..fef33be 100644
--- a/docs/VERSIONING.md
+++ b/docs/VERSIONING.md
@@ -86,5 +86,5 @@ The version badge at the top of the README always shows the newest release.
 
 [`.github/workflows/pages.yml`](../.github/workflows/pages.yml) publishes `site/` together with
 the Markdown files of `docs/` to GitHub Pages
-(https://openomsi-project.github.io/openOMSI/) whenever they change on `main`. The site renders
+(https://omsimods.com.br) whenever they change on `main`. The site renders
 the Markdown in the browser, so a documentation change is one edit in `docs/`.
diff --git a/scripts/build-server.sh b/scripts/build-server.sh
index 8585306..8283308 100755
--- a/scripts/build-server.sh
+++ b/scripts/build-server.sh
@@ -3,6 +3,10 @@
 # given as the first argument. The server is the game binary started with --server; the
 # folder gets start.sh and, on the first start, server.cfg with its defaults.
 set -eu
+# NOTICE: the dedicated server is no longer built or packaged, on any architecture. Its source
+# is kept untouched for the future multiplayer redo (docs/removed/MULTIPLAYER_DISABLED.md).
+echo "Dedicated server builds are disabled (see docs/removed/MULTIPLAYER_DISABLED.md)." >&2
+exit 1
 repo="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
 cd "$repo"
 dest="${1:-$repo/dist/server}"
diff --git a/scripts/server/start.cmd b/scripts/server/start.cmd
index 8a0b8f4..200294d 100644
--- a/scripts/server/start.cmd
+++ b/scripts/server/start.cmd
@@ -1,4 +1,6 @@
 @echo off
+echo The dedicated server is disabled.
+exit /b 1
 rem Start the Project Depot dedicated server: start.cmd C:\path\to\OMSI2 [server.cfg]
 rem (the OMSI 2 folder: a complete original installation, as the players have it; mods
 rem go into this folder's Vehicles, maps, Sceneryobjects ... as in the game's content folder)
diff --git a/scripts/server/start.sh b/scripts/server/start.sh
index ab84c28..e7699a1 100755
--- a/scripts/server/start.sh
+++ b/scripts/server/start.sh
@@ -3,6 +3,9 @@
 # (the OMSI 2 folder: a complete original installation, as the players have it; mods
 # go into this folder's Vehicles, maps, Sceneryobjects … as in the game's content folder)
 set -eu
+# The dedicated server is disabled (docs/removed/MULTIPLAYER_DISABLED.md).
+echo "The dedicated server is disabled." >&2
+exit 1
 here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
 root="${1:?usage: start.sh /path/to/OMSI2 [server.cfg]}"
 cfg="${2:-$here/server.cfg}"
diff --git a/site/app.js b/site/app.js
index bfca48d..76d5e91 100644
--- a/site/app.js
+++ b/site/app.js
@@ -1,6 +1,6 @@
 // openOMSI website: a small hash router that shows the overview, the download page and the
 // Markdown files of docs/ (copied next to this page by .github/workflows/pages.yml).
-const REPO = "openOMSI-Project/openOMSI";
+const REPO = "OmsiModsBR/depot";
 const DOCS = [
   { file: "USER_GUIDE", title: "User guide", icon: "sports_esports" },
   { file: "ANDROID", title: "Android & mobile", icon: "smartphone" },
diff --git a/site/index.html b/site/index.html
index 3d4deb5..eef7734 100644
--- a/site/index.html
+++ b/site/index.html
@@ -29,7 +29,7 @@
   <a class="icon-btn" href="https://discord.gg/VG2EKVafYG" aria-label="Discord" title="Discord">
     <svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor" aria-hidden="true"><path d="M20.32 4.37A19.8 19.8 0 0 0 15.39 2.8a13.8 13.8 0 0 0-.63 1.3 18.4 18.4 0 0 0-5.52 0 13.2 13.2 0 0 0-.64-1.3 19.7 19.7 0 0 0-4.93 1.53C.53 9.05-.32 13.58.1 18.06a19.9 19.9 0 0 0 6.04 3.05 14.7 14.7 0 0 0 1.29-2.1 12.9 12.9 0 0 1-2.04-.97l.5-.39a14.2 14.2 0 0 0 12.22 0l.5.39c-.65.39-1.33.71-2.04.98.37.74.8 1.44 1.29 2.1a19.8 19.8 0 0 0 6.04-3.06c.5-5.18-.84-9.67-3.58-13.69zM8.02 15.33c-1.18 0-2.16-1.09-2.16-2.42s.95-2.42 2.16-2.42 2.18 1.09 2.16 2.42c0 1.33-.95 2.42-2.16 2.42zm7.97 0c-1.18 0-2.16-1.09-2.16-2.42s.95-2.42 2.16-2.42 2.18 1.09 2.16 2.42c0 1.33-.94 2.42-2.16 2.42z"/></svg>
   </a>
-  <a class="icon-btn" href="https://github.com/openOMSI-Project/openOMSI" aria-label="GitHub" title="GitHub">
+  <a class="icon-btn" href="https://github.com/OmsiModsBR/depot" aria-label="GitHub" title="GitHub">
     <svg viewBox="0 0 16 16" width="24" height="24" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>
   </a>
 </header>
@@ -46,7 +46,7 @@
     <a href="https://discord.gg/VG2EKVafYG"><span class="material-icons">forum</span>Discord</a>
     <a href="https://buymeacoffee.com/usonskyyy" target="_blank" rel="noopener"><span class="material-icons">local_cafe</span>Buy me a coffee</a>
     <a href="https://ko-fi.com/usonance" target="_blank" rel="noopener"><span class="material-icons">volunteer_activism</span>Ko-fi</a>
-    <a href="https://github.com/openOMSI-Project/openOMSI"><span class="material-icons">code</span>Source code</a>
+    <a href="https://github.com/OmsiModsBR/depot"><span class="material-icons">code</span>Source code</a>
     <a href="#/releases" data-route="/releases"><span class="material-icons">new_releases</span>Releases</a>
     <a href="#/issues" data-route="/issues"><span class="material-icons">bug_report</span>Issues</a>
   </div>
@@ -67,7 +67,7 @@
   <div id="view"></div>
   <footer class="footer">
     openOMSI is an independent project, not affiliated with the makers of OMSI 2.
-    OMSI&nbsp;2 is required to play. · <a href="https://github.com/openOMSI-Project/openOMSI/blob/main/LICENSE">MIT License</a>
+    OMSI&nbsp;2 is required to play. · <a href="https://github.com/OmsiModsBR/depot/blob/main/LICENSE">MIT License</a>
   </footer>
 </main>
 
```
