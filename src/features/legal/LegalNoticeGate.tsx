import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useAppLanguage } from "../i18n/I18nProvider";
import { languageRegistry, supportedLanguages } from "../../shared/languages";
import type { LanguagePreference } from "../../shared/types";
import { api, isTauriRuntime } from "../../shared/api";
import styles from "./LegalNoticeGate.module.scss";

const READ_SECONDS = 10;
const languageOptions = supportedLanguages.map((code) => ({
  code,
  label: languageRegistry[code].nativeLabel,
}));

export function LegalNoticeGate({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  const { preference, setLanguage } = useAppLanguage();
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [accepted, setAccepted] = useState<boolean | null>(null);
  const [seconds, setSeconds] = useState(READ_SECONDS);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    if (!isTauriRuntime()) {
      setAccepted(false);
      return;
    }
    void api.getLegalNoticeStatus()
      .then((status) => {
        if (!active) return;
        setError(null);
        setAccepted(status.accepted);
      })
      .catch(() => {
        if (!active) return;
        setError(t("legalNotice.loadError"));
        setAccepted(false);
      });
    return () => { active = false; };
  }, [t]);

  useEffect(() => {
    if (accepted !== false) return;
    const timer = window.setInterval(() => {
      setSeconds((current) => {
        if (current <= 1) {
          window.clearInterval(timer);
          return 0;
        }
        return current - 1;
      });
    }, 1_000);
    return () => window.clearInterval(timer);
  }, [accepted]);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (accepted !== false || !dialog) return;
    if (!dialog.open) dialog.showModal();
    return () => { if (dialog.open) dialog.close(); };
  }, [accepted]);

  if (accepted === null) return null;
  if (accepted) return children;

  const quit = () => {
    if (isTauriRuntime()) {
      void api.quitApplication();
    } else {
      window.close();
    }
  };

  const accept = async () => {
    if (seconds > 0 || submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      if (isTauriRuntime()) await api.acceptLegalNotice();
      setAccepted(true);
    } catch {
      setError(t("legalNotice.acceptError"));
      setSubmitting(false);
    }
  };

  return (
    <dialog
      aria-labelledby="legal-notice-title"
      className={styles.dialog}
      onCancel={(event) => { event.preventDefault(); quit(); }}
      ref={dialogRef}
    >
      <header className={styles.header}>
        <div>
          <span>Lyrics Plus</span>
          <h1 id="legal-notice-title">{t("legalNotice.title")}</h1>
        </div>
        <label>
          {t("legalNotice.language")}
          <select
            value={preference}
            onChange={(event) => {
              setError(null);
              void setLanguage(event.currentTarget.value as LanguagePreference)
                .catch(() => setError(t("legalNotice.languageError")));
            }}
          >
            <option value="system">{t("common.language.system")}</option>
            {languageOptions.map(({ code, label }) => <option key={code} value={code}>{label}</option>)}
          </select>
        </label>
      </header>

      <div className={styles.content}>
        <p>{t("legalNotice.welcome")}</p>
        <p>{t("legalNotice.project")}</p>

        <h2>{t("legalNotice.freeTitle")}</h2>
        <p>{t("legalNotice.freeBody")}</p>
        <p>{t("legalNotice.licenseBody")}</p>
        <p>{t("legalNotice.officialBody")}</p>

        <h2>{t("legalNotice.copyrightTitle")}</h2>
        <p>{t("legalNotice.copyrightOwnerBody")}</p>
        <p>{t("legalNotice.copyrightScopeBody")}</p>

        <h2>{t("legalNotice.onlineTitle")}</h2>
        <p>{t("legalNotice.onlineDataBody")}</p>
        <p>{t("legalNotice.onlineServiceBody")}</p>

        <h2>{t("legalNotice.responsibilityTitle")}</h2>
        <p>{t("legalNotice.responsibilityBody")}</p>
        <p>{t("legalNotice.rightsBody")}</p>
        <p>{t("legalNotice.asIsBody")}</p>
      </div>

      {error && <p className={styles.error} role="alert">{error}</p>}
      <footer className={styles.footer}>
        <button className={styles.cancel} type="button" onClick={quit}>{t("legalNotice.cancel")}</button>
        <button
          className={styles.agree}
          disabled={seconds > 0 || submitting}
          type="button"
          onClick={() => void accept()}
        >
          {seconds > 0
            ? t("legalNotice.agreeCountdown", { count: seconds })
            : t("legalNotice.agree")}
        </button>
      </footer>
    </dialog>
  );
}
