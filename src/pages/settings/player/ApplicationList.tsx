import { Music2, X } from "lucide-react";

import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { IconButton } from "@/components/ui/icon-button";
import { Item, ItemActions, ItemContent, ItemGroup, ItemMedia, ItemTitle } from "@/components/ui/item";
import type { RegisteredApplication } from "@/shared/types";

import styles from "../settings.module.scss";

export function ApplicationList({ applications, icons, names, busy, emptyLabel, removeLabel, onRemove }: {
  applications: RegisteredApplication[];
  icons: Record<string, string>;
  names: Record<string, string>;
  busy: boolean;
  emptyLabel: string;
  removeLabel: string;
  onRemove: (bundleId: string) => void;
}) {
  if (applications.length === 0) {
    return (
      <Empty className={styles.applicationEmpty}>
        <EmptyHeader>
          <EmptyMedia variant="icon"><Music2 /></EmptyMedia>
          <EmptyTitle>{emptyLabel}</EmptyTitle>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <ItemGroup className={styles.applicationList}>
      {applications.map((application) => {
        const displayName = names[application.bundleId] ?? application.name;
        return (
          <Item variant="muted" className={styles.applicationItem} key={application.bundleId}>
            <ItemMedia variant={icons[application.bundleId] ? "image" : "icon"}>
              {icons[application.bundleId] ? <img alt="" src={icons[application.bundleId]} /> : <Music2 />}
            </ItemMedia>
            <ItemContent className={styles.applicationContent}><ItemTitle className={styles.applicationName} title={displayName}>{displayName}</ItemTitle></ItemContent>
            <ItemActions>
              <IconButton label={`${removeLabel} ${displayName}`} tooltip={removeLabel} variant="ghost" size="icon-sm" disabled={busy} onClick={() => onRemove(application.bundleId)}><X /></IconButton>
            </ItemActions>
          </Item>
        );
      })}
    </ItemGroup>
  );
}
