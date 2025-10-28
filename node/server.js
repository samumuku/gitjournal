import express from "express";
import path from "path";
import { fileURLToPath } from "url";
import fetch from "node-fetch";
import dotenv from "dotenv";

dotenv.config();
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const app = express();
const PORT = process.env.PORT || 5173;
const GH = "https://api.github.com";

app.set("view engine", "ejs");
app.set("views", path.join(__dirname, "views"));
app.use(express.urlencoded({ extended: true }));
app.use(express.static(path.join(__dirname, "public"))); // optionnel pour CSS/images

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

// Page d'accueil + génération serveur
app.get(["/", "/jdt"], async (req, res) => {
  try {
    const defaultRepoUrl = req.query.repo || process.env.DEFAULT_REPO_URL || "";
    const { owner, repo } = parseRepoUrl(defaultRepoUrl);
    const since = req.query.since || process.env.JOURNAL_START_DATE || "";
    const selectedBranch = req.query.branch || process.env.DEFAULT_BRANCH || "main";

    let branches = [];
    let entries = [];
    let totals = { minutes: 0, h: 0, m: 0 };
    if (owner && repo) {
      branches = await fetchBranches(owner, repo);
      const branch = branches.includes(selectedBranch) ? selectedBranch : branches[0] || "main";
      const raw = await fetchAllCommits({ owner, repo, branch, since });
      entries = raw.map(groom);
      totals = totalDuration(entries);
      return res.render("index", {
        defaultRepoUrl,
        owner,
        repo,
        branches,
        selectedBranch: branch,
        since,
        entries,
        totals
      });
    }

    return res.render("index", {
      defaultRepoUrl,
      owner: null,
      repo: null,
      branches: [],
      selectedBranch,
      since,
      entries: [],
      totals
    });
  } catch (e) {
    console.error(e);
    res.status(500).send(e.message);
  }
});

app.listen(PORT, () => {
  console.log(`Journal de travail (SSR) sur http://localhost:${PORT}`);
});
