import { Fragment, useCallback, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ArrowLeft, ChevronLeft, ChevronRight, MoreHorizontal, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router";

import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger } from "@/components/ui/alert-dialog";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button, type ButtonProps } from "@/components/ui/button";
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbPage, BreadcrumbSeparator, BreadcrumbList } from "@/components/ui/breadcrumb";
import { CardAction, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from "@/components/ui/item";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import styles from "./library.module.scss";

const libraryPageSizes = [20, 50, 100] as const;
type PageItem = number | "start-ellipsis" | "end-ellipsis";

export const librarySections = ["songs", "lyrics", "artists"] as const;
export type LibrarySection = (typeof librarySections)[number];

export type LibraryTrailEntry = {
  section: LibrarySection;
  id: number | null;
  label: string;
  to: string;
};

export type LibraryNavigationState = {
  libraryTrail: LibraryTrailEntry[];
};

export type LibraryBreadcrumbItem = {
  key: string;
  label: string;
  current?: boolean;
  onClick?: () => void;
};

type OpenLibraryDetailOptions = {
  replace?: boolean;
  replaceCurrent?: boolean;
};

type LibraryNavigationOptions = {
  section: LibrarySection;
  sectionLabel: string;
  detailsLabel: string;
  detailId: number | null;
  detailLabel?: string;
  detailPath?: string;
};

const libraryRootPath = "/settings/library";

function libraryListPath(section: LibrarySection) {
  return `${libraryRootPath}/${section}`;
}

function libraryDetailPath(section: LibrarySection, id: number) {
  return `${libraryListPath(section)}/${id}`;
}

function isLibrarySection(value: unknown): value is LibrarySection {
  return typeof value === "string" && librarySections.includes(value as LibrarySection);
}

function isSafeLibraryId(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function isLibraryTrailEntry(value: unknown): value is LibraryTrailEntry {
  if (!value || typeof value !== "object") return false;
  const item = value as Partial<LibraryTrailEntry>;
  const validId = item.id === null || isSafeLibraryId(item.id);
  return isLibrarySection(item.section)
    && validId
    && typeof item.label === "string"
    && item.label.trim().length > 0
    && typeof item.to === "string"
    && item.to.startsWith(`${libraryRootPath}/`);
}

function readLibraryTrail(state: unknown): LibraryTrailEntry[] {
  if (!state || typeof state !== "object") return [];
  const value = (state as Partial<LibraryNavigationState>).libraryTrail;
  if (!Array.isArray(value)) return [];
  return value.filter(isLibraryTrailEntry).map((item) => ({
    section: item.section,
    id: item.id,
    label: item.label.trim(),
    to: item.to,
  }));
}

function libraryTrailKey(entry: Pick<LibraryTrailEntry, "section" | "id">) {
  return `${entry.section}:${entry.id ?? "list"}`;
}

function stateForLibraryTrail(trail: LibraryTrailEntry[]): LibraryNavigationState | undefined {
  return trail.length ? { libraryTrail: trail } : undefined;
}

export function useLibraryNavigation({
  section,
  sectionLabel,
  detailsLabel,
  detailId,
  detailLabel,
  detailPath,
}: LibraryNavigationOptions) {
  const location = useLocation();
  const navigate = useNavigate();
  const routeTrail = useMemo(() => readLibraryTrail(location.state), [location.state]);
  const listEntry = useMemo<LibraryTrailEntry>(() => ({
    section,
    id: null,
    label: sectionLabel,
    to: libraryListPath(section),
  }), [section, sectionLabel]);
  const contextTrail = routeTrail.length ? routeTrail : [listEntry];
  const currentEntry = detailId !== null && isSafeLibraryId(detailId) ? {
    section,
    id: detailId,
    label: detailLabel?.trim() || detailsLabel,
    to: detailPath ?? libraryDetailPath(section, detailId),
  } satisfies LibraryTrailEntry : null;
  const parentTrail = currentEntry
    ? contextTrail.filter((entry) => libraryTrailKey(entry) !== libraryTrailKey(currentEntry))
    : contextTrail;

  const navigateWithTrail = useCallback((to: string, trail: LibraryTrailEntry[], replace = true) => {
    const state = stateForLibraryTrail(trail);
    navigate(to, state ? { replace, state } : { replace });
  }, [navigate]);

  const back = useCallback(() => {
    const parent = parentTrail[parentTrail.length - 1];
    if (!parent) {
      navigateWithTrail(libraryListPath(section), []);
      return;
    }
    navigateWithTrail(parent.to, parentTrail.slice(0, -1));
  }, [navigateWithTrail, parentTrail, section]);

  const openDetail = useCallback((
    targetSection: LibrarySection,
    targetId: number,
    targetLabel: string,
    options: OpenLibraryDetailOptions = {},
  ) => {
    if (!isSafeLibraryId(targetId)) return;
    const target: LibraryTrailEntry = {
      section: targetSection,
      id: targetId,
      label: targetLabel.trim() || detailsLabel,
      to: libraryDetailPath(targetSection, targetId),
    };
    const fullTrail = options.replaceCurrent || !currentEntry
      ? parentTrail
      : [...parentTrail, currentEntry];
    const existingIndex = fullTrail.findIndex((entry) => libraryTrailKey(entry) === libraryTrailKey(target));
    const targetTrail = existingIndex >= 0 ? fullTrail.slice(0, existingIndex) : fullTrail;
    navigateWithTrail(target.to, targetTrail, options.replace ?? false);
  }, [currentEntry, detailsLabel, navigateWithTrail, parentTrail]);

  const breadcrumbs = useMemo<LibraryBreadcrumbItem[]>(() => {
    const items: LibraryBreadcrumbItem[] = [];
    parentTrail.forEach((entry, index) => {
      items.push({
        key: libraryTrailKey(entry),
        label: entry.label,
        onClick: () => navigateWithTrail(entry.to, parentTrail.slice(0, index)),
      });
    });
    if (currentEntry) {
      items.push({
        key: libraryTrailKey(currentEntry),
        label: currentEntry.label,
        current: true,
      });
    }
    return items;
  }, [currentEntry, navigateWithTrail, parentTrail]);

  return { back, breadcrumbs, openDetail, trail: parentTrail };
}

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

export function LibraryToolbar({ query, onQueryChange, filters }: {
  query: string;
  onQueryChange: (value: string) => void;
  filters?: ReactNode;
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
      </div>
      {filters ? <div className={styles.toolbarFilters}>{filters}</div> : null}
    </CardHeader>
  );
}

export function PageControls({ page, pageSize, total, actions, onPageChange, onPageSizeChange, compact = false }: {
  page: number;
  pageSize: number;
  total: number;
  actions?: ReactNode;
  onPageChange: (page: number) => void;
  onPageSizeChange: (pageSize: number) => void;
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const pageItems = visiblePageItems(page, totalPages);
  const content = (
    <>
      <div className={styles.paginationLeading}>
        <span>{t("library.manager.pageSummary", { page, totalPages, total })}</span>
        {actions ? <div className={styles.paginationActions}>{actions}</div> : null}
      </div>
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
          <AlertDialogAction disabled={disabled} variant={confirmVariant} onClick={() => void onConfirm()}>{label}</AlertDialogAction>
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

export function LibraryBreadcrumbs({ items }: { items: LibraryBreadcrumbItem[] }) {
  const { t } = useTranslation();
  return (
    <Breadcrumb className={styles.breadcrumb} aria-label={t("library.manager.breadcrumbLabel")}>
      <BreadcrumbList>
        {items.map((item, index) => (
          <Fragment key={item.key}>
            {index > 0 ? <BreadcrumbSeparator /> : null}
            <BreadcrumbItem>
              {item.current ? (
                <BreadcrumbPage title={item.label}>{item.label}</BreadcrumbPage>
              ) : (
                <BreadcrumbLink
                  render={<button type="button" />}
                  title={item.label}
                  onClick={item.onClick}
                >
                  {item.label}
                </BreadcrumbLink>
              )}
            </BreadcrumbItem>
          </Fragment>
        ))}
      </BreadcrumbList>
    </Breadcrumb>
  );
}

export function LibraryDetailHeader({ title, description, status, actions, onBack, breadcrumbs }: {
  title: ReactNode;
  description?: ReactNode;
  status?: ReactNode;
  actions?: ReactNode;
  onBack: () => void;
  breadcrumbs?: LibraryBreadcrumbItem[];
}) {
  const { t } = useTranslation();
  const hasBreadcrumbs = Boolean(breadcrumbs?.length);
  return (
    <CardHeader className={styles.detailHeader}>
      <div className={styles.detailNavigation}>
        <Button type="button" variant="ghost" size="sm" className={styles.backButton} onClick={onBack}>
          <ArrowLeft data-icon="inline-start" />{t("library.manager.back")}
        </Button>
        {hasBreadcrumbs ? (
          <>
            <Separator className={styles.detailNavigationSeparator} orientation="vertical" aria-hidden="true" />
            <LibraryBreadcrumbs items={breadcrumbs!} />
          </>
        ) : null}
      </div>
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
