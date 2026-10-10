# Removido: contador "playing now" (Cloudflare Worker de presença)

Data: 2026-10-10. Decisão de Leo (OmsiModsBR Team). Nenhum commit feito junto com esta alteração.

## O que era

Telemetria herdada do openOMSI 0.2.16: a cada 10 min o jogo enviava um id aleatório de sessão,
a versão e o SO (`/ping`, e `/bye` ao fechar) para um Cloudflare Worker (`services/presence/`)
hospedado em `openomsi.savvabestbrother.workers.dev`, conta do upstream que o Depot não controla.
O site lia a contagem em `/players`. Vinha **ligada por padrão** (opt-out).

## O que foi retirado

- `services/presence/` inteiro (`worker.js`, `wrangler.toml`, `README.md`).
- `crates/omsi-app/src/presence.rs` e o `mod presence` / campo `App.presence` / chamada `Presence::start()` / `self.presence = None`.
- Opção da UI do launcher "Count me in the website's \"playing now\" (anonymous)" e sua tradução em `locales/app.yml`.
- A flag `presence` em `omsi-launcher-core` (padrão, lista de booleanos, serialização do `settings.cfg`).
- Variáveis `OMSI_NO_PRESENCE` e `OMSI_PRESENCE_URL` (não existem mais).
- O chip "N playing now" do site (`site/app.js`, `index.html`, `style.css`) e o parágrafo "Playing now" do `docs/USER_GUIDE.md`.

Um `presence=` antigo no `settings.cfg` de usuários é simplesmente ignorado (a verificar em teste real).
**Não** afeta o Discord Rich Presence (`discord.rs`), que é outro módulo, local.

## Pendências

- Não consegui compilar nem rodar testes neste ambiente (rustc 1.97.0 abaixo do exigido por `reedsolomon-rs`; faltam libs de sistema do wayland). Rodar `cargo build`/`cargo test` antes do commit.
- O Worker já publicado continua no ar na conta do upstream; o Depot só deixa de falar com ele.
- Referências antigas no histórico do `CHANGELOG.md` (openOMSI) foram mantidas.
- Futuro: diagnóstico/contabilização próprio, em projeto derivado.

## Cópia das alterações (diff completo contra HEAD d28c9f1)

Inclui o conteúdo integral dos arquivos removidos. Para restaurar: salvar o bloco abaixo como `presence-worker-removal.patch` e rodar `git apply -R presence-worker-removal.patch`.

