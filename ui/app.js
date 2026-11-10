// Opal (Tauri edition) — one script renders whichever widget or panel the
// window was opened for (?w=<id> or ?panel=widgets|settings&w=<id>).
"use strict";

const T = window.__TAURI__;
const invoke = T.core.invoke;
const listen = T.event.listen;
const win = T.window.getCurrentWindow();
const params = new URLSearchParams(location.search);
const PANEL = params.get("panel");
const WID = params.get("w") || (PANEL ? "settings" : "launcher");

const $ = (s, el = document) => el.querySelector(s);
const plate = $("#plate"), body = $("#body"), ctl = $("#ctl");
const esc = s => String(s ?? "").replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const pad = n => String(n).padStart(2, "0");
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

let S = null;          // settings
let THEMES = [], WIDGETS = [], DESIGNS = [];
let teardown = [];     // timers of the current render

const ICON = {
  gear: '<svg class="i" viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  close: '<svg class="i" viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>',
  search: '<svg class="i" viewBox="0 0 24 24"><circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.2 4.2"/></svg>',
  plus: '<svg class="i" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg>',
  prev: '<svg class="i" viewBox="0 0 24 24"><path d="M7 6v12M18 6l-8 6 8 6z" fill="currentColor"/></svg>',
  next: '<svg class="i" viewBox="0 0 24 24"><path d="M17 6v12M6 6l8 6-8 6z" fill="currentColor"/></svg>',
  play: '<svg class="i" viewBox="0 0 24 24"><path d="M8 5l11 7-11 7z" fill="currentColor"/></svg>',
  pause: '<svg class="i" viewBox="0 0 24 24"><path d="M8 5v14M16 5v14" stroke-width="3"/></svg>',
  left: '<svg class="i" viewBox="0 0 24 24"><path d="M15 6l-6 6 6 6"/></svg>',
  right: '<svg class="i" viewBox="0 0 24 24"><path d="M9 6l6 6-6 6"/></svg>',
  pin: '<svg class="i" viewBox="0 0 24 24"><path d="M12 17v5M5 17h14l-2-5V4H7v8z"/></svg>',
  reset: '<svg class="i" viewBox="0 0 24 24"><path d="M4 12a8 8 0 1 0 2.3-5.6L4 8.7M4 4v4.7h4.7"/></svg>',
  skip: '<svg class="i" viewBox="0 0 24 24"><path d="M6 5l9 7-9 7zM18 5v14"/></svg>',
  grid: '<svg class="i" viewBox="0 0 24 24"><rect x="4" y="4" width="6.5" height="6.5" rx="1.8"/><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.8"/><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.8"/><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.8"/></svg>',
};
const WIDGET_GLYPH = { launcher: "▦", clock: "◷", notes: "✎", calendar: "▤", music: "♫", weather: "☀", system: "▥", pomodoro: "◔", photos: "▣", lunar: "☾", todo: "☑", countdown: "⏳" };

// ------------------------------------------------------------- settings --

const themeOf = w => (S.widgetTheme?.[w]) || S.theme;
const designOf = w => w === "clock" ? (S.clock?.theme || "lumen") : (S.widgetDesign?.[w] || (designsFor(w)[0]?.id ?? "lumen"));
const designsFor = w => (WIDGETS.find(x => x.id === w)?.designs) || DESIGNS;
const appearance = w => S.widgetAppearance?.[w] || {};
const frostedOf = w => (S.widgetFrosted && w in S.widgetFrosted) ? S.widgetFrosted[w] : (w === "clock" ? S.clock.frostedGlass : true);
const veilOf = w => clamp(appearance(w).veil ?? S.veil, 0, 100);
const frostOf = w => clamp(appearance(w).frostStrength ?? S.frostStrength, 0, 100);
const blurOf = w => clamp(appearance(w).blurSize ?? S.blurSize, 0, 80);
const wallFrostOf = w => appearance(w).wallpaperFrost ?? S.wallpaperFrost;
const opacityOf = w => Math.max(10, S.widgetOpacity?.[w] ?? S.backgroundOpacity);

let saveTimer = 0;
function saveSettings(now = false) {
  clearTimeout(saveTimer);
  const go = () => invoke("save_settings", { settings: S }).catch(e => toast(String(e)));
  if (now) go(); else saveTimer = setTimeout(go, 160);
}

async function updateExtras(mutate) {
  const ex = await invoke("get_extras");
  mutate(ex);
  await invoke("save_extras", { extras: ex });
  return ex;
}

// ---------------------------------------------------------------- glass --

let WALL = null;
async function loadWallpaper() {
  try { WALL = await invoke("wallpaper"); } catch { WALL = null; }
  placeWall();
}

async function placeWall() {
  const wall = $("#wall");
  const w = PANEL ? "settings" : WID;
  const frostOn = frostedOf(w) && wallFrostOf(w);
  if (!WALL || !WALL.url || !frostOn || veilOf(w) >= 100) { wall.style.display = "none"; return; }
  const [pos, sf] = await Promise.all([win.outerPosition(), win.scaleFactor()]);
  const r = plate.getBoundingClientRect();
  // "Fill" on the primary monitor, in physical pixels.
  const scale = Math.max(WALL.screenW / WALL.width, WALL.screenH / WALL.height);
  const fw = WALL.width * scale, fh = WALL.height * scale;
  const fx = (WALL.screenW - fw) / 2, fy = (WALL.screenH - fh) / 2;
  const ox = pos.x / sf + r.left - 160, oy = pos.y / sf + r.top - 160;
  const light = isLight(w);
  const blur = blurOf(w) * (S.performanceMode ? .62 : 1) + (themeOf(w) === "porcelain" ? 3 : 0);
  Object.assign(wall.style, {
    display: "", backgroundImage: `url("${WALL.url}")`,
    backgroundSize: `${fw / sf}px ${fh / sf}px`,
    backgroundPosition: `${fx / sf - ox}px ${fy / sf - oy}px`,
    filter: `blur(${blur}px) saturate(1.45) brightness(${light ? 1.02 : .84})`,
    opacity: .22 + frostOf(w) / 100 * .78,
  });
}

function surfaceFor(w) {
  const d = designsFor(w).find(x => x.id === designOf(w));
  return d && d.surface ? d : null;
}
function isLight(w) {
  const sd = PANEL ? null : surfaceFor(w);
  if (sd) return sd.light;
  return !!THEMES.find(t => t.id === themeOf(w))?.light;
}

function applyGlass() {
  const w = PANEL ? "settings" : WID;
  const theme = THEMES.find(t => t.id === themeOf(w)) || THEMES[0];
  const sd = PANEL ? null : surfaceFor(w);
  const frostOn = frostedOf(w) && wallFrostOf(w);
  const tint = frostOn ? veilOf(w) / 100 : 1;
  const th = $("#theme");
  th.style.background = sd ? sd.surface : theme.css;
  th.style.opacity = sd ? Math.max(tint, .9) : tint;
  plate.classList.toggle("light", isLight(w));
  plate.classList.toggle("flat", !!sd);
  plate.style.setProperty("--surface", PANEL ? 1 : opacityOf(w) / 100);
  plate.style.setProperty("--grain", frostOn ? .04 + frostOf(w) / 100 * .12 : .05);
  plate.dataset.theme = theme.id;
  // Opal's moving rim and sheen rest in performance mode.
  plate.classList.toggle("still", !!S.performanceMode);
  const rim = $("#rim");
  if (S.performanceMode) rim.pauseAnimations(); else rim.unpauseAnimations();
  placeWall();
}

// ------------------------------------------------------------- chrome --

function toast(msg, ms = 2200) {
  const t = $("#toast");
  t.textContent = msg; t.hidden = false;
  clearTimeout(toast.t); toast.t = setTimeout(() => (t.hidden = true), ms);
}

function showMenu(x, y, items) {
  const m = $("#menu");
  m.innerHTML = "";
  for (const it of items) {
    if (it === "-") { m.appendChild(document.createElement("hr")); continue; }
    const b = document.createElement("button");
    b.textContent = it.label;
    b.onclick = () => { hideMenu(); it.run(); };
    m.appendChild(b);
  }
  m.hidden = false;
  const r = m.getBoundingClientRect();
  m.style.left = clamp(x, 4, innerWidth - r.width - 4) + "px";
  m.style.top = clamp(y, 4, innerHeight - r.height - 4) + "px";
}
function hideMenu() { $("#menu").hidden = true; }
addEventListener("mousedown", e => { if (!e.target.closest("#menu")) hideMenu(); }, true);

// Anything clickable must be listed here, or mousedown starts a window drag
// and the click is lost (theme chips were).
const INTERACTIVE = "button,input,textarea,select,a,label,[contenteditable],[data-t],[data-d],.no-drag,.menu,.seek,.cal-grid .d,.tile,.result,.tick,.theme-chip,.sw";
plate.addEventListener("mousedown", e => {
  if (e.button !== 0 || e.target.closest(INTERACTIVE) || e.target.id === "grip") return;
  win.startDragging();
});
$("#grip").addEventListener("mousedown", e => { e.preventDefault(); win.startResizeDragging("SouthEast"); });
addEventListener("contextmenu", e => { if (!e.target.closest("input,textarea")) e.preventDefault(); });

function buildControls() {
  ctl.innerHTML = "";
  const add = (icon, title, run) => {
    const b = document.createElement("button");
    b.innerHTML = icon; b.title = title; b.onclick = run; ctl.appendChild(b);
  };
  if (PANEL) {
    add(ICON.close, "Close", () => invoke("close_self"));
  } else {
    add(ICON.gear, "Widget settings", () => invoke("open_widget_settings", { id: WID }));
    add(ICON.close, "Hide widget", () => invoke("set_widget_open", { id: WID, open: false }));
  }
}

function every(ms, fn) { fn(); const t = setInterval(fn, ms); teardown.push(() => clearInterval(t)); }

function render() {
  teardown.forEach(f => f()); teardown = [];
  body.innerHTML = "";
  body.className = "plate-body";
  applyGlass();
  if (PANEL === "widgets") return renderWidgetsPanel();
  if (PANEL === "settings") return renderSettingsPanel(params.get("w") || "launcher");
  (RENDER[WID] || renderLauncher)();
}

// ------------------------------------------------------------- launcher --

function accentFor(seed) {
  const A = [0x7c9cff, 0xc084fc, 0xf472b6, 0xfb923c, 0x34d399, 0x22d3ee, 0xfacc15, 0xf87171, 0xa3e635, 0x818cf8];
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) { h ^= seed.charCodeAt(i); h = Math.imul(h, 16777619); }
  return "#" + A[Math.abs(h | 0) % 10].toString(16).padStart(6, "0");
}

const iconCache = new Map();
function iconInto(el, path, name, size) {
  const key = path + "|" + (name || "");
  const letter = () => {
    el.outerHTML = `<div class="letter" style="background:linear-gradient(150deg, ${accentFor(name || path)}, #00000055)">${esc((name || "?").trim()[0]?.toUpperCase() || "?")}</div>`;
  };
  const set = url => { if (url) { el.src = url; } else letter(); };
  if (iconCache.has(key)) return set(iconCache.get(key));
  invoke("icon", { path, name }).then(u => { iconCache.set(key, u); set(u); }).catch(letter);
}

