// Project Depot temporary website: a small hash router that shows the overview, the download page and the
// Markdown files of docs/ (copied next to this page by .github/workflows/pages.yml).
const REPO = "OmsiModsBR/depot";
const DOCS = [
  { file: "USER_GUIDE", title: "User guide", icon: "sports_esports" },
  { file: "MODDING", title: "Modding beyond OMSI 2", icon: "handyman" },
  { file: "PBR", title: "PBR materials", icon: "texture" },
  { file: "BUILDING", title: "Building", icon: "build" },
  { file: "FORMATS", title: "Content formats", icon: "description" },
  { file: "ARCHITECTURE", title: "Architecture", icon: "account_tree" },
  { file: "ROUTES", title: "Routes", icon: "alt_route" },
  { file: "VERSIONING", title: "Versioning & releases", icon: "new_releases" },
];
const PLATFORMS = [
  { key: "windows-x64", name: "plat.windows", icon: "desktop_windows", note: "plat.windows.note", os: "windows" },
];
// The dedicated server was only published with 0.2.17 and is discontinued (see docs/removed/MULTIPLAYER_DISABLED.md).
const SERVERS = [
  { key: "server-windows-x64", name: "plat.server", icon: "dns", note: "plat.server.note" },
];

// Which build fits the visitor, roughly: the one to put first and mark.
function visitorOs() {
  const ua = navigator.userAgent || "";
  if (/Android/i.test(ua)) return "android";
  if (/Windows/i.test(ua)) return /ARM|aarch64/i.test(ua) ? "windows-arm" : "windows";
  if (/Mac OS X|Macintosh/i.test(ua)) return "mac";
  if (/Linux/i.test(ua)) return /aarch64|arm64/i.test(ua) ? "linux-arm" : "linux";
  return "";
}

const view = document.getElementById("view");
const drawer = document.getElementById("drawer");
const scrim = document.getElementById("scrim");

function drawDocLinks() {
  document.getElementById("doc-links").innerHTML = DOCS.map(d =>
    `<a href="#/docs/${d.file}" data-route="/docs/${d.file}"><span class="material-icons">${d.icon}</span>${t("doc." + d.file)}</a>`).join("");
}
drawDocLinks();

function toggleDrawer(open) {
  drawer.classList.toggle("open", open);
  scrim.classList.toggle("open", open);
}
document.getElementById("menu-btn").onclick = () => toggleDrawer(!drawer.classList.contains("open"));
scrim.onclick = () => toggleDrawer(false);

