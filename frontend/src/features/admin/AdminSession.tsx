import { createContext, useContext, useEffect, useState } from "react";
import { apiRequest } from "../../lib/api/client";
import type { Session } from "../../lib/api/types";

interface AdminContextValue {
  session: Session | null;
  loading: boolean;
  setSession: (session: Session | null) => void;
}

const AdminContext = createContext<AdminContextValue | null>(null);

export function AdminSessionProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    apiRequest<Session>("/api/v1/admin/session")
      .then(setSession)
      .catch(() => setSession(null))
      .finally(() => setLoading(false));
  }, []);

  return (
    <AdminContext.Provider value={{ session, loading, setSession }}>
      {children}
    </AdminContext.Provider>
  );
}

export function useAdminSession() {
  const context = useContext(AdminContext);
  if (!context) throw new Error("管理员上下文缺失");
  return context;
}

export function csrfHeaders(session: Session) {
  return { "X-CSRF-Token": session.csrf_token };
}