// Quick shortcuts in the dock's search pill: widgets (show / hide) and a few
// Windows tools. Which ones show is chosen in App Dock settings.
const svg = d => `<svg class="i" viewBox="0 0 24 24">${d}</svg>`;
const QUICK = [
  { id: "notes", name: "Sticky notes", widget: true, icon: svg('<path d="M5 4h14v11l-5 5H5z"/><path d="M14 20v-5h5"/>') },
  { id: "calendar", name: "Calendar", widget: true, icon: svg('<rect x="4" y="5" width="16" height="15" rx="2.5"/><path d="M4 10h16M9 3v4M15 3v4"/>') },
  { id: "music", name: "Music", widget: true, icon: svg('<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="16.5" cy="16" r="2.5"/>') },
  { id: "todo", name: "To-do", widget: true, icon: svg('<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8 12l3 3 5-6"/>') },
  { id: "countdown", name: "Đếm ngược", widget: true, icon: svg('<path d="M7 3h10M7 21h10M8 3c0 5 8 5 8 9s-8 4-8 9M16 3c0 5-8 5-8 9"/>') },
  { id: "clock", name: "Clock", widget: true, icon: svg('<circle cx="12" cy="12" r="8"/><path d="M12 7v5l3 2"/>') },
  { id: "weather", name: "Weather", widget: true, icon: svg('<circle cx="9" cy="9" r="3.5"/><path d="M9 2.5v1.5M2.5 9H4M4.4 4.4l1 1M13.6 4.4l-1 1"/><path d="M8 19h9a3.5 3.5 0 0 0 0-7 5 5 0 0 0-8.5 2"/>') },
  { id: "pomodoro", name: "Focus timer", widget: true, icon: svg('<circle cx="12" cy="13" r="7"/><path d="M12 9v4l2.5 1.5M10 3h4"/>') },
  { id: "lunar", name: "Âm lịch", widget: true, icon: svg('<path d="M16.5 4.5a8 8 0 1 0 3 10.5A6.5 6.5 0 0 1 16.5 4.5z"/>') },
  { id: "system", name: "System", widget: true, icon: svg('<rect x="4" y="5" width="16" height="11" rx="2"/><path d="M8 20h8M12 16v4M7 12l2.5-3 2.5 2 3-4"/>') },
  { id: "photos", name: "Photos", widget: true, icon: svg('<rect x="4" y="5" width="16" height="14" rx="2.5"/><circle cx="9" cy="10" r="1.6"/><path d="M5 17l5-4.5 3.5 3 2.5-2 3 3"/>') },
  { id: "snip", name: "Chụp màn hình", target: "ms-screenclip:", icon: svg('<path d="M4 8V5.5A1.5 1.5 0 0 1 5.5 4H8M16 4h2.5A1.5 1.5 0 0 1 20 5.5V8M20 16v2.5a1.5 1.5 0 0 1-1.5 1.5H16M8 20H5.5A1.5 1.5 0 0 1 4 18.5V16"/><circle cx="12" cy="12" r="3"/>') },
  { id: "taskmgr", name: "Task Manager", target: "taskmgr.exe", icon: svg('<path d="M3 12h4l2.5-6 5 12 2.5-6h4"/>') },
  { id: "explorer", name: "File Explorer", target: "explorer.exe", icon: svg('<path d="M3 7.5A1.5 1.5 0 0 1 4.5 6H10l2 2h7.5A1.5 1.5 0 0 1 21 9.5v8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z"/>') },
  { id: "winsettings", name: "Windows Settings", target: "ms-settings:", icon: svg('<path d="M4 7h10M18 7h2M4 17h4M12 17h8"/><circle cx="16" cy="7" r="2"/><circle cx="10" cy="17" r="2"/>') },
];
const QUICK_DEFAULT = ["notes", "calendar", "music", "todo"];
const quickIds = () => Array.isArray(S.quickActions) ? S.quickActions : QUICK_DEFAULT;

async function renderLauncher() {
  const { items } = await invoke("get_state");
  const cols = S.columns, size = S.iconSize;
  const quick = quickIds().map(id => QUICK.find(q => q.id === id)).filter(Boolean);
  body.innerHTML = `
    ${S.showSearch ? `<div class="search">${ICON.search}<input id="q" placeholder="Search apps and files" spellcheck="false">
      ${quick.length ? `<span class="sdiv"></span>${quick.map(q => `<button class="sbtn${q.widget ? " qw" : ""}" data-q="${q.id}" title="${esc(q.name)}">${q.icon}</button>`).join("")}` : ""}
      <span class="sdiv"></span><button class="sbtn" id="sw" title="Widgets">${ICON.grid}</button><button class="sbtn" id="sg" title="Settings">${ICON.gear}</button></div>` : `<div class="head"><h1>Apps</h1></div>`}
    <div class="grow scroll" id="area"><div class="grid" id="grid" style="grid-template-columns:repeat(${cols},1fr);--icon:${size}px"></div></div>`;
  const grid = $("#grid"), area = $("#area");
  if (S.showSearch) {
    plate.classList.add("inline-ctl");
    teardown.push(() => plate.classList.remove("inline-ctl"));
    $("#sw").onclick = () => invoke("open_widgets_panel");
    $("#sg").onclick = () => invoke("open_widget_settings", { id: "launcher" });
    // Widget shortcuts light up while that widget is shown.
    let open = {};
    const mark = async () => {
      open = Object.fromEntries((await invoke("widgets_list")).map(w => [w.id, w.open]));
      body.querySelectorAll(".sbtn.qw").forEach(b => b.classList.toggle("on", !!open[b.dataset.q]));
    };
    mark();
    const un = await listen("widgets-changed", mark); teardown.push(un);
    body.querySelectorAll("[data-q]").forEach(b => b.onclick = () => {
      const q = QUICK.find(x => x.id === b.dataset.q);
      if (q.widget) invoke("set_widget_open", { id: q.id, open: !open[q.id] });
      else invoke("open_program", { target: q.target, kind: "file" }).catch(e => toast(String(e)));
    });
  }
  items.forEach((it, idx) => {
    const b = document.createElement("button");
    b.className = "tile"; b.dataset.idx = idx; b.title = it.name;
    b.innerHTML = `<img alt="">${S.showLabels ? `<span>${esc(it.name)}</span>` : ""}`;
    iconInto(b.querySelector("img"), it.iconSource || it.target, it.kind === "aumid" ? it.name : null, size);
    b.onclick = () => { if (!b.dataset.dragged) invoke("launch_item", { id: it.id }).catch(e => toast(String(e))); };
    b.oncontextmenu = e => showMenu(e.clientX, e.clientY, [
      { label: "Open", run: () => invoke("launch_item", { id: it.id }) },
      { label: "Show in folder", run: () => invoke("reveal_item", { id: it.id }) },
      { label: "Rename…", run: () => renameTile(b, it) },
      "-",
      { label: "Remove from Opal", run: () => invoke("remove_item", { id: it.id }) },
    ]);
    reorderable(b, idx, grid);
    grid.appendChild(b);
  });
  const add = document.createElement("button");
  add.className = "tile add"; add.title = "Add anything";
  add.innerHTML = `<div class="letter">+</div>${S.showLabels ? "<span>Add</span>" : ""}`;
  add.onclick = e => { const r = add.getBoundingClientRect(); addMenu(r.left, r.bottom); };
  grid.appendChild(add);

  const q = $("#q");
  if (q) {
    let sel = 0, results = [];
    const run = async () => {
      const text = q.value.trim();
      if (!text) { grid.hidden = false; $("#results")?.remove(); return; }
      results = await invoke("search_programs", { query: text });
      grid.hidden = true; sel = 0;
      let box = $("#results");
      if (!box) { box = document.createElement("div"); box.id = "results"; box.className = "results"; area.appendChild(box); }
      const pinned = new Set(items.map(i => i.target.toLowerCase()));
      box.innerHTML = results.length ? "" : `<div class="empty">No matches</div>`;
      results.forEach((p, i) => {
        const r = document.createElement("div");
        r.className = "result" + (i === 0 ? " sel" : "");
        r.innerHTML = `<img alt=""><div class="name">${esc(p.name)}</div>${pinned.has(p.target.toLowerCase()) ? "" : `<button class="icon-btn" title="Pin to Opal">${ICON.pin}</button>`}`;
        iconInto(r.querySelector("img"), p.icon_path, p.kind === "aumid" ? p.name : null, 26);
        r.onclick = e => {
          if (e.target.closest("button")) { invoke("pin_program", { name: p.name, target: p.target, kind: p.kind, iconPath: p.icon_path }); return; }
          invoke("open_program", { target: p.target, kind: p.kind }).catch(err => toast(String(err)));
          q.value = ""; run();
        };
        box.appendChild(r);
      });
    };
    let t = 0;
    q.oninput = () => { clearTimeout(t); t = setTimeout(run, 90); };
    q.onkeydown = e => {
      const rows = [...document.querySelectorAll(".result")];
      if (e.key === "Escape") { q.value = ""; run(); }
      if (!rows.length) return;
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        sel = clamp(sel + (e.key === "ArrowDown" ? 1 : -1), 0, rows.length - 1);
        rows.forEach((r, i) => r.classList.toggle("sel", i === sel));
        rows[sel].scrollIntoView({ block: "nearest" });
      }
      if (e.key === "Enter") rows[sel]?.click();
    };
  }
  const onState = await listen("state-changed", () => { if (!q || !q.value) render(); });
  teardown.push(onState);
}

function addMenu(x, y) {
  showMenu(x, y, [
    { label: "Add files or apps…", run: () => invoke("pick_and_add") },
    { label: "Add a folder…", run: async () => { const f = await invoke("pick_folder"); if (f) invoke("add_paths", { paths: [f] }); } },
    { label: "This PC", run: () => invoke("add_place", { place: "thispc" }) },
    { label: "Recycle Bin", run: () => invoke("add_place", { place: "recycle" }) },
    "-",
    { label: "Widgets…", run: () => invoke("open_widgets_panel") },
  ]);
}

function renameTile(tile, it) {
  const span = tile.querySelector("span") || tile;
  const input = document.createElement("input");
  input.value = it.name;
  input.style.cssText = "width:100%;font-size:11.5px;text-align:center;border-radius:6px;border:1px solid var(--hair-strong);background:var(--search-bg);outline:0";
  span.replaceWith(input); input.focus(); input.select();
  const done = ok => { if (ok && input.value.trim()) invoke("rename_item", { id: it.id, name: input.value }); else render(); };
  input.onkeydown = e => { if (e.key === "Enter") done(true); if (e.key === "Escape") done(false); };
  input.onblur = () => done(true);
}

// Pointer-based reordering (WebView2 swallows HTML5 drag while file drop is on).
function reorderable(tile, idx, grid) {
  tile.addEventListener("pointerdown", e => {
    if (e.button !== 0) return;
    const sx = e.clientX, sy = e.clientY;
    let dragging = false, target = null;
    const move = ev => {
      if (!dragging && Math.hypot(ev.clientX - sx, ev.clientY - sy) > 6) { dragging = true; tile.classList.add("dragging"); }
      if (!dragging) return;
      const over = document.elementFromPoint(ev.clientX, ev.clientY)?.closest(".tile:not(.add)");
      grid.querySelectorAll(".drop-before").forEach(t => t.classList.remove("drop-before"));
      if (over && over !== tile) { over.classList.add("drop-before"); target = +over.dataset.idx; } else target = null;
    };
    const up = () => {
      removeEventListener("pointermove", move); removeEventListener("pointerup", up);
      if (dragging) {
        tile.dataset.dragged = "1"; setTimeout(() => delete tile.dataset.dragged, 50);
        tile.classList.remove("dragging");
        if (target != null) invoke("move_item", { from: idx, to: target });
      }
    };
    addEventListener("pointermove", move); addEventListener("pointerup", up);
  });
}

