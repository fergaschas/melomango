import { installPluginRequirement, pluginRequirements, type SourceInfo } from "./api";

/**
 * Installs whatever the plugin still needs (e.g. yt-dlp for YouTube), reporting progress.
 * Returns the names of what was installed. Throws if something is missing and the plugin
 * can't install it by itself.
 */
export async function setupRequirements(
  plugin: Pick<SourceInfo, "id" | "name">,
  onProgress: (text: string) => void,
): Promise<string[]> {
  const requirements = await pluginRequirements(plugin.id);
  const installed: string[] = [];
  for (const r of requirements.filter((r) => !r.installed)) {
    if (!r.installable) {
      throw new Error(
        `${plugin.name} necesita ${r.name} y no se puede instalar automáticamente en este equipo. Instálalo a mano y vuelve a intentarlo.`,
      );
    }
    onProgress(`Instalando ${r.name}, que necesita ${plugin.name}… puede tardar un minuto.`);
    const after = await installPluginRequirement(plugin.id, r.id);
    if (!after.find((x) => x.id === r.id)?.installed) {
      throw new Error(`No se pudo instalar ${r.name}.`);
    }
    installed.push(r.name);
  }
  return installed;
}
