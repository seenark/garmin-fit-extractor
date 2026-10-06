import { SQL } from "bun";
import { isAbsolute, resolve } from "node:path";

const repositoryRoot = resolve(import.meta.dir, "..");
const bun = process.execPath;
const bunx = Bun.which("bunx") ?? "bunx";
const cargo = Bun.which("cargo") ?? "cargo";
const apiAddress = "127.0.0.1:3000";
const webAddress = "127.0.0.1:5173";
const healthUrl = `http://${webAddress}/healthz`;
const readinessTimeoutMs = 60_000;

export function testDatabaseConfiguration(): { databaseUrl: string; pgdata: string } {
  const databaseUrl = process.env.TEST_DATABASE_URL;
  const pgdata = process.env.PGDATA;
  let url: URL;
  try {
    url = new URL(databaseUrl ?? "");
  } catch {
    throw new Error("TEST_DATABASE_URL must explicitly identify disposable PostgreSQL.");
  }
  if (
    !["postgres:", "postgresql:"].includes(url.protocol) ||
    !url.hostname || !url.username || url.pathname.length < 2 ||
    !pgdata || !isAbsolute(pgdata)
  ) {
    throw new Error("Tests require explicit disposable TEST_DATABASE_URL and absolute PGDATA.");
  }
  return { databaseUrl: databaseUrl!, pgdata };
}

async function verifyTestDatabase(databaseUrl: string, pgdata: string): Promise<void> {
  const sql = new SQL(databaseUrl, { max: 1, connectionTimeout: 5 });
  try {
    const [identity] = await sql`
      SELECT current_setting('data_directory') AS directory,
             current_setting('server_version_num')::integer AS version
    `;
    if (identity.directory !== pgdata || identity.version < 180000 || identity.version >= 190000) {
      throw new Error("Tests require the exact PGDATA identity of disposable PostgreSQL 18.");
    }
  } finally {
    await sql.close();
  }
}

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
    stdin: "ignore",
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

  process.child.kill("SIGTERM");

  const stopped = await Promise.race([
    process.child.exited.then(() => true),
    Bun.sleep(5_000).then(() => false),
  ]);
  if (!stopped && process.child.exitCode === null) {
    process.child.kill("SIGKILL");
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
  const { databaseUrl, pgdata } = testDatabaseConfiguration();
  await verifyTestDatabase(databaseUrl, pgdata);
  const environment: Record<string, string | undefined> = {
    ...process.env,
    GARMIN_FIT_BIND: apiAddress,
    DATABASE_URL: databaseUrl,
    GARMIN_FIT_TEST_AUTH: "true",
  };
  const testMode = process.argv.slice(2).includes("--test");
  const started: ManagedProcess[] = [];
  let failure: unknown;

  try {
    const build = Bun.spawn(
      [cargo, "build", "--locked", "-p", "garmin-fit-extractor-api", "--message-format=json-render-diagnostics"],
      { cwd: repositoryRoot, env: environment, stdout: "pipe", stderr: "inherit" },
    );
    const buildOutput = await new Response(build.stdout).text();
    if ((await build.exited) !== 0) {
      throw new Error("API build failed.");
    }
    const artifacts = buildOutput.trim().split("\n").map((line) => JSON.parse(line));
    const executable = artifacts.find((artifact) =>
      artifact.reason === "compiler-artifact" &&
      artifact.target.name === "garmin-fit-extractor-api" &&
      artifact.target.kind.includes("bin") &&
      artifact.executable
    )?.executable;
    if (!executable) {
      throw new Error("Cargo did not report the freshly built API executable.");
    }
    delete environment.RUNS_DECODER_EXECUTABLE;

    if (testMode) {
      environment.RUNS_DECODER_EXECUTABLE = executable;
      for (const command of [
        [bun, "--no-env-file", "run", "--filter", "*", "test"],
        [cargo, "test", "--locked", "--workspace", "--", "--test-threads=1"],
      ]) {
        const child = Bun.spawn(command, {
          cwd: repositoryRoot, env: environment, stdout: "inherit", stderr: "inherit",
        });
        if ((await child.exited) !== 0) {
          throw new Error(`${command[0]} tests failed.`);
        }
      }
      return;
    }

    const browser = startProcess(
      "Playwright browser installation",
      [bunx, "--no-env-file", "--no-install", "playwright", "install", "chromium"],
      environment,
      resolve(repositoryRoot, "apps/web"),
    );
    started.push(browser);
    if ((await browser.child.exited) !== 0) {
      throw new Error("Playwright browser installation failed.");
    }

    const api = startProcess(
      "API",
      [executable],
      environment,
    );
    started.push(api);

    const web = startProcess(
      "web server",
      [
        bunx,
        "--no-env-file",
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
        "--no-env-file",
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

if (import.meta.main) {
  await run();
}
