import type { TFunction } from "i18next";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { InputGroup, InputGroupInput, InputGroupText } from "@/components/ui/input-group";
import { Input } from "@/components/ui/input";
import type { SearchFormState } from "./utils";
import styles from "../QuickLyricsWindow.module.scss";

type QuickLyricsSearchFormProps = {
  searchForm: SearchFormState;
  titleInvalid: boolean;
  artistInvalid: boolean;
  durationInvalid: boolean;
  formDisabled: boolean;
  searching: boolean;
  onSearch: () => void | Promise<void>;
  onUpdateField: (field: keyof SearchFormState, value: string) => void;
  t: TFunction;
};

export function QuickLyricsSearchForm({
  searchForm,
  titleInvalid,
  artistInvalid,
  durationInvalid,
  formDisabled,
  searching,
  onSearch,
  onUpdateField,
  t,
}: QuickLyricsSearchFormProps) {
  return (
    <form className={styles.search} onSubmit={(event) => { event.preventDefault(); void onSearch(); }}>
      <FieldGroup className={styles.formGrid}>
        <Field data-invalid={titleInvalid}>
          <FieldLabel htmlFor="quick-lyrics-title">{t("quickLyrics.titleField")}</FieldLabel>
          <Input
            aria-invalid={titleInvalid}
            autoComplete="off"
            disabled={formDisabled}
            id="quick-lyrics-title"
            placeholder={t("quickLyrics.titlePlaceholder")}
            value={searchForm.title}
            onChange={(event) => onUpdateField("title", event.currentTarget.value)}
          />
          <FieldError>{titleInvalid ? t("quickLyrics.titleRequired") : null}</FieldError>
        </Field>
        <Field data-invalid={artistInvalid}>
          <FieldLabel htmlFor="quick-lyrics-artist">{t("quickLyrics.artistField")}</FieldLabel>
          <Input
            aria-invalid={artistInvalid}
            autoComplete="off"
            disabled={formDisabled}
            id="quick-lyrics-artist"
            placeholder={t("quickLyrics.artistPlaceholder")}
            value={searchForm.artist}
            onChange={(event) => onUpdateField("artist", event.currentTarget.value)}
          />
          <FieldError>{artistInvalid ? t("quickLyrics.artistRequired") : null}</FieldError>
        </Field>
        <Field>
          <FieldLabel htmlFor="quick-lyrics-album">{t("quickLyrics.albumField")}</FieldLabel>
          <Input
            autoComplete="off"
            disabled={formDisabled}
            id="quick-lyrics-album"
            placeholder={t("quickLyrics.albumPlaceholder")}
            value={searchForm.album}
            onChange={(event) => onUpdateField("album", event.currentTarget.value)}
          />
        </Field>
        <Field data-invalid={durationInvalid}>
          <FieldLabel htmlFor="quick-lyrics-duration-minutes">{t("quickLyrics.durationField")}</FieldLabel>
          <InputGroup className={styles.durationInput}>
            <InputGroupInput
              aria-invalid={durationInvalid}
              aria-label={t("quickLyrics.durationMinutes")}
              autoComplete="off"
              className={styles.durationSegment}
              disabled={formDisabled}
              id="quick-lyrics-duration-minutes"
              inputMode="numeric"
              placeholder={t("quickLyrics.durationMinutesPlaceholder")}
              value={searchForm.durationMinutes}
              onChange={(event) => onUpdateField("durationMinutes", event.currentTarget.value)}
            />
            <InputGroupText aria-hidden="true" className={styles.durationSeparator}>:</InputGroupText>
            <InputGroupInput
              aria-invalid={durationInvalid}
              aria-label={t("quickLyrics.durationSeconds")}
              autoComplete="off"
              className={styles.durationSegment}
              disabled={formDisabled}
              id="quick-lyrics-duration-seconds"
              inputMode="numeric"
              maxLength={2}
              placeholder={t("quickLyrics.durationSecondsPlaceholder")}
              value={searchForm.durationSeconds}
              onChange={(event) => onUpdateField("durationSeconds", event.currentTarget.value)}
            />
          </InputGroup>
          <FieldError>{durationInvalid ? t("quickLyrics.durationInvalid") : null}</FieldError>
        </Field>
        <Button className={styles.searchButton} disabled={formDisabled} type="submit">{searching ? t("common.actions.searching") : t("common.actions.search")}</Button>
      </FieldGroup>
      <FieldDescription className={styles.searchHint}>{t("quickLyrics.searchRuleHint")} {t("quickLyrics.durationFuzzyHint")}</FieldDescription>
    </form>
  );
}
