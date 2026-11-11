(() => {
  "use strict";

  const invoke = window.__TAURI__.core.invoke;
  const $ = (sel, root = document) => root.querySelector(sel);

  const state = {
    settings: null,
    info: null,
    models: [],
    jobs: [],
    selectedId: null,
    draft: null,
    jobsSig: "",
    modelsSig: "",
    previewSig: "",
    toastTimer: null,
    modalOpen: false,
    lexicon: [],
    wordbank: [],
    learning: null,
    topicKind: "tractate",
    topic: {
      id: "",
      label: "",
      he: "",
      kind: "",
      terms: [],
      technical: [],
      context: "",
      daf: "",
      manual: false,
    },
    topicDraft: null,
    audioJobId: null,
    follow: true,
    uncertainTarget: null,
  };

  /* ---------- utils ---------- */

  const escapeHtml = (s) =>
    String(s ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");

  function fmtDuration(sec) {
    const total = Math.max(0, Math.round(sec || 0));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    if (h > 0) return `${h} h ${String(m).padStart(2, "0")}`;
    if (m > 0) return `${m} min ${String(s).padStart(2, "0")}`;
    return `${s} s`;
  }

  function fmtStamp(sec) {
    const total = Math.max(0, Math.round(sec || 0));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    return h > 0
      ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
      : `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }

  function fmtSize(bytes) {
    if (!bytes) return "—";
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1).replace(".", ",")} Go`;
    return `${Math.round(bytes / 1e6)} Mo`;
  }

  function titleOf(name) {
    const base = String(name || "").replace(/\.[^.]+$/, "");
    let cleaned = base.replace(/[_-]+/g, " ").replace(/\s+/g, " ").trim();
    if (!cleaned) return "Transcription";
    return cleaned.replace(/^[a-z]/, (c) => c.toUpperCase());
  }

  function baseName(path) {
    return String(path || "").split(/[\\/]/).pop() || path;
  }

  function isRtl(text) {
    const he = (text.match(/[\u0590-\u05FF\uFB1D-\uFB4F]/g) || []).length;
    const latin = (text.match(/[A-Za-z]/g) || []).length;
    return he > 0 && he >= (he + latin) * 0.3;
  }

  function langLabel(code, short = false) {
    const map = {
      he: ["Hébreu / araméen", "Hébreu"],
      fr: ["Français", "Français"],
      en: ["Anglais", "Anglais"],
      yi: ["Yiddish", "Yiddish"],
      ar: ["Arabe", "Arabe"],
      es: ["Espagnol", "Espagnol"],
      de: ["Allemand", "Allemand"],
      it: ["Italien", "Italien"],
      pt: ["Portugais", "Portugais"],
      ru: ["Russe", "Russe"],
      auto: ["Détection automatique", "Auto"],
    };
    return (map[code] || ["Langue", "Langue"])[short ? 1 : 0];
  }

  function toast(message, ms = 3200) {
    const t = $("#toast");
    t.textContent = message;
    t.hidden = false;
    clearTimeout(state.toastTimer);
    state.toastTimer = setTimeout(() => {
      t.hidden = true;
    }, ms);
  }

  function modelById(id) {
    return state.models.find((m) => m.id === id);
  }

  /* ---------- lexique & sujet ---------- */

  function normKey(value) {
    return String(value || "")
      .normalize("NFKD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase()
      .replace(/[^\p{L}\p{N}]+/gu, " ")
      .trim();
  }

  function lexEntry(id) {
    return (state.lexicon || []).find((e) => e.id === id);
  }

  function lexByHe(he) {
    return (state.lexicon || []).find((e) => e.he === he);
  }

  function termLabel(term) {
    const e = lexByHe(term);
    return e ? e.fr : term;
  }

  function topicCoreTerms(entry) {
    if (entry.kind === "tractate") {
      return ["גמרא", "רש״י", "תוספות", "סוגיא", "קושיא", "תירוץ"];
    }
    if (entry.kind === "parsha") {
      return ["פרשת השבוע", "פירוש רש״י", "מדרש"];
    }
    return ["שולחן ערוך", "רמ״א", "הלכה"];
  }

  function buildTopicTerms(entry) {
    const terms = [entry.he, ...(entry.related || []), ...topicCoreTerms(entry)];
    return [...new Set(terms.filter(Boolean))].slice(0, 34);
  }

  function applyTopicToDraft(entry, manual = true) {
    if (!state.topicDraft) return;
    state.topicDraft.id = entry.id;
    state.topicDraft.label = entry.fr;
    state.topicDraft.he = entry.he;
    state.topicDraft.kind = entry.kind;
    state.topicDraft.terms = buildTopicTerms(entry);
    state.topicDraft.manual = manual;
    state.topicKind = entry.kind;
  }

  function detectTopic(fileName) {
    const raw = String(fileName || "").replace(/\.[^.]+$/, "");
    const hay = " " + normKey(raw) + " ";
    let best = null;
    let bestLen = 0;
    for (const entry of state.lexicon || []) {
      if (!["tractate", "parsha", "halakha"].includes(entry.kind)) continue;
      const candidates = [entry.he, entry.fr, ...(entry.aliases || [])];
      for (const c of candidates) {
        const needle = normKey(c);
        if (needle.length < 3) continue;
        if (hay.includes(" " + needle + " ") || hay.includes(" " + needle)) {
          if (needle.length > bestLen) {
            best = entry;
            bestLen = needle.length;
          }
        }
      }
    }
    let daf = "";
    const m =
      raw.match(/(?:daf|page|דף)\s*([0-9]{1,3})\s*([ab])?/i) ||
      raw.match(/(?:^|[^0-9])([0-9]{1,3})\s*([ab])(?:[^0-9]|$)/i);
    if (m) daf = `${m[1]}${m[2] ? m[2].toLowerCase() : ""}`;
    return best ? { entry: best, daf } : null;
  }

  function renderTopicPill() {
    const name = $("#topicName");
    const badge = $("#topicTermsBadge");
    if (!name) return;
    if (state.topic.id) {
      name.textContent = state.topic.he
        ? `${state.topic.he} · ${state.topic.label}`
        : state.topic.label;
      name.title = name.textContent;
      name.dir = isRtl(state.topic.he) ? "rtl" : "ltr";
      badge.textContent = state.topic.daf
        ? `daf ${state.topic.daf} · ${state.topic.terms.length} termes`
        : `${state.topic.terms.length} termes`;
    } else {
      name.textContent = "Détection automatique";
      name.dir = "ltr";
      badge.textContent = "";
    }
  }

  function slugify(value) {
    return normKey(value).replace(/\s+/g, "-");
  }

  async function loadLexicon() {
    try {
      const [lexRes, bankRes] = await Promise.all([fetch("lexicon.json"), fetch("wordbank.json")]);
      const lex = await lexRes.json();
      const bank = await bankRes.json();
      state.lexicon = lex.entries || [];
      state.wordbank = bank.entries || [];
    } catch (err) {
      console.warn("lexique indisponible", err);
      state.lexicon = [];
      state.wordbank = [];
    }
  }

  /* ---------- jobs ---------- */

  function pickDefault(jobs) {
    if (!jobs.length) return null;
    const active = jobs.find((j) => j.status === "working" || j.status === "queued");
    if (active) return active.id;
    return jobs[jobs.length - 1].id;
  }

  async function refreshJobs(force = false) {
    try {
      const jobs = await invoke("get_jobs");
      const sig = JSON.stringify(
        jobs.map((j) => [
          j.id,
          j.status,
          Math.round(j.progress * 10),
          j.stage,
          j.paragraphs ? j.paragraphs.length : 0,
          j.outputs ? j.outputs.docx : "",
          j.words,
          j.error,
        ])
      );
      if (!force && sig === state.jobsSig) return;
      state.jobsSig = sig;
      state.jobs = jobs;
      if (!state.selectedId || !jobs.some((j) => j.id === state.selectedId)) {
        state.selectedId = pickDefault(jobs);
      }
      renderQueue();
      renderPreview();
      renderModelChip();
      renderBanner();
    } catch (err) {
      console.warn("get_jobs", err);
    }
  }

  async function refreshLearning() {
    try {
      state.learning = await invoke("get_learning");
      renderLearning();
    } catch (err) {
      console.warn("get_learning", err);
    }
  }

  function renderLearning() {
    const root = $("#learningSummary");
    if (!root) return;
    const learning = state.learning || { corrections: [], terms: [] };
    const count = learning.corrections?.length || 0;
    if (!count) {
      root.innerHTML = "Aucune correction apprise pour l'instant.";
      return;
    }
    const items = learning.corrections
      .slice()
      .sort((a, b) => (b.count || 0) - (a.count || 0))
      .slice(0, 4)
      .map(
        (c) =>
          `<li><span class="pair">${escapeHtml(c.from)} → ${escapeHtml(c.to)}</span><span>${c.count}×</span></li>`
      )
      .join("");
    root.innerHTML = `<strong>${count} correction${count > 1 ? "s" : ""}</strong> apprise${count > 1 ? "s" : ""} · ${learning.terms?.length || 0} termes mémorisés.<ul class="learning-list">${items}</ul>`;
  }

  async function refreshModels() {
    try {
      const models = await invoke("get_models");
      const sig = JSON.stringify(
        models.map((m) => [m.id, m.installed, m.downloading, Math.round(m.downloaded / 2e6), m.error])
      );
      if (sig === state.modelsSig) return;
      state.modelsSig = sig;
      state.models = models;
      renderModelChip();
      renderBanner();
      if (state.modalOpen) renderModels();
    } catch (err) {
      console.warn("get_models", err);
    }
  }

  function statusLabel(job) {
    switch (job.status) {
      case "queued":
        return "En attente";
      case "working":
        return job.stage || "Transcription…";
      case "done":
        return `${job.words} mots · prêt`;
      case "cancelled":
        return "Annulé";
      case "error":
        return job.error || "Erreur";
      default:
        return job.status;
    }
  }

  function jobIcon(job) {
    if (job.status === "done") {
      return `<svg viewBox="0 0 20 20" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 10.5l3.6 3.6L16 5.6"/></svg>`;
    }
    if (job.status === "error") {
      return `<svg viewBox="0 0 20 20" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M10 5v6"/><circle cx="10" cy="14.6" r="0.9" fill="currentColor" stroke="none"/></svg>`;
    }
    if (job.status === "working") {
      return `<svg viewBox="0 0 20 20" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M10 3v3M10 14v3M3 10h3M14 10h3M5 5l2 2M13 13l2 2M15 5l-2 2M7 13l-2 2"/></svg>`;
    }
    return `<svg viewBox="0 0 20 20" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M5 3.5h6l4 4v9a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1Z"/><path d="M11 3.5v4h4"/></svg>`;
  }

  function renderQueue() {
    const queue = $("#queue");
    const empty = $("#queueEmpty");
    $("#queueCount").textContent = String(state.jobs.length);
    if (!state.jobs.length) {
      queue.innerHTML = `<div class="queue-empty" id="queueEmpty">Aucun fichier pour l'instant. Vos transcriptions s'afficheront au fil de l'eau.</div>`;
      return;
    }
    queue.innerHTML = state.jobs
      .map((job) => {
        const active = job.id === state.selectedId ? " active" : "";
        const pct = Math.max(0, Math.min(100, job.progress || 0));
        const canCancel = job.status === "working" || job.status === "queued";
        const stage =
          job.status === "error"
            ? `<span class="err" title="${escapeHtml(job.error || "")}">${escapeHtml(statusLabel(job))}</span>`
            : `<span>${escapeHtml(statusLabel(job))}</span>${
                job.status === "working" ? `<span>· ${Math.round(pct)} %</span>` : ""
              }`;
        return `
        <div class="job ${job.status}${active}" data-id="${job.id}" role="button" tabindex="0">
          <div class="job-icon">${jobIcon(job)}</div>
          <div class="job-main">
            <div class="job-title">
              <span class="job-name" dir="auto" title="${escapeHtml(job.name)}">${escapeHtml(job.name)}</span>
              ${job.duration ? `<span class="job-dur">${fmtDuration(job.duration)}</span>` : ""}
            </div>
            <div class="progress"><i style="width:${pct}%"></i></div>
            <div class="job-stage">${stage}</div>
          </div>
          ${
            canCancel
              ? `<button class="job-x" data-cancel="${job.id}" type="button" title="Annuler" aria-label="Annuler">×</button>`
              : ""
          }
        </div>`;
      })
      .join("");
  }

  function renderPreview() {
    const preview = $("#preview");
    if (!state.jobs.length) {
      state.previewSig = "empty";
      preview.innerHTML = `
        <div class="empty">
          <div class="aleph" aria-hidden="true">א</div>
          <h2>Le texte de votre cours, posé sur la page.</h2>
          <p>Déposez un enregistrement : Plume le transcrit, reformate les paragraphes et prépare un document Word prêt à annoter.</p>
          <div class="empty-features">
            <span class="pill">Word mis en page</span>
            <span class="pill">Hébreu &amp; araméen RTL</span>
            <span class="pill">100 % hors ligne</span>
          </div>
        </div>`;
      return;
    }

    const job = state.jobs.find((j) => j.id === state.selectedId) || state.jobs[state.jobs.length - 1];
    if (!job) return;
    if (job.status !== "done" || !job.playback) {
      const player = $("#audioEl");
      if (player && !player.paused) player.pause();
    }
    const sig = `${job.id}|${job.status}|${job.paragraphs?.length}|${job.words}|${job.timestamps}|${
      job.outputs?.docx || ""
    }|${Math.round(job.progress)}`;
    if (sig === state.previewSig) return;
    state.previewSig = sig;

    if (job.status === "queued" || job.status === "working") {
      preview.innerHTML = `
        <div class="processing">
          <div class="kicker">Transcription en cours</div>
          <h2 dir="auto">${escapeHtml(job.name)}</h2>
          <div class="stage">${escapeHtml(job.stage || "Préparation…")}</div>
          <div class="big-progress"><span class="num">${Math.round(job.progress || 0)}</span><span class="pct">%</span></div>
          <div class="progress"><i style="width:${Math.max(2, job.progress || 0)}%"></i></div>
          <div class="proc-actions"><button class="soft-btn" data-act="cancel" data-id="${job.id}" type="button">Annuler</button></div>
          <div class="hint">Whisper travaille sur votre machine. Les fichiers longs peuvent prendre plusieurs minutes ; vous pouvez continuer à utiliser la fenêtre.</div>
        </div>`;
      return;
    }

    if (job.status === "error" || job.status === "cancelled") {
      const cancelled = job.status === "cancelled";
      preview.innerHTML = `
        <div class="error-card">
          <div class="error-badge">${cancelled ? "—" : "!"}</div>
          <h2>${cancelled ? "Transcription annulée" : "La transcription n'a pas abouti"}</h2>
          <p dir="auto">${escapeHtml(cancelled ? "Vous pouvez relancer ce fichier quand vous voulez." : job.error || "Erreur inconnue")}</p>
          <div class="actions">
            <button class="primary small" data-act="retry" data-id="${job.id}" type="button">Réessayer</button>
            <button class="soft-btn" data-act="settings" type="button">Réglages</button>
          </div>
        </div>`;
      return;
    }

    // Terminé
    const paras = job.paragraphs || [];
    const uncertainCount = paras.reduce((n, p) => n + (p.uncertain?.length || 0), 0);
    const transcript = paras.length
      ? paras
          .map((p, index) => {
            const rtl = isRtl(p.text);
            const ts = job.timestamps ? `<span class="ts">[${fmtStamp(p.start)}]</span>` : "";
            return `<p dir="auto" class="${rtl ? "rtl" : ""}" data-start="${p.start}" data-end="${p.end || p.start}" data-index="${index}">${ts}${renderParagraphHtml(p)}</p>`;
          })
          .join("")
      : `<p dir="auto">Aucune parole détectée dans ce fichier.</p>`;
    const docx = job.outputs?.docx;
    const canPlay = !!job.playback;

    preview.innerHTML = `
      <div class="result">
        <div class="result-head">
          <div class="eyebrow">Transcription terminée</div>
          <div class="result-title-row">
            <h2 dir="auto">${escapeHtml(titleOf(job.name))}</h2>
          </div>
          <div class="meta">
            <span class="tag">${fmtDuration(job.duration)}</span>
            <span class="tag">${job.words} mots</span>
            <span class="tag lang">${escapeHtml(langLabel(job.language, true))}</span>
            <span class="tag">${escapeHtml((modelById(job.model) || {}).name || job.model)}</span>
            ${job.topic ? `<span class="tag">${escapeHtml(job.topic)}</span>` : ""}
            ${uncertainCount ? `<span class="tag" style="color:#a33b32;border-color:#ecd0c9;background:#fdf0ed">${uncertainCount} à vérifier</span>` : ""}
          </div>
          <div class="actions">
            <button class="primary" data-act="export" data-fmt="docx" data-id="${job.id}" type="button">Télécharger Word</button>
            <button data-act="export" data-fmt="txt" data-id="${job.id}" type="button">TXT</button>
            <button data-act="export" data-fmt="md" data-id="${job.id}" type="button">Markdown</button>
            <button data-act="export" data-fmt="srt" data-id="${job.id}" type="button">SRT</button>
            <button class="icon-action" data-act="copy" data-id="${job.id}" type="button">
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><rect x="9" y="9" width="11" height="12" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h9"/></svg>
              Copier
            </button>
            <button class="icon-action" data-act="reveal" data-id="${job.id}" type="button">
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H10l2 2h6.5A2.5 2.5 0 0 1 21 9.5v8A2.5 2.5 0 0 1 18.5 20h-13A2.5 2.5 0 0 1 3 17.5Z"/></svg>
              Finder
            </button>
          </div>
        </div>
        ${
          canPlay
            ? `<div class="player" data-job="${job.id}">
                <button class="play-btn" data-act="play" type="button" aria-label="Lecture">${playIcon(false)}</button>
                <div class="skip-group">
                  <button class="skip-btn" data-act="back10" type="button" title="Reculer de 10 secondes">−10</button>
                  <button class="skip-btn" data-act="fwd10" type="button" title="Avancer de 10 secondes">+10</button>
                </div>
                <div class="seek-block">
                  <input class="seek" id="seek" type="range" min="0" max="${job.duration || 0}" step="0.1" value="0" aria-label="Position dans l'audio" />
                  <div class="times"><span id="curTime">00:00</span><span id="durTime">${fmtStamp(job.duration)}</span></div>
                </div>
                <label class="follow"><input type="checkbox" id="followToggle" ${state.follow ? "checked" : ""}/> Suivre</label>
              </div>`
            : ""
        }
        <div class="transcript-wrap"><div class="transcript">${transcript}</div></div>
        <div class="result-foot">
          <span title="${escapeHtml(docx || "")}">${docx ? "Word et texte enregistrés dans " + escapeHtml(baseName(docx)) : "Export disponible ci-dessus"}</span>
          <button data-act="openout" type="button">Ouvrir le dossier</button>
        </div>
      </div>`;
    setupPlayer(job);
  }

  function renderParagraphHtml(p) {
    const flags = (p.uncertain || []).slice().sort((a, b) => a.offset - b.offset);
    if (!flags.length) return escapeHtml(p.text);
    const chars = Array.from(p.text);
    let out = "";
    let cursor = 0;
    for (const flag of flags) {
      if (flag.offset < cursor || flag.offset + flag.len > chars.length) continue;
      out += escapeHtml(chars.slice(cursor, flag.offset).join(""));
      const word = chars.slice(flag.offset, flag.offset + flag.len).join("");
      const tip = flag.suggestion
        ? `Suggestion : ${flag.suggestion}`
        : "Terme peu sûr — vérifiez l'audio";
      out += `<span class="uncertain" data-uncertain="1" data-word="${escapeHtml(word)}" data-sugg="${escapeHtml(flag.suggestion || "")}" title="${escapeHtml(tip)}">${escapeHtml(word)}</span>`;
      cursor = flag.offset + flag.len;
    }
    out += escapeHtml(chars.slice(cursor).join(""));
    return out;
  }

  function playIcon(playing) {
    return playing
      ? `<svg viewBox="0 0 20 20" width="15" height="15" fill="currentColor"><rect x="5" y="4" width="3.4" height="12" rx="1"/><rect x="11.6" y="4" width="3.4" height="12" rx="1"/></svg>`
      : `<svg viewBox="0 0 20 20" width="15" height="15" fill="currentColor" style="margin-left:2px"><path d="M6 4.2v11.6a.8.8 0 0 0 1.2.7l9-5.8a.8.8 0 0 0 0-1.4l-9-5.8A.8.8 0 0 0 6 4.2Z"/></svg>`;
  }

  function currentAudioJob() {
    return state.jobs.find((j) => j.id === state.audioJobId);
  }

  function setupPlayer(job) {
    const audio = $("#audioEl");
    if (!job.playback) {
      audio.pause();
      audio.removeAttribute("src");
      state.audioJobId = null;
      return;
    }
    if (state.audioJobId !== job.id) {
      audio.pause();
      state.audioJobId = job.id;
      try {
        audio.src = window.__TAURI__.core.convertFileSrc(job.playback);
      } catch (err) {
        audio.src = job.playback;
      }
      audio.load();
    }
    const seek = $("#seek");
    if (seek) {
      seek.max = job.duration || 0;
      seek.value = audio.currentTime || 0;
    }
    updatePlayerUi();
    highlightAt(audio.currentTime || 0, false);
  }

  function updatePlayerUi() {
    const job = currentAudioJob();
    const audio = $("#audioEl");
    if (!job) return;
    const seek = $("#seek");
    const cur = audio.currentTime || 0;
    const dur = Number.isFinite(audio.duration) && audio.duration > 0 ? audio.duration : job.duration || 0;
    if (seek) {
      seek.max = dur;
      if (!seek.matches(":active")) seek.value = cur;
      const pct = dur ? (cur / dur) * 100 : 0;
      seek.style.setProperty("--p", `${pct}%`);
    }
    const curEl = $("#curTime");
    const durEl = $("#durTime");
    if (curEl) curEl.textContent = fmtStamp(cur);
    if (durEl) durEl.textContent = fmtStamp(dur);
    const btn = document.querySelector('.play-btn[data-act="play"]');
    if (btn) btn.innerHTML = playIcon(!audio.paused);
  }

  function highlightAt(time, scroll = true) {
    const paras = [...document.querySelectorAll("#preview .transcript p[data-start]")];
    if (!paras.length) return;
    let active = paras[0];
    for (const p of paras) {
      if (parseFloat(p.dataset.start) <= time + 0.18) active = p;
      else break;
    }
    if (!active) return;
    const changed = !active.classList.contains("active");
    paras.forEach((p) => p.classList.toggle("active", p === active));
    if (changed && scroll && state.follow) {
      const wrap = document.querySelector("#preview .transcript-wrap");
      if (!wrap) return;
      const wr = wrap.getBoundingClientRect();
      const pr = active.getBoundingClientRect();
      const top = wrap.scrollTop + (pr.top - wr.top) - wrap.clientHeight / 2 + pr.height / 2;
      wrap.scrollTo({ top: Math.max(0, top), behavior: "smooth" });
    }
  }

  function seekTo(time) {
    const audio = $("#audioEl");
    if (!state.audioJobId) return;
    audio.currentTime = Math.max(0, Math.min(time, audio.duration || time));
    updatePlayerUi();
    highlightAt(audio.currentTime);
  }

  function showUncertainPop(span, job) {
    const word = span.dataset.word || span.textContent;
    const suggestion = span.dataset.sugg || "";
    const pop = $("#uncertainPop");
    state.uncertainTarget = { jobId: job.id, word, suggestion };
    $("#uncertainWord").textContent = word;
    const apply = $("#uncertainApply");
    if (suggestion) {
      apply.hidden = false;
      apply.innerHTML = `Remplacer par <strong>${escapeHtml(suggestion)}</strong> <span style="color:#a3947f">· apprendre</span>`;
    } else {
      apply.hidden = true;
    }
    $("#uncertainHint").textContent = suggestion
      ? "La correction est appliquée au texte, au Word, et apprise pour les prochaines transcriptions."
      : "Ce mot a été transcrit avec peu de confiance. Écoutez le passage pour vérifier.";
    const rect = span.getBoundingClientRect();
    pop.hidden = false;
    const width = 226;
    let left = rect.left + rect.width / 2 - width / 2;
    left = Math.max(12, Math.min(left, window.innerWidth - width - 12));
    let top = rect.bottom + 8;
    if (top + pop.offsetHeight > window.innerHeight - 12) top = rect.top - pop.offsetHeight - 8;
    pop.style.left = `${left}px`;
    pop.style.top = `${Math.max(12, top)}px`;
  }

  function hideUncertainPop() {
    $("#uncertainPop").hidden = true;
    state.uncertainTarget = null;
  }

  async function applyUncertain() {
    const target = state.uncertainTarget;
    if (!target || !target.suggestion) return;
    hideUncertainPop();
    try {
      const updated = await invoke("learn_correction", {
        id: target.jobId,
        from: target.word,
        to: target.suggestion,
      });
      const idx = state.jobs.findIndex((j) => j.id === updated.id);
      if (idx >= 0) state.jobs[idx] = updated;
      state.previewSig = "";
      renderQueue();
      renderPreview();
      refreshLearning();
      toast(`Correction apprise : ${target.word} → ${target.suggestion}`);
    } catch (err) {
      toast(String(err));
    }
  }

  function renderModelChip() {
    if (!state.settings) return;
    const m = modelById(state.settings.model);
    const installed = m ? m.installed : false;
    $("#modelChipText").textContent = installed ? m.name : "Modèle à installer";
    $("#modelDot").classList.toggle("warn", !installed);
  }

  function renderBanner() {
    const banner = $("#modelBanner");
    if (!state.settings) return;
    const m = modelById(state.settings.model);
    if (!m || m.installed) {
      banner.hidden = true;
      return;
    }
    banner.hidden = false;
    const pct = m.downloading && m.downloaded ? Math.round((m.downloaded / (m.size || 1)) * 100) : 0;
    $("#bannerTitle").textContent = m.downloading
      ? `Téléchargement de ${m.name} — ${pct} %`
      : `Le modèle ${m.name} n'est pas encore installé`;
    $("#bannerText").textContent = m.downloading
      ? `${fmtSize(m.downloaded)} sur ${fmtSize(m.size)}. La transcription démarrera dès qu'il sera prêt.`
      : `Téléchargez-le une fois (${fmtSize(m.size)}) pour transcrire vos cours hors ligne.`;
    const btn = $("#bannerBtn");
    btn.disabled = !!m.downloading;
    btn.textContent = m.downloading ? `${pct} %` : "Télécharger";
  }

  /* ---------- sujet du cours ---------- */

  function openTopic() {
    state.topicDraft = JSON.parse(JSON.stringify(state.topic));
    state.topicKind = state.topicDraft.kind || "tractate";
    $("#topicModal").hidden = false;
    $("#topicSearch").value = "";
    $("#topicDaf").value = state.topicDraft.daf || "";
    $("#topicContext").value = state.topicDraft.context || "";
    renderTopicTabs();
    renderTopicList();
    renderTopicSelected();
    renderTechChips();
    renderTechDatalist();
  }

  function closeTopic() {
    $("#topicModal").hidden = true;
    state.topicDraft = null;
  }

  function renderTopicTabs() {
    document.querySelectorAll("#topicTabs button").forEach((btn) => {
      btn.classList.toggle("active", btn.dataset.kind === state.topicKind);
    });
  }

  function renderTopicList() {
    const root = $("#topicList");
    if (!root || !state.topicDraft) return;
    const query = normKey($("#topicSearch").value);
    if (state.topicKind === "lexeme") {
      if (query.length < 2) {
        root.innerHTML = `<p class="field-note" style="grid-column:1/-1">Tapez au moins deux lettres pour chercher dans les ${state.wordbank.length} mots du dictionnaire (hébreu biblique et talmudique). Un clic ajoute le mot aux termes techniques.</p>`;
        return;
      }
      const found = (state.wordbank || [])
        .filter((e) => {
          const hay = normKey([e.he, e.tr, e.gloss].join(" "));
          return hay.includes(query);
        })
        .slice(0, 60);
      root.innerHTML = found
        .map(
          (e) => `
        <button class="topic-item" data-word="${escapeHtml(e.he)}" type="button" title="${escapeHtml(e.gloss || "")}">
          <span class="he" dir="rtl">${escapeHtml(e.he)}</span>
          <span class="fr">${escapeHtml(e.tr || e.src || "")}</span>
        </button>`
        )
        .join("");
      return;
    }
    const items = (state.lexicon || []).filter((e) => {
      if (e.kind !== state.topicKind) return false;
      if (!query) return true;
      const hay = normKey([e.he, e.fr, ...(e.aliases || [])].join(" "));
      return hay.includes(query);
    });
    root.innerHTML = items
      .map(
        (e) => `
        <button class="topic-item${state.topicDraft.id === e.id ? " selected" : ""}" data-topic-id="${e.id}" type="button">
          <span class="he" dir="rtl">${escapeHtml(e.he)}</span>
          <span class="fr">${escapeHtml(e.fr)}</span>
        </button>`
      )
      .join("");
  }

  function renderTopicSelected() {
    const draft = state.topicDraft;
    if (!draft) return;
    $("#topicSelectedName").textContent = draft.id
      ? `${draft.he} · ${draft.label}`
      : "Aucun — détection au moment de l'import";
    $("#topicSelectedName").dir = draft.he && isRtl(draft.he) ? "rtl" : "ltr";
    const chips = draft.terms
      .map(
        (term) => `
        <span class="chip-term">
          <span class="he" dir="rtl">${escapeHtml(term)}</span>
          <button data-term-remove="${escapeHtml(term)}" type="button" title="Retirer">×</button>
        </span>`
      )
      .join("");
    $("#topicTermChips").innerHTML =
      chips || `<span class="field-note">Aucun terme prédéfini pour ce sujet.</span>`;
  }

  function renderTechChips() {
    const draft = state.topicDraft;
    if (!draft) return;
    $("#techChips").innerHTML =
      draft.technical
        .map(
          (term) => `
        <span class="chip-term">
          <span class="he" dir="rtl">${escapeHtml(term)}</span>
          <button data-tech-remove="${escapeHtml(term)}" type="button" title="Retirer">×</button>
        </span>`
        )
        .join("") || "";
  }

  function renderTechDatalist() {
    const dl = $("#techDatalist");
    if (!dl) return;
    dl.innerHTML = (state.lexicon || [])
      .slice(0, 320)
      .map((e) => `<option value="${escapeHtml(e.he)}">${escapeHtml(e.fr)}</option>`)
      .join("");
  }

  function addTechTerm(raw) {
    if (!state.topicDraft) return;
    let value = String(raw || "").trim();
    if (!value) return;
    const entry =
      lexByHe(value) ||
      (state.lexicon || []).find((e) => normKey(e.fr) === normKey(value));
    if (entry) value = entry.he;
    if (!state.topicDraft.technical.includes(value)) {
      state.topicDraft.technical.push(value);
    }
    renderTechChips();
  }

  async function applyTopic() {
    if (!state.topicDraft) return;
    state.topicDraft.daf = $("#topicDaf").value.trim();
    state.topicDraft.context = $("#topicContext").value;
    state.topicDraft.manual = true;
    state.topic = JSON.parse(JSON.stringify(state.topicDraft));
    closeTopic();
    renderTopicPill();
    if (state.settings) {
      state.settings.topicId = state.topic.id;
      state.settings.customPrompt = state.topic.context;
      state.settings.technicalTerms = state.topic.technical;
      try {
        await invoke("save_settings", { next: state.settings });
      } catch (err) {
        console.warn("save topic", err);
      }
    }
    toast(
      state.topic.id
        ? `Sujet appliqué : ${state.topic.label} · ${state.topic.terms.length} termes`
        : "Sujet réinitialisé."
    );
  }

  function resetTopicDraft() {
    state.topicDraft = {
      id: "",
      label: "",
      he: "",
      kind: "",
      terms: [],
      technical: [],
      context: "",
      daf: "",
      manual: false,
    };
    state.topicKind = "tractate";
    $("#topicDaf").value = "";
    $("#topicContext").value = "";
    renderTopicTabs();
    renderTopicList();
    renderTopicSelected();
    renderTechChips();
  }

  /* ---------- actions ---------- */

  async function pickFiles() {
    try {
      const paths = await invoke("pick_files");
      if (paths && paths.length) await enqueue(paths);
    } catch (err) {
      toast(String(err));
    }
  }

  function autoDetectTopic(paths) {
    if (!paths || !paths.length || state.topic.manual) return;
    const name = baseName(paths[0]);
    const found = detectTopic(name);
    if (!found) return;
    state.topic.id = found.entry.id;
    state.topic.label = found.entry.fr;
    state.topic.he = found.entry.he;
    state.topic.kind = found.entry.kind;
    state.topic.terms = buildTopicTerms(found.entry);
    state.topic.daf = found.daf || "";
    state.topic.manual = false;
    renderTopicPill();
    setTimeout(
      () =>
        toast(
          `Sujet détecté : ${found.entry.fr} · ${state.topic.terms.length} termes préparés`,
          3600
        ),
      400
    );
  }

  async function enqueue(paths) {
    if (!paths || !paths.length) return;
    autoDetectTopic(paths);
    const label = state.topic.id
      ? `${state.topic.he} · ${state.topic.label}${state.topic.daf ? " · daf " + state.topic.daf : ""}`
      : "";
    const context = [state.topic.context, state.topic.daf ? `Daf ${state.topic.daf}` : ""]
      .filter(Boolean)
      .join(". ");
    try {
      await invoke("enqueue_files", {
        paths,
        overrides: {
          topicTerms: state.topic.terms,
          topic: label,
          technicalTerms: state.topic.technical,
          context,
        },
      });
      await refreshJobs(true);
    } catch (err) {
      const msg = String(err);
      if (msg.startsWith("MODEL_MISSING:")) {
        toast("Téléchargez d'abord un modèle de transcription.");
        openSettings();
      } else {
        toast(msg);
      }
    }
  }

  async function cancelJob(id) {
    try {
      await invoke("cancel_job", { id });
      await refreshJobs(true);
    } catch (err) {
      toast(String(err));
    }
  }

  async function exportJob(id, fmt) {
    try {
      const path = await invoke("export_job", { id, format: fmt });
      toast(`Enregistré : ${baseName(path)}`);
    } catch (err) {
      toast(String(err));
    }
  }

  async function copyJob(job) {
    const text = (job.paragraphs || [])
      .map((p) => (job.timestamps ? `[${fmtStamp(p.start)}] ${p.text}` : p.text))
      .join("\n\n");
    try {
      await navigator.clipboard.writeText(text);
      toast("Transcription copiée.");
    } catch {
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
      toast("Transcription copiée.");
    }
  }

  function currentJob() {
    return state.jobs.find((j) => j.id === state.selectedId);
  }

  /* ---------- settings ---------- */

  function openSettings() {
    if (!state.settings) return;
    state.draft = JSON.parse(JSON.stringify(state.settings));
    state.modalOpen = true;
    $("#settingsModal").hidden = false;
    renderSettingsFields();
    renderModels();
    refreshModels();
    refreshLearning();
    $("#saveState").textContent = "";
  }

  function closeSettings() {
    state.modalOpen = false;
    $("#settingsModal").hidden = true;
    state.draft = null;
  }

  function renderSettingsFields() {
    if (!state.draft) return;
    $("#language").value = state.draft.language;
    $("#preset").value = state.draft.preset;
    $("#glossary").value = (state.draft.glossary || []).join("\n");
    $("#corrections").value = (state.draft.corrections || [])
      .map((c) => `${c.from} = ${c.to}`)
      .join("\n");
    $("#customPrompt").value = state.draft.customPrompt || "";
    $("#normalizeQuotes").checked = !!state.draft.normalizeQuotes;
    $("#timestamps").checked = !!state.draft.timestamps;
    $("#outDir").textContent = state.draft.outputDir || "—";
    $("#outDir").title = state.draft.outputDir || "";
  }

  function renderModels() {
    const root = $("#modelList");
    if (!root || !state.draft) return;
    root.innerHTML = state.models
      .map((m) => {
        const selected = state.draft.model === m.id;
        const pct = m.downloading ? Math.min(100, Math.round((m.downloaded / (m.size || 1)) * 100)) : 0;
        let side = "";
        if (m.id === "custom") {
          side = m.installed
            ? `<span class="model-installed">✓ importé</span>`
            : `<button class="model-dl" data-import="1" type="button">Importer…</button>`;
        } else if (m.installed) {
          side = `<span class="model-installed">✓ installé</span><span class="model-size">${fmtSize(m.size)}</span>`;
        } else if (m.downloading) {
          side = `<span class="model-size">${pct} %</span>`;
        } else {
          side = `<span class="model-size">${fmtSize(m.size)}</span><button class="model-dl" data-dl="${m.id}" type="button">Télécharger</button>`;
        }
        return `
          <div class="model${selected ? " selected" : ""}" data-model="${m.id}" role="radio" aria-checked="${selected}" tabindex="0">
            <span class="radio" aria-hidden="true"></span>
            <div class="model-text">
              <span class="model-name">${escapeHtml(m.name)}${
                m.id === "turbo" ? '<span class="rec">recommandé hébreu</span>' : ""
              }</span>
              <span class="model-sub">${escapeHtml(m.tagline)}${m.error ? " · erreur : " + escapeHtml(m.error) : ""}</span>
            </div>
            <div class="model-side">${side}</div>
            ${m.downloading ? `<div class="model-progress"><i style="width:${pct}%"></i></div>` : ""}
          </div>`;
      })
      .join("");
  }

  function parseCorrections(raw) {
    return raw
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => {
        const parts = line.split(/\s*(?:=|→|\|)\s*/);
        if (parts.length < 2) return null;
        return { from: parts[0], to: parts.slice(1).join(" = ") };
      })
      .filter(Boolean);
  }

  async function saveSettings() {
    if (!state.draft) return;
    state.draft.language = $("#language").value;
    state.draft.preset = $("#preset").value;
    state.draft.glossary = $("#glossary")
      .value.split("\n")
      .map((s) => s.trim())
      .filter(Boolean);
    state.draft.corrections = parseCorrections($("#corrections").value);
    state.draft.customPrompt = $("#customPrompt").value;
    state.draft.normalizeQuotes = $("#normalizeQuotes").checked;
    state.draft.timestamps = $("#timestamps").checked;
    try {
      await invoke("save_settings", { next: state.draft });
      state.settings = JSON.parse(JSON.stringify(state.draft));
      $("#saveState").textContent = "Réglages enregistrés.";
      renderModelChip();
      renderBanner();
      setTimeout(() => {
        if (state.modalOpen) closeSettings();
      }, 350);
      toast("Réglages enregistrés.");
      state.previewSig = "";
      renderPreview();
    } catch (err) {
      $("#saveState").textContent = "Impossible d'enregistrer.";
      toast(String(err));
    }
  }

  async function download(id) {
    try {
      await invoke("download_model", { id });
      toast("Téléchargement lancé…");
      await refreshModels();
    } catch (err) {
      toast(String(err));
    }
  }

  /* ---------- events ---------- */

  function bindUI() {
    $("#chooseBtn").addEventListener("click", (e) => {
      e.stopPropagation();
      pickFiles();
    });
    $("#dropzone").addEventListener("click", (e) => {
      if (e.target.closest("button")) return;
      pickFiles();
    });
    $("#dropzone").addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        pickFiles();
      }
    });

    $("#topicPill").addEventListener("click", openTopic);
    $("#closeTopic").addEventListener("click", closeTopic);
    $("#topicModal").addEventListener("click", (e) => {
      if (e.target.id === "topicModal") closeTopic();
    });
    $("#topicTabs").addEventListener("click", (e) => {
      const btn = e.target.closest("button[data-kind]");
      if (!btn) return;
      state.topicKind = btn.dataset.kind;
      renderTopicTabs();
      renderTopicList();
    });
    $("#topicSearch").addEventListener("input", renderTopicList);
    $("#topicList").addEventListener("click", (e) => {
      const word = e.target.closest("[data-word]");
      if (word) {
        addTechTerm(word.dataset.word);
        toast(`Terme ajouté : ${word.dataset.word}`);
        return;
      }
      const item = e.target.closest("[data-topic-id]");
      if (!item || !state.topicDraft) return;
      const entry = lexEntry(item.dataset.topicId);
      if (!entry) return;
      applyTopicToDraft(entry, true);
      renderTopicList();
      renderTopicSelected();
    });
    $("#topicTermChips").addEventListener("click", (e) => {
      const btn = e.target.closest("[data-term-remove]");
      if (!btn || !state.topicDraft) return;
      state.topicDraft.terms = state.topicDraft.terms.filter((t) => t !== btn.dataset.termRemove);
      renderTopicSelected();
    });
    $("#topicAddAll").addEventListener("click", () => {
      if (!state.topicDraft || !state.topicDraft.id) return;
      const entry = lexEntry(state.topicDraft.id);
      if (entry) state.topicDraft.terms = buildTopicTerms(entry);
      renderTopicSelected();
    });
    $("#topicClearTerms").addEventListener("click", () => {
      if (!state.topicDraft) return;
      state.topicDraft.terms = [];
      renderTopicSelected();
    });
    $("#techAdd").addEventListener("click", () => {
      addTechTerm($("#techInput").value);
      $("#techInput").value = "";
    });
    $("#techInput").addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        addTechTerm($("#techInput").value);
        $("#techInput").value = "";
      }
    });
    $("#techChips").addEventListener("click", (e) => {
      const btn = e.target.closest("[data-tech-remove]");
      if (!btn || !state.topicDraft) return;
      state.topicDraft.technical = state.topicDraft.technical.filter(
        (t) => t !== btn.dataset.techRemove
      );
      renderTechChips();
    });
    $("#topicApply").addEventListener("click", applyTopic);
    $("#topicReset").addEventListener("click", resetTopicDraft);
    $("#uncertainApply").addEventListener("click", applyUncertain);
    $("#learningReset")?.addEventListener("click", async () => {
      try {
        await invoke("reset_learning");
        await refreshLearning();
        toast("Apprentissage réinitialisé.");
      } catch (err) {
        toast(String(err));
      }
    });

    const audio = $("#audioEl");
    audio.addEventListener("timeupdate", () => {
      updatePlayerUi();
      highlightAt(audio.currentTime || 0);
    });
    audio.addEventListener("loadedmetadata", updatePlayerUi);
    audio.addEventListener("play", updatePlayerUi);
    audio.addEventListener("pause", updatePlayerUi);
    audio.addEventListener("ended", updatePlayerUi);

    $("#settingsBtn").addEventListener("click", openSettings);
    $("#modelChip").addEventListener("click", openSettings);
    $("#closeSettings").addEventListener("click", closeSettings);
    $("#saveSettings").addEventListener("click", saveSettings);
    $("#settingsModal").addEventListener("click", (e) => {
      if (e.target.id === "settingsModal") closeSettings();
    });

    $("#bannerBtn").addEventListener("click", () => {
      if (state.settings) download(state.settings.model);
    });

    $("#pickOut").addEventListener("click", async () => {
      if (!state.draft) return;
      try {
        const dir = await invoke("pick_output_dir");
        if (dir) {
          state.draft.outputDir = dir;
          $("#outDir").textContent = dir;
          $("#outDir").title = dir;
        }
      } catch (err) {
        toast(String(err));
      }
    });

    $("#openOut").addEventListener("click", () => {
      invoke("open_output_dir").catch((e) => toast(String(e)));
    });

    $("#modelList").addEventListener("click", async (e) => {
      const imp = e.target.closest("[data-import]");
      if (imp) {
        e.stopPropagation();
        try {
          await invoke("import_model");
          await refreshModels();
          if (state.draft) {
            state.draft.model = "custom";
            renderModels();
          }
          toast("Modèle affiné importé.");
        } catch (err) {
          toast(String(err));
        }
        return;
      }
      const dl = e.target.closest("[data-dl]");
      if (dl) {
        e.stopPropagation();
        download(dl.dataset.dl);
        return;
      }
      const card = e.target.closest(".model");
      if (card && state.draft) {
        state.draft.model = card.dataset.model;
        renderModels();
        renderModelChip();
      }
    });

    $("#queue").addEventListener("click", (e) => {
      const cancel = e.target.closest("[data-cancel]");
      if (cancel) {
        e.stopPropagation();
        cancelJob(cancel.dataset.cancel);
        return;
      }
      const row = e.target.closest(".job");
      if (row) {
        state.selectedId = row.dataset.id;
        state.previewSig = "";
        renderQueue();
        renderPreview();
      }
    });

    $("#preview").addEventListener("click", (e) => {
      const job = currentJob();
      const uncertain = e.target.closest("[data-uncertain]");
      if (uncertain && job) {
        showUncertainPop(uncertain, job);
        return;
      }
      const para = e.target.closest(".transcript p[data-start]");
      if (para && state.audioJobId) {
        seekTo(parseFloat(para.dataset.start));
        return;
      }
      const btn = e.target.closest("[data-act]");
      if (!btn) return;
      const act = btn.dataset.act;
      const audio = $("#audioEl");
      if (act === "cancel") cancelJob(btn.dataset.id);
      else if (act === "retry" && job) enqueue([job.path]);
      else if (act === "settings") openSettings();
      else if (act === "export" && job) exportJob(job.id, btn.dataset.fmt);
      else if (act === "copy" && job) copyJob(job);
      else if (act === "reveal" && job)
        invoke("reveal", { path: job.outputs?.docx || job.path }).catch((err) => toast(String(err)));
      else if (act === "openout") invoke("open_output_dir").catch((err) => toast(String(err)));
      else if (act === "play") {
        if (audio.paused) audio.play().catch((err) => toast(String(err)));
        else audio.pause();
      } else if (act === "back10") seekTo((audio.currentTime || 0) - 10);
      else if (act === "fwd10") seekTo((audio.currentTime || 0) + 10);
    });

    $("#preview").addEventListener("input", (e) => {
      if (e.target.id === "seek") {
        seekTo(parseFloat(e.target.value));
      }
    });

    $("#preview").addEventListener("change", (e) => {
      if (e.target.id === "followToggle") {
        state.follow = e.target.checked;
      }
    });

    document.addEventListener("click", (e) => {
      if (!e.target.closest("#uncertainPop") && !e.target.closest("[data-uncertain]")) {
        hideUncertainPop();
      }
    });

    document.addEventListener("keydown", (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === ",") {
        e.preventDefault();
        openSettings();
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "o") {
        e.preventDefault();
        pickFiles();
      }
      if (e.key === "Escape") {
        if (state.modalOpen) closeSettings();
        if (!$("#topicModal").hidden) closeTopic();
        hideUncertainPop();
      }
    });

    window.addEventListener("dragover", (e) => e.preventDefault());
    window.addEventListener("drop", (e) => e.preventDefault());
  }

  async function connectDragDrop() {
    try {
      const webview = window.__TAURI__.webview.getCurrentWebview();
      await webview.onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          $("#dropzone").classList.add("drag");
        } else if (payload.type === "leave") {
          $("#dropzone").classList.remove("drag");
        } else if (payload.type === "drop") {
          $("#dropzone").classList.remove("drag");
          if (payload.paths && payload.paths.length) enqueue(payload.paths);
        }
      });
    } catch (err) {
      console.warn("drag drop indisponible", err);
    }
  }

  async function boot() {
    if (navigator.userAgent.includes("Windows")) document.body.classList.add("windows");
    bindUI();
    try {
      state.settings = await invoke("get_settings");
      state.info = await invoke("app_info");
    } catch (err) {
      toast("Impossible de charger les réglages : " + err);
    }
    await loadLexicon();
    if (state.settings) {
      state.topic.context = state.settings.customPrompt || "";
      state.topic.technical = state.settings.technicalTerms || [];
    }
    if (state.settings?.topicId) {
      const entry = lexEntry(state.settings.topicId);
      if (entry) {
        state.topic.id = entry.id;
        state.topic.label = entry.fr;
        state.topic.he = entry.he;
        state.topic.kind = entry.kind;
        state.topic.terms = buildTopicTerms(entry);
        state.topic.manual = true;
      }
    }
    renderTopicPill();
    await refreshLearning();
    await refreshModels();
    await refreshJobs(true);
    await connectDragDrop();
    renderModelChip();
    renderBanner();
    setInterval(() => {
      refreshJobs();
      if (state.modalOpen || state.models.some((m) => m.downloading)) refreshModels();
    }, 750);
    if (state.settings && !(modelById(state.settings.model) || {}).installed) {
      setTimeout(openSettings, 500);
    }
  }

  boot();
})();
