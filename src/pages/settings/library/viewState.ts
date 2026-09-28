import { useMemo } from "react";
import { useSearchParams } from "react-router";

import type { LibraryLyricStatus } from "@/shared/types/lyrics";
import type { LibrarySection } from "./shared";

type LyricStatusFilter = LibraryLyricStatus | "all";
type SourceFilter = "all" | "managed" | "cache" | "local" | "legacy";
type LibraryView = "list" | "similar";

export type LibraryViewState = {
  query: string;
  page: number;
  pageSize: number;
  status: LyricStatusFilter;
  sourceKind: SourceFilter;
  view: LibraryView;
  similarPage: number;
  similarPageSize: number;
};

const prefixes: Record<LibrarySection, string> = { songs: "s", lyrics: "l", artists: "a" };
const defaults: LibraryViewState = {
  query: "", page: 1, pageSize: 20, status: "all", sourceKind: "all",
  view: "list", similarPage: 1, similarPageSize: 20,
};
const keys: Record<keyof LibraryViewState, string> = {
  query: "q", page: "page", pageSize: "size", status: "status",
  sourceKind: "source", view: "view", similarPage: "similarPage",
  similarPageSize: "similarSize",
};

function positivePage(value: string | null) {
  const number = Number(value);
  return value && Number.isSafeInteger(number) && number > 0 ? number : 1;
}

function pageSize(value: string | null) {
  const number = Number(value);
  return number === 50 || number === 100 ? number : 20;
}

export function readLibraryViewState(params: URLSearchParams, section: LibrarySection): LibraryViewState {
  const get = (key: keyof LibraryViewState) => params.get(`${prefixes[section]}.${keys[key]}`);
  const status = get("status");
  const sourceKind = get("sourceKind");
  return {
    query: get("query") ?? "",
    page: positivePage(get("page")),
    pageSize: pageSize(get("pageSize")),
    status: section === "lyrics" && (status === "inUse" || status === "candidate" || status === "unbound") ? status : "all",
    sourceKind: section === "lyrics" && (sourceKind === "managed" || sourceKind === "cache" || sourceKind === "local" || sourceKind === "legacy") ? sourceKind : "all",
    view: section !== "artists" && get("view") === "similar" ? "similar" : "list",
    similarPage: positivePage(get("similarPage")),
    similarPageSize: pageSize(get("similarPageSize")),
  };
}

export function useLibraryViewState(section: LibrarySection) {
  const [params, setParams] = useSearchParams();
  const state = useMemo(() => readLibraryViewState(params, section), [params, section]);

  // 仅更新当前分类的字段，保留其他分类在地址中的工作位置。
  const update = (patch: Partial<LibraryViewState>, replace = false) => {
    setParams((current) => {
      const next = new URLSearchParams(current);
      for (const [field, value] of Object.entries(patch) as Array<[keyof LibraryViewState, LibraryViewState[keyof LibraryViewState]]>) {
        const key = `${prefixes[section]}.${keys[field]}`;
        if (value === defaults[field]) next.delete(key);
        else next.set(key, String(value));
      }
      return next;
    }, { replace });
  };

  return { ...state, update };
}