// ---------------------------------------------------------------- clock --

function zoned(now = new Date()) {
  const tz = S.clock.timeZone;
  if (!tz || tz === "local") return { h: now.getHours(), m: now.getMinutes(), s: now.getSeconds(), d: now };
  try {
    const parts = Object.fromEntries(new Intl.DateTimeFormat("en-GB", { timeZone: tz, hour: "2-digit", minute: "2-digit", second: "2-digit", year: "numeric", month: "2-digit", day: "2-digit", hourCycle: "h23" }).formatToParts(now).map(p => [p.type, p.value]));
    const d = new Date(+parts.year, +parts.month - 1, +parts.day, +parts.hour, +parts.minute, +parts.second);
    return { h: +parts.hour, m: +parts.minute, s: +parts.second, d };
  } catch { return { h: now.getHours(), m: now.getMinutes(), s: now.getSeconds(), d: now }; }
}
function hm(z) {
  const h12 = S.clock.hour12;
  const h = h12 ? ((z.h + 11) % 12) + 1 : z.h;
  return { hh: h12 ? String(h) : pad(h), mm: pad(z.m), ss: pad(z.s), ampm: h12 ? (z.h < 12 ? "AM" : "PM") : "" };
}
const dateLine = d => d.toLocaleDateString("vi-VN", { weekday: "long", day: "numeric", month: "long" });
const timeHTML = (t, secs = S.clock.showSeconds) => `${t.hh}:${t.mm}${secs ? `<span class="sec">${t.ss}</span>` : ""}${t.ampm ? `<span class="ampm">${t.ampm}</span>` : ""}`;

function dialSVG(z) {
  const a = (v, max) => (v / max) * Math.PI * 2 - Math.PI / 2;
  const hand = (cls, ang, len, w) => `<line class="${cls}" x1="50" y1="50" x2="${50 + Math.cos(ang) * len}" y2="${50 + Math.sin(ang) * len}" stroke-width="${w}"/>`;
  let ticks = "";
  for (let i = 0; i < 12; i++) { const g = a(i, 12); ticks += `<line class="tick" x1="${50 + Math.cos(g) * 40}" y1="${50 + Math.sin(g) * 40}" x2="${50 + Math.cos(g) * 44}" y2="${50 + Math.sin(g) * 44}"/>`; }
  return `<svg class="dial" viewBox="0 0 100 100"><circle class="face" cx="50" cy="50" r="47"/>${ticks}
    ${hand("h", a((z.h % 12) + z.m / 60, 12), 24, 3.5)}${hand("m", a(z.m + z.s / 60, 60), 34, 2.4)}${S.clock.showSeconds ? hand("s", a(z.s, 60), 38, 1.2) : ""}<circle cx="50" cy="50" r="2.5" fill="currentColor"/></svg>`;
}

function renderClock() {
  const design = designOf("clock");
  const el = document.createElement("div");
  body.appendChild(el);
  const greet = h => h < 5 ? "Good night" : h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
  const draw = () => {
    const z = zoned(), t = hm(z);
    switch (design) {
      case "sunset":
        el.className = "clock ck-center ck-sunset";
        el.innerHTML = `<div><div class="greet">${greet(z.h)}</div><div class="time">${timeHTML(t)}</div><div class="date">${dateLine(z.d)}</div></div>`; break;
      case "paper":
        el.className = "clock ck-center ck-paper";
        el.innerHTML = `<div><div class="time">${timeHTML(t)}</div><div class="date">${dateLine(z.d)}</div></div>`; break;
      case "neon": {
        el.className = "clock ck-center ck-neon";
        const frac = (z.h * 3600 + z.m * 60 + z.s) / 86400, C = 2 * Math.PI * 44;
        const ang = frac * Math.PI * 2 - Math.PI / 2;
        let labels = "";
        for (const h of [0, 6, 12, 18]) { const g = h / 24 * Math.PI * 2 - Math.PI / 2; labels += `<text x="${50 + Math.cos(g) * 36}" y="${50 + Math.sin(g) * 36}">${pad(h)}</text>`; }
        el.innerHTML = `<svg class="orbit" viewBox="0 0 100 100"><circle class="track" cx="50" cy="50" r="44"/><circle class="arc" cx="50" cy="50" r="44" stroke-dasharray="${C * frac} ${C}" transform="rotate(-90 50 50)"/><circle class="dot" r="2.6" cx="${50 + Math.cos(ang) * 44}" cy="${50 + Math.sin(ang) * 44}"/>${labels}</svg>
          <div style="position:relative"><div class="time">${timeHTML(t, false)}</div><div class="date">${dateLine(z.d)}</div></div>`; break;
      }
      case "modular": {
        el.className = "clock";
        const digits = (t.hh.padStart(2, "0") + t.mm).split("");
        const colors = ["#ff6b4a", "#ffb547", "#3fb6a8", "#5a6cff"];
        el.innerHTML = `<div class="blocks4">${digits.map((d, i) => `<div style="background:${colors[i]}">${d}</div>`).join("")}</div>`; break;
      }
      case "ocean":
        el.className = "clock ck-center";
        el.innerHTML = `<div class="flap"><div class="card">${t.hh.padStart(2, "0")}</div><div class="card">${t.mm}</div></div><div class="date">${dateLine(z.d)}${t.ampm ? " · " + t.ampm : ""}</div>`; break;
      case "rose": {
        el.className = "clock";
        const day = (z.h * 60 + z.m) / 1440, hour = (z.m * 60 + z.s) / 3600, min = z.s / 60;
        const row = (n, v) => `<div class="bar-row"><span>${n}</span><div class="bar"><i style="width:${v * 100}%"></i></div><span>${Math.round(v * 100)}%</span></div>`;
        el.innerHTML = `<div class="daybook"><div><div class="time">${timeHTML(t)}</div><div class="date">${dateLine(z.d)}</div></div>${row("Day", day)}${row("Hour", hour)}${row("Minute", min)}</div>`; break;
      }
      case "aurora": {
        el.className = "clock";
        const rows = 5, mid = z.h + z.m / 60;
        let hrs = "";
        for (let i = -2; i <= 2; i++) { const h = (z.h + i + 24) % 24; hrs += `<div class="hr" style="top:${((i + 2) / rows) * 100 - (z.m / 60) * (100 / rows) + 50 / rows * 0}%">${pad(h)}:00</div>`; }
        el.innerHTML = `<div class="timeline"><div class="col" style="justify-content:center"><div class="time">${timeHTML(t)}</div><div class="date">${dateLine(z.d)}</div></div><div class="ruler">${hrs}<div class="now" style="top:${(2 / rows) * 100}%"></div></div></div>`;
        void mid; break;
      }
      case "terminal":
        el.className = "clock ck-terminal";
        el.innerHTML = `<div class="date">&gt; time --now</div><div class="time">${t.hh}:${t.mm}${S.clock.showSeconds ? ":" + t.ss : ""}${t.ampm ? " " + t.ampm : ""}</div><div class="date">${z.d.toISOString().slice(0, 10)} · ${S.clock.timeZone === "local" ? Intl.DateTimeFormat().resolvedOptions().timeZone : S.clock.timeZone}</div>`; break;
      default:
        el.className = "clock ck-lumen";
        el.innerHTML = `<div><div class="time">${timeHTML(t)}</div><div class="date">${dateLine(z.d)}</div></div>${dialSVG(z)}`;
    }
  };
  every(1000, draw);
}

// ---------------------------------------------------------------- notes --

const NOTE_COLORS = { amber: "#fff0bf", rose: "#ffd6dd", sky: "#d4ebff", mint: "#d3f5e3", violet: "#e6dcff", peach: "#ffe2cc", slate: "#dde3ec" };
async function renderNotes() {
  let notes = await invoke("get_notes");
  const design = designOf("notes");
  body.innerHTML = `<div class="head"><h1>Sticky notes</h1><button class="btn add-note" id="addn">${ICON.plus} New</button></div><div class="notes nt-${design}"><div class="grow scroll"><div class="board" id="board"></div></div></div>`;
  let t = 0;
  const persist = () => { clearTimeout(t); t = setTimeout(() => invoke("save_notes", { notes }), 400); };
  const draw = () => {
    const board = $("#board"); board.innerHTML = "";
    const sorted = [...notes].sort((a, b) => (b.pinned - a.pinned) || (b.updatedAt - a.updatedAt));
    if (!sorted.length) board.innerHTML = `<div class="empty" style="grid-column:1/-1;padding:30px 0">No notes yet</div>`;
    for (const n of sorted) {
      const el = document.createElement("div");
      el.className = "note" + (n.pinned ? " pinned" : "");
      el.style.setProperty("--tint", NOTE_COLORS[n.color] || NOTE_COLORS.amber);
      el.innerHTML = `<textarea spellcheck="false" placeholder="Write something…">${esc(n.text)}</textarea>
        <div class="meta">${Object.entries(NOTE_COLORS).map(([k, c]) => `<span class="sw no-drag" data-c="${k}" style="background:${c}"></span>`).join("")}
        <span class="grow"></span><button data-a="pin">${n.pinned ? "Unpin" : "Pin"}</button><button data-a="del">Delete</button></div>`;
      const ta = el.querySelector("textarea");
      ta.oninput = () => { n.text = ta.value; n.updatedAt = Date.now(); persist(); };
      el.querySelectorAll(".sw").forEach(s => s.onclick = () => { n.color = s.dataset.c; persist(); draw(); });
      el.querySelector('[data-a=pin]').onclick = () => { n.pinned = !n.pinned; persist(); draw(); };
      el.querySelector('[data-a=del]').onclick = () => { notes = notes.filter(x => x !== n); persist(); draw(); };
      board.appendChild(el);
    }
  };
  $("#addn").onclick = () => {
    const now = Date.now();
    notes.push({ id: Math.random().toString(36).slice(2) + now.toString(36), text: "", color: "amber", pinned: false, createdAt: now, updatedAt: now });
    persist(); draw();
    setTimeout(() => $("#board textarea")?.focus(), 0);
  };
  draw();
}

// ------------------------------------------------------------- calendar --

