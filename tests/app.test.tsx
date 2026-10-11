import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "../src/App";
import { size } from "../src/api";
const bridge = vi.hoisted(() => ({
  invoke: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  invoke: bridge.invoke,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (event: string, callback: (event: { payload: unknown }) => void) => {
      bridge.listeners.set(event, callback);
      return () => bridge.listeners.delete(event);
    },
  ),
}));
const progress = {
  scan_id: "scan",
  files: 2,
  bytes: 3000,
  candidates: 1,
  candidate_bytes: 1000,
  errors: 0,
  skipped: 0,
  status: "complete",
};
const candidate = {
  id: "allowed",
  path: "C:\\Temp\\old.tmp",
  bytes: 1000,
  modified: 0,
  category: "Temporary",
  eligible: true,
  reason: "Review the temporary file",
  duplicate_group: null,
};
const protectedItem = {
  ...candidate,
  id: "protected",
  path: "C:\\Documents\\keep.txt",
  bytes: 2000,
  category: "Protected",
  eligible: false,
};
beforeEach(() => {
  bridge.invoke.mockReset();
  bridge.invoke.mockImplementation(
    async (name: string, args?: Record<string, unknown>) => {
      switch (name) {
        case "get_settings":
          return {
            protected: [],
            patterns: [],
            exclusions: [],
            theme: "dark",
            provider: "offline",
            model: "",
            cloud_consent: false,
          };
        case "drives":
          return [
            {
              path: "C:\\",
              name: "Test disk",
              total: 100000,
              available: 50000,
            },
          ];
        case "scan_history":
          return [progress];
        case "review_history":
        case "recovery_history":
          return [];
        case "scan_view":
          return {
            progress,
            categories: [{ category: "Temporary", files: 1, bytes: 1000 }],
            items:
              args?.filter === "eligible"
                ? [candidate]
                : [candidate, protectedItem],
            total: args?.filter === "eligible" ? 1 : 2,
          };
        case "cleanup_preview":
          return {
            id: "preview",
            items: [candidate],
            bytes: 1000,
            executable: false,
            notice: "Dry run only. No files changed.",
          };
        case "save_settings":
        case "open_setup_link":
          return null;
        case "propose_protections":
          return {
            protected: ["C:\\Fixture"],
            patterns: [],
            exclusions: [],
            theme: "dark",
            provider: "offline",
            model: "",
            cloud_consent: false,
          };
        default:
          throw new Error(`Unexpected command: ${name}`);
      }
    },
  );
});
afterEach(cleanup);
describe("review workflow", () => {
  it("refreshes fast scan results when completion arrives before an older initial response", async () => {
    const user = userEvent.setup();
    const base = bridge.invoke.getMockImplementation()!;
    const freshProgress = {
      ...progress,
      scan_id: "fresh",
      files: 1,
      bytes: 1000,
    };
    const freshItem = {
      ...candidate,
      id: "fresh-item",
      path: "C:\\Temp\\fresh.tmp",
    };
    let first = true;
    let finish: ((value: unknown) => void) | undefined;
    bridge.invoke.mockImplementation(
      (name: string, args?: Record<string, unknown>) => {
        if (name === "start_scan") return Promise.resolve("fresh");
        if (name === "scan_view" && args?.scan === "fresh") {
          if (first) {
            first = false;
            return new Promise((resolve) => {
              finish = resolve;
            });
          }
          return Promise.resolve({
            progress: freshProgress,
            items: [freshItem],
            categories: [],
            total: 1,
          });
        }
        return base(name, args);
      },
    );
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Analyze" }));
    await waitFor(() => expect(finish).toBeDefined());
    await act(async () => {
      bridge.listeners.get("scan-progress")?.({ payload: freshProgress });
      finish?.({
        progress: { ...freshProgress, files: 0, status: "scanning" },
        items: [],
        categories: [],
        total: 0,
      });
    });
    expect(await screen.findByText("fresh.tmp", { exact: true })).toBeTruthy();
    expect(screen.getByText("Scan complete", { exact: true })).toBeTruthy();
  });
  it("only installs local AI after an explicit click and saves the verified model without cloud consent", async () => {
    const user = userEvent.setup();
    const base = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation(
      async (name: string, args?: Record<string, unknown>) => {
        if (name === "setup_local_llm")
          return { running: true, models: ["qwen2.5:1.5b"] };
        return base(name, args);
      },
    );
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.selectOptions(screen.getByLabelText("AI provider"), "ollama");
    expect(
      bridge.invoke.mock.calls.some(([name]) => name === "setup_local_llm"),
    ).toBe(false);
    await user.click(
      screen.getByRole("button", { name: "Install Ollama & download model" }),
    );
    await waitFor(() =>
      expect(bridge.invoke).toHaveBeenCalledWith("setup_local_llm", {
        approved: true,
      }),
    );
    await waitFor(() =>
      expect(bridge.invoke).toHaveBeenCalledWith("save_settings", {
        settings: expect.objectContaining({
          provider: "ollama",
          model: "qwen2.5:1.5b",
          cloud_consent: false,
        }),
      }),
    );
  });
  it("does not save settings if local setup is incomplete", async () => {
    const user = userEvent.setup();
    const base = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation(
      async (name: string, args?: Record<string, unknown>) => {
        if (name === "setup_local_llm") return { running: true, models: [] };
        return base(name, args);
      },
    );
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.selectOptions(screen.getByLabelText("AI provider"), "ollama");
    await user.click(
      screen.getByRole("button", { name: "Install Ollama & download model" }),
    );
    expect((await screen.findByRole("alert")).textContent).toContain(
      "not verified",
    );
    expect(
      bridge.invoke.mock.calls.some(([name]) => name === "save_settings"),
    ).toBe(false);
  });
  it("clears incompatible model choices when switching providers and displays cloud setup steps", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.selectOptions(screen.getByLabelText("AI provider"), "ollama");
    await user.type(screen.getByLabelText("Model name"), "qwen2.5:1.5b");
    await user.selectOptions(
      screen.getByLabelText("AI provider"),
      "openrouter",
    );
    expect(
      (screen.getByLabelText("Model name") as HTMLInputElement).value,
    ).toBe("");
    expect(
      screen.getByRole("link", { name: "API keys" }).getAttribute("href"),
    ).toBe("https://openrouter.ai/settings/keys");
    expect(screen.getByRole("region", { name: "Cloud AI setup" })).toBeTruthy();
    await user.click(screen.getByRole("link", { name: "API keys" }));
    expect(bridge.invoke).toHaveBeenCalledWith("open_setup_link", {
      url: "https://openrouter.ai/settings/keys",
    });
  });
  it("does not claim unchanged files after a cleanup execution failure", async () => {
    const user = userEvent.setup();
    const base = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation(
      async (name: string, args?: Record<string, unknown>) => {
        if (name === "cleanup_preview")
          return { ...(await base(name, args)), executable: true };
        if (name === "execute_cleanup")
          throw new Error("Journal write failed after an operation");
        return base(name, args);
      },
    );
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Cleaner" }));
    await user.click(
      await screen.findByRole("checkbox", { name: "Select C:\\Temp\\old.tmp" }),
    );
    await user.click(screen.getByRole("button", { name: "Review cleanup" }));
    await user.click(
      await screen.findByRole("checkbox", { name: /I reviewed these files/ }),
    );
    await user.click(
      screen.getByRole("button", { name: "Approve & quarantine 1 files" }),
    );
    expect(
      await screen.findByText(/Some files may already have moved/),
    ).toBeTruthy();
    expect(screen.queryByText("Done · no files changed")).toBeNull();
    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain(
        "Journal write failed",
      ),
    );
    expect(
      screen.queryByRole("button", { name: "Approve & quarantine 1 files" }),
    ).toBeNull();
  });
  it("requires a separate approval before invoking quarantine", async () => {
    const user = userEvent.setup();
    const base = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation(
      async (name: string, args?: Record<string, unknown>) => {
        if (name === "cleanup_preview")
          return { ...(await base(name, args)), executable: true };
        if (name === "execute_cleanup")
          return { quarantined: 1, failed: 0, cancelled: false };
        return base(name, args);
      },
    );
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Cleaner" }));
    await user.click(
      await screen.findByRole("checkbox", { name: "Select C:\\Temp\\old.tmp" }),
    );
    await user.click(screen.getByRole("button", { name: "Review cleanup" }));
    const execute = await screen.findByRole("button", {
      name: "Approve & quarantine 1 files",
    });
    expect((execute as HTMLButtonElement).disabled).toBe(true);
    expect(
      bridge.invoke.mock.calls.some(([name]) => name === "execute_cleanup"),
    ).toBe(false);
    await user.click(
      screen.getByRole("checkbox", { name: /I reviewed these files/ }),
    );
    await user.click(execute);
    await waitFor(() =>
      expect(bridge.invoke).toHaveBeenCalledWith("execute_cleanup", {
        preview: "preview",
        approved: true,
      }),
    );
    expect(await screen.findByText(/1 quarantined · 0 failed/)).toBeTruthy();
  });
  it("requires confirmation before restoring a recovery record", async () => {
    const user = userEvent.setup();
    const base = bridge.invoke.getMockImplementation()!;
    bridge.invoke.mockImplementation(
      async (name: string, args?: Record<string, unknown>) => {
        if (name === "recovery_history")
          return [
            {
              id: "record",
              preview: "preview",
              created: 1,
              item: candidate,
              stored: "C:\\Quarantine\\record.held",
              status: "quarantined",
              error: null,
            },
          ];
        if (name === "restore_item") return null;
        return base(name, args);
      },
    );
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Recovery" }));
    await user.click(
      await screen.findByRole("button", { name: "Review restore" }),
    );
    const button = screen.getByRole("button", { name: "Restore file" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(
      bridge.invoke.mock.calls.some(([name]) => name === "restore_item"),
    ).toBe(false);
    await user.click(
      screen.getByRole("checkbox", { name: "I approve restoring this file." }),
    );
    await user.click(button);
    await waitFor(() =>
      expect(bridge.invoke).toHaveBeenCalledWith("restore_item", {
        id: "record",
        approved: true,
      }),
    );
  });
  it("offers cancellation before the first duplicate progress event", async () => {
    const user = userEvent.setup();
    const base = bridge.invoke.getMockImplementation()!;
    let finish: ((value: number) => void) | undefined;
    bridge.invoke.mockImplementation(
      (name: string, args?: Record<string, unknown>) => {
        if (name === "find_duplicates")
          return new Promise<number>((resolve) => {
            finish = resolve;
          });
        if (name === "cancel_scan") {
          finish?.(0);
          return Promise.resolve(null);
        }
        return base(name, args);
      },
    );
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Cleaner" }));
    await user.click(
      screen.getByRole("button", { name: "Find exact duplicates" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Cancel duplicate pass" }),
    );
    expect(bridge.invoke).toHaveBeenCalledWith("cancel_scan", undefined);
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Cancel duplicate pass" }),
      ).toBeNull(),
    );
  });
  it("requires confirmation for a locally interpreted protection goal", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "AI assistant" }));
    await user.type(screen.getByLabelText("Cleanup goal"), "Keep C:\\Fixture");
    await user.click(
      screen.getByRole("button", {
        name: "Propose protections from this goal",
      }),
    );
    await screen.findByRole("dialog", { name: "Confirm protection changes" });
    expect(bridge.invoke).toHaveBeenCalledWith("propose_protections", {
      goal: "Keep C:\\Fixture",
    });
    expect(
      bridge.invoke.mock.calls.filter(([name]) => name === "save_settings"),
    ).toHaveLength(0);
  });
  it("uses real inventory from bridge and disables protected file selection", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Cleaner" }));
    const locked = await screen.findByRole("checkbox", {
      name: "Select C:\\Documents\\keep.txt",
    });
    expect((locked as HTMLInputElement).disabled).toBe(true);
    expect(
      bridge.invoke.mock.calls.some(([name]) => name.includes("delete")),
    ).toBe(false);
  });
  it("requires explicit per-item selection and previews IDs only", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Cleaner" }));
    const button = screen.getByRole("button", {
      name: "Review cleanup",
    });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select C:\\Temp\\old.tmp" }),
    );
    await user.click(button);
    await screen.findByRole("dialog", { name: "Your dry-run preview" });
    expect(bridge.invoke).toHaveBeenCalledWith("cleanup_preview", {
      scan: "scan",
      ids: ["allowed"],
    });
    expect(screen.getByText("Dry run only. No files changed.")).toBeTruthy();
  });
  it("shows backend errors without claiming a successful cleanup", async () => {
    bridge.invoke.mockImplementationOnce(async () => {
      throw new Error("Database unavailable");
    });
    render(<App />);
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Database unavailable",
    );
  });
  it("requires review and confirmation before saving a protection rule", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Protected items" }));
    await user.selectOptions(screen.getByLabelText("Rule type"), "patterns");
    await user.type(screen.getByLabelText("Protection rule"), "*.save");
    await user.click(screen.getByRole("button", { name: "Review rule" }));
    expect(
      bridge.invoke.mock.calls.filter(([name]) => name === "save_settings"),
    ).toHaveLength(0);
    await user.click(
      screen.getByRole("button", { name: "Confirm and save rules" }),
    );
    await waitFor(() =>
      expect(
        bridge.invoke.mock.calls.filter(([name]) => name === "save_settings"),
      ).toHaveLength(1),
    );
  });
  it("never enables cloud consent automatically", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("option", { name: /Test disk/ });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.selectOptions(
      screen.getByLabelText("AI provider"),
      "openrouter",
    );
    expect((screen.getByRole("checkbox") as HTMLInputElement).checked).toBe(
      false,
    );
  });
});
it("formats storage without invented data", () => {
  expect(size(0)).toBe("0 B");
  expect(size(1024 ** 3)).toBe("1.0 GB");
});
