import { Navigate, Outlet } from "react-router-dom";
import { useEffect, useState } from "react";
import { getToken, getRole, setRole, setUser } from "@/lib/auth";
import { Api } from "@/lib/api";

export default function ProtectedRoute() {
  const token = getToken();
  const [ready, setReady] = useState(getRole() !== null);

  useEffect(() => {
    if (!token || ready) return;
    Api.getMe()
      .then((me) => {
        setRole(me.role);
        setUser(me.username);
      })
      .catch(() => {})
      .finally(() => setReady(true));
  }, [token, ready]);

  if (!token) return <Navigate to="/login" replace />;
  if (!ready) return null;
  return <Outlet />;
}
