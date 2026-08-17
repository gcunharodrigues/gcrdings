import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { providerTransferCanRetry, providerTransferError } from "../../src/types/verifiable-record";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

describe("optional provider authorization renderer contract", () => {
  test("an ambiguous accepted transfer is explicitly non-retryable", () => {
    expect(providerTransferError({ code: "outcome_unknown" })).toContain("retry are disabled");
    expect(providerTransferCanRetry("outcome_unknown")).toBe(false);
    expect(providerTransferCanRetry("request_failed")).toBe(true);
    const dialog = fs.readFileSync(path.join(ROOT, "frontend/src/components/MeetingDetails/ExternalTransferDialog.tsx"), "utf8");
    const hook = fs.readFileSync(path.join(ROOT, "frontend/src/hooks/meeting-details/useExternalProvider.ts"), "utf8");
    expect(hook).toContain("errorCode");
    expect(dialog).toContain("providerTransferCanRetry(transfer.errorCode)");
  });
  test("Settings exposes a masked Keychain lifecycle and per-task enablement without loading secrets", () => {
    const componentPath = path.join(ROOT, "frontend/src/components/ExternalProviderSettings.tsx");
    expect(fs.existsSync(componentPath)).toBe(true);
    const component = fs.existsSync(componentPath) ? fs.readFileSync(componentPath, "utf8") : "";
    const settings = fs.readFileSync(path.join(ROOT, "frontend/src/app/settings/page.tsx"), "utf8");
    const context = fs.readFileSync(path.join(ROOT, "frontend/src/contexts/ConfigContext.tsx"), "utf8");
    expect(component).toContain('type="password"');
    expect(component).toContain("Save credential");
    expect(component).toContain("Replace credential");
    expect(component).toContain("Test credential");
    expect(component).toContain("Remove credential");
    expect(component).toContain("Enable Agent Handoff transfer");
    expect(component).toContain("credentialMask");
    expect(component).toContain("endpoint: string");
    expect(component).toContain("model: string");
    expect(component).toContain("setEndpoint(next.endpoint)");
    expect(component).toContain("setModel(next.model)");
    expect(settings).toContain("<ExternalProviderSettings");
    expect(context).not.toContain("api_get_api_key");
    expect(context).not.toContain("customConfig.apiKey");
  });

  test("Session preview is accessible, displays exact facts, and submits only the opaque digest", () => {
    const dialogPath = path.join(ROOT, "frontend/src/components/MeetingDetails/ExternalTransferDialog.tsx");
    const hookPath = path.join(ROOT, "frontend/src/hooks/meeting-details/useExternalProvider.ts");
    expect(fs.existsSync(dialogPath)).toBe(true);
    expect(fs.existsSync(hookPath)).toBe(true);
    const dialog = fs.existsSync(dialogPath) ? fs.readFileSync(dialogPath, "utf8") : "";
    const hook = fs.existsSync(hookPath) ? fs.readFileSync(hookPath, "utf8") : "";
    const workspace = fs.readFileSync(path.join(ROOT, "frontend/src/app/meeting-details/page-content.tsx"), "utf8");
    expect(dialog).toContain("Provider");
    expect(dialog).toContain("Session");
    expect(dialog).toContain("Purpose");
    expect(dialog).toContain("Data sent");
    expect(dialog).toContain("Transcript revision");
    expect(dialog).toContain('aria-describedby=');
    expect(hook).toContain("previewDigest");
    expect(hook).toContain("api_confirm_provider_transfer");
    expect(hook).not.toContain("payload:");
    expect(hook).not.toContain("video");
    expect(workspace).toContain("<ExternalTransferDialog");
  });

  test("cancel closes locally and cannot invoke provider confirmation", () => {
    const dialogPath = path.join(ROOT, "frontend/src/components/MeetingDetails/ExternalTransferDialog.tsx");
    const dialog = fs.existsSync(dialogPath) ? fs.readFileSync(dialogPath, "utf8") : "";
    const hook = fs.readFileSync(path.join(ROOT, "frontend/src/hooks/meeting-details/useExternalProvider.ts"), "utf8");
    expect(dialog).toContain("Cancel");
    expect(hook).toContain("setOpen(false)");
    expect(dialog).toContain("{transfer.open && (");
    expect(dialog).not.toMatch(/Cancel[\s\S]{0,200}confirmTransfer/);
  });
});
