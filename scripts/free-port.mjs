import { execSync } from "node:child_process";

const port = Number(process.argv[2] || 1420);

function pidsOnPort(p) {
  const ids = new Set();

  try {
    const out = execSync("netstat -ano -p tcp", { encoding: "utf8" });
    for (const line of out.split(/\r?\n/)) {
      const trimmed = line.trim();
      // Local address column contains :port
      // Example: TCP    0.0.0.0:1420    0.0.0.0:0    LISTENING    1234
      const parts = trimmed.split(/\s+/);
      if (parts.length < 5) continue;
      const local = parts[1] || "";
      if (!local.endsWith(`:${p}`)) continue;
      const state = parts[3] || "";
      if (!/LISTEN/i.test(state) && state !== "侦听") continue;
      const pid = parts[parts.length - 1];
      if (/^\d+$/.test(pid) && pid !== "0") ids.add(pid);
    }
  } catch {
    // ignore
  }

  if (!ids.size) {
    try {
      const ps = execSync(
        `powershell -NoProfile -Command "Get-NetTCPConnection -LocalPort ${p} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess -Unique"`,
        { encoding: "utf8" },
      );
      for (const line of ps.split(/\r?\n/)) {
        const pid = line.trim();
        if (/^\d+$/.test(pid) && pid !== "0") ids.add(pid);
      }
    } catch {
      // ignore
    }
  }

  return [...ids];
}

const pids = pidsOnPort(port);
if (!pids.length) {
  console.log(`[INFO] Port ${port} is free`);
  process.exit(0);
}

for (const pid of pids) {
  try {
    execSync(`taskkill /PID ${pid} /T /F`, { stdio: "inherit" });
    console.log(`[INFO] Killed PID ${pid} on port ${port}`);
  } catch {
    console.log(`[WARN] Could not kill PID ${pid}`);
  }
}

execSync("ping -n 2 127.0.0.1 >nul", { shell: true, stdio: "ignore" });
