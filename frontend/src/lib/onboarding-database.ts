type DatabaseInvoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export async function initializeFirstLaunchDatabase(
  invoke: DatabaseInvoke,
): Promise<"imported" | "fresh"> {
  const legacyPath = await invoke<string | null>("check_default_legacy_database");
  if (legacyPath) {
    await invoke("import_and_initialize_database", { legacyDbPath: legacyPath });
    return "imported";
  }

  await invoke("initialize_fresh_database");
  return "fresh";
}
