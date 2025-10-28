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

function ghHeaders() {
  const h = { Accept: "application/vnd.github+json" };
  if (process.env.GITHUB_TOKEN) h["Authorization"] = `Bearer ${process.env.GITHUB_TOKEN}`;
  return h;
}

app.use(express.static(path.join(__dirname, "public")));

// --- API ---
// Branches: /api/branches?owner=:o&repo=:r
app.get("/api/branches", async (req, res) => {
  const { owner, repo } = req.query;
  if (!owner || !repo) return res.status(400).json({ error: "owner et repo sont requis" });
  const url = `${GH}/repos/${owner}/${repo}/branches?per_page=100`;
  try {
    const r = await fetch(url, { headers: ghHeaders() });
    const data = await r.json();
    if (!r.ok) return res.status(r.status).json(data);
    res.json(data.map((b) => ({ name: b.name })));
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

// Commits paginés: /api/commits?owner=:o&repo=:r&branch=:b&since=:iso
app.get("/api/commits", async (req, res) => {
  const { owner, repo, branch = process.env.DEFAULT_BRANCH || "main", since } = req.query;
  if (!owner || !repo) return res.status(400).json({ error: "owner et repo sont requis" });

  const perPage = 100;
  let page = 1;
  let all = [];
  let keepGoing = true;

  while (keepGoing) {
    const params = new URLSearchParams({ sha: branch, per_page: String(perPage), page: String(page) });
    if (since) params.set("since", since);

    const url = `${GH}/repos/${owner}/${repo}/commits?${params}`;
    const r = await fetch(url, { headers: ghHeaders() });
    if (!r.ok) {
      const err = await r.text();
      return res.status(r.status).send(err);
    }
    const batch = await r.json();
    all = all.concat(batch);
    if (batch.length < perPage) keepGoing = false;
    else page++;
  }

  res.json(all);
});

// Page d’accueil
app.get(["/", "/jdt"], (req, res) => {
  res.sendFile(path.join(__dirname, "public", "gitjournal.html"));
});

app.listen(PORT, () => {
  console.log(`Journal de travail dispo sur http://localhost:${PORT}`);
});
