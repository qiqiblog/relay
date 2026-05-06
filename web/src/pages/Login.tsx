import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Network } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Api, ApiError } from "@/lib/api";
import { setToken, setUser, setRole, getToken } from "@/lib/auth";
import { toast } from "sonner";

export default function Login() {
  const navigate = useNavigate();
  const [mode, setMode] = useState<"login" | "bootstrap" | "loading">("loading");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (getToken()) {
      navigate("/", { replace: true });
      return;
    }
    Api.authStatus()
      .then((s) => setMode(s.bootstrapped ? "login" : "bootstrap"))
      .catch(() => setMode("login"));
  }, [navigate]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      if (mode === "bootstrap") {
        await Api.bootstrap(username, password);
        toast.success("管理员创建成功，正在登录…");
      }
      const r = await Api.login(username, password);
      setToken(r.token);
      setUser(r.username ?? username);
      setRole(r.role);
      navigate("/", { replace: true });
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : String(e));
    } finally {
      setSubmitting(false);
    }
  };

  if (mode === "loading") {
    return <div className="flex min-h-screen items-center justify-center text-muted-foreground">…</div>;
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-muted/30 p-4">
      <Card className="w-full max-w-sm">
        <CardHeader className="space-y-1">
          <div className="flex items-center gap-2 text-lg font-semibold">
            <Network className="h-5 w-5" /> relay
          </div>
          <CardTitle className="text-xl">
            {mode === "bootstrap" ? "创建管理员" : "登录"}
          </CardTitle>
          <CardDescription>
            {mode === "bootstrap"
              ? "尚未配置用户，请创建首个管理员账号。"
              : "请输入管理员凭据。"}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="u">用户名</Label>
              <Input id="u" value={username} onChange={(e) => setUsername(e.target.value)} required autoFocus />
            </div>
            <div className="space-y-2">
              <Label htmlFor="p">密码</Label>
              <Input id="p" type="password" value={password} onChange={(e) => setPassword(e.target.value)} required />
            </div>
            <Button className="w-full" disabled={submitting}>
              {submitting ? "…" : mode === "bootstrap" ? "创建并登录" : "登录"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
