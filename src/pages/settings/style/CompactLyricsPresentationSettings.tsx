import { useTranslation } from "react-i18next";

import type { CompactLyricsPresentation } from "../../../shared/types";
import { SelectRow, ToggleRow } from "../shared/components";

type Props = {
  value: CompactLyricsPresentation;
  onChange: (patch: Partial<CompactLyricsPresentation>) => void;
};

/** 三种紧凑歌词共用的展示配置块；载体专属配置留在各自分组中。 */
export default function CompactLyricsPresentationSettings({ value, onChange }: Props) {
  const { t } = useTranslation();
  return <>
    <SelectRow
      label={t("settings.overlay.lyricLayout")}
      value={value.layout}
      options={[["single", t("overlay.layout.single")], ["double", t("overlay.layout.double")]]}
      onChange={(layout) => onChange({ layout: layout as CompactLyricsPresentation["layout"] })}
    />
    <SelectRow
      label={t("settings.overlay.doubleLineMode")}
      description={t("settings.overlay.doubleLineModeHint")}
      disabled={value.layout !== "double"}
      value={value.doubleLineMode}
      options={[["rolling", t("settings.overlay.doubleLineRolling")], ["alternating", t("settings.overlay.doubleLineAlternating")]]}
      onChange={(doubleLineMode) => onChange({ doubleLineMode: doubleLineMode as CompactLyricsPresentation["doubleLineMode"] })}
    />
    <ToggleRow
      label={t("settings.overlay.showTranslation")}
      value={value.showTranslation}
      onChange={(showTranslation) => onChange({ showTranslation })}
    />
    <ToggleRow
      label={t("settings.overlay.showRomanization")}
      value={value.showRomanization}
      onChange={(showRomanization) => onChange({ showRomanization })}
    />
    {value.showTranslation && value.showRomanization && (
      <SelectRow
        label={t("settings.overlay.supportingPriority")}
        value={value.supportingPriority}
        options={[["translation", t("settings.overlay.supportingTranslation")], ["romanization", t("settings.overlay.supportingRomanization")]]}
        onChange={(supportingPriority) => onChange({ supportingPriority: supportingPriority as CompactLyricsPresentation["supportingPriority"] })}
      />
    )}
  </>;
}
