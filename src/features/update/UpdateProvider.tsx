import { createContext, useContext } from "react";
import { UpdateDialog } from "./UpdateDialog";
import { useUpdateController, type UpdateContextValue } from "./useUpdateController";

export type { UpdateKind, UpdateStatus } from "./useUpdateController";

const UpdateContext = createContext<UpdateContextValue | null>(null);

export function UpdateProvider({ children }: { children: React.ReactNode }) {
  const controller = useUpdateController();

  return (
    <UpdateContext.Provider value={controller.value}>
      {children}
      <UpdateDialog {...controller.dialog} />
    </UpdateContext.Provider>
  );
}

export function useUpdates() {
  const value = useContext(UpdateContext);
  if (!value) throw new Error("useUpdates must be used within UpdateProvider");
  return value;
}
