import { Navigate, Outlet } from "react-router-dom";
import { getRole } from "@/lib/auth";

export default function RequireAdmin() {
  if (getRole() !== "admin") {
    return <Navigate to="/me" replace />;
  }
  return <Outlet />;
}
