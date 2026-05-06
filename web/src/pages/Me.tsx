import { useState } from "react";
import useSWR from "swr";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Api } from "@/lib/api";
import { toast } from "sonner";


export default function MePage() {
  const { data: me } = useSWR("me", Api.getMe);

  const [oldPwd, setOldPwd] = useState("");
  const [newPwd, setNewPwd] = useState("");
  const [confirmPwd, setConfirmPwd] = useState("");
  const [saving, setSaving] = useState(false);

  const changePassword = async () => {
    if (newPwd.length < 8) {
      toast.error("新密码至少 8 位");
      return;
    }
    if (newPwd !== confirmPwd) {
      toast.error("两次输入的新密码不一致");
      return;
    }
    setSaving(true);
    try {
      await Api.changeOwnPassword({ old_password: oldPwd, new_password: newPwd });
      toast.success("密码已更新");
      setOldPwd("");
      setNewPwd("");
      setConfirmPwd("");
    } catch (e: any) {
      toast.error(e?.message ?? "修改失败");
    } finally {
      setSaving(false);
    }
  };

  if (!me) return null;

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold">我的</h1>

      <Card>
        <CardContent className="pt-6">
          <div className="flex items-center gap-4">
            <div className="h-14 w-14 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
              <span className="text-xl font-semibold text-primary select-none">
                {me.username.charAt(0).toUpperCase()}
              </span>
            </div>
            <div className="min-w-0 space-y-2">
              <div className="flex items-center gap-2 flex-wrap">
                <span className="text-lg font-semibold">{me.username}</span>
                <Badge variant="secondary">{me.role}</Badge>
                <Badge variant={me.status === "active" ? "success" : "outline"}>{me.status}</Badge>
              </div>
              <div className="flex gap-5 text-sm flex-wrap">
                {me.group_name && (
                  <div>
                    <span className="text-sm text-muted-foreground block">套餐</span>
                    <span className="font-medium">{me.group_name}</span>
                  </div>
                )}
                <div>
                  <span className="text-sm text-muted-foreground block">到期</span>
                  <span className="font-medium">{me.expires_at ? new Date(me.expires_at).toLocaleDateString() : "无限期"}</span>
                </div>
                <div>
                  <span className="text-sm text-muted-foreground block">转发数</span>
                  <span className="font-medium">{me.forward_count}</span>
                </div>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>修改密码</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3 max-w-md">
          <div className="space-y-1.5">
            <Label>当前密码</Label>
            <Input type="password" value={oldPwd} onChange={(e) => setOldPwd(e.target.value)} />
          </div>
          <div className="space-y-1.5">
            <Label>新密码</Label>
            <Input type="password" value={newPwd} onChange={(e) => setNewPwd(e.target.value)} />
          </div>
          <div className="space-y-1.5">
            <Label>确认新密码</Label>
            <Input type="password" value={confirmPwd} onChange={(e) => setConfirmPwd(e.target.value)} />
          </div>
          <Button onClick={changePassword} disabled={saving}>
            {saving ? "保存中…" : "修改密码"}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