```diff
diff --git a/crates/omsi-app/locales/app.yml b/crates/omsi-app/locales/app.yml
index f02bf00..fa757ba 100644
--- a/crates/omsi-app/locales/app.yml
+++ b/crates/omsi-app/locales/app.yml
@@ -83,11 +83,6 @@ _version: 2
   zh-tw: "遊戲執行期間通知我有新版本"
   pt: "Avisar sobre uma nova versão durante a sessão"
   pt-pt: "Avisar-me sobre uma nova versão durante a sessão"
-"Count me in the website's \"playing now\" (anonymous)":
-  ca: "Compta'm a «jugant ara» del web (anònim)"
-  zh-tw: "將我計入網站的「目前遊玩人數」（匿名）"
-  pt: "Incluir-me no \"jogando agora\" do site (anônimo)"
-  pt-pt: "Incluir-me no \"a jogar agora\" do site (anónimo)"
 "Radio stations":
   ca: "Emissores de ràdio"
   zh-tw: "廣播電台"
diff --git a/crates/omsi-app/src/app.rs b/crates/omsi-app/src/app.rs
index e65c817..938a41d 100644
--- a/crates/omsi-app/src/app.rs
+++ b/crates/omsi-app/src/app.rs
@@ -336,8 +336,6 @@ pub(crate) struct App {
     pub(crate) notices: Vec<crate::ui::Notice>,
     /// The look for a newer release during the session (cards over the navigator).
     pub(crate) update_watch: crate::update_watch::UpdateWatch,
-    /// "Playing now" on the website (None: not counted, setting `presence`).
-    pub(crate) presence: Option<crate::presence::Presence>,
     /// What the log has said (see applog.rs).
     pub(crate) log_state: crate::applog::LogState,
     /// The driver's personnel file and this session's statistics.
diff --git a/crates/omsi-app/src/app_events.rs b/crates/omsi-app/src/app_events.rs
index 698980b..dd34d7f 100644
--- a/crates/omsi-app/src/app_events.rs
+++ b/crates/omsi-app/src/app_events.rs
@@ -3175,8 +3175,6 @@ impl ApplicationHandler for App {
     fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
         crate::game_lists::flush_settings(true);
         self.finish_session();
-        // ("playing now" ends with the game)
-        self.presence = None;
         if let Some(lan) = self.lan.take() {
             // dropping the session says goodbye (BYE) to the host or the players
             drop(lan);
diff --git a/crates/omsi-app/src/launcher/pages.rs b/crates/omsi-app/src/launcher/pages.rs
index 2bfa236..4bc0960 100644
--- a/crates/omsi-app/src/launcher/pages.rs
+++ b/crates/omsi-app/src/launcher/pages.rs
@@ -1184,7 +1184,6 @@ fn general_tab(ui: &mut Ui, s: &mut Value, dirty: &mut f32, out: &mut Outside, c
     toggle_setting(ui, s, dirty, c.row(), "Look for updates when the launcher starts", "update_check");
     toggle_setting(ui, s, dirty, c.row(), "Install updates without asking", "update_auto");
     toggle_setting(ui, s, dirty, c.row(), "Tell me about a new version during a session", "update_notify");
-    toggle_setting(ui, s, dirty, c.row(), "Count me in the website's \"playing now\" (anonymous)", "presence");
     {
         use crate::updater::Status;
         let r = c.row();
@@ -2798,7 +2797,7 @@ mod settings_tests {
         let general = vec![
             "s-lang", "set-machine_translation", "set-launcher_rest", "set-discord_status", "set-voice_chat", "s-uiscale", "set-ui_scale_window", "s-uiop", "set-tooltips", "set-show_fps", "set-notes", "set-chat", "s-chatsize", "set-name_tags",
             "set-navigator", "set-nav_arrows", "set-nav_ai", "corner-top-left", "corner-top-right", "corner-bottom-left", "corner-bottom-right",
-            "set-update_check", "set-update_auto", "set-update_notify", "set-presence", "s-upd-check", "s-upd-github", "s-reset",
+            "set-update_check", "set-update_auto", "set-update_notify", "s-upd-check", "s-upd-github", "s-reset",
         ];
         vec![graphics, driving, camera, sound, gameplay, general]
     }
diff --git a/crates/omsi-app/src/lib.rs b/crates/omsi-app/src/lib.rs
index 2872486..aa31d61 100644
--- a/crates/omsi-app/src/lib.rs
+++ b/crates/omsi-app/src/lib.rs
@@ -30,7 +30,6 @@ mod placing;
 mod mt;
 mod updater;
 mod update_watch;
-mod presence;
 mod ambience;
 mod camera_arm;
 mod career;
@@ -604,7 +603,6 @@ pub(crate) fn make_app(mut args: Args, server_cfg: Option<server::ServerCfg>) ->
         notices: Vec::new(),
         update_watch: crate::update_watch::UpdateWatch::new(),
         // (a server counts its players by their own games, not itself)
-        presence: if is_server { None } else { crate::presence::Presence::start() },
         log_state: Default::default(),
         plugins: None,
         career: Default::default(),
diff --git a/crates/omsi-app/src/presence.rs b/crates/omsi-app/src/presence.rs
deleted file mode 100644
index 97f16da..0000000
--- a/crates/omsi-app/src/presence.rs
+++ /dev/null
@@ -1,186 +0,0 @@
-//! "Playing now": while a game session runs it tells the project's presence service (a
-//! Cloudflare Worker, `services/presence/`) every few minutes that one more openOMSI is being
-//! played, and says goodbye when it ends. The website and the README show the count.
-//!
-//! What goes out is a random id made for this session alone (not kept anywhere, a new one
-//! every start), the game's version and the kind of system (windows, macos, linux,
-//! android) - no name, no map, nothing of the computer. The service keeps an id for 25
-//! minutes after its last word and stores no addresses.
-//!
-//! Setting `presence` (on by default; the launcher's Settings → General: "Count me on the
-//! website's 'playing now'"). `OMSI_NO_PRESENCE=1` switches it off, `OMSI_PRESENCE_URL`
-//! points it at another service (tests).
-
-use std::sync::atomic::{AtomicBool, Ordering};
-use std::sync::Arc;
-use std::time::Duration;
-
-/// The project's presence service (`services/presence/`, deployed with wrangler).
-pub(crate) const SERVICE: &str = "https://openomsi.savvabestbrother.workers.dev";
-/// How often a running game says it is still there (the service forgets one after 25 min).
-/// (Every three minutes, a few hundred players used up the free plan's 100 000 requests a
-/// day by the evening, and the counter answered nobody until midnight UTC.)
-const EVERY: Duration = Duration::from_secs(600);
-/// How long a game stays quiet when the service says it has too many requests (429).
-const BACK_OFF: Duration = Duration::from_secs(1800);
-
-/// The presence of this session: dropping it says goodbye.
-pub(crate) struct Presence {
-    stop: Arc<AtomicBool>,
-    thread: Option<std::thread::JoinHandle<()>>,
-}
-
-/// A fresh random id: two hashes of the clock by the process's own random hasher keys.
-fn session_id() -> String {
-    use std::hash::{BuildHasher, Hasher};
-    let keys = std::collections::hash_map::RandomState::new();
-    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
-    let half = |salt: u64| {
-        let mut h = keys.build_hasher();
-        h.write_u128(now);
-        h.write_u32(std::process::id());
-        h.write_u64(salt);
-        h.finish()
-    };
-    format!("{:016x}{:016x}", half(1), half(2))
-}
-
-/// The system, as the service counts it.
-fn system() -> &'static str {
-    if cfg!(target_os = "android") {
-        "android"
-    } else if cfg!(windows) {
-        "windows"
-    } else if cfg!(target_os = "macos") {
-        "macos"
-    } else {
-        "linux"
-    }
-}
-
-/// Whether the player lets the game be counted (`presence`, on unless switched off).
-fn allowed() -> bool {
-    if omsi_cfg::env::var_os("OMSI_NO_PRESENCE").is_some() {
-        return false;
-    }
-    let text = std::fs::read_to_string(omsi_launcher_lib::data_dir().join("settings.cfg")).ok();
-    omsi_launcher_lib::settings_from_text(text.as_deref()).get("presence").and_then(|v| v.as_bool()).unwrap_or(true)
-}
-
-/// What a post came to.
-#[derive(PartialEq, Clone, Copy)]
-enum Sent {
-    Ok,
-    /// the service is over its request limit: wait long before asking again
-    Limited,
-    Failed,
-}
-
-fn post(agent: &ureq::Agent, url: &str, body: &serde_json::Value) -> Sent {
-    match agent.post(url).set("Content-Type", "application/json").send_string(&body.to_string()) {
-        Ok(_) => Sent::Ok,
-        Err(ureq::Error::Status(429, _)) => Sent::Limited,
-        Err(_) => Sent::Failed,
-    }
-}
-
-/// A session's own offset into the ping period (0..60 s, from its id): the games started
-/// together after an update do not all ping in the same second.
-fn jitter(id: &str) -> Duration {
-    let n = u64::from_str_radix(&id[..8.min(id.len())], 16).unwrap_or(0);
-    Duration::from_secs(n % 60)
-}
-
-impl Presence {
-    /// Start saying "playing" (in the background); None when the player does not want it.
-    pub(crate) fn start() -> Option<Presence> {
-        if !allowed() {
-            log::info!("presence: not counted on the website (setting presence=0)");
-            return None;
-        }
-        let base = omsi_cfg::env::var("OMSI_PRESENCE_URL").unwrap_or_else(|_| SERVICE.to_string());
-        let base = base.trim_end_matches('/').to_string();
-        let stop = Arc::new(AtomicBool::new(false));
-        let flag = stop.clone();
-        let thread = std::thread::Builder::new()
-            .name("presence".into())
-            .spawn(move || {
-                let agent = ureq::AgentBuilder::new()
-                    .timeout_connect(Duration::from_secs(10))
-                    .timeout(Duration::from_secs(15))
-                    .user_agent(&format!("depot/{}", crate::updater::current_version()))
-                    .build();
-                let id = session_id();
-                let hello = serde_json::json!({ "id": id, "v": crate::updater::current_version(), "os": system() });
-                let mut said = false;
-                let mut first = true;
-                while !flag.load(Ordering::Relaxed) {
-                    let sent = post(&agent, &format!("{base}/ping"), &hello);
-                    let ok = sent == Sent::Ok;
-                    if ok != said || first {
-                        log::info!("presence: {}", match sent {
-                            Sent::Ok => "counted as playing",
-                            Sent::Limited => "the service is over its request limit (asking again in half an hour)",
-                            Sent::Failed => "the service did not answer (tried again later)",
-                        });
-                    }
-                    said = ok;
-                    let wait = if sent == Sent::Limited { BACK_OFF } else if first { EVERY + jitter(&id) } else { EVERY };
-                    first = false;
-                    // (a second at a time: the goodbye must not wait ten minutes)
-                    let mut slept = Duration::ZERO;
-                    while slept < wait && !flag.load(Ordering::Relaxed) {
-                        std::thread::sleep(Duration::from_secs(1));
-                        slept += Duration::from_secs(1);
-                    }
-                }
-                if said {
-                    post(&agent, &format!("{base}/bye"), &serde_json::json!({ "id": id }));
-                }
-            })
-            .ok()?;
-        Some(Presence { stop, thread: Some(thread) })
-    }
-}
-
-impl Drop for Presence {
-    fn drop(&mut self) {
-        self.stop.store(true, Ordering::Relaxed);
-        // (the goodbye has a few seconds; a game that ends does not hang on the network)
-        if let Some(t) = self.thread.take() {
-            let until = std::time::Instant::now() + Duration::from_secs(3);
-            while !t.is_finished() && std::time::Instant::now() < until {
-                std::thread::sleep(Duration::from_millis(50));
-            }
-        }
-    }
-}
-
-#[cfg(test)]
-mod tests {
-    #[test]
-    fn a_session_id_is_32_hex_digits_and_new_each_time() {
-        let (a, b) = (super::session_id(), super::session_id());
-        assert_eq!(a.len(), 32);
-        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
-        assert_ne!(a, b);
-    }
-
-    /// Against a service that runs (`OMSI_PRESENCE_URL=http://127.0.0.1:8787` with
-    /// `npx wrangler dev` in services/presence): counted while it runs, gone after.
-    #[test]
-    #[ignore = "needs the presence service running at OMSI_PRESENCE_URL"]
-    fn a_session_is_counted_while_it_runs() {
-        let base = std::env::var("OMSI_PRESENCE_URL").expect("OMSI_PRESENCE_URL");
-        let count = || ureq::get(&format!("{base}/players")).call().unwrap().into_string().map(|t| serde_json::from_str::<serde_json::Value>(&t).unwrap()).unwrap()["players"].as_u64().unwrap();
-        let before = count();
-        let p = super::Presence::start().expect("presence allowed");
-        std::thread::sleep(std::time::Duration::from_secs(2));
-        // (the counter's answer is cached for two minutes)
-        std::thread::sleep(std::time::Duration::from_secs(121));
-        assert_eq!(count(), before + 1);
-        drop(p);
-        std::thread::sleep(std::time::Duration::from_secs(121));
-        assert_eq!(count(), before);
-    }
-}
diff --git a/crates/omsi-launcher-core/src/lib.rs b/crates/omsi-launcher-core/src/lib.rs
index 45d1ee8..44ebc16 100644
--- a/crates/omsi-launcher-core/src/lib.rs
+++ b/crates/omsi-launcher-core/src/lib.rs
@@ -1968,9 +1968,8 @@ pub fn settings_from_text(text: Option<&str>) -> Value {
     v["head_idle"] = json!(0.0);
     v["head_idle_pace"] = json!(1.0);
     // updates from the GitHub releases: looked for when the launcher starts (and during a
-    // session, said over the navigator), installed after asking (or at once); and whether
-    // the game is counted on the website's "playing now"
-    for (k, d) in [("update_check", json!(true)), ("update_auto", json!(false)), ("update_notify", json!(true)), ("presence", json!(true))] {
+    // session, said over the navigator), installed after asking (or at once)
+    for (k, d) in [("update_check", json!(true)), ("update_auto", json!(false)), ("update_notify", json!(true))] {
         v[k] = d;
     }
     let Some(t) = text else { return v };
@@ -2041,7 +2040,7 @@ pub fn settings_from_text(text: Option<&str>) -> Value {
             "seat_x" | "seat_y" | "seat_z" => v[&k] = json!(val.parse::<f64>().map(|x| x.clamp(-1.5, 1.5)).unwrap_or(0.0)),
             "seat_pitch_deg" => v[&k] = json!(val.parse::<f64>().ok().filter(|x| x.is_finite()).unwrap_or(0.0).clamp(-45.0, 45.0)),
             "head_tracking_yaw_sens" | "head_tracking_pitch_sens" | "head_tracking_roll_sens" | "head_tracking_x_sens" | "head_tracking_y_sens" | "head_tracking_z_sens" => v[&k] = json!(val.parse::<f64>().ok().filter(|x| x.is_finite()).map(|x| x.clamp(0.0, 100.0)).unwrap_or(100.0)),
-            "nav_arrows" | "nav_ai" | "get_up" | "time_sync" | "metar_sync" | "ui_scale_window" | "notes" | "machine_translation" | "update_check" | "update_auto" | "update_notify" | "presence" | "reflections" | "steering_linear" | "old_steering" | "red_steer_spd" | "ff_invert" | "ff_enabled" | "brake_hold" | "auto_clutch" | "momentary_gears" | "auto_shift" | "mouse_steering" | "mouse_right_off" | "mouse_smooth" | "blinker_cancel" => v[&k] = json!(b(val)),
+            "nav_arrows" | "nav_ai" | "get_up" | "time_sync" | "metar_sync" | "ui_scale_window" | "notes" | "machine_translation" | "update_check" | "update_auto" | "update_notify" | "reflections" | "steering_linear" | "old_steering" | "red_steer_spd" | "ff_invert" | "ff_enabled" | "brake_hold" | "auto_clutch" | "momentary_gears" | "auto_shift" | "mouse_steering" | "mouse_right_off" | "mouse_smooth" | "blinker_cancel" => v[&k] = json!(b(val)),
             "info_bar" => v[&k] = json!(b(val)),
             "windy_trees" => v[&k] = json!(b(val)),
             "time_speed" => v[&k] = json!(val.trim_start_matches(['x', 'X']).parse::<f64>().map(|x| x.clamp(1.0, 30.0)).map(|x| if x.fract() == 0.0 { format!("{}", x as i64) } else { x.to_string() }).unwrap_or_else(|_| "1".into())),
@@ -2298,7 +2297,7 @@ pub fn settings_to_text(v: &Value, old: Option<&str>) -> String {
         b("precision_zoom", false),
     );
     let text = format!(
-        "{text}pax_voices={}\nnav_arrows={}\nnav_ai={}\nget_up={}\ntime_speed={}\nmachine_translation={}\nshadow_casters={}\nshadow_blobs={}\nctrl_deadzone={}\nupdate_check={}\nupdate_auto={}\nupdate_notify={}\npresence={}\nreflections={}\nmouse_sens={}\ngraphics_api={}\nctrl_off={}\nsteering_linear={}\nold_steering={}\nred_steer_spd={}\nff_invert={}\nwheel_range={}\nwheel_lock={}\nfov={}\ncamera_collision={}\npedal_throttle={}\npedal_brake={}\nseat_x={}\nseat_y={}\nseat_z={}\nseat_pitch_deg={}\nsteer_look={}\nhead_tracking={}\nhead_tracking_yaw_sens={}\nhead_tracking_pitch_sens={}\nhead_tracking_roll_sens={}\nhead_tracking_x_sens={}\nhead_tracking_y_sens={}\nhead_tracking_z_sens={}\nhead_tracking_invert_yaw={}\nhead_tracking_invert_pitch={}\nhead_tracking_invert_roll={}\nhead_tracking_invert_x={}\nhead_tracking_invert_y={}\nhead_tracking_invert_z={}\nff_enabled={}\nbrake_hold={}\nauto_clutch={}\nmomentary_gears={}\nauto_shift={}\nled_glow={}\nled_mips={}\nnight_brightness={}\nui_scale={}\nui_scale_window={}\nchat_size={}\nnotes={}\nmouse_steering={}\nmouse_right_off={}\nmouse_smooth={}\nblinker_cancel={}\nff_road_vib={}\nff_engine_vib={}\nff_fade={}\n",
+        "{text}pax_voices={}\nnav_arrows={}\nnav_ai={}\nget_up={}\ntime_speed={}\nmachine_translation={}\nshadow_casters={}\nshadow_blobs={}\nctrl_deadzone={}\nupdate_check={}\nupdate_auto={}\nupdate_notify={}\nreflections={}\nmouse_sens={}\ngraphics_api={}\nctrl_off={}\nsteering_linear={}\nold_steering={}\nred_steer_spd={}\nff_invert={}\nwheel_range={}\nwheel_lock={}\nfov={}\ncamera_collision={}\npedal_throttle={}\npedal_brake={}\nseat_x={}\nseat_y={}\nseat_z={}\nseat_pitch_deg={}\nsteer_look={}\nhead_tracking={}\nhead_tracking_yaw_sens={}\nhead_tracking_pitch_sens={}\nhead_tracking_roll_sens={}\nhead_tracking_x_sens={}\nhead_tracking_y_sens={}\nhead_tracking_z_sens={}\nhead_tracking_invert_yaw={}\nhead_tracking_invert_pitch={}\nhead_tracking_invert_roll={}\nhead_tracking_invert_x={}\nhead_tracking_invert_y={}\nhead_tracking_invert_z={}\nff_enabled={}\nbrake_hold={}\nauto_clutch={}\nmomentary_gears={}\nauto_shift={}\nled_glow={}\nled_mips={}\nnight_brightness={}\nui_scale={}\nui_scale_window={}\nchat_size={}\nnotes={}\nmouse_steering={}\nmouse_right_off={}\nmouse_smooth={}\nblinker_cancel={}\nff_road_vib={}\nff_engine_vib={}\nff_fade={}\n",
         match v.get("pax_voices").and_then(|x| x.as_str()).unwrap_or("all") {
             "tickets" => "tickets",
             "off" => "off",
@@ -2319,7 +2318,6 @@ pub fn settings_to_text(v: &Value, old: Option<&str>) -> String {
         b("update_check", true),
         b("update_auto", false),
         b("update_notify", true),
-        b("presence", true),
         b("reflections", true),
         f("mouse_sens", 1.0).clamp(0.1, 3.0),
         match v.get("graphics_api").and_then(|x| x.as_str()).unwrap_or("auto") {
diff --git a/docs/USER_GUIDE.md b/docs/USER_GUIDE.md
index f1d5ed7..b3fb5a1 100644
--- a/docs/USER_GUIDE.md
+++ b/docs/USER_GUIDE.md
@@ -89,14 +89,6 @@ goes on). `OMSI_NO_UPDATE=1` switches the checks off; `OMSI_UPDATE_URL` points t
 release description (GitHub's format; `file://` works, for testing - the game then looks after
 3 s).
 
-**Playing now.** While a session runs the game tells the project's counter (a Cloudflare
-Worker, `services/presence/`) every ten minutes that it is being played, and says goodbye
-when it ends; the website and the README show how many play right now. What goes out is a
-random id made new for each session, the version and the kind of system - nothing else, and
-the counter keeps no addresses. Settings → General → "Count me in the website's \"playing
-now\"" (on by default) or `OMSI_NO_PRESENCE=1` switch it off; a dedicated server is never
-counted.
-
 **O** switches mouse steering on and off, as in OMSI (Omsi.exe's own formula): the cursor's
 place across the whole window is the steering from full left to full right lock
 (`[inv_min_turnradius]` of the bus), from the middle up to the top edge is the throttle and
diff --git a/services/presence/.gitignore b/services/presence/.gitignore
deleted file mode 100644
index 41a2578..0000000
--- a/services/presence/.gitignore
+++ /dev/null
@@ -1,2 +0,0 @@
-.wrangler/
-node_modules/
diff --git a/services/presence/README.md b/services/presence/README.md
deleted file mode 100644
index b665110..0000000
--- a/services/presence/README.md
+++ /dev/null
@@ -1,26 +0,0 @@
-# Presence: "playing now"
-
-A Cloudflare Worker that counts the openOMSI games being played right now. A running game
-posts `/ping` every ten minutes and `/bye` when it ends (`crates/omsi-app/src/presence.rs`,
-setting `presence`, on by default; after a 429 it waits half an hour); a session counts for
-25 minutes after its last ping.
-What it gets is a random id made for that session, the game's version and the kind of system -
-no name, and no address is stored.
-
-- `GET /players` - `{"players": 12, "systems": {"windows": 9, "linux": 2, "android": 1}, "updated": "..."}`
-  (the website reads it)
-- `GET /badge` - the count for a [shields.io endpoint badge](https://shields.io/badges/endpoint-badge)
-  (the README shows it)
-
-## Deploy
-
-Cloudflare's Workers Builds deploys it from this repository on every push to `main`
-(Worker `openomsi`, root directory `services/presence`, deploy command `npx wrangler deploy`).
-By hand: `npx wrangler login` once, then `npx wrangler deploy` in this folder.
-
-It runs on Cloudflare's free plan (a Worker and one Durable Object, which keeps the sessions in memory and nothing in its storage): 100 000
-requests a day, each ping one. That holds about 650 players at once around the clock (more
-in practice: nobody plays all day); over it Cloudflare answers everything with error 1027
-until midnight UTC. Workers Paid (5 USD a month, 10 million requests) lifts the limit. The address
-`wrangler deploy` prints goes into `SERVICE` in `crates/omsi-app/src/presence.rs`, the badge in
-`README.md` and `PRESENCE` in `site/app.js`.
diff --git a/services/presence/worker.js b/services/presence/worker.js
deleted file mode 100644
index d888d6f..0000000
--- a/services/presence/worker.js
+++ /dev/null
@@ -1,118 +0,0 @@
-// The "playing now" counter of openOMSI: a running game says every three minutes that it is
-// being played (crates/omsi-app/src/presence.rs; every ten minutes, every three in games
-// before 0.1.1552), the website and the README badge read how many are. One Durable Object
-// keeps the sessions in memory (a random id each, its system and the game's version, the time of its
-// last word) and forgets one 25 minutes after that. Everything has to fit the free plan's
-// 100 000 requests a day: over it Cloudflare answers every request with error 1027 until
-// midnight UTC.
-// No address and nothing else about a player is kept.
-//
-//   POST /ping  {"id": "<32 hex>", "v": "0.1.1512", "os": "windows"}   -> 204
-//   POST /bye   {"id": "<32 hex>"}                                       -> 204
-//   GET  /players -> {"players": 12, "systems": {"windows": 9, ...}, "updated": "..."}
-//   GET  /badge   -> the same count for a shields.io endpoint badge
-
-import { DurableObject } from "cloudflare:workers";
-
-const ALIVE_MS = 25 * 60 * 1000;
-const SYSTEMS = ["windows", "macos", "linux", "android"];
-
-export class Presence extends DurableObject {
-  // (in memory, not in the object's storage: a session lives 25 minutes, so a restarted
-  // object is right again within one ping period, and nothing is read or written per ping)
-  constructor(ctx, env) {
-    super(ctx, env);
-    this.sessions = new Map();
-  }
-
-  forget(now) {
-    for (const [id, s] of this.sessions) {
-      if (s.seen < now - ALIVE_MS) this.sessions.delete(id);
-    }
-  }
-
-  async ping(id, os, v) {
-    this.sessions.set(id, { seen: Date.now(), os, v });
-  }
-
-  async bye(id) {
-    this.sessions.delete(id);
-  }
-
-  async count() {
-    this.forget(Date.now());
-    const systems = {};
-    for (const s of this.sessions.values()) {
-      systems[s.os] = (systems[s.os] || 0) + 1;
-    }
-    return { players: this.sessions.size, systems, updated: new Date().toISOString() };
-  }
-}
-
-const CORS = {
-  "Access-Control-Allow-Origin": "*",
-  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
-  "Access-Control-Allow-Headers": "Content-Type",
-};
-
-function json(body, status = 200, extra = {}) {
-  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json", ...CORS, ...extra } });
-}
-
-async function body(request) {
-  try {
-    const b = await request.json();
-    return b && typeof b === "object" ? b : null;
-  } catch {
-    return null;
-  }
-}
-
-const validId = (id) => typeof id === "string" && /^[0-9a-f]{32}$/.test(id);
-
-export default {
-  async fetch(request, env, ctx) {
-    try {
-      return await handle(request, env, ctx);
-    } catch (e) {
-      // (the counter unreachable: say why, and ask the games to wait as for a 429)
-      return new Response(`openOMSI presence: ${e}\n`, { status: 503, headers: { "Content-Type": "text/plain", "Retry-After": "1800", ...CORS } });
-    }
-  },
-};
-
-async function handle(request, env, ctx) {
-  const url = new URL(request.url);
-  const counter = env.PRESENCE.get(env.PRESENCE.idFromName("openomsi"));
-  if (request.method === "OPTIONS") {
-    return new Response(null, { status: 204, headers: CORS });
-  }
-  if (request.method === "POST" && url.pathname === "/ping") {
-    const b = await body(request);
-    if (!b || !validId(b.id)) return json({ error: "bad id" }, 400);
-    const os = SYSTEMS.includes(b.os) ? b.os : "other";
-    const v = typeof b.v === "string" ? b.v.slice(0, 32) : "";
-    await counter.ping(b.id, os, v);
-    return new Response(null, { status: 204, headers: CORS });
-  }
-  if (request.method === "POST" && url.pathname === "/bye") {
-    const b = await body(request);
-    if (b && validId(b.id)) await counter.bye(b.id);
-    return new Response(null, { status: 204, headers: CORS });
-  }
-  if (request.method === "GET" && (url.pathname === "/players" || url.pathname === "/badge")) {
-    // (read at most every two minutes from the counter, and cached by browsers and
-    // shields.io as long: the website and the badge can be asked as often as anybody likes)
-    const cache = caches.default;
-    const key = new Request(url.origin + url.pathname);
-    const hit = await cache.match(key);
-    if (hit) return hit;
-    const c = await counter.count();
-    const out = url.pathname === "/badge"
-      ? json({ schemaVersion: 1, label: "playing now", message: String(c.players), color: c.players > 0 ? "brightgreen" : "lightgrey", cacheSeconds: 300 }, 200, { "Cache-Control": "public, max-age=120" })
-      : json(c, 200, { "Cache-Control": "public, max-age=120" });
-    ctx.waitUntil(cache.put(key, out.clone()));
-    return out;
-  }
-  return new Response("openOMSI presence: GET /players, GET /badge\n", { status: url.pathname === "/" ? 200 : 404, headers: { "Content-Type": "text/plain", ...CORS } });
-}
diff --git a/services/presence/wrangler.toml b/services/presence/wrangler.toml
deleted file mode 100644
index 512ab23..0000000
--- a/services/presence/wrangler.toml
+++ /dev/null
@@ -1,12 +0,0 @@
-# The "playing now" counter of openOMSI (see README.md beside this file).
-name = "openomsi"
-main = "worker.js"
-compatibility_date = "2026-09-01"
-
-[[durable_objects.bindings]]
-name = "PRESENCE"
-class_name = "Presence"
-
-[[migrations]]
-tag = "v1"
-new_sqlite_classes = ["Presence"]
diff --git a/site/app.js b/site/app.js
index f80fb0d..bfca48d 100644
--- a/site/app.js
+++ b/site/app.js
@@ -1,8 +1,6 @@
 // openOMSI website: a small hash router that shows the overview, the download page and the
 // Markdown files of docs/ (copied next to this page by .github/workflows/pages.yml).
 const REPO = "openOMSI-Project/openOMSI";
-// the "playing now" counter (services/presence/): the games running right now
-const PRESENCE = "https://openomsi.savvabestbrother.workers.dev";
 const DOCS = [
   { file: "USER_GUIDE", title: "User guide", icon: "sports_esports" },
   { file: "ANDROID", title: "Android & mobile", icon: "smartphone" },
@@ -81,31 +79,11 @@ function showTemplate(id) {
 
 async function home() {
   showTemplate("home");
-  playingNow();
   const rel = await latestRelease();
   const chip = document.querySelector("#version-chip span:last-child");
   if (chip) chip.textContent = rel.tag_name ? `Latest: ${rel.tag_name.replace(/^v/, "")}` : "No release yet";
 }
 
-// How many play openOMSI right now, asked again every five minutes while the page shows it
-// (the counter runs on Cloudflare's free plan: every request counts against its daily limit).
-let playingTimer = null;
-async function playingNow() {
-  clearTimeout(playingTimer);
-  const chip = document.getElementById("players-chip");
-  if (!chip) return;
-  try {
-    const r = await fetch(`${PRESENCE}/players`);
-    const p = r.ok ? await r.json() : null;
-    if (p && typeof p.players === "number") {
-      chip.lastElementChild.textContent = `${p.players} playing now`;
-      chip.title = Object.entries(p.systems || {}).map(([os, n]) => `${os}: ${n}`).join(", ");
-      chip.hidden = false;
-    }
-  } catch { /* (the counter is out of reach: the chip stays hidden) */ }
-  playingTimer = setTimeout(playingNow, 300000);
-}
-
 async function download() {
   showTemplate("download");
   const rel = await latestRelease();
diff --git a/site/index.html b/site/index.html
index 4271dc7..3d4deb5 100644
--- a/site/index.html
+++ b/site/index.html
@@ -83,7 +83,6 @@
     </div>
     <div class="chip chip-warn"><span class="material-icons">construction</span><span>Early release: expect bugs</span></div>
     <div class="chip" id="version-chip"><span class="material-icons">sell</span><span>latest version…</span></div>
-    <div class="chip chip-live" id="players-chip" hidden><span class="material-icons">sports_esports</span><span></span></div>
   </section>
 
   <div class="content">
diff --git a/site/style.css b/site/style.css
index e33cbe2..f44c122 100644
--- a/site/style.css
+++ b/site/style.css
@@ -307,6 +307,3 @@ h1 { line-height: 1.25; }
 .dl-mine { outline-offset: -2px; }
 .dl-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
 .dl-badge { white-space: nowrap; font-size: 12px; font-weight: 600; line-height: 1; padding: 5px 10px; border-radius: 999px; background: var(--primary); color: #fff; }
-
-/* "playing now": a green dot in front of the count */
-.chip-live .material-icons { color: #2ea043; }
```
