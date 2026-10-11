// Local Windows integration test. Uses only files created in its own temporary fixture.
// Requires a release build, WebView2 and an interactive Windows session.
const { chromium } = require("playwright");
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const net = require("node:net");
const assert = require("node:assert/strict");
async function port() {
  const server = net.createServer();
  await new Promise((resolve, reject) =>
    server.once("error", reject).listen(0, "127.0.0.1", resolve),
  );
  const number = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return number;
}
(async () => {
  if (process.platform !== "win32")
    throw new Error("This test requires Windows.");
  const executable = path.resolve(
    __dirname,
    "../src-tauri/target/release/cluttersift.exe",
  );
  if (!fs.existsSync(executable))
    throw new Error("Build the release app first: npm run desktop:build");
  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), "cluttersift-smoke-"));
  fs.writeFileSync(
    path.join(fixture, "first.zip"),
    "Synthetic duplicate fixture, not a real archive",
  );
  fs.writeFileSync(
    path.join(fixture, "second.zip"),
    "Synthetic duplicate fixture, not a real archive",
  );
  const candidate = path.join(
    os.tmpdir(),
    `cluttersift-smoke-${require("node:crypto").randomUUID()}.tmp`,
  );
  fs.writeFileSync(candidate, "Synthetic quarantine roundtrip fixture", {
    flag: "wx",
  });
  const old = new Date(Date.now() - 8 * 86400000);
  fs.utimesSync(candidate, old, old);
  const debugPort = await port();
  const app = spawn(executable, [], {
    env: {
      ...process.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${debugPort}`,
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: false,
  });
  let nativeLog = "";
  for (const stream of [app.stdout, app.stderr])
    stream.on("data", (data) => {
      nativeLog = (nativeLog + data).slice(-32000);
    });
  app.on("exit", (code, signal) => {
    nativeLog += `\nApp exit: ${code} / ${signal}`;
  });
  let browser;
  try {
    let ready = false;
    for (let n = 0; n < 120; n++) {
      if (app.exitCode !== null)
        throw new Error("Native app exited during startup.");
      try {
        const r = await fetch(`http://127.0.0.1:${debugPort}/json/version`);
        if (r.ok) {
          ready = true;
          break;
        }
      } catch {}
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    if (!ready)
      throw new Error(
        "WebView2 debugging transport unavailable; native UI flow not verified.",
      );
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${debugPort}`);
    const page = browser.contexts()[0].pages()[0];
    assert.ok(page, "Expected an app webview");
    page.setDefaultTimeout(30000);
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("crash", () => {
      nativeLog += "\nRenderer crashed";
    });
    await page.getByRole("heading", { name: "Cleaner" }).waitFor();
    const native = await page.evaluate(() => !!window.__TAURI_INTERNALS__);
    assert.equal(native, true, "Must use the real native bridge");
    await page.getByRole("button", { name: "Cleaner", exact: true }).click();
    await page
      .getByRole("textbox", { name: /Folder or drive path/ })
      .fill(fixture);
    await page.getByRole("button", { name: "Analyze", exact: true }).click();
    await page.getByText("Scan complete", { exact: true }).waitFor();
    await page.getByText("first.zip", { exact: true }).waitFor();
    await page.getByText("second.zip", { exact: true }).waitFor();
    const history = await page.evaluate(() =>
      window.__TAURI_INTERNALS__.invoke("scan_history"),
    );
    const scan = history[0].scan_id;
    const view = await page.evaluate(
      (scan) =>
        window.__TAURI_INTERNALS__.invoke("scan_view", {
          scan,
          offset: 0,
          filter: "",
        }),
      scan,
    );
    assert.equal(view.progress.files, 2);
    assert.equal(view.progress.status, "complete");
    assert.equal(view.items.length, 2);
    await page.getByRole("button", { name: "Find exact duplicates" }).click();
    await page.getByText(/2 matching files confirmed/).waitFor();
    const duplicateView = await page.evaluate(
      (scan) =>
        window.__TAURI_INTERNALS__.invoke("scan_view", {
          scan,
          offset: 0,
          filter: "",
        }),
      scan,
    );
    assert.equal(
      duplicateView.items.filter((i) => i.duplicate_group).length,
      2,
    );
    assert.ok(duplicateView.items.every((i) => !i.eligible));
    const denied = await page.evaluate(
      async ({ scan, id }) => {
        try {
          await window.__TAURI_INTERNALS__.invoke("cleanup_preview", {
            scan,
            ids: [id],
          });
          return false;
        } catch {
          return true;
        }
      },
      { scan, id: view.items[0].id },
    );
    assert.equal(denied, true, "Backend must reject ineligible files");
    await page.getByRole("button", { name: "Cleaner", exact: true }).click();
    assert.equal(
      await page.getByRole("button", { name: "Review cleanup" }).isDisabled(),
      true,
    );
    await page
      .getByRole("button", { name: "AI assistant", exact: true })
      .click();
    await page
      .getByRole("textbox", { name: "Cleanup goal" })
      .fill(`Keep everything in "${fixture}"`);
    await page
      .getByRole("button", { name: "Propose protections from this goal" })
      .click();
    await page
      .getByRole("dialog", { name: "Confirm protection changes" })
      .waitFor();
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    assert.equal(await page.getByRole("dialog").count(), 0);
    const settings = await page.evaluate(() =>
      window.__TAURI_INTERNALS__.invoke("get_settings"),
    );
    if (settings.provider === "offline") {
      await page
        .getByRole("button", { name: "Ask assistant", exact: true })
        .click();
      await page.getByText(/Offline review:/).waitFor();
    }
    assert.equal(
      fs.readFileSync(path.join(fixture, "first.zip"), "utf8"),
      "Synthetic duplicate fixture, not a real archive",
    );
    await page.getByRole("button", { name: "Cleaner", exact: true }).click();
    await page
      .getByRole("textbox", { name: /Folder or drive path/ })
      .fill(candidate);
    await page.getByRole("button", { name: "Analyze", exact: true }).click();
    await page.getByText(path.basename(candidate), { exact: true }).waitFor();
    await page.getByText("Scan complete", { exact: true }).waitFor();
    const candidateInventory = await page.evaluate(async () => {
      const h = await window.__TAURI_INTERNALS__.invoke("scan_history");
      return window.__TAURI_INTERNALS__.invoke("scan_view", {
        scan: h[0].scan_id,
        offset: 0,
        filter: "",
      });
    });
    assert.equal(candidateInventory.items.length, 1);
    assert.equal(
      candidateInventory.items[0].eligible,
      true,
      "Synthetic direct-temp file must be eligible under current settings",
    );
    await page.getByRole("button", { name: "Cleaner", exact: true }).click();
    await page
      .getByRole("checkbox", {
        name: `Select ${candidateInventory.items[0].path}`,
      })
      .check();
    await page.getByRole("button", { name: "Review cleanup" }).click();
    const approve = page.getByRole("button", {
      name: "Approve & quarantine 1 files",
    });
    await approve.waitFor();
    fs.mkdirSync(path.resolve(__dirname, "../artifacts"), { recursive: true });
    await page.screenshot({
      path: path.resolve(
        __dirname,
        "../artifacts/native-quarantine-approval.png",
      ),
    });
    assert.equal(await approve.isDisabled(), true);
    assert.equal(
      fs.existsSync(candidate),
      true,
      "Preview must leave synthetic source intact",
    );
    await page
      .getByRole("checkbox", { name: /I reviewed these files/ })
      .check();
    await approve.click();
    await page
      .getByText(/1 quarantined · 0 failed/)
      .or(page.locator(".modal .banner.error"))
      .first()
      .waitFor();
    if (await page.locator(".modal .banner.error").count())
      throw new Error(await page.locator(".modal .banner.error").innerText());
    await page.getByText(/1 quarantined · 0 failed/).waitFor();
    assert.equal(fs.existsSync(candidate), false);
    const record = await page.evaluate(async (original) => {
      const records =
        await window.__TAURI_INTERNALS__.invoke("recovery_history");
      return records.find((r) => r.item.path === original);
    }, candidateInventory.items[0].path);
    assert.ok(record);
    assert.equal(record.status, "quarantined");
    await page.screenshot({
      path: path.resolve(
        __dirname,
        "../artifacts/native-quarantine-recovery.png",
      ),
    });
    assert.equal(
      fs.readFileSync(record.stored, "utf8"),
      "Synthetic quarantine roundtrip fixture",
    );
    const row = page
      .locator(".history-row")
      .filter({ hasText: candidateInventory.items[0].path });
    await row.getByRole("button", { name: "Review restore" }).click();
    const restore = page.getByRole("button", {
      name: "Restore file",
      exact: true,
    });
    assert.equal(await restore.isDisabled(), true);
    await page
      .getByRole("checkbox", { name: "I approve restoring this file." })
      .check();
    await restore.click();
    await page
      .getByText("File restored to its original location.", { exact: true })
      .waitFor();
    assert.equal(
      fs.readFileSync(candidate, "utf8"),
      "Synthetic quarantine roundtrip fixture",
    );
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    for (const theme of ["dark", "light"]) {
      await page.getByRole("combobox", { name: "Theme" }).selectOption(theme);
      assert.equal(
        await page.evaluate(
          () => getComputedStyle(document.documentElement).colorScheme,
        ),
        theme,
      );
    }
    await page
      .getByRole("combobox", { name: "AI provider" })
      .selectOption("ollama");
    await page.getByRole("button", { name: "Check local setup" }).click();
    await page
      .getByText(/Ollama is (not running|running\. Local models:)/)
      .waitFor();
    const unapprovedSetup = await page.evaluate(async () => {
      try {
        await window.__TAURI_INTERNALS__.invoke("setup_local_llm", {
          approved: false,
        });
        return false;
      } catch {
        return true;
      }
    });
    assert.equal(
      unapprovedSetup,
      true,
      "Local installation requires explicit authorization",
    );
    await page
      .getByRole("combobox", { name: "AI provider" })
      .selectOption("openrouter");
    await page.getByRole("region", { name: "Cloud AI setup" }).waitFor();
    assert.equal(await page.getByRole("checkbox").isChecked(), false);
    assert.deepEqual(
      await page.evaluate(() =>
        window.__TAURI_INTERNALS__.invoke("get_settings"),
      ),
      settings,
      "Setup inspection must not change saved preferences",
    );
    assert.deepEqual(errors, [], "Native renderer must not throw");
    console.log(
      "PASS: native Windows launch; real IPC; scans; duplicates; quarantine/restore roundtrip; theme/provider dropdown selection; read-only local service check; unapproved installation rejected; cloud guide and consent; saved settings unchanged; zero renderer errors. Installer/model download not executed.",
    );
  } catch (error) {
    fs.mkdirSync(path.resolve(__dirname, "../artifacts"), { recursive: true });
    fs.writeFileSync(
      path.resolve(__dirname, "../artifacts/native-smoke-error.txt"),
      `${error.stack}\n${nativeLog}`,
    );
    if (browser) {
      const page = browser.contexts()[0]?.pages()[0];
      if (page && !page.isClosed()) {
        fs.writeFileSync(
          path.resolve(__dirname, "../artifacts/native-smoke-dom.txt"),
          await page
            .locator("body")
            .innerText()
            .catch(() => "unavailable"),
        );
        await page
          .screenshot({
            path: path.resolve(
              __dirname,
              "../artifacts/native-smoke-error.png",
            ),
          })
          .catch(() => {});
      }
    }
    throw error;
  } finally {
    if (browser) await browser.close().catch(() => {});
    app.kill();
    if (
      path.dirname(path.resolve(candidate)) === path.resolve(os.tmpdir()) &&
      path.basename(candidate).startsWith("cluttersift-smoke-")
    )
      fs.rmSync(candidate, { force: true });
    // Validate the exact directory created above before recursively removing synthetic fixtures.
    const resolved = path.resolve(fixture);
    if (
      path.dirname(resolved) === path.resolve(os.tmpdir()) &&
      path.basename(resolved).startsWith("cluttersift-smoke-")
    )
      fs.rmSync(resolved, { recursive: true, force: true });
  }
})().catch((e) => {
  console.error(e.message);
  process.exitCode = 1;
});
