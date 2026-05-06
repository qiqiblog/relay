import { createContext, useCallback, useContext, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";

type ConfirmFn = (message: string, title?: string) => Promise<boolean>;

const ConfirmContext = createContext<ConfirmFn>(() => Promise.resolve(false));

export function ConfirmProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<{ message: string; title?: string } | null>(null);
  const resolveRef = useRef<(v: boolean) => void>(() => {});

  const confirm: ConfirmFn = useCallback((message, title) =>
    new Promise((resolve) => {
      resolveRef.current = resolve;
      setState({ message, title });
    }), []);

  const settle = (ok: boolean) => {
    resolveRef.current(ok);
    setState(null);
  };

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      <Dialog open={!!state} onOpenChange={(o) => !o && settle(false)}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>{state?.title ?? "确认操作"}</DialogTitle>
            <DialogDescription>{state?.message}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => settle(false)}>取消</Button>
            <Button variant="destructive" onClick={() => settle(true)}>确认</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </ConfirmContext.Provider>
  );
}

export function useConfirm(): ConfirmFn {
  return useContext(ConfirmContext);
}
