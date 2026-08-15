import type { TFunction } from "i18next";
import type { PlaybackSnapshot } from "../../shared/types";

export function localizedSource(source: string, t: TFunction): string {
  switch (source) {
    case "本地导入":
    case "手动导入":
    case "local_import":
      return t("common.source.localImport");
    case "本地文件":
    case "local_file":
      return t("common.source.localFile");
    case "测试":
    case "test":
      return t("common.source.test");
    default:
      return source;
  }
}

export function playbackStatusText(snapshot: PlaybackSnapshot, t: TFunction): string | null {
  switch (snapshot.errorCode) {
    case "waiting": return t("player.waiting");
    case "not_installed": return t("player.notInstalled", {
      player: snapshot.player === "apple_music" ? "Apple Music" : snapshot.player === "spotify" ? "Spotify" : t("settings.app.playerSystem"),
    });
    case "automation_denied": return t("player.automationDenied");
    case "response_timeout": return t("player.responseTimeout");
    case "invalid_response": return t("player.invalidResponse");
    case "multiple_playing": return t("player.multiplePlaying");
    case "no_unique_player": return t("player.noUniquePlayer");
    case "source_not_allowed": return null;
    case "unavailable": return t("player.unavailable");
    default: return snapshot.error ? t("player.unavailable") : null;
  }
}