let calView = null, calSel = null;
async function renderCalendar() {
  const design = designOf("calendar");
  const today = new Date(); today.setHours(0, 0, 0, 0);
  calView ||= new Date(today.getFullYear(), today.getMonth(), 1);
  calSel ||= today;
  let data = { config: { weekStartsOn: 1 }, events: [], status: "" };
  const iso = d => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const eventsOn = d => { const k = iso(d); return data.events.filter(e => e.date <= k && k < (e.end > e.date ? e.end : e.date + "~")); };
  const draw = () => {
    const ws = data.config.weekStartsOn === 0 ? 0 : 1;
    const first = new Date(calView);
    const lead = (first.getDay() - ws + 7) % 7;
    const start = new Date(first); start.setDate(1 - lead);
    const names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let grid = "";
    for (let i = 0; i < 7; i++) grid += `<div class="wd">${names[(i + ws) % 7]}</div>`;
    for (let i = 0; i < 42; i++) {
      const d = new Date(start); d.setDate(start.getDate() + i);
      const cls = ["d", d.getMonth() !== calView.getMonth() && "out", +d === +today && "today", +d === +calSel && "sel", eventsOn(d).length && "ev"].filter(Boolean).join(" ");
      grid += `<div class="${cls}" data-t="${+d}">${d.getDate()}</div>`;
    }
    const sel = eventsOn(calSel);
    const evHTML = sel.length ? sel.slice(0, 6).map(e => `<div><time>${e.time ? e.time.slice(0, 5) : "All day"}</time><span>${esc(e.title)}</span></div>`).join("")
      : (data.status ? `<div class="faint">${esc(data.status)}</div>` : "");
    const split = design !== "lumen";
    // The split designs leave the month column narrow: "11/2026".
    const month = split ? `${calView.getMonth() + 1}/${calView.getFullYear()}` : calView.toLocaleDateString("vi-VN", { month: "long", year: "numeric" });
    const daysIn = new Date(today.getFullYear(), today.getMonth() + 1, 0).getDate();
    body.innerHTML = `<div class="cal cl-${design}">
      ${split ? `<div class="cal-side"><div class="wk">${calSel.toLocaleDateString("vi-VN", { weekday: "long" })}</div><div class="big">${calSel.getDate()}</div><div class="dim" style="color:inherit;opacity:.75">${calView.toLocaleDateString("vi-VN", { month: "long", year: "numeric" })}</div>
        ${design === "grove" ? `<div class="progress"><i style="width:${today.getDate() / daysIn * 100}%"></i></div><small style="opacity:.7;margin-top:4px">${Math.round(today.getDate() / daysIn * 100)}% of the month</small>` : ""}
        <div class="cal-events" style="margin-top:12px">${evHTML}</div></div>` : ""}
      <div class="cal-month"><div class="cal-nav">${split ? "" : `<b>${month}</b>`}<button id="pm">${ICON.left}</button><button id="tm" title="Today">•</button><button id="nm">${ICON.right}</button></div>
        <div class="cal-grid">${grid}</div>${split ? "" : `<div class="cal-events scroll">${evHTML}</div>`}</div></div>`;
    $("#pm").onclick = () => { calView = new Date(calView.getFullYear(), calView.getMonth() - 1, 1); draw(); };
    $("#nm").onclick = () => { calView = new Date(calView.getFullYear(), calView.getMonth() + 1, 1); draw(); };
    $("#tm").onclick = () => { calView = new Date(today.getFullYear(), today.getMonth(), 1); calSel = today; draw(); };
    body.querySelectorAll(".cal-grid .d").forEach(c => c.onclick = () => { calSel = new Date(+c.dataset.t); draw(); });
  };
  const load = async () => { data = await invoke("calendar_get"); draw(); };
  every(20000, load);
  const un = await listen("calendar-changed", load); teardown.push(un);
}

// ---------------------------------------------------------------- music --

const ICON_EXT = svg('<path d="M14 4h6v6M20 4l-8 8M18 14v4.5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 4 18.5v-11A1.5 1.5 0 0 1 5.5 6H10"/>');
const ICON_LIST = svg('<path d="M4 6h11M4 11h11M4 16h7"/><path d="M17 13v6.5"/><circle cx="15.5" cy="19.5" r="1.8"/><path d="M17 13l3 1"/>');
const ICON_NOTE = svg('<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="16.5" cy="16" r="2.5"/>');
const ICON_RELOAD = svg('<path d="M20 12a8 8 0 1 1-2.3-5.6L20 8.7M20 4v4.7h-4.7"/>');

// The music widget. Two sources: Opal's own YouTube Music window (track,
// queue, full control) and, otherwise, whatever Windows reports as playing.
async function renderMusic() {
  const design = designOf("music");
  let snap = null, ytm = null, fetched = 0, listOpen = false;
  const fmt = s => s > 0 ? `${Math.floor(s / 60)}:${pad(Math.floor(s % 60))}` : "0:00";
  const fromYtm = () => !!(ytm && ytm.title);
  const cmd = (action, value) => (fromYtm()
    ? invoke("ytm_control", { action: action === "seek" ? "seek" : action, arg: value })
    : invoke("media_command", { action, value })).catch(e => toast(String(e))).then(() => setTimeout(poll, 350));
  const openPlayer = () => invoke("ytm_open").catch(e => toast(String(e)));
  const resizeTo = async (w, h) => {
    const LS = (T.window && T.window.LogicalSize) || (T.dpi && T.dpi.LogicalSize);
    if (LS) await win.setSize(new LS(Math.round(w + 12), Math.round(h + 12)));
  };

  // Coverflow: the original's YouTube Music widget — queue covers in a fan,
  // a floating playback deck and the playlist underneath.
  let grown = false;
  const drawFlow = () => {
    // The fan needs room; a widget sized for the compact player grows once.
    if (!grown && innerHeight < 330) { grown = true; resizeTo(Math.max(innerWidth - 12, 560), 380); }
    const q = fromYtm() ? (ytm.queue || []) : [];
    let cur = q.findIndex(x => x.current);
    const now = fromYtm()
      ? { title: ytm.title, artist: ytm.artist, cover: ytm.cover, playing: ytm.playing }
      : snap?.now ? { title: snap.now.title, artist: snap.now.artist || snap.now.album, cover: snap.cover, playing: snap.now.playing } : null;
    if (cur < 0 && now && q.length) cur = q.findIndex(x => x.title === now.title);
    const card = (k, item, idx) => {
      const art = item?.thumb || item?.cover;
      return `<div class="mf-card ${k === 0 ? "mid" : ""}" style="--k:${k};--a:${Math.abs(k)}" ${idx != null ? `data-qi="${idx}"` : ""}>
        <div class="art" style="${art ? `background-image:url('${esc(art)}')` : `--tint:${["#3f6b52", "#8a3f55", "#5b4bd6", "#5d5d6e", "#2f7f86"][k + 2]}`}">${art ? "" : `${ICON_NOTE}<span>OPAL / MUSIC</span>`}</div>
        <div class="cap"><b>${esc(item?.title || "Your music, in focus")}</b><small>${esc(item?.artist || "YouTube Music")}</small></div></div>`;
    };
    let cards = "";
    for (let k = -2; k <= 2; k++) {
      if (now && q.length && cur >= 0) { const i = cur + k; if (i >= 0 && i < q.length) cards += card(k, k === 0 ? { ...q[i], thumb: now.cover || q[i].thumb } : q[i], k === 0 ? null : i); }
      else if (now) { if (k === 0) cards += card(0, { title: now.title, artist: now.artist, cover: now.cover }); }
      else cards += card(k, null, null);
    }
    const pill = now
      ? `<div class="mf-pill"><div class="th" style="${now.cover ? `background-image:url('${esc(now.cover)}')` : ""}"></div><div class="nm"><b>${esc(now.title)}</b><small>${esc(now.artist || "")}</small></div>
          <span class="wave ${now.playing ? "on" : ""}"><i></i><i></i><i></i><i></i><i></i></span><div class="prog" id="seek"><i id="bar"></i></div></div>`
      : `<button class="mf-pill open" id="op"><div class="th">${ICON_NOTE}</div><div class="nm"><b>Open your player</b><small>Sign in once. Stay in the music.</small></div><span class="wave"><i></i><i></i><i></i><i></i><i></i></span></button>`;
    const list = listOpen ? `<div class="mf-list"><div class="mf-lh"><div><b>PLAYLIST</b><small>${q.length} tracks</small></div><span class="grow"></span>
        <button class="icon-btn" id="lr" title="Refresh">${ICON_RELOAD}</button><button class="icon-btn" id="lc" title="Close">${ICON.close}</button></div>
        <div class="mf-rows scroll">${q.length ? q.map((x, i) => `<div class="mf-row ${i === cur ? "cur" : ""}" data-qi="${i}"><div class="th" style="${x.thumb ? `background-image:url('${esc(x.thumb)}')` : ""}"></div>
          <div class="nm"><b>${esc(x.title)}</b><small>${esc([x.artist, x.duration].filter(Boolean).join(" · "))}</small></div>${i === cur ? `<span class="pl">${ICON.play}</span>` : ""}</div>`).join("")
        : `<div class="empty" style="padding:20px 0">${fromYtm() ? "Danh sách trống" : "Mở YouTube Music của Opal để xem danh sách đang nghe"}</div>`}</div></div>` : "";
    body.innerHTML = `<div class="mf ${listOpen ? "with-list" : ""}"><div class="mf-head">${ICON_NOTE}<span>${fromYtm() || !now ? "YouTube Music" : esc(snap.now.app || "Music")}</span></div>
      <div class="mf-stage">${cards}</div>
      <div class="mf-deck"><button class="icon-btn" data-c="prev">${ICON.prev}</button><button class="icon-btn" data-c="play">${now?.playing ? ICON.pause : ICON.play}</button><button class="icon-btn" data-c="next">${ICON.next}</button>
        ${pill}<button class="icon-btn" id="ox" title="Open YouTube Music">${ICON_EXT}</button><button class="icon-btn ${listOpen ? "on" : ""}" id="ql" title="Playlist">${ICON_LIST}</button></div>${list}</div>`;
    body.querySelectorAll("[data-c]").forEach(b => b.onclick = () => now ? cmd(b.dataset.c) : openPlayer());
    body.querySelectorAll("[data-qi]").forEach(b => b.onclick = () => fromYtm() ? cmd("queue", +b.dataset.qi) : openPlayer());
    $("#ox").onclick = openPlayer;
    if ($("#op")) $("#op").onclick = openPlayer;
    $("#ql").onclick = async () => {
      listOpen = !listOpen;
      // Grow / shrink the widget to fit the playlist.
      await resizeTo(innerWidth - 12, innerHeight - 12 + (listOpen ? 280 : -280));
      draw();
    };
    if ($("#lc")) $("#lc").onclick = () => $("#ql").click();
    if ($("#lr")) $("#lr").onclick = poll;
    if ($("#seek")) $("#seek").onclick = e => {
      const r = e.currentTarget.getBoundingClientRect(), f = (e.clientX - r.left) / r.width;
      if (fromYtm() && ytm.duration > 0) cmd("seek", f * ytm.duration);
      else if (snap?.now?.can_seek) cmd("seek", snap.now.start + f * (snap.now.duration - snap.now.start));
    };
    tick();
  };

  const draw = () => {
    if (design === "coverflow") return drawFlow();
    if (fromYtm()) snap = { now: { title: ytm.title, artist: ytm.artist, album: ytm.album, app: "YouTube Music", playing: ytm.playing, position: ytm.position, duration: ytm.duration, can_seek: true, start: 0 }, cover: ytm.cover, age_ms: 0 };
    const n = snap?.now;
    if (!n) {
      body.innerHTML = `<div class="music"><div class="cover"></div><div class="track"><div class="t">Nothing playing</div><div class="a">Play something in any app</div>
        <div class="idle-players"><button class="btn" id="wmp">Media Player</button><button class="btn" id="ytm">YouTube Music</button></div></div></div>`;
      $("#wmp").onclick = () => invoke("open_player", { which: "wmp" }).catch(e => toast(String(e)));
      $("#ytm").onclick = openPlayer;
      return;
    }
    const cover = snap.cover ? `background-image:url('${snap.cover}')` : "";
    const transport = `<div class="transport"><button class="icon-btn" data-c="prev">${ICON.prev}</button><button class="icon-btn play" data-c="play">${n.playing ? ICON.pause : ICON.play}</button><button class="icon-btn" data-c="next">${ICON.next}</button></div>`;
    // Browsers often report no length; then there is nothing to seek.
    const seek = n.duration - n.start > 1 ? `<div class="seek" id="seek"><i id="bar"></i></div><div class="times"><span id="pos">0:00</span><span>${fmt(n.duration - n.start)}</span></div>` : `<div class="app">${esc(n.app)}</div>`;
    const info = `<div class="t">${esc(n.title)}</div><div class="a">${esc(n.artist || n.album)}</div>`;
    // Sleeve needs room for the art; a short widget falls back to Compact.
    if (design === "sleeve" && innerHeight >= 250) body.innerHTML = `<div class="music mu-sleeve"><div class="cover" style="${cover}"></div><div class="track">${info}</div><div style="width:80%">${seek}</div>${transport}</div>`;
    else if (design === "coverflow") body.innerHTML = `<div class="music mu-coverflow"><div class="stack"><div class="cover b1" style="${cover}"></div><div class="cover b2" style="${cover}"></div><div class="cover f" style="${cover}"></div></div><div class="track">${info}${seek}${transport}</div></div>`;
    else if (design === "turntable") body.innerHTML = `<div class="music mu-turntable" style="position:relative"><div class="vinyl ${n.playing ? "spin" : ""}"><div class="label" style="${cover}"></div></div><div class="arm" style="${n.playing ? "" : "transform:rotate(0deg)"}"></div><div class="deck track" style="margin-left:18px">${info}${seek}${transport}</div></div>`;
    else body.innerHTML = `<div class="music"><div class="cover" style="${cover}"></div><div class="track">${info}${seek}${transport}</div></div>`;
    body.querySelectorAll("[data-c]").forEach(b => b.onclick = () => cmd(b.dataset.c));
    if ($("#seek")) $("#seek").onclick = e => {
      if (!n.can_seek) return;
      const r = e.currentTarget.getBoundingClientRect();
      cmd("seek", n.start + (e.clientX - r.left) / r.width * (n.duration - n.start));
    };
    tick();
  };
  const tick = () => {
    if (!$("#bar")) return;
    let pos, len;
    if (fromYtm()) { len = Math.max(1, ytm.duration); pos = ytm.position + (ytm.playing ? (Date.now() - fetched) / 1000 : 0); }
    else {
      const n = snap?.now; if (!n) return;
      pos = n.position - n.start + (n.playing ? (snap.age_ms + (Date.now() - fetched)) / 1000 : 0);
      len = Math.max(1, n.duration - n.start);
    }
    $("#bar").style.width = clamp(pos / len, 0, 1) * 100 + "%";
    if ($("#pos")) $("#pos").textContent = fmt(Math.min(pos, len));
  };
  let key = null;  // null: the first poll always draws (even with nothing playing)
  const poll = async () => {
    [snap, ytm] = await Promise.all([invoke("media_now"), invoke("ytm_now").catch(() => null)]);
    fetched = Date.now();
    const n = snap.now;
    const k = fromYtm()
      ? ["y", ytm.title, ytm.artist, ytm.playing, ytm.cover, (ytm.queue || []).length, (ytm.queue || []).findIndex(x => x.current)].join("|")
      : n ? [n.title, n.artist, n.playing, !!snap.cover].join("|") : "";
    if (k !== key) { key = k; draw(); } else tick();
  };
  every(1000, poll);
  every(250, tick);

  // Music-reactive effects: levels of what the speakers play (~30 fps).
  // rAF pauses while the widget is covered, and capture stops with it.
  if (S.musicFx !== false) {
    let alive = true, busy = false, lastT = 0;
    const BAR_BANDS = [1, 3, 6, 9, 12];
    const loop = async t => {
      if (!alive) return;
      requestAnimationFrame(loop);
      if (busy || t - lastT < 33) return;
      busy = true; lastT = t;
      try {
        const l = await invoke("audio_levels");
        const root = body.querySelector(".mf, .music");
        if (root) {
          root.classList.add("fx");
          root.style.setProperty("--bass", l.bass.toFixed(3));
          root.style.setProperty("--lvl", l.level.toFixed(3));
        }
        body.querySelectorAll(".wave i").forEach((b, i) => { b.style.height = (3 + l.bands[BAR_BANDS[i] ?? i] * 11).toFixed(1) + "px"; });
      } catch { } finally { busy = false; }
    };
    requestAnimationFrame(loop);
    teardown.push(() => { alive = false; });
  }
}

