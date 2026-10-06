import { createContext, useContext, useState, type Dispatch, type ReactNode, type SetStateAction } from "react";

const RunSelectionContext = createContext<[Set<string>, Dispatch<SetStateAction<Set<string>>>] | null>(null);

export function useRunSelection() {
  const selection = useContext(RunSelectionContext);
  if (!selection) throw new Error("Run selection requires an authenticated user");
  return selection;
}

export function RunSelectionProvider({ children }: { children: ReactNode }) {
  const selection = useState<Set<string>>(() => new Set());
  return <RunSelectionContext.Provider value={selection}>{children}</RunSelectionContext.Provider>;
}
