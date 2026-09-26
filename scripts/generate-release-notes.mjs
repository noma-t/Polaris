// 前回のリリースタグから HEAD までのコミットを Conventional Commits の type ごとに分類し、
// Markdown のリリースノートを標準出力に書き出す。
// 使い方: node scripts/generate-release-notes.mjs
import { execFileSync } from "node:child_process";

const SECTIONS = [
  { title: "新機能", types: ["feat"] },
  { title: "不具合修正", types: ["fix"] },
  { title: "改善", types: ["perf", "refactor"] },
];

const git = (...args) => execFileSync("git", args, { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();

function findPreviousTag() {
  try {
    return git("describe", "--tags", "--abbrev=0", "--match", "v*", "HEAD");
  } catch {
    return null; // 初回リリース
  }
}

const previousTag = findPreviousTag();
const range = previousTag ? `${previousTag}..HEAD` : "HEAD";
const subjects = git("log", range, "--no-merges", "--format=%s").split("\n").filter(Boolean);

const COMMIT_PATTERN = /^(\w+)(?:\([^)]*\))?!?:\s*(.+)$/;
const lines = SECTIONS.map(({ title, types }) => {
  const items = subjects
    .map((subject) => subject.match(COMMIT_PATTERN))
    .filter((match) => match && types.includes(match[1]))
    .map((match) => `- ${match[2]}`);
  return items.length ? `## ${title}\n${items.join("\n")}` : null;
}).filter(Boolean);

process.stdout.write(lines.length ? `${lines.join("\n\n")}\n` : "- 軽微な修正\n");
