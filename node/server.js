import express from "express";
import path from "path";
import { fileURLToPath } from "url";
import fetch from "node-fetch";
import dotenv from "dotenv";
import expressLayouts from "express-ejs-layouts";
import open from "open";
import fs from "fs/promises";
import crypto from "crypto";

dotenv.config();
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const app = express();
const PORT = process.env.PORT || 5173;
const GH = "https://api.github.com";

app.set("view engine", "ejs");
app.set("views", path.join(__dirname, "views"));
app.use(expressLayouts);
app.set("layout", "layout"); // => views/layout.ejs
app.use(express.urlencoded({ extended: true }));
app.use(express.static(path.join(__dirname, "public"))); // optionnel pour CSS/images
app.use(express.json());

function ghHeaders() {
  const h = { Accept: "application/vnd.github+json" };
  if (process.env.GITHUB_TOKEN) h["Authorization"] = `Bearer ${process.env.GITHUB_TOKEN}`;
  return h;
}

function parseRepoUrl(repoUrl) {
  if (!repoUrl) return {};
  const m = repoUrl.match(/github\.com\/([^/]+)\/([^/#?]+)/i);
  if (!m) return {};
  return { owner: m[1], repo: m[2] };
}

async function fetchBranches(owner, repo) {
  const url = `${GH}/repos/${owner}/${repo}/branches?per_page=100`;
  const r = await fetch(url, { headers: ghHeaders() });
  if (!r.ok) throw new Error(`GitHub branches error ${r.status}`);
  const data = await r.json();
  return data.map((b) => b.name);
}

async function fetchAllCommits({ owner, repo, branch, since }) {
  const perPage = 100;
  let page = 1;
  let all = [];
  let keep = true;
  while (keep) {
    const params = new URLSearchParams({ sha: branch, per_page: `${perPage}`, page: `${page}` });
    if (since) params.set("since", since);
    const url = `${GH}/repos/${owner}/${repo}/commits?${params}`;
    const r = await fetch(url, { headers: ghHeaders() });
    if (!r.ok) throw new Error(`GitHub commits error ${r.status}`);
    const batch = await r.json();
    all = all.concat(batch);
    keep = batch.length === perPage;
    page++;
  }
  return all;
}

function groom(commit) {
  // 1ère ligne=titre, 2e ligne=meta [hh][mm][status], reste=description
  let duration = 0;
  let status = "";
  const lines = commit.commit.message.split("\n").filter((l) => l.trim() !== "");
  if (lines.length > 1) {
    const metaLine = lines[1];
    const matches = [...metaLine.matchAll(/\[(.*?)\]/g)].map((m) => m[1]);
    if (matches.length) {
      for (const m of matches) {
        const nums = m.match(/\d+/g);
        if (!nums) {
          status = m;
        } else if (nums.length < 3) {
          nums.forEach((n) => {
            duration = duration * 60 + parseInt(n);
          });
        }
      }
    }
  }
  return {
    sha: commit.sha,
    name: lines[0] || commit.commit.message.split("\n")[0],
    description: lines.slice(2).join("\n"),
    date: commit.commit.author.date,
    duration,
    status,
    author: commit.author?.login || commit.commit?.author?.name || "?",
    url: commit.html_url
  };
}

function totalDuration(commits) {
  const mins = commits.reduce((acc, c) => acc + (c.duration || 0), 0);
  const h = Math.floor(mins / 60),
    m = mins % 60;
  return { minutes: mins, h, m };
}

// helpers de format
const fmtDayLabel = (d) =>
  new Intl.DateTimeFormat("fr-FR", { day: "2-digit", month: "short", year: "numeric" }).format(new Date(d));

const toDayKey = (isoLike) => new Date(isoLike).toISOString().slice(0, 10); // "YYYY-MM-DD"

const sumMinutes = (items) => items.reduce((acc, c) => acc + (c.duration || 0), 0);

// entries: [{ date: ISO, duration: minutes, ... }]
function groupByDay(entries) {
  // assure l'ordre chronologique croissant (ou inverse si tu préfères)
  const sorted = [...entries].sort((a, b) => new Date(a.date) - new Date(b.date));

  const groupsMap = new Map(); // préserve l'ordre d'insertion
  for (const c of sorted) {
    const key = toDayKey(c.date);
    if (!groupsMap.has(key)) groupsMap.set(key, []);
    groupsMap.get(key).push(c);
  }

  // transforme en array de groupes avec label + totals
  const groups = [];
  for (const [day, commits] of groupsMap.entries()) {
    const minutes = sumMinutes(commits);
    groups.push({
      day, // "2025-01-10"
      label: fmtDayLabel(day), // "10 janv. 2025"
      commits,
      total: {
        minutes,
        h: Math.floor(minutes / 60),
        m: minutes % 60
      }
    });
  }
  return groups;
}

// === Exceptions.json (lecture/écriture) ===
const DATA_DIR = path.join(__dirname, "data");
const EXCEPTIONS_PATH = path.join(DATA_DIR, "exceptions.json");

async function ensureDataFile() {
  await fs.mkdir(DATA_DIR, { recursive: true });
  try {
    await fs.access(EXCEPTIONS_PATH);
  } catch {
    await fs.writeFile(EXCEPTIONS_PATH, "[]", "utf-8");
  }
}

async function readExceptions() {
  await ensureDataFile();
  const raw = await fs.readFile(EXCEPTIONS_PATH, "utf-8");
  const arr = JSON.parse(raw);
  // normalisation très légère
  return Array.isArray(arr) ? arr : [];
}

async function writeExceptions(arr) {
  await ensureDataFile();
  await fs.writeFile(EXCEPTIONS_PATH, JSON.stringify(arr, null, 2), "utf-8");
}

function validateException(x) {
  // minimal: name, date (ISO), duration en minutes
  if (!x || typeof x !== "object") return "Objet invalide";
  if (!x.name) return "Champ 'name' requis";
  if (!x.date || isNaN(new Date(x.date))) return "Champ 'date' invalide (ISO attendu)";
  if (x.duration == null || isNaN(Number(x.duration))) return "Champ 'duration' requis (minutes)";
  return null;
}

// Page d'accueil + génération serveur
app.get(["/", "/jdt"], async (req, res) => {
  try {
    const defaultRepoUrl = process.env.REPO_URL || "";
    const { owner, repo } = parseRepoUrl(defaultRepoUrl);

    const date = new Date(process.env.JOURNAL_START_DATE);

    const since = new Intl.DateTimeFormat("fr-FR", {
      day: "2-digit",
      month: "short",
      year: "numeric"
    }).format(date);

    const branch = process.env.BRANCH || "main";

    // ...
    const raw = await fetchAllCommits({ owner, repo, branch, since });
    let entries = raw.map(groom).filter((c) => c.duration > 0);

    // lire les exceptions depuis le JSON
    const exc = await readExceptions();

    // fusionner commits + exceptions
    const patched = entries.concat(exc);

    // grouper + totaux
    const groups = groupByDay(patched);
    const totals = totalDuration(entries);

    return res.render("index", {
      defaultRepoUrl,
      owner,
      repo,
      selectedBranch: branch,
      since,
      groups,
      totals
    });
  } catch (e) {
    console.error(e);
    res.status(500).send(e.message);
  }
});

app.listen(PORT, () => {
  const url = `http://localhost:${PORT}/jdt`;
  console.log(`Journal de travail dispo sur ${url}`);
  open(url); // 👈 ouvre automatiquement ton navigateur par défaut
});