// -------------------------------------------------------------- weather --

function wxIcon(code, day = true) {
  const sun = `<circle cx="12" cy="12" r="4.5" fill="#ffd36b" stroke="none"/><g stroke="#ffd36b">${[0, 45, 90, 135, 180, 225, 270, 315].map(a => { const r = a * Math.PI / 180; return `<line x1="${12 + Math.cos(r) * 7}" y1="${12 + Math.sin(r) * 7}" x2="${12 + Math.cos(r) * 9.5}" y2="${12 + Math.sin(r) * 9.5}"/>`; }).join("")}</g>`;
  const moon = `<path d="M15.5 4.5a7.5 7.5 0 1 0 4 10.4A6 6 0 0 1 15.5 4.5z" fill="#dfe6ff" stroke="none"/>`;
  const cloud = (y = 0, c = "#e9eef7") => `<path d="M7 ${18 + y}h10.5a3.5 3.5 0 0 0 0-7 5 5 0 0 0-9.6-1.2A3.9 3.9 0 0 0 7 ${18 + y}z" fill="${c}" stroke="none"/>`;
  const drops = c => `<g stroke="${c}" stroke-width="1.6">${[8, 12, 16].map(x => `<line x1="${x}" y1="19.5" x2="${x - 1}" y2="22"/>`).join("")}</g>`;
  let inner;
  if (code <= 1) inner = day ? sun : moon;
  else if (code <= 3) inner = `<g transform="translate(-3 -4) scale(.8)">${day ? sun : moon}</g>${cloud(0)}`;
  else if (code <= 48) inner = `${cloud(-2, "#c9d1de")}<g stroke="#c9d1de"><line x1="5" y1="20" x2="19" y2="20"/><line x1="7" y1="22.5" x2="17" y2="22.5"/></g>`;
  else if (code <= 67 || (code >= 80 && code <= 82)) inner = `${cloud(-3, "#cfd7e6")}${drops("#7cc5ff")}`;
  else if (code <= 77 || code === 85 || code === 86) inner = `${cloud(-3)}<g fill="#fff" stroke="none">${[8, 12, 16].map(x => `<circle cx="${x}" cy="21" r="1.2"/>`).join("")}</g>`;
  else inner = `${cloud(-3, "#aab3c6")}<path d="M12 16l-2 4h3l-1.5 3.5" stroke="#ffd36b" stroke-width="1.6" fill="none"/>`;
  return `<svg class="wx-icon" viewBox="0 0 24 24" stroke-width="1.6" stroke-linecap="round" fill="none">${inner}</svg>`;
}
const WX_DESC = c => ({ 0: "Clear sky", 1: "Mainly clear", 2: "Partly cloudy", 3: "Overcast", 45: "Fog", 48: "Rime fog", 51: "Light drizzle", 53: "Drizzle", 55: "Heavy drizzle", 61: "Light rain", 63: "Rain", 65: "Heavy rain", 71: "Light snow", 73: "Snow", 75: "Heavy snow", 80: "Rain showers", 81: "Rain showers", 82: "Violent showers", 95: "Thunderstorm", 96: "Thunderstorm, hail", 99: "Thunderstorm, hail" }[c] || "—");

async function renderWeather() {
  const design = designOf("weather");
  const ex = await invoke("get_extras");
  const cacheKey = "wx:" + ex.weatherCity + ":" + ex.weatherFahrenheit;
  let w = null;
  try { w = JSON.parse(localStorage.getItem(cacheKey) || "null"); } catch { }
  const r1 = v => Math.round(v);
  const draw = err => {
    if (!w) { body.innerHTML = `<div class="empty">${esc(err || "Loading weather…")}</div>`; return; }
    const desc = WX_DESC(w.code);
    if (design === "minimal") {
      body.innerHTML = `<div class="wx wx-minimal"><div class="place">${esc(w.place)}</div><div class="temp">${r1(w.temp)}°</div><div class="dim">${desc} · H ${r1(w.hi)}° L ${r1(w.lo)}° · ${r1(w.humidity)}% · ${r1(w.wind)} km/h</div></div>`;
    } else if (design === "forecast") {
      const lo = Math.min(...w.daily.map(d => d[2])), hi = Math.max(...w.daily.map(d => d[1])), span = Math.max(1, hi - lo);
      body.innerHTML = `<div class="wx wx-forecast"><div class="head" style="margin:0"><h1>${esc(w.place)} · ${r1(w.temp)}${w.unit}</h1></div>
        <div class="col grow" style="gap:6px;justify-content:space-around">${w.daily.map(d => `<div class="day"><span>${esc(d[0])}</span>${wxIcon(d[3])}<span class="dim">${r1(d[2])}°</span><div class="range"><i style="left:${(d[2] - lo) / span * 100}%;right:${100 - (d[1] - lo) / span * 100}%"></i></div><span>${r1(d[1])}°</span></div>`).join("")}</div></div>`;
    } else {
      body.innerHTML = `<div class="wx"><div class="head" style="margin:0"><h1>${esc(w.place)}</h1></div>
        <div class="now">${wxIcon(w.code, w.is_day)}<div><div class="temp">${r1(w.temp)}°</div></div><div class="col" style="gap:2px;margin-left:auto;text-align:right"><div class="desc">${desc}</div><div class="dim">H ${r1(w.hi)}° · L ${r1(w.lo)}°</div><div class="faint">${r1(w.humidity)}% · ${r1(w.wind)} km/h</div></div></div>
        <div class="hours">${w.hourly.map(h => `<div><span class="faint">${esc(h[0])}h</span>${wxIcon(h[2])}<span>${r1(h[1])}°</span></div>`).join("")}</div></div>`;
    }
  };
  draw();
  const load = async () => {
    try { w = await invoke("weather", { city: ex.weatherCity, fahrenheit: ex.weatherFahrenheit }); localStorage.setItem(cacheKey, JSON.stringify(w)); draw(); }
    catch (e) { if (!w) draw(String(e)); }
  };
  every(20 * 60 * 1000, load);
}

// --------------------------------------------------------------- system --

