import { getRole } from "@/lib/auth";
import Dashboard from "@/pages/Dashboard";
import UserDashboard from "@/pages/UserDashboard";

export default function RoleHome() {
  if (getRole() === "admin") return <Dashboard />;
  return <UserDashboard />;
}
