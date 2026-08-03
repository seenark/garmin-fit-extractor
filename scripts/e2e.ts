import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const repositoryRoot = resolve(import.meta.dir, "..");
const bun = process.execPath;
const bunx = Bun.which("bunx") ?? "bunx";
const cargo = Bun.which("cargo") ?? "cargo";
const apiAddress = "127.0.0.1:3000";
const webAddress = "127.0.0.1:5173";
const healthUrl = `http://${webAddress}/healthz`;
const readinessTimeoutMs = 60_000;

type ManagedProcess = {
  readonly label: string;
  readonly command: readonly string[];
  readonly child: Bun.Subprocess<"pipe", "pipe", "inherit">;
  readonly stdout: Promise<string>;
  readonly stderr: Promise<string>;
};

function startProcess(
  label: string,
  command: readonly string[],
  environment: Record<string, string | undefined>,
  workingDirectory = repositoryRoot,
): ManagedProcess {
  const child = Bun.spawn(command, {
    cwd: workingDirectory,
    env: environment,
    stdout: "pipe",
    stderr: "pipe",
    stdin: "inherit",
    detached: true,
  });

  return {
    label,
    command,
    child,
    stdout: new Response(child.stdout).text(),
    stderr: new Response(child.stderr).text(),
  };
}

async function stopProcess(process: ManagedProcess): Promise<void> {
  if (process.child.exitCode !== null) {
    return;
  }

  try {
    globalThis.process.kill(-process.child.pid, "SIGTERM");
  } catch {
    process.child.kill("SIGTERM");
  }

  const stopped = await Promise.race([
    process.child.exited.then(() => true),
    Bun.sleep(5_000).then(() => false),
  ]);
  if (!stopped && process.child.exitCode === null) {
    try {
      globalThis.process.kill(-process.child.pid, "SIGKILL");
    } catch {
      process.child.kill("SIGKILL");
    }
    await process.child.exited;
  }
}

async function processOutput(process: ManagedProcess): Promise<string> {
  const [stdout, stderr] = await Promise.all([process.stdout, process.stderr]);
  const output = [stdout.trim(), stderr.trim()].filter(Boolean).join("\n");
  return output
    ? `$ ${process.command.join(" ")}\n${output}`
    : `$ ${process.command.join(" ")}\n(no output)`;
}

async function waitForHealth(api: ManagedProcess, web: ManagedProcess): Promise<void> {
  const deadline = Date.now() + readinessTimeoutMs;
  let lastFailure = "the health endpoint did not respond";

  while (Date.now() < deadline) {
    if (api.child.exitCode !== null || web.child.exitCode !== null) {
      const exited = api.child.exitCode !== null ? api : web;
      throw new Error(`${exited.label} exited before readiness.`);
    }

    try {
      const response = await fetch(healthUrl);
      if (response.ok && (await response.text()) === '{"status":"ok"}') {
        return;
      }
      lastFailure = `received HTTP ${response.status} from ${healthUrl}`;
    } catch (error) {
      lastFailure = error instanceof Error ? error.message : String(error);
    }

    await Bun.sleep(200);
  }

  throw new Error(`Timed out waiting for ${healthUrl}: ${lastFailure}`);
}

async function run(): Promise<void> {
  const temporaryDirectory = await mkdtemp(join(tmpdir(), "garmin-fit-extractor-e2e-"));
  const databasePath = join(temporaryDirectory, "garmin-fit-extractor.sqlite3");
  const databaseUrl = `sqlite://${databasePath}`;
  const environment = {
    ...process.env,
    GARMIN_FIT_BIND: apiAddress,
    GARMIN_FIT_DATABASE_URL: databaseUrl,
  };
  const started: ManagedProcess[] = [];
  let failure: unknown;

  try {
    const browser = startProcess(
      "Playwright browser installation",
      [bunx, "--no-install", "playwright", "install", "chromium"],
      environment,
      resolve(repositoryRoot, "apps/web"),
    );
    started.push(browser);
    if ((await browser.child.exited) !== 0) {
      throw new Error("Playwright browser installation failed.");
    }

    const api = startProcess(
      "API",
      [cargo, "run", "-p", "garmin-fit-extractor-api"],
      environment,
    );
    started.push(api);

    const web = startProcess(
      "web server",
      [
        bunx,
        "--no-install",
        "vite",
        "--host",
        "127.0.0.1",
        "--port",
        "5173",
        "--strictPort",
      ],
      environment,
      resolve(repositoryRoot, "apps/web"),
    );
    started.push(web);

    await waitForHealth(api, web);

    const playwright = startProcess(
      "Playwright",
      [
        bunx,
        "--no-install",
        "playwright",
        "test",
        "--config=playwright.config.ts",
      ],
      {
        ...environment,
        PLAYWRIGHT_BASE_URL: `http://${webAddress}`,
      },
      resolve(repositoryRoot, "apps/web"),
    );
    started.push(playwright);

    const exitCode = await playwright.child.exited;
    if (exitCode !== 0) {
      throw new Error(`Playwright exited with code ${exitCode}.`);
    }
  } catch (error) {
    failure = error;
  } finally {
    const cleanup = await Promise.allSettled(
      [...started].reverse().map(stopProcess),
    );
    await rm(temporaryDirectory, { force: true, recursive: true });
    const cleanupFailure = cleanup.find(
      (result) => result.status === "rejected",
    );
    if (failure === undefined && cleanupFailure?.status === "rejected") {
      failure = cleanupFailure.reason;
    }
  }

  if (failure !== undefined) {
    const outputs = await Promise.all(started.map(processOutput));
    const reason = failure instanceof Error ? failure.message : String(failure);
    throw new Error(`${reason}\n\nChild process output:\n${outputs.join("\n\n")}`);
  }
}

await run();