async function renderSystem() {
  const design = designOf("system");
  const ex = await invoke("get_extras");
  const gb = b => (b / 1073741824).toFixed(1);
  const draw = s => {
    const memP = s.mem_used / Math.max(1, s.mem_total) * 100, diskP = s.disk_used / Math.max(1, s.disk_total) * 100;
    if (design === "rings") {
      const ring = (name, v, color, sub) => { const C = 2 * Math.PI * 34; return `<div class="ring"><svg viewBox="0 0 84 84"><circle class="bg" cx="42" cy="42" r="34"/><circle class="fg" cx="42" cy="42" r="34" stroke="${color}" stroke-dasharray="${C}" stroke-dashoffset="${C * (1 - v / 100)}"/></svg><b>${Math.round(v)}%</b><span class="dim">${name}</span><small class="faint">${sub}</small></div>`; };
      body.innerHTML = `<div class="sys"><div class="head" style="margin:0"><h1>System</h1></div><div class="rings">${ring("CPU", s.cpu, "#7c9cff", "")}${ring("Memory", memP, "#c084fc", gb(s.mem_used) + " GB")}${ring("Disk", diskP, "#34d399", esc(s.disk_name))}</div>
        ${ex.systemShowNetwork ? `<div class="net"><span>↓ ${s.downText}</span><span>${s.uptime}</span><span>↑ ${s.upText}</span></div>` : ""}</div>`;
    } else if (design === "terminal") {
      const bar = v => "#".repeat(Math.round(v / 5)).padEnd(20, ".");
      body.innerHTML = `<div class="sys sys-terminal"><div>$ opal --stats</div><div>cpu  [${bar(s.cpu)}] ${s.cpu.toFixed(0).padStart(3)}%</div><div>mem  [${bar(memP)}] ${gb(s.mem_used)}/${gb(s.mem_total)}G</div><div>disk [${bar(diskP)}] ${esc(s.disk_name)} ${gb(s.disk_used)}/${gb(s.disk_total)}G</div>
        ${ex.systemShowNetwork ? `<div>net  ↓ ${s.downText}  ↑ ${s.upText}</div>` : ""}<div>${s.uptime}</div><div>_</div></div>`;
    } else {
      const meter = (n, v, txt) => `<div class="meter"><span class="dim">${n}</span><div class="dots">${Array.from({ length: 16 }, (_, i) => `<i class="${i < Math.round(v / 100 * 16) ? "on" : ""}"></i>`).join("")}</div><span class="v">${txt}</span></div>`;
      body.innerHTML = `<div class="sys"><div class="head" style="margin:0"><h1>System · CPU ${Math.round(s.cpu)}%</h1></div>${ex.systemShowGraph ? `<canvas id="cv"></canvas>` : ""}
        ${meter("CPU", s.cpu, Math.round(s.cpu) + "%")}${meter("Memory", memP, gb(s.mem_used) + " / " + gb(s.mem_total) + " GB")}${meter("Disk " + esc(s.disk_name), diskP, Math.round(diskP) + "%")}
        ${ex.systemShowNetwork ? `<div class="net"><span>↓ ${s.downText}</span><span>${s.uptime}</span><span>↑ ${s.upText}</span></div>` : ""}</div>`;
      const cv = $("#cv");
      if (cv) {
        const dpr = devicePixelRatio, W = cv.clientWidth, H = cv.clientHeight;
        cv.width = W * dpr; cv.height = H * dpr;
        const g = cv.getContext("2d"); g.scale(dpr, dpr);
        const hist = s.cpu_history, n = Math.max(2, hist.length);
        const accent = getComputedStyle(plate).getPropertyValue("--accent").trim() || "#9db4ff";
        g.beginPath();
        hist.forEach((v, i) => { const x = i / (n - 1) * W, y = H - v / 100 * (H - 4) - 2; i ? g.lineTo(x, y) : g.moveTo(x, y); });
        g.strokeStyle = accent; g.lineWidth = 1.6; g.stroke();
        g.lineTo(W * (hist.length - 1) / (n - 1), H); g.lineTo(0, H); g.closePath();
        const grad = g.createLinearGradient(0, 0, 0, H); grad.addColorStop(0, accent + "55"); grad.addColorStop(1, accent + "00");
        g.fillStyle = grad; g.fill();
      }
    }
  };
  every(1500, async () => draw(await invoke("system_stats")));
}

// ------------------------------------------------------------- pomodoro --

async function renderPomodoro() {
  const design = designOf("pomodoro");
  const ex = await invoke("get_extras");
  const LEN = { focus: ex.focusMinutes, short: ex.shortBreakMinutes, long: ex.longBreakMinutes };
  const NAME = { focus: "Focus", short: "Short break", long: "Long break" };
  let st;
  try { st = JSON.parse(localStorage.getItem("pomo") || "null"); } catch { }
  st ||= { mode: "focus", running: false, endAt: 0, left: LEN.focus * 60, rounds: 0 };
  const save = () => localStorage.setItem("pomo", JSON.stringify(st));
  const total = () => LEN[st.mode] * 60;
  const remaining = () => st.running ? Math.max(0, (st.endAt - Date.now()) / 1000) : st.left;
  const setMode = m => { st = { ...st, mode: m, running: false, left: LEN[m] * 60 }; save(); draw(); };
  const finish = () => {
    invoke("beep");
    if (st.mode === "focus") { st.rounds++; setMode(st.rounds % 4 === 0 ? "long" : "short"); } else setMode("focus");
    toast(NAME[st.mode] + " — ready");
  };
  const draw = () => {
    const left = remaining(), frac = 1 - left / total();
    const txt = `${pad(Math.floor(left / 60))}:${pad(Math.floor(left % 60))}`;
    const pills = `<div class="pills">${Object.keys(LEN).map(m => `<button class="${m === st.mode ? "on" : ""}" data-m="${m}">${NAME[m]}</button>`).join("")}</div>`;
    const btns = `<div class="transport"><button class="icon-btn" data-a="reset" title="Reset">${ICON.reset}</button><button class="icon-btn play" data-a="toggle">${st.running ? ICON.pause : ICON.play}</button><button class="icon-btn" data-a="skip" title="Skip">${ICON.skip}</button></div>`;
    let face;
    if (design === "minimal") face = `<div class="digits">${txt}</div><div class="thin"><i style="width:${frac * 100}%"></i></div><div class="dim">${NAME[st.mode]}</div>`;
    else if (design === "tomato") face = `<div class="tomato"><div class="body"><div class="fill" style="height:${frac * 100}%"></div></div><div class="leaf"></div><div class="lbl">${txt}</div></div><div class="dim">${NAME[st.mode]}</div>`;
    else { const C = 2 * Math.PI * 45; face = `${pills}<div class="pring"><svg viewBox="0 0 100 100"><circle class="bg" cx="50" cy="50" r="45"/><circle class="fg" cx="50" cy="50" r="45" stroke-dasharray="${C}" stroke-dashoffset="${C * (1 - frac)}"/></svg><div class="lbl">${txt}</div></div>`; }
    body.innerHTML = `<div class="pomo">${face}${btns}<div class="rounds">Round ${st.rounds % 4 + 1} of 4 · ${st.rounds} done</div></div>`;
    body.querySelectorAll("[data-m]").forEach(b => b.onclick = () => setMode(b.dataset.m));
    body.querySelector("[data-a=toggle]").onclick = () => {
      if (st.running) { st.left = remaining(); st.running = false; } else { st.endAt = Date.now() + st.left * 1000; st.running = true; }
      save(); draw();
    };
    body.querySelector("[data-a=reset]").onclick = () => setMode(st.mode);
    body.querySelector("[data-a=skip]").onclick = finish;
  };
  every(500, () => { if (st.running && remaining() <= 0) finish(); else draw(); });
}

// --------------------------------------------------------------- photos --

async function renderPhotos() {
  const design = designOf("photos");
  const ex = await invoke("get_extras");
  if (!ex.photosFolder) {
    body.innerHTML = `<div class="empty"><div>Choose a folder of photos</div><button class="btn" id="pick">Choose folder…</button></div>`;
    $("#pick").onclick = async () => { const f = await invoke("pick_folder"); if (f) { await updateExtras(e => e.photosFolder = f); render(); } };
    return;
  }
  body.innerHTML = `<div class="empty">Loading photos…</div>`;
  const list = await invoke("list_photos", { folder: ex.photosFolder });
  if (!list.length) {
    body.innerHTML = `<div class="empty"><div>No photos in<br><small class="faint">${esc(ex.photosFolder)}</small></div><button class="btn" id="pick">Choose folder…</button></div>`;
    $("#pick").onclick = async () => { const f = await invoke("pick_folder"); if (f) { await updateExtras(e => e.photosFolder = f); render(); } };
    return;
  }
  body.style.padding = "0";
  const frame = design === "polaroid" ? `<div class="ph-polaroid" id="fr"></div><div class="ph-caption" id="cap"></div>`
    : design === "film" ? `<div class="sprockets" style="top:8px"></div><div class="ph-film" id="fr"></div><div class="sprockets" style="bottom:8px"></div>`
      : `<div class="photos" id="fr"></div>`;
  body.innerHTML = frame;
  const fr = $("#fr");
  let i = 0;
  const show = async () => {
    const path = list[i++ % list.length];
    const url = await invoke("photo", { path });
    if (!url) return;
    const img = new Image(); img.src = url; img.style.opacity = 0;
    fr.appendChild(img);
    await img.decode().catch(() => { });
    img.style.opacity = 1;
    const cap = $("#cap"); if (cap) cap.textContent = path.split(/[\\/]/).pop().replace(/\.[^.]+$/, "");
    setTimeout(() => { while (fr.children.length > 1) fr.firstChild.remove(); }, 900);
  };
  every(Math.max(5, ex.photosInterval) * 1000, show);
  plate.ondblclick = () => show();
  teardown.push(() => { plate.ondblclick = null; body.style.padding = ""; });
}

// ---------------------------------------------------------------- lunar --

async function renderLunar() {
  const design = designOf("lunar");
  const ex = await invoke("get_extras");
  const WD = ["Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy", "Chủ Nhật"];
  const draw = async () => {
    const l = await invoke("lunar_today");
    const s = l.solar, lu = l.lunar;
    const hours = ex.lunarShowHours ? `<div class="hours"><b>Giờ hoàng đạo:</b> ${l.gioHoangDao.join(", ")}</div>` : "";
    const term = ex.lunarShowTerm ? ` · Tiết ${l.tietKhi}` : "";
    if (design === "toloch") {
      body.innerHTML = `<div class="lunar lu-toloch"><div class="hdr">THÁNG ${s.month} · ${s.year}</div><div class="big">${s.day}</div><div style="font-weight:600">${WD[s.weekday]}</div>
        <div style="font-size:13px;margin-top:4px">Âm lịch: ngày ${lu.day} tháng ${esc(lu.monthName)}${lu.leap ? " (nhuận)" : ""}</div><div style="font-size:12px;opacity:.7">Năm ${l.yearCanChi} · Ngày ${l.dayCanChi}${term}</div></div>`;
    } else if (design === "minimal") {
      body.innerHTML = `<div class="lunar lu-minimal"><div class="lbl">Âm lịch</div><div class="big">${lu.day}/${lu.month}${lu.leap ? "<small>n</small>" : ""}</div><div class="cc">Tháng ${esc(lu.monthName)} năm ${l.yearCanChi}</div><div class="faint">${WD[s.weekday]}, ${s.day}/${s.month}/${s.year}${term}</div></div>`;
    } else {
      body.innerHTML = `<div class="lunar"><div class="head" style="margin:0"><h1>${WD[s.weekday]}</h1></div>
        <div class="pair"><div><div class="lbl">Dương lịch</div><div class="big">${s.day}</div><div class="cc">Tháng ${s.month}, ${s.year}</div></div><div class="sep"></div>
        <div><div class="lbl">Âm lịch</div><div class="big">${lu.day}</div><div class="cc">Tháng ${esc(lu.monthName)}${lu.leap ? " nhuận" : ""} · ${l.yearCanChi}</div></div></div>
        <div class="cc">Ngày ${l.dayCanChi} · Tháng ${l.monthCanChi}${term}</div>${hours}</div>`;
    }
  };
  every(60000, draw);
}

// ----------------------------------------------------------------- todo --

