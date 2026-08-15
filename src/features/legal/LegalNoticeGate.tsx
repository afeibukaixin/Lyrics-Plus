import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useAppLanguage } from "../i18n/I18nProvider";
import { languageRegistry, supportedLanguages } from "../../shared/languages";
import type { LanguagePreference } from "../../shared/types";
import { api, isTauriRuntime } from "../../shared/api";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

const READ_SECONDS = 10;
const languageOptions = supportedLanguages.map((code) => ({
  code,
  label: languageRegistry[code].nativeLabel,
}));

export function LegalNoticeGate({ children }: { children: React.ReactNode }) {
  const { t } = useTranslation();
  const { preference, setLanguage } = useAppLanguage();
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
    <Dialog open disablePointerDismissal onOpenChange={(open) => { if (!open) quit(); }}>
      <DialogContent className="grid h-[min(640px,calc(100vh-48px))] w-[min(760px,calc(100vw-48px))] grid-rows-[auto_minmax(0,1fr)_auto_auto] gap-0 overflow-hidden p-0" showClose={false}>
      <DialogHeader className="flex flex-row items-center justify-between gap-5 border-b border-border px-[22px] pb-4 pt-[18px]">
        <div>
          <DialogTitle className="text-xl" id="legal-notice-title">{t("legalNotice.title")}</DialogTitle>
          <DialogDescription className="sr-only">{t("legalNotice.welcome")}</DialogDescription>
        </div>
        <Field className="w-[150px] gap-1">
          <FieldLabel>{t("legalNotice.language")}</FieldLabel>
          <Select
            items={[{ value: "system", label: t("common.language.system") }, ...languageOptions.map(({ code, label }) => ({ value: code, label }))]}
            value={preference}
            onValueChange={(value) => {
              setError(null);
              void setLanguage(value as LanguagePreference)
                .catch(() => setError(t("legalNotice.languageError")));
            }}
          >
            <SelectTrigger className="w-full" aria-label={t("legalNotice.language")}><SelectValue /></SelectTrigger>
            <SelectContent><SelectGroup><SelectItem value="system">{t("common.language.system")}</SelectItem>{languageOptions.map(({ code, label }) => <SelectItem key={code} value={code}>{label}</SelectItem>)}</SelectGroup></SelectContent>
          </Select>
        </Field>
      </DialogHeader>

      <ScrollArea className="min-h-0"><div className="px-[22px] py-[18px] [&_h2]:mb-2 [&_h2]:mt-6 [&_h2]:text-base [&_h2]:font-medium [&_p]:mb-2.5 [&_p]:text-sm [&_p]:leading-relaxed [&_p]:text-muted-foreground [&_p:first-child]:font-medium [&_p:first-child]:text-foreground">
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
      </div></ScrollArea>

      {error && <Alert variant="destructive" className="mx-6 my-2"><AlertDescription>{error}</AlertDescription></Alert>}
      <DialogFooter className="flex justify-end gap-2.5 border-t border-border bg-card px-[22px] py-3">
        <Button className="min-w-[132px]" variant="secondary" type="button" onClick={quit}>{t("legalNotice.cancel")}</Button>
        <Button
          className="min-w-[132px]"
          disabled={seconds > 0 || submitting}
          type="button"
          onClick={() => void accept()}
        >
          {seconds > 0
            ? t("legalNotice.agreeCountdown", { count: seconds })
            : t("legalNotice.agree")}
        </Button>
      </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
