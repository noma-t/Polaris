// Kill any process listening on the port of `build.devUrl` in tauri.conf.json,
// so that `tauri dev` does not fail with "Port XXXX is already in use".
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";

const tauriConf = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);
const port = Number(new URL(tauriConf.build.devUrl).port);

function findListeningPids(port) {
  try {
    const output =
      process.platform === "win32"
        ? execSync(
            `powershell -NoProfile -Command "Get-NetTCPConnection -LocalPort ${port} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess"`,
            { encoding: "utf8" },
          )
        : execSync(`lsof -ti tcp:${port} -sTCP:LISTEN`, { encoding: "utf8" });
    return [...new Set(output.split(/\s+/).filter(Boolean).map(Number))];
  } catch {
    // No listener found (lsof exits with 1) or command unavailable
    return [];
  }
}

for (const pid of findListeningPids(port)) {
  try {
    process.kill(pid, "SIGKILL");
    console.log(`[kill-dev-port] Killed PID ${pid} listening on port ${port}`);
  } catch (error) {
    console.warn(`[kill-dev-port] Failed to kill PID ${pid}: ${error.message}`);
  }
}
