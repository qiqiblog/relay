import { useState } from "react";
import useSWR from "swr";
import { Plus, Pencil, Trash2, Network, Check } from "lucide-react";
import { useConfirm } from "@/hooks/useConfirm";
import { Card, CardContent } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Api, type UserGroup,
} from "@/lib/api";
import { toast } from "sonner";

// ── 主页面 ────────────────────────────────────────────────────────────────────

export default function UserGroupsPage() {
  const confirm = useConfirm();
  const { data: groups = [], mutate } = useSWR("user-groups", Api.listUserGroups);

  const [newName, setNewName] = useState("");
  const [newOpen, setNewOpen] = useState(false);
  const [creating, setCreating] = useState(false);

  const [manageGroup, setManageGroup] = useState<UserGroup | null>(null);

  const create = async () => {
    if (!newName.trim()) { toast.error("套餐名不能为空"); return; }
    setCreating(true);
    try {
      await Api.createUserGroup({ name: newName.trim() });
      setNewOpen(false);
      setNewName("");
      mutate();
      toast.success("已创建");
    } catch (e: any) {
      toast.error(e?.message ?? "创建失败");
    } finally {
      setCreating(false);
    }
  };

  const remove = async (g: UserGroup) => {
    if (!await confirm(`确定删除套餐「${g.name}」？隧道分配模板将一并删除（已同步的 user_tunnels 不受影响）。`)) return;
    try {
      await Api.deleteUserGroup(g.id);
      mutate();
      toast.success("已删除");
    } catch (e: any) {
      toast.error(e?.message ?? "删除失败");
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">套餐</h1>
        <Button onClick={() => setNewOpen(true)}>
          <Plus className="mr-1 h-4 w-4" /> 新建套餐
        </Button>
      </div>

      <Card>
        <CardContent className="p-0">
          <ScrollArea>
            <Table className="min-w-[640px] table-fixed">
              <colgroup>
                <col style={{ width: "9rem" }} />
                <col style={{ width: "5rem" }} />
                <col style={{ width: "5rem" }} />
                <col style={{ width: "7rem" }} />
                <col style={{ width: "7rem" }} />
                <col style={{ width: "7rem" }} />
                <col />
                <col style={{ width: "6rem" }} />
              </colgroup>
              <TableHeader>
                <TableRow>
                  <TableHead>套餐名</TableHead>
                  <TableHead>隧道数</TableHead>
                  <TableHead>成员数</TableHead>
                  <TableHead>流量限制</TableHead>
                  <TableHead>限速</TableHead>
                  <TableHead>转发上限</TableHead>
                  <TableHead>备注</TableHead>
                  <TableHead>操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {groups.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={8} className="py-12 text-center">
                      <div className="flex flex-col items-center gap-2 text-muted-foreground">
                        <Network className="h-8 w-8 opacity-40" />
                        <span className="text-sm">暂无套餐</span>
                      </div>
                    </TableCell>
                  </TableRow>
                ) : (
                  groups.map((g) => (
                    <TableRow key={g.id}>
                      <TableCell>
                        <button
                          className="font-medium hover:underline text-left"
                          onClick={() => setManageGroup(g)}
                        >
                          {g.name}
                        </button>
                      </TableCell>
                      <TableCell>
                        <Badge variant="outline" className="text-sm px-2 py-0.5">
                          {g.tunnel_count} 条
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <Badge variant="secondary">{g.member_count} 人</Badge>
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        {g.flow_limit_bytes > 0
                          ? `${(g.flow_limit_bytes / 1_073_741_824).toFixed(0)} GB`
                          : "不限"}
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        {g.speed_limit_kbps > 0
                          ? `${(g.speed_limit_kbps / 125).toFixed(0)} Mbps`
                          : "不限"}
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        {g.forward_limit > 0 ? `${g.forward_limit} 条` : "不限"}
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        {g.remark || "—"}
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-1">
                          <Button size="icon" variant="ghost" onClick={() => setManageGroup(g)} title="配置">
                            <Pencil className="h-4 w-4" />
                          </Button>
                          <Button
                            size="icon"
                            variant="ghost"
                            className="text-destructive hover:text-destructive"
                            onClick={() => remove(g)}
                            title="删除"
                          >
                            <Trash2 className="h-4 w-4" />
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </ScrollArea>
        </CardContent>
      </Card>

      {/* 新建套餐 */}
      <Dialog open={newOpen} onOpenChange={setNewOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>新建套餐</DialogTitle>
          </DialogHeader>
          <div className="space-y-1.5">
            <Label>套餐名</Label>
            <Input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && create()}
              autoFocus
            />
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setNewOpen(false)} disabled={creating}>取消</Button>
            <Button onClick={create} disabled={creating}>{creating ? "创建中…" : "创建"}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 套餐配置面板 */}
      {manageGroup && (
        <GroupManageDialog
          group={manageGroup}
          onClose={() => { setManageGroup(null); mutate(); }}
        />
      )}
    </div>
  );
}

// ── 套餐配置面板 ───────────────────────────────────────────────────────────────

function GroupManageDialog({ group, onClose }: { group: UserGroup; onClose: () => void }) {
  const [name, setName] = useState(group.name);
  const [remark, setRemark] = useState(group.remark ?? "");
  // 流量 GB 显示值（内部存储为字符串便于 Input 绑定）
  const [limitGb, setLimitGb] = useState(
    group.flow_limit_bytes > 0
      ? String(group.flow_limit_bytes / 1_073_741_824)
      : ""
  );
  const [speedMbps, setSpeedMbps] = useState(
    group.speed_limit_kbps > 0 ? String(group.speed_limit_kbps / 125) : ""
  );
  const [tunnelLimit, setTunnelLimit] = useState(
    group.forward_limit > 0 ? String(group.forward_limit) : ""
  );
  const [saving, setSaving] = useState(false);


  const save = async () => {
    if (!name.trim()) { toast.error("套餐名不能为空"); return; }
    const gb = limitGb.trim() ? Number(limitGb) : 0;
    const mbps = speedMbps.trim() ? Number(speedMbps) : 0;
    const tl = tunnelLimit.trim() ? Number(tunnelLimit) : 0;
    if (!isFinite(gb) || gb < 0) { toast.error("流量限制格式不正确"); return; }
    if (!isFinite(mbps) || mbps < 0) { toast.error("限速格式不正确"); return; }
    if (!isFinite(tl) || tl < 0 || !Number.isInteger(tl)) { toast.error("转发上限须为非负整数"); return; }
    setSaving(true);
    try {
      await Api.updateUserGroup(group.id, {
        name: name.trim(),
        remark,
        flow_limit_gb: gb,
        speed_limit_kbps: Math.round(mbps * 125),
        forward_limit: tl,
      });
      toast.success("已保存");
      onClose();
    } catch (e: any) {
      toast.error(e?.message ?? "保存失败");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>配置套餐</DialogTitle>
        </DialogHeader>

        {/* 套餐基本信息 + 限制参数编辑区 */}
        <div className="space-y-2 pb-3 border-b">
          {/* 第一行：套餐名 + 备注 */}
          <div className="flex gap-2 items-end">
            <div className="space-y-1 flex-1">
              <Label className="text-xs text-muted-foreground">套餐名</Label>
              <Input value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div className="space-y-1 flex-1">
              <Label className="text-xs text-muted-foreground">备注</Label>
              <Input value={remark} onChange={(e) => setRemark(e.target.value)} placeholder="可选" />
            </div>
          </div>
          {/* 第二行：流量 + 限速 + 转发上限 */}
          <div className="flex gap-2 items-end">
            <div className="space-y-1 flex-1">
              <Label className="text-xs text-muted-foreground">流量 GB</Label>
              <Input
                type="number" min={0}
                value={limitGb}
                onChange={(e) => setLimitGb(e.target.value)}
                placeholder="不填=不限"
              />
            </div>
            <div className="space-y-1 flex-1">
              <Label className="text-xs text-muted-foreground">限速 Mbps</Label>
              <Input
                type="number" min={0} step="1"
                value={speedMbps}
                onChange={(e) => setSpeedMbps(e.target.value)}
                placeholder="不填=不限"
              />
            </div>
            <div className="space-y-1 flex-1">
              <Label className="text-xs text-muted-foreground">转发上限（条）</Label>
              <Input
                type="number" min={0} step="1"
                value={tunnelLimit}
                onChange={(e) => setTunnelLimit(e.target.value)}
                placeholder="不填=不限"
              />
            </div>
          </div>
        </div>

        <div className="min-h-[240px]">
          <TunnelsTab groupId={group.id} />
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>关闭</Button>
          <Button onClick={save} disabled={saving}>
            {saving ? "保存中…" : "保存"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ── 隧道配置 ──────────────────────────────────────────────────────────────────

function TunnelsTab({ groupId }: { groupId: string }) {
  const { data: groupTunnels = [], mutate } = useSWR(
    ["group-tunnels", groupId],
    () => Api.listGroupTunnels(groupId),
  );
  const { data: allTunnels = [] } = useSWR("tunnels", Api.listTunnels);

  const [toggling, setToggling] = useState<Set<string>>(new Set());

  const assignedMap = new Map(groupTunnels.map((gt) => [gt.tunnel_id, gt]));

  const toggle = async (tunnelId: string, checked: boolean) => {
    setToggling((prev) => new Set(prev).add(tunnelId));
    try {
      if (checked) {
        await Api.createGroupTunnel(groupId, { tunnel_id: tunnelId });
      } else {
        const gt = assignedMap.get(tunnelId)!;
        await Api.deleteGroupTunnel(groupId, gt.id);
      }
      mutate();
    } catch (e: any) {
      toast.error(e?.message ?? "操作失败");
    } finally {
      setToggling((prev) => { const s = new Set(prev); s.delete(tunnelId); return s; });
    }
  };

  if (allTunnels.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-muted-foreground">暂无可用隧道</p>
    );
  }

  return (
    <div className="flex flex-wrap gap-2">
      {allTunnels.map((t) => {
        const checked = assignedMap.has(t.id);
        const busy = toggling.has(t.id);
        return (
          <label
            key={t.id}
            className={`flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-sm cursor-pointer select-none transition-colors ${
              busy ? "opacity-50 cursor-not-allowed" : "hover:bg-muted/50"
            } ${checked ? "border-primary bg-primary/5" : "border-input"}`}
          >
            <input
              type="checkbox"
              checked={checked}
              disabled={busy}
              onChange={(e) => toggle(t.id, e.target.checked)}
              className="sr-only"
            />
            <span className={`flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded-sm border transition-colors ${checked ? "bg-primary border-primary" : "border-input bg-background"}`}>
              {checked && <Check className="h-2.5 w-2.5 text-primary-foreground" strokeWidth={3} />}
            </span>
            <span className="font-medium">{t.name}</span>
            <span className="text-xs text-muted-foreground">{(t.protocols ?? []).map((p) => p.toUpperCase()).join("+")}</span>
          </label>
        );
      })}
    </div>
  );
}