async function renderTodo() {
  const design = designOf("todo");
  let ex = await invoke("get_extras");
  const draw = () => {
    const todos = ex.todos;
    const left = todos.filter(t => !t.done).length;
    body.innerHTML = `<div class="todo td-${design}"><div class="head" style="margin:0"><h1>To-do · ${left} left</h1></div>
      <div class="add"><input id="nt" placeholder="Add a task" spellcheck="false"><button class="btn" id="ab">${ICON.plus}</button></div>
      <div class="grow scroll"><div class="list">${todos.map((t, i) => `<div class="item ${t.done ? "done" : ""}" data-i="${i}"><span class="tick"></span><span>${esc(t.text)}</span><button class="x" title="Delete">${ICON.close}</button></div>`).join("") || `<div class="empty" style="padding:24px 0">Nothing to do 🎉</div>`}</div></div>
      ${todos.some(t => t.done) ? `<button class="btn" id="clr" style="align-self:flex-start">Clear completed</button>` : ""}</div>`;
    const save = mutate => updateExtras(e => { mutate(e); }).then(e => { ex = e; draw(); });
    const add = () => { const v = $("#nt").value.trim(); if (v) save(e => e.todos.push({ id: Math.random().toString(36).slice(2), text: v, done: false, createdAt: Date.now() })); };
    $("#ab").onclick = add;
    $("#nt").onkeydown = e => { if (e.key === "Enter") add(); };
    body.querySelectorAll(".item").forEach(row => {
      const id = todos[+row.dataset.i].id;
      row.querySelector(".tick").onclick = () => save(e => { const t = e.todos.find(x => x.id === id); if (t) t.done = !t.done; });
      row.querySelector(".x").onclick = () => save(e => { e.todos = e.todos.filter(x => x.id !== id); });
    });
    const clr = $("#clr"); if (clr) clr.onclick = () => save(e => { e.todos = e.todos.filter(x => !x.done); });
  };
  draw();
  const un = await listen("extras-changed", async () => { if (document.activeElement?.id !== "nt") { ex = await invoke("get_extras"); draw(); } });
  teardown.push(un);
}

// ------------------------------------------------------------ countdown --

const WD_SHORT = ["T2", "T3", "T4", "T5", "T6", "T7", "CN"];
async function renderCountdown() {
  const design = designOf("countdown");
  let items = [], adding = false;
  const left = d => d === 0 ? "Hôm nay" : d === 1 ? "Ngày mai" : `${d} ngày`;
  const when = it => { const [y, m, d] = it.date.split("-"); return `${WD_SHORT[it.weekday]}, ${+d}/${+m}/${y}`; };
  // How close it is: full when today, fading out over the item's natural span.
  const near = it => clamp(1 - it.days / (it.kind === "moon" ? 30 : 365), .04, 1);
  const del = it => it.kind === "custom" ? `<button class="x no-drag" data-del="${esc(it.id)}" title="Xoá">${ICON.close}</button>` : "";
  const form = `<div class="cd-form no-drag"><input id="cdn" placeholder="Tên (giỗ ông, sinh nhật…)" spellcheck="false">
      <div class="row"><input id="cdd" type="number" min="1" max="30" placeholder="Ngày"><input id="cdm" type="number" min="1" max="12" placeholder="Tháng">
      <label class="row cd-lunar"><input type="checkbox" id="cdl" checked> Âm lịch</label><button class="btn primary" id="cds">Lưu</button></div></div>`;
  const draw = () => {
    let list;
    if (!items.length) list = `<div class="empty">Chưa có ngày nào. Bấm + để thêm.</div>`;
    else if (design === "hero") {
      const [first, ...rest] = items;
      list = `<div class="cd-hero cd-${first.kind}"><div class="cd-num opal-text">${first.days}</div><div class="cd-unit">${first.days === 0 ? "Hôm nay" : "ngày nữa"}</div>
          <div class="cd-name">${esc(first.name)}</div><div class="faint">${when(first)} · ${esc(first.lunar)}</div></div>
        <div class="cd-rest">${rest.slice(0, 3).map(it => `<div class="cd-row cd-${it.kind}"><i></i><span class="grow">${esc(it.name)}</span><b>${left(it.days)}</b>${del(it)}</div>`).join("")}</div>`;
    } else if (design === "tickets") {
      list = `<div class="cd-tickets">${items.map(it => `<div class="cd-ticket cd-${it.kind}"><div class="cd-stub"><b>${it.days}</b><small>${it.days === 0 ? "hôm nay" : "ngày"}</small></div>
          <div class="cd-info"><div class="cd-name">${esc(it.name)}</div><div class="faint">${when(it)} · ${esc(it.lunar)}</div></div>${del(it)}</div>`).join("")}</div>`;
    } else {
      list = `<div class="cd-list">${items.map(it => `<div class="cd-item cd-${it.kind}"><i></i><div class="grow"><div class="cd-name">${esc(it.name)}</div>
          <div class="faint">${when(it)} · ${esc(it.lunar)}</div><div class="cd-bar"><span style="width:${near(it) * 100}%"></span></div></div>
          <div class="cd-left"><b>${it.days === 0 ? "★" : it.days}</b><small>${it.days === 0 ? "hôm nay" : "ngày"}</small></div>${del(it)}</div>`).join("")}</div>`;
    }
    body.innerHTML = `<div class="countdown"><div class="head" style="margin:0"><h1>Đếm ngược</h1><button class="icon-btn no-drag" id="cda" title="Thêm ngày">${adding ? ICON.close : ICON.plus}</button></div>
      ${adding ? form : ""}<div class="grow scroll">${list}</div></div>`;
    $("#cda").onclick = () => { adding = !adding; draw(); if (adding) $("#cdn").focus(); };
    body.querySelectorAll("[data-del]").forEach(b => b.onclick = () => updateExtras(e => { e.countdowns = e.countdowns.filter(c => c.id !== b.dataset.del); }));
    const save = $("#cds");
    if (save) save.onclick = async () => {
      const name = $("#cdn").value.trim(), day = +$("#cdd").value, month = +$("#cdm").value, lunar = $("#cdl").checked;
      if (!name || !(day >= 1 && day <= (lunar ? 30 : 31)) || !(month >= 1 && month <= 12)) { toast("Nhập tên, ngày và tháng hợp lệ"); return; }
      adding = false;
      document.activeElement?.blur();
      draw();
      await updateExtras(e => e.countdowns.push({ id: Math.random().toString(36).slice(2), name, day, month, lunar }));
    };
  };
  const load = async () => { items = await invoke("countdowns"); if (!document.activeElement?.closest?.(".cd-form")) draw(); };
  every(10 * 60 * 1000, load);
  const un = await listen("extras-changed", load); teardown.push(un);
}

const RENDER = { launcher: renderLauncher, clock: renderClock, notes: renderNotes, calendar: renderCalendar, music: renderMusic, weather: renderWeather, system: renderSystem, pomodoro: renderPomodoro, photos: renderPhotos, lunar: renderLunar, todo: renderTodo, countdown: renderCountdown };

// --------------------------------------------------------------- panels --

async function renderWidgetsPanel() {
  const list = await invoke("widgets_list");
  body.innerHTML = `<div class="panel"><div class="head" style="margin-bottom:2px"><h2>Widgets</h2></div><div class="sub">Opal · Tauri edition</div>
    <div class="row" style="margin:14px 0 4px"><button class="btn" id="all">Show all</button><button class="btn" id="none">Hide all</button><span class="grow"></span><button class="btn" id="glob">${ICON.gear} Settings</button></div>
    <div class="grow scroll" style="margin-top:8px"><div class="wlist">${list.map(w => {
      const d = WIDGETS.find(x => x.id === w.id)?.designs || [];
      return `<div class="witem"><div class="ico">${WIDGET_GLYPH[w.id] || "•"}</div><div class="nm">${esc(w.name)}<small>${esc(d.find(x => x.id === designOf(w.id))?.name || "")} · ${d.length} design${d.length === 1 ? "" : "s"}</small></div>
        <button class="icon-btn" data-s="${w.id}" title="Settings">${ICON.gear}</button><button class="switch ${w.open ? "on" : ""}" data-w="${w.id}"></button></div>`;
    }).join("")}</div></div>
    <div class="foot"><button class="btn" id="cfg">Open config folder</button><span class="grow"></span><button class="btn" id="quit">Quit Opal</button></div></div>`;
  body.querySelectorAll("[data-w]").forEach(b => b.onclick = () => { b.classList.toggle("on"); invoke("set_widget_open", { id: b.dataset.w, open: b.classList.contains("on") }); });
  body.querySelectorAll("[data-s]").forEach(b => b.onclick = () => invoke("open_widget_settings", { id: b.dataset.s }));
  const setAll = open => Promise.all(list.map(w => invoke("set_widget_open", { id: w.id, open })));
  $("#all").onclick = () => setAll(true);
  $("#none").onclick = () => setAll(false);
  $("#glob").onclick = () => invoke("open_widget_settings", { id: "launcher" });
  $("#cfg").onclick = () => invoke("config_folder");
  $("#quit").onclick = () => invoke("quit");
  const un = await listen("widgets-changed", () => render()); teardown.push(un);
}

const TIMEZONES = ["local", "UTC", "Asia/Ho_Chi_Minh", "Asia/Bangkok", "Asia/Singapore", "Asia/Shanghai", "Asia/Tokyo", "Asia/Seoul", "Asia/Kolkata", "Asia/Dubai", "Europe/London", "Europe/Paris", "Europe/Berlin", "Europe/Moscow", "America/New_York", "America/Chicago", "America/Denver", "America/Los_Angeles", "America/Sao_Paulo", "Australia/Sydney", "Pacific/Auckland"];

