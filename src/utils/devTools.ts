import type { DevTool, DevToolPreset } from "../models/types";

export function mergeDevTools(stored: DevTool[], presets: DevToolPreset[]): DevTool[] {
  const storedMap = new Map(stored.map((tool) => [tool.id, tool]));
  const presetIds = new Set(presets.map((preset) => preset.id));

  const merged: DevTool[] = presets.map((preset) => {
    const storedTool = storedMap.get(preset.id);
    return {
      id: preset.id,
      name: preset.name,
      commandPath: preset.commandPath,
      arguments: preset.arguments,
      enabled: storedTool?.enabled ?? true,
      isPreset: true,
    };
  });

  for (const tool of stored) {
    if (!presetIds.has(tool.id)) {
      merged.push({
        ...tool,
        enabled: tool.enabled ?? true,
        isPreset: tool.isPreset ?? false,
      });
    }
  }

  return merged;
}
