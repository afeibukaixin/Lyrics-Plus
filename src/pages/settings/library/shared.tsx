import { useCallback, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { ArrowLeft, ChevronLeft, ChevronRight, MoreHorizontal, Search } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger } from "@/components/ui/alert-dialog";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button, type ButtonProps } from "@/components/ui/button";
import { CardAction, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from "@/components/ui/item";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import styles from "./library.module.scss";

const libraryPageSizes = [20, 50, 100] as const;
type PageItem = number | "start-ellipsis" | "end-ellipsis";

type TruncatedTextVariant = "title" | "meta" | "body";

export function TruncatedText({ children, variant = "body" }: {
  children: string;
  variant?: TruncatedTextVariant;
}) {
  const textRef = useRef<HTMLElement | null>(null);
  const [isTruncated, setIsTruncated] = useState(false);

  const updateTruncation = useCallback(() => {
    const text = textRef.current;
    const nextIsTruncated = Boolean(text && text.scrollWidth > text.clientWidth);
    setIsTruncated((current) => current === nextIsTruncated ? current : nextIsTruncated);
  }, []);

  useLayoutEffect(() => {
    const text = textRef.current;
    if (!text) return;

    updateTruncation();
    const observer = new ResizeObserver(updateTruncation);
    observer.observe(text);
    return () => observer.disconnect();
  }, [children, updateTruncation]);

  const trigger = variant === "title"
    ? <strong ref={textRef} className={styles.truncatedText} data-variant={variant} tabIndex={isTruncated ? 0 : undefined} />
    : <span ref={textRef} className={styles.truncatedText} data-variant={variant} tabIndex={isTruncated ? 0 : undefined} />;

  return (
    <Tooltip disabled={!isTruncated}>
      <TooltipTrigger render={trigger}>
        {children}
      </TooltipTrigger>
      <TooltipContent className={styles.truncatedTooltip}>{children}</TooltipContent>
    </Tooltip>
  );
}

function visiblePageItems(page: number, totalPages: number): PageItem[] {
  if (totalPages <= 7) return Array.from({ length: totalPages }, (_, index) => index + 1);
  if (page <= 4) return [1, 2, 3, 4, 5, "end-ellipsis", totalPages];
  if (page >= totalPages - 3) return [1, "start-ellipsis", totalPages - 4, totalPages - 3, totalPages - 2, totalPages - 1, totalPages];
  return [1, "start-ellipsis", page - 1, page, page + 1, "end-ellipsis", totalPages];
}

export function LibraryToolbar({ query, onQueryChange, filters, actions }: {
  query: string;
  onQueryChange: (value: string) => void;
  filters?: ReactNode;
  actions?: ReactNode;
}) {
  const { t } = useTranslation();

  return (
    <CardHeader className={styles.toolbar}>
      <div className={styles.toolbarSearchRow}>
        <InputGroup className={styles.searchBox}>
          <InputGroupAddon><Search data-icon="inline-start" aria-hidden="true" /></InputGroupAddon>
          <InputGroupInput
            type="search"
            autoComplete="off"
            aria-label={t("library.manager.search")}
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder={t("library.manager.searchPlaceholder")}
          />
        </InputGroup>
        {filters ? <div className={styles.toolbarFilters}>{filters}</div> : null}
      </div>
      {actions ? <div className={styles.toolbarActions}>{actions}</div> : null}
    </CardHeader>
  );
}

export function PageControls({ page, pageSize, total, onPageChange, onPageSizeChange, compact = false }: {
  page: number;
  pageSize: number;
  total: number;
  onPageChange: (page: number) => void;
  onPageSizeChange: (pageSize: number) => void;
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const pageItems = visiblePageItems(page, totalPages);
  const content = (
    <>
      <span>{t("library.manager.pageSummary", { page, totalPages, total })}</span>
      <div className={styles.paginationControls}>
        <Select value={String(pageSize)} onValueChange={(value) => { if (value !== null) onPageSizeChange(Number(value)); }}>
          <SelectTrigger className={styles.pageSizeTrigger} aria-label={t("library.manager.itemsPerPage")}><SelectValue /></SelectTrigger>
          <SelectContent><SelectGroup>{libraryPageSizes.map((value) => <SelectItem value={String(value)} key={value}>{t("library.manager.pageSizeOption", { count: value })}</SelectItem>)}</SelectGroup></SelectContent>
        </Select>
        <nav className={styles.pageButtons} aria-label={t("library.manager.paginationLabel")}>
          <Button size="icon-sm" variant="outline" disabled={page <= 1} onClick={() => onPageChange(page - 1)} aria-label={t("library.manager.previousPage")}><ChevronLeft data-icon="inline-start" /></Button>
          {pageItems.map((item) => typeof item === "number" ? (
            <Button
              size="icon-sm"
              variant={item === page ? "outline" : "ghost"}
              aria-current={item === page ? "page" : undefined}
              aria-label={t("library.manager.goToPage", { page: item })}
              key={item}
              onClick={() => onPageChange(item)}
            >{item}</Button>
          ) : <span className={styles.pageEllipsis} aria-hidden="true" key={item}><MoreHorizontal /></span>)}
          <Button size="icon-sm" variant="outline" disabled={page >= totalPages} onClick={() => onPageChange(page + 1)} aria-label={t("library.manager.nextPage")}><ChevronRight data-icon="inline-end" /></Button>
        </nav>
      </div>
    </>
  );
  return compact
    ? <div className={styles.pagination} data-compact="true">{content}</div>
    : <CardFooter className={styles.pagination}>{content}</CardFooter>;
}

export function ConfirmAction({ title, description, label, onConfirm, triggerVariant = "outline", confirmVariant = "destructive", disabled }: {
  title: string;
  description: ReactNode;
  label: string;
  onConfirm: () => void | Promise<void>;
  triggerVariant?: ButtonProps["variant"];
  confirmVariant?: ButtonProps["variant"];
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <AlertDialog>
      <AlertDialogTrigger render={<Button type="button" size="sm" variant={triggerVariant} disabled={disabled} />}>{label}</AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader><AlertDialogTitle>{title}</AlertDialogTitle><AlertDialogDescription>{description}</AlertDialogDescription></AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("common.actions.cancel")}</AlertDialogCancel>
          <AlertDialogAction variant={confirmVariant} onClick={() => void onConfirm()}>{label}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export function LibraryState({ state, message }: {
  state: "loading" | "error" | "empty";
  message: string;
}) {
  if (state === "error") {
    return <Alert variant="destructive" className={styles.stateAlert}><AlertDescription>{message}</AlertDescription></Alert>;
  }
  return (
    <Empty className={styles.libraryState}>
      {state === "loading" ? <Spinner /> : null}
      <EmptyHeader><EmptyTitle>{message}</EmptyTitle></EmptyHeader>
    </Empty>
  );
}

export function LibraryDetailHeader({ title, description, status, actions, onBack }: {
  title: ReactNode;
  description?: ReactNode;
  status?: ReactNode;
  actions?: ReactNode;
  onBack: () => void;
}) {
  const { t } = useTranslation();
  return (
    <CardHeader className={styles.detailHeader}>
      <Button type="button" variant="ghost" size="sm" className={styles.backButton} onClick={onBack}>
        <ArrowLeft data-icon="inline-start" />{t("library.manager.back")}
      </Button>
      <div className={styles.detailTitleRow}>
        <CardTitle>{title}</CardTitle>
        {status}
      </div>
      {description ? <CardDescription>{description}</CardDescription> : null}
      {actions ? <CardAction>{actions}</CardAction> : null}
    </CardHeader>
  );
}

export function LibraryDetailSection({ title, actions, children }: {
  title: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className={styles.detailSection}>
      <header className={styles.detailSectionHeader}>
        <h2 className={styles.detailSectionTitle}>{title}</h2>
        {actions ? <div className={styles.detailSectionActions}>{actions}</div> : null}
      </header>
      <div className={styles.detailSectionContent}>{children}</div>
    </section>
  );
}

export function LibraryRelationList({ children }: { children: ReactNode }) {
  return <ItemGroup className={styles.relationList}>{children}</ItemGroup>;
}

export function LibraryRelationItem({ title, description, actions }: {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <Item variant="muted" className={styles.relation}>
      <ItemContent>
        <ItemTitle>{title}</ItemTitle>
        {description ? <ItemDescription>{description}</ItemDescription> : null}
      </ItemContent>
      {actions ? <ItemActions>{actions}</ItemActions> : null}
    </Item>
  );
}

export function formatDuration(value: number | null) {
  if (value === null) return "—";
  const seconds = Math.round(value / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}