async function renderSettingsPanel(w) {
  const name = WIDGETS.find(x => x.id === w)?.name || w;
  const designs = designsFor(w);
  const ex = await invoke("get_extras");
  const cal = w === "calendar" ? (await invoke("calendar_get")).config : null;
  const cur = designOf(w);
  const app = appearance(w);
  const scope = w === "launcher" ? "global" : "widget";
  const field = (label, ctrl, hint = "") => `<div class="field"><label>${label}${hint ? `<small>${hint}</small>` : ""}</label>${ctrl}</div>`;
  const sw = (key, on) => `<button class="switch ${on ? "on" : ""}" data-k="${key}"></button>`;
  const range = (key, v, min, max) => `<input type="range" data-k="${key}" min="${min}" max="${max}" value="${v}"><span class="val">${v}</span>`;
  const themeChips = `<div class="themes">${w === "launcher" ? "" : `<div class="theme-chip global ${S.widgetTheme?.[w] ? "" : "on"}" data-t="">Global</div>`}${THEMES.map(t => `<div class="theme-chip ${(w === "launcher" ? S.theme : S.widgetTheme?.[w]) === t.id ? "on" : ""}" data-t="${t.id}" title="${esc(t.name)}" style="background:${t.css}"></div>`).join("")}</div>`;

  let specific = "";
  if (w === "launcher") specific = field("Columns", range("columns", S.columns, 3, 12)) + field("Icon size", range("iconSize", S.iconSize, 36, 60)) + field("Search box", sw("showSearch", S.showSearch)) + field("Labels", sw("showLabels", S.showLabels))
    + field("Start with Windows", sw("startWithWindows", S.startWithWindows)) + field("Performance mode", sw("performanceMode", S.performanceMode), "Lighter blur")
    + field("Bảng nổi lên trên", sw("panelsOnTop", !!S.panelsOnTop), "Bảng Widgets và cài đặt hiện trên các cửa sổ khác")
    + field("Chìm xuống nền sau (giây)", range("sinkDelay", S.sinkDelay ?? 10, 0, 30), "Widget đang dùng nằm trên cùng, để yên chừng này giây thì chìm xuống hình nền");
  if (w === "clock") specific = field("12-hour clock", sw("clock.hour12", S.clock.hour12)) + field("Show seconds", sw("clock.showSeconds", S.clock.showSeconds))
    + field("Time zone", `<select data-k="clock.timeZone">${TIMEZONES.map(z => `<option value="${z}" ${S.clock.timeZone === z ? "selected" : ""}>${z === "local" ? "This PC" : z}</option>`).join("")}</select>`);
  if (w === "calendar") specific = field("Calendar feed", `<input type="text" data-c="icsUrl" value="${esc(cal.icsUrl)}" placeholder="https://…/basic.ics">`, "ICS link from Google / Outlook")
    + field("Week starts on", `<select data-c="weekStartsOn"><option value="1" ${cal.weekStartsOn !== 0 ? "selected" : ""}>Monday</option><option value="0" ${cal.weekStartsOn === 0 ? "selected" : ""}>Sunday</option></select>`);
  if (w === "weather") specific = field("City", `<input type="text" data-x="weatherCity" value="${esc(ex.weatherCity)}" placeholder="Locate automatically">`) + field("Fahrenheit", sw("x.weatherFahrenheit", ex.weatherFahrenheit));
  if (w === "photos") specific = field("Folder", `<button class="btn" id="pickf">Choose…</button>`, esc(ex.photosFolder)) + field("Seconds per photo", `<input type="number" min="5" max="3600" data-x="photosInterval" value="${ex.photosInterval}">`);
  if (w === "pomodoro") specific = field("Focus (min)", `<input type="number" min="1" max="180" data-x="focusMinutes" value="${ex.focusMinutes}">`) + field("Short break (min)", `<input type="number" min="1" max="60" data-x="shortBreakMinutes" value="${ex.shortBreakMinutes}">`) + field("Long break (min)", `<input type="number" min="1" max="120" data-x="longBreakMinutes" value="${ex.longBreakMinutes}">`);
  if (w === "system") specific = field("CPU graph", sw("x.systemShowGraph", ex.systemShowGraph)) + field("Network", sw("x.systemShowNetwork", ex.systemShowNetwork));
  if (w === "lunar") specific = field("Giờ hoàng đạo", sw("x.lunarShowHours", ex.lunarShowHours)) + field("Tiết khí", sw("x.lunarShowTerm", ex.lunarShowTerm));
  if (w === "countdown") specific = field("Tết Nguyên Đán", sw("x.countdownShowTet", ex.countdownShowTet)) + field("Rằm và Mùng 1", sw("x.countdownShowMoon", ex.countdownShowMoon), "Ngày cúng sắp tới theo âm lịch");
  if (w === "music") specific = field("Hiệu ứng theo nhạc", sw("musicFx", S.musicFx !== false), "Sóng nhạc, bìa nhún theo bass, quầng sáng theo độ lớn")
    + field("Open a player", `<div class="row"><button class="btn" id="wmp">Media Player</button><button class="btn" id="ytm">YouTube Music</button></div>`);

  const g = k => scope === "global" ? S[k] : (app[k] ?? S[k]);
  body.innerHTML = `<div class="panel"><div class="head" style="margin-bottom:2px"><h2>${esc(name)}</h2></div><div class="sub">${esc(designs.find(d => d.id === cur)?.description || "")}</div>
    <div class="grow scroll" style="margin-top:6px;padding-right:4px">
      ${designs.length > 1 ? `<h3>Design</h3><div class="designs">${designs.map(d => `<button class="design ${d.id === cur ? "on" : ""}" data-d="${d.id}"><div class="sw" style="background:${d.surface || THEMES.find(t => t.id === themeOf(w))?.css};color:${d.light ? "#191627" : "#fff"}">${esc(d.name)}</div><b>${esc(d.name)}</b><small>${esc(d.description)}</small></button>`).join("")}</div>` : ""}
      <h3>${w === "launcher" ? "Theme (all widgets)" : "Theme"}</h3>${themeChips}
      ${specific ? `<h3>${esc(name)}</h3><div class="card">${specific}</div>` : ""}
      ${w === "launcher" ? `<h3>Gọi nhanh (trong ô tìm kiếm)</h3><div class="qchips">${QUICK.map(q => `<button class="qchip ${quickIds().includes(q.id) ? "on" : ""}" data-qa="${q.id}">${q.icon}${esc(q.name)}</button>`).join("")}</div>` : ""}
      <h3>Glass${scope === "global" ? " (all widgets)" : ""}</h3><div class="card">
        ${scope === "widget" ? field("Frosted glass", sw("frosted", frostedOf(w))) : ""}
        ${field("See the wallpaper", sw("ap.wallpaperFrost", wallFrostOf(w)))}
        ${field("Frost", range("ap.frostStrength", g("frostStrength"), 0, 100))}
        ${field("Blur", range("ap.blurSize", g("blurSize"), 0, 80))}
        ${field("Tint", range("ap.veil", g("veil"), 0, 100), "Theme colour over the glass")}
        ${field("Opacity", range("opacity", opacityOf(w), 10, 100))}
      </div>
    </div>
    <div class="foot">${w !== "launcher" ? `<button class="btn" id="hide">Hide widget</button>` : `<button class="btn" id="wp">Widgets…</button>`}<span class="grow"></span>${scope === "widget" ? `<button class="btn" id="rst">Reset glass</button>` : ""}<button class="btn primary" id="done">Done</button></div></div>`;

  const setKey = (k, v) => {
    if (k.startsWith("clock.")) S.clock[k.slice(6)] = v;
    else if (k === "frosted") (S.widgetFrosted ||= {})[w] = v;
    else if (k === "opacity") { if (scope === "global") S.backgroundOpacity = v; else (S.widgetOpacity ||= {})[w] = v; }
    else if (k.startsWith("ap.")) { const f = k.slice(3); if (scope === "global") S[f] = v; else ((S.widgetAppearance ||= {})[w] ||= {})[f] = v; }
    else S[k] = v;
  };
  body.querySelectorAll("[data-qa]").forEach(c => c.onclick = () => {
    const id = c.dataset.qa, list = [...quickIds()];
    const i = list.indexOf(id);
    if (i >= 0) list.splice(i, 1); else list.push(id);
    S.quickActions = list; c.classList.toggle("on", i < 0);
    saveSettings(true);
  });
  body.querySelectorAll(".switch[data-k]").forEach(b => b.onclick = async () => {
    b.classList.toggle("on"); const on = b.classList.contains("on"), k = b.dataset.k;
    if (k.startsWith("x.")) { await updateExtras(e => e[k.slice(2)] = on); return; }
    setKey(k, on); saveSettings(true); applyGlass();
  });
  body.querySelectorAll("input[type=range][data-k]").forEach(r => r.oninput = () => {
    r.nextElementSibling.textContent = r.value; setKey(r.dataset.k, +r.value); saveSettings();
  });
  body.querySelectorAll("select[data-k]").forEach(s => s.onchange = () => { setKey(s.dataset.k, s.value); saveSettings(true); });
  body.querySelectorAll("[data-x]").forEach(i => i.onchange = () => updateExtras(e => e[i.dataset.x] = i.type === "number" ? +i.value : i.value));
  body.querySelectorAll("[data-c]").forEach(i => i.onchange = async () => {
    const c = (await invoke("calendar_get")).config;
    c[i.dataset.c] = i.dataset.c === "weekStartsOn" ? +i.value : i.value.trim();
    invoke("calendar_save", { config: c });
  });
  body.querySelectorAll("[data-d]").forEach(b => b.onclick = () => {
    if (w === "clock") S.clock.theme = b.dataset.d; else (S.widgetDesign ||= {})[w] = b.dataset.d;
    saveSettings(true); render();
  });
  body.querySelectorAll("[data-t]").forEach(c => c.onclick = () => {
    const t = c.dataset.t;
    if (w === "launcher") S.theme = t; else if (t) (S.widgetTheme ||= {})[w] = t; else delete S.widgetTheme[w];
    saveSettings(true); render();
  });
  $("#pickf") && ($("#pickf").onclick = async () => { const f = await invoke("pick_folder"); if (f) { await updateExtras(e => e.photosFolder = f); render(); } });
  $("#wmp") && ($("#wmp").onclick = () => invoke("open_player", { which: "wmp" }));
  $("#ytm") && ($("#ytm").onclick = () => invoke("open_player", { which: "ytm" }));
  $("#hide") && ($("#hide").onclick = () => { invoke("set_widget_open", { id: w, open: false }); invoke("close_self"); });
  $("#wp") && ($("#wp").onclick = () => invoke("open_widgets_panel"));
  $("#rst") && ($("#rst").onclick = () => { delete S.widgetAppearance?.[w]; delete S.widgetOpacity?.[w]; delete S.widgetFrosted?.[w]; saveSettings(true); render(); });
  $("#done").onclick = () => invoke("close_self");
}

// ----------------------------------------------------------------- boot --

(async function main() {
  const b = await invoke("boot", { widget: PANEL ? "settings" : WID });
  S = b.settings; THEMES = b.themes; WIDGETS = b.widgets; DESIGNS = b.designs;
  buildControls();
  render();
  loadWallpaper();
  setInterval(loadWallpaper, 30000);
  win.onMoved(() => placeWall());
  // Tell Opal the widget is in use, so it stays in front until left alone.
  if (!PANEL) {
    // Clicks, scrolls and keys bring the widget forward; moving the pointer
    // over it only keeps it there.
    let last = 0;
    const touch = e => {
      const t = Date.now(), raise = e.type !== "pointermove";
      if (raise || t - last > 1000) { last = t; invoke("widget_touch", { raise }).catch(() => {}); }
    };
    for (const ev of ["pointerdown", "pointermove", "wheel", "keydown"]) addEventListener(ev, touch, { passive: true, capture: true });
  }
  let rt = 0;
  win.onResized(() => { placeWall(); clearTimeout(rt); if (["system", "clock"].includes(WID) && !PANEL) rt = setTimeout(render, 200); });
  await listen("settings-changed", e => {
    const before = JSON.stringify(S);
    S = e.payload;
    if (PANEL === "settings") { applyGlass(); return; }  // don't rebuild the form under the cursor
    if (before !== JSON.stringify(S)) render();
  });
  await listen("extras-changed", () => { if (["weather", "system", "pomodoro", "photos", "lunar"].includes(WID) && !PANEL) render(); });
  if (WID === "launcher" && !PANEL) {
    T.webview.getCurrentWebview().onDragDropEvent(e => {
      const p = e.payload;
      let ov = $("#drop");
      if (p.type === "over" || p.type === "enter") {
        if (!ov) { ov = document.createElement("div"); ov.id = "drop"; ov.className = "drop-overlay"; ov.textContent = "Drop to add to Opal"; plate.appendChild(ov); }
      } else {
        ov?.remove();
        if (p.type === "drop" && p.paths?.length) invoke("add_paths", { paths: p.paths });
      }
    });
  }
})().catch(e => { document.body.insertAdjacentHTML("beforeend", `<pre style="color:#f88;position:fixed;inset:10px;white-space:pre-wrap;z-index:99">${esc(e && e.stack || e)}</pre>`); });