let release = null;
let releaseAt = 0;
async function latestRelease() {
  // (a release comes with every push: what was fetched ten minutes ago may be old)
  if (release && Date.now() - releaseAt < 120000) return release;
  releaseAt = Date.now();
  try {
    const r = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`);
    release = r.ok ? await r.json() : { none: true };
  } catch { release = { none: true }; }
  return release;
}

function slug(text) {
  return text.toLowerCase().replace(/<[^>]+>/g, "").replace(/[^\w\- ]+/g, "").trim().replace(/\s+/g, "-");
}

function showTemplate(id) {
  view.innerHTML = "";
  view.appendChild(document.getElementById(id).content.cloneNode(true));
  applyLang(view);
}

async function home() {
  showTemplate("home");
  const rel = await latestRelease();
  const chip = document.querySelector("#version-chip span:last-child");
  if (chip) chip.textContent = rel.tag_name ? t("chip.latest", rel.tag_name.replace(/^v/, "")) : t("chip.none");
}

async function download() {
  showTemplate("download");
  const rel = await latestRelease();
  const grid = document.getElementById("dl-grid");
  const ver = document.getElementById("dl-version");
  if (!rel.tag_name) {
    ver.textContent = t("dl.none");
    return;
  }
  const v = String(rel.tag_name).replace(/[^0-9A-Za-z._-]/g, "").replace(/^v/, "");
  ver.innerHTML = `${t("dl.latest", v)} · ${new Date(rel.published_at).toLocaleDateString(lang)}`;
  // Each file by its exact name: matched by its ending alone, "-windows-x64.zip" was the
  // dedicated server's zip as well, and the Windows button downloaded the server.
  const card = p => {
    const file = `depot-${v}-${p.key}.${p.ext || "zip"}`;
    const a = (rel.assets || []).find(a => a.name === file);
    const size = a ? ` · ${(a.size / 1048576).toFixed(0)} MB` : "";
    const mine = p.os && p.os === visitorOs();
    return `<div class="card elevation-${mine ? 3 : 1} dl-card${mine ? " dl-mine" : ""}"><div class="dl-head"><span class="material-icons card-icon">${p.icon}</span>${mine ? `<span class="dl-badge">${t("dl.mine")}</span>` : ""}</div>
      <h3>${t(p.name)}</h3><p>${t(p.note)}${size}</p>
      ${a ? `<a class="btn btn-contained" href="${esc(safeUrl(a.browser_download_url))}" download><span class="material-icons">download</span>${t("dl.download")}</a>`
          : `<p>${t("dl.notin")}</p>`}</div>`;
  };
  const games = [...PLATFORMS].sort((x, y) => (y.os === visitorOs()) - (x.os === visitorOs()));
  grid.innerHTML = games.map(card).join("")
    + `<h2 class="section-title" style="grid-column:1/-1">${t("dl.server")}</h2>`
    + SERVERS.map(card).join("");
}

async function doc(name, anchor) {
  const meta = DOCS.find(d => d.file === name);
  if (!meta) { view.innerHTML = `<div class="content"><div class="doc"><h1>${t("doc.notfound")}</h1><p>${t("doc.nosuch")}</p></div></div>`; return; }
  view.innerHTML = `<div class="content"><div class="doc"><div class="loading">${t("loading")}</div></div></div>`;
  let md;
  try {
    const r = await fetch(`docs/${name}.md`);
    if (!r.ok) throw new Error(r.status);
    md = await r.text();
  } catch {
    view.querySelector(".doc").innerHTML = `<h1>${t("doc.notfound")}</h1><p>${t("doc.noload")}</p>`;
    return;
  }
  const box = view.querySelector(".doc");
  box.innerHTML = `<div class="doc-toolbar"><a href="https://github.com/${REPO}/blob/main/docs/${name}.md">
    <span class="material-icons" style="font-size:18px">edit</span> ${t("doc.edit")}</a></div>${lang === "pt-BR" ? `<p class="muted">${t("doc.enonly")}</p>` : ""}` + clean(marked.parse(md));
  document.title = `${t("doc." + name)} · Project Depot`;
  // heading anchors
  box.querySelectorAll("h1, h2, h3, h4").forEach(h => {
    h.id = slug(h.textContent);
    if (h.tagName !== "H1") h.insertAdjacentHTML("beforeend", `<a class="anchor" href="#/docs/${name}#${h.id}">#</a>`);
  });
  // links between documents stay inside the site; other repository files go to GitHub
  box.querySelectorAll("a[href]").forEach(a => {
    const href = a.getAttribute("href");
    if (/^(https?:|mailto:|#)/.test(href)) return;
    const m = href.match(/^(?:\.\/)?([A-Z_]+)\.md(?:#(.*))?$/);
    if (m && DOCS.some(d => d.file === m[1])) {
      a.setAttribute("href", `#/docs/${m[1]}${m[2] ? "#" + m[2] : ""}`);
    } else {
      a.setAttribute("href", `https://github.com/${REPO}/blob/main/docs/${href}`);
    }
  });
  if (anchor) document.getElementById(anchor)?.scrollIntoView();
  else window.scrollTo(0, 0);
}


// --- Releases and issues, read from the GitHub API (60 requests an hour per visitor
// without a token: every list is fetched once per visit and kept)
const cache = {};
async function gh(path) {
  if (cache[path]) return cache[path];
  const r = await fetch(`https://api.github.com/repos/${REPO}/${path}`, { headers: { Accept: "application/vnd.github+json" } });
  if (!r.ok) throw new Error(r.status === 403 ? t("api.limit") : t("api.status", r.status));
  return (cache[path] = await r.json());
}
// release asset links must be https on github.com (or the API's own download hosts)
function safeUrl(u) {
  try {
    const x = new URL(u);
    if (x.protocol === "https:" && (x.hostname === "github.com" || x.hostname.endsWith(".githubusercontent.com"))) return x.href;
  } catch {}
  return "#";
}
function esc(t) {
  return String(t ?? "").replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
// What may stay in rendered Markdown: no scripts, frames, forms, buttons, <base>, styles or
// event handlers (DOMPurify's allow-list does the rest); links only https, mailto, in-page or
// relative.
const SANITIZE = {
  FORBID_TAGS: ["base", "style", "form", "button", "svg", "math", "link", "meta"],
  FORBID_ATTR: ["style", "class", "formaction", "xlink:href"],
  ALLOWED_URI_REGEXP: /^(?:https?:|mailto:|[^a-z]|[a-z0-9+.\-]+(?:[^a-z0-9+.\-:]|$))/i,
};
function clean(html) { return DOMPurify.sanitize(html, SANITIZE); }
function md(text) {
  return clean(marked.parse(text || ""));
}
function ago(date) {
  const s = (Date.now() - new Date(date)) / 1000;
  for (const [n, u] of [[31536000, "year"], [2592000, "month"], [86400, "day"], [3600, "hour"], [60, "minute"]]) {
    if (s >= n) return t("ago.n", Math.floor(s / n), u);
  }
  return t("ago.now");
}
function labelChips(labels) {
  return (labels || []).map(l => `<span class="label" style="--lc:#${esc(l.color)}">${esc(l.name)}</span>`).join("");
}

async function releases() {
  view.innerHTML = `<div class="content"><h1>${t("rel.h")}</h1>
    <p class="lead">${t("rel.lead")}</p>
    <div id="rel-list"><div class="loading">${t("loading")}</div></div></div>`;
  document.title = `${t("rel.h")} · Project Depot`;
  const box = document.getElementById("rel-list");
  let list;
  try { list = await gh("releases?per_page=30"); } catch (e) { box.innerHTML = `<p>${esc(e.message)} <a href="https://github.com/${REPO}/releases">${t("rel.ongh")}</a></p>`; return; }
  if (!list.length) { box.innerHTML = `<p>${t("rel.none")}</p>`; return; }
  box.innerHTML = list.map((r, i) => {
    const assets = (r.assets || []).map(a => `<a class="asset" href="${esc(safeUrl(a.browser_download_url))}"><span class="material-icons">download</span>${esc(a.name)} <small>${(a.size / 1048576).toFixed(0)} MB</small></a>`).join("");
    return `<details class="card elevation-1 release"${i === 0 ? " open" : ""}>
      <summary><span class="rel-tag">${esc(r.tag_name.replace(/^v/, ""))}</span>
        ${i === 0 ? `<span class="label" style="--lc:#2da44e">${t("rel.latest")}</span>` : ""}${r.prerelease ? `<span class="label" style="--lc:#bf8700">${t("rel.pre")}</span>` : ""}
        <span class="rel-name">${esc(r.name && r.name !== r.tag_name ? r.name : "")}</span>
        <span class="spacer"></span><span class="muted">${new Date(r.published_at).toLocaleDateString(lang)}</span></summary>
      <div class="doc rel-body">${md(r.body) || `<p class='muted'>${t("rel.nonotes")}</p>`}</div>
      ${assets ? `<div class="assets">${assets}</div>` : ""}
    </details>`;
  }).join("");
}

let issueState = "open", issueQuery = "";
async function issues(number) {
  if (number) return issue(number);
  view.innerHTML = `<div class="content"><h1>${t("iss.h")}</h1>
    <p class="lead">${t("iss.lead")}</p>
    <p class="issue-new"><a class="btn btn-contained" href="https://github.com/${REPO}/issues/new"><span class="material-icons">add</span>${t("iss.new")}</a></p>
    <div class="issue-bar">
      <div class="tabs"><button data-s="open">${t("iss.open")}</button><button data-s="closed">${t("iss.closed")}</button><button data-s="all">${t("iss.all")}</button></div>
      <input id="iq" type="search" placeholder="${t("iss.filter")}" value="${esc(issueQuery)}">
    </div>
    <div id="issue-list"><div class="loading">${t("loading")}</div></div></div>`;
  document.title = `${t("iss.h")} · Project Depot`;
  view.querySelectorAll(".tabs button").forEach(b => {
    b.classList.toggle("on", b.dataset.s === issueState);
    b.onclick = () => { issueState = b.dataset.s; issues(); };
  });
  const box = document.getElementById("issue-list");
  let list;
  try { list = await gh(`issues?state=${issueState}&per_page=100&sort=updated`); } catch (e) { box.innerHTML = `<p>${esc(e.message)} <a href="https://github.com/${REPO}/issues">${t("iss.ongh")}</a></p>`; return; }
  // (the issues list also carries pull requests)
  list = list.filter(i => !i.pull_request);
  const draw = () => {
    const q = issueQuery.trim().toLowerCase();
    const shown = list.filter(i => !q || i.title.toLowerCase().includes(q) || (i.labels || []).some(l => l.name.toLowerCase().includes(q)) || String(i.number) === q.replace("#", ""));
    box.innerHTML = shown.length ? `<div class="card elevation-1 issue-list">${shown.map(i => `
      <a class="issue-row" href="#/issues/${i.number}">
        <span class="material-icons ${i.state === "open" ? "st-open" : "st-closed"}">${i.state === "open" ? "radio_button_unchecked" : "check_circle"}</span>
        <span class="issue-main"><span class="issue-title">${esc(i.title)}</span> ${labelChips(i.labels)}
          <span class="muted">#${i.number} · ${esc(i.user?.login)} · ${t("iss.updated", ago(i.updated_at))}</span></span>
        ${i.comments ? `<span class="muted cm"><span class="material-icons">chat_bubble_outline</span>${i.comments}</span>` : ""}
      </a>`).join("")}</div>` : `<p class="muted">${t("iss.nothing")}</p>`;
  };
  document.getElementById("iq").oninput = e => { issueQuery = e.target.value; draw(); };
  draw();
}

async function issue(n) {
  view.innerHTML = `<div class="content"><p><a href="#/issues"><span class="material-icons" style="font-size:18px;vertical-align:-3px">arrow_back</span> ${t("iss.back")}</a></p><div id="issue-box"><div class="loading">${t("loading")}</div></div></div>`;
  const box = document.getElementById("issue-box");
  let i, comments;
  try { [i, comments] = await Promise.all([gh(`issues/${n}`), gh(`issues/${n}/comments?per_page=100`)]); }
  catch (e) { box.innerHTML = `<p>${esc(e.message)}</p>`; return; }
  document.title = `#${i.number} ${i.title} · Project Depot`;
  const post = (who, when, body) => `<div class="card elevation-1 comment"><div class="comment-head"><img src="${esc(who?.avatar_url)}&s=48" alt=""><b>${esc(who?.login)}</b><span class="muted">${ago(when)}</span></div><div class="doc">${md(body) || `<p class='muted'>${t("iss.nodesc")}</p>`}</div></div>`;
  box.innerHTML = `<h1 class="issue-head">${esc(i.title)} <span class="muted">#${i.number}</span></h1>
    <p><span class="label" style="--lc:${i.state === "open" ? "#2da44e" : "#8250df"}">${i.state === "open" ? t("iss.stopen") : t("iss.stclosed")}</span> ${labelChips(i.labels)}</p>
    ${post(i.user, i.created_at, i.body)}${comments.map(c => post(c.user, c.created_at, c.body)).join("")}
    <p><a class="btn btn-outlined" href="${esc(i.html_url)}"><span class="material-icons">reply</span>${t("iss.comment")}</a></p>`;
  window.scrollTo(0, 0);
}

function route() {
  const hash = location.hash.replace(/^#/, "") || "/";
  const [path, anchor] = hash.split("#");
  document.querySelectorAll(".drawer a[data-route]").forEach(a => a.classList.toggle("active", a.dataset.route === path || (a.dataset.route === "/issues" && path.startsWith("/issues/"))));
  toggleDrawer(false);
  document.title = "Project Depot";
  if (path.startsWith("/docs/")) return doc(path.slice(6), anchor);
  if (path === "/download") return download();
  if (path === "/releases") return releases();
  if (path.startsWith("/issues")) return issues(path.split("/")[2]);
  window.scrollTo(0, 0);
  return home();
}
window.addEventListener("hashchange", route);

// Theme: the app bar button goes system -> light -> dark -> system; the choice is kept in
// localStorage (applied before the first paint by the script in <head>)
(() => {
  const btn = document.getElementById("theme-btn");
  if (!btn) return;
  const root = document.documentElement;
  const names = { "": "Theme: system", light: "Theme: light", dark: "Theme: dark" };
  const icons = { "": "brightness_auto", light: "light_mode", dark: "dark_mode" };
  const show = () => {
    const t = root.dataset.theme || "";
    btn.querySelector(".material-icons").textContent = icons[t];
    btn.title = names[t];
    btn.setAttribute("aria-label", names[t]);
  };
  btn.onclick = () => {
    const next = { "": "light", light: "dark", dark: "" }[root.dataset.theme || ""];
    if (next) root.dataset.theme = next; else delete root.dataset.theme;
    try { next ? localStorage.setItem("theme", next) : localStorage.removeItem("theme"); } catch {}
    show();
  };
  show();
})();
// Language: EN / PT-BR buttons in the app bar (the choice is kept in localStorage)
(() => {
  const bar = document.getElementById("lang-btns");
  if (!bar) return;
  bar.onclick = e => {
    const b = e.target.closest("button[data-lang]");
    if (!b) return;
    setLang(b.dataset.lang);
    applyLang();
    drawDocLinks();
    route();
  };
})();
applyLang();
route();
