import { useState } from "react";
import useSWR from "swr";
import { toast } from "sonner";
import { Copy, Check } from "lucide-react";
import ReactMarkdown from "react-markdown";
import { Api, ApiError } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";

export default function ConfigPage() {
  const { data: cfg, mutate } = useSWR("system-config", Api.getConfig);
  const { data: sysVersion, mutate: mutateVersion } = useSWR(
    "system-version",
    Api.getSystemVersion,
  );
  const { data: branding, mutate: mutateBranding } = useSWR(
    "system-branding",
    Api.getBranding,
  );

  const [enabled, setEnabled] = useState<boolean | undefined>(undefined);
  const [title, setTitle] = useState<string | undefined>(undefined);
  const [content, setContent] = useState<string | undefined>(undefined);
  const [saving, setSaving] = useState(false);
  const [channelSaving, setChannelSaving] = useState(false);
  const [copied, setCopied] = useState(false);
  const [upgradeRegion, setUpgradeRegion] = useState<"global" | "cn">("global");
  const [brandDraft, setBrandDraft] = useState<string | undefined>(undefined);
  const [brandSaving, setBrandSaving] = useState(false);

  const effectiveEnabled = enabled ?? cfg?.announcement_enabled ?? false;
  const effectiveTitle = title ?? cfg?.announcement_title ?? "";
  const effectiveContent = content ?? cfg?.announcement_content ?? "";

  const save = async () => {
    setSaving(true);
    try {
      await Api.updateConfig({
        announcement_enabled: effectiveEnabled,
        announcement_title: effectiveTitle,
        announcement_content: effectiveContent,
      });
      await mutate();
      setEnabled(undefined);
      setTitle(undefined);
      setContent(undefined);
      toast.success("配置已保存");
    } catch (e: any) {
      toast.error(e?.message ?? "保存失败");
    } finally {
      setSaving(false);
    }
  };

  const setChannel = async (next: "stable" | "rc") => {
    if (sysVersion?.channel === next || channelSaving) return;
    setChannelSaving(true);
    try {
      await Api.setUpgradeChannel(next);
      await mutateVersion();
      toast.success(`已切换升级通道：${next}`);
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : String(e));
    } finally {
      setChannelSaving(false);
    }
  };

  const masterVersion = sysVersion?.master_version ?? "—";
  const masterTagged = masterVersion.startsWith("v") ? masterVersion : `v${masterVersion}`;
  const upgradeTarget = sysVersion?.latest_stable?.tag ?? masterTagged;
  const masterUpgradeCmd = (region: "global" | "cn" = upgradeRegion) => {
    const scriptUrl = region === "cn"
      ? `${window.location.origin}/scripts/install.sh`
      : "https://raw.githubusercontent.com/0xUnixIO/relay/main/install.sh";
    return `curl -fsSL ${scriptUrl} | sudo bash -s -- --update --version ${upgradeTarget}`;
  };
  const copyCmd = async () => {
    try {
      await navigator.clipboard.writeText(masterUpgradeCmd(upgradeRegion));
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      toast.error("复制失败");
    }
  };

  const effectiveBrand = brandDraft ?? branding?.brand_name ?? "RELAY";
  const brandDirty = brandDraft !== undefined && brandDraft.trim() !== (branding?.brand_name ?? "");
  const saveBrand = async () => {
    const next = effectiveBrand.trim();
    if (!next || next === (branding?.brand_name ?? "")) return;
    setBrandSaving(true);
    try {
      await Api.setBranding(next);
      await mutateBranding();
      setBrandDraft(undefined);
      toast.success("品牌名称已保存（刷新页面生效）");
    } catch (e) {
      toast.error(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBrandSaving(false);
    }
  };

  return (
    <div className="space-y-6 max-w-2xl">
      <div>
        <h1 className="text-xl font-semibold">系统配置</h1>
        <p className="text-sm text-muted-foreground mt-1">全局设置，对所有用户生效。</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">品牌名称</CardTitle>
          <CardDescription>
            显示在侧边栏左上角与登录页。最长 32 个字符，会自动以大写形式展示。
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex gap-2">
            <Input
              placeholder="RELAY"
              value={effectiveBrand}
              maxLength={32}
              onChange={(e) => setBrandDraft(e.target.value)}
            />
            <Button onClick={saveBrand} disabled={!brandDirty || brandSaving}>
              {brandSaving ? "保存中…" : "保存"}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">公告弹窗</CardTitle>
          <CardDescription>启用后，所有登录用户首次打开面板时将看到此公告。</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center gap-3">
            <Switch
              id="ann-enabled"
              checked={effectiveEnabled}
              onCheckedChange={setEnabled}
            />
            <Label htmlFor="ann-enabled">启用公告</Label>
          </div>

          <div className="space-y-1.5">
            <Label>标题</Label>
            <Input
              placeholder="公告标题"
              value={effectiveTitle}
              onChange={(e) => setTitle(e.target.value)}
              disabled={!effectiveEnabled}
            />
          </div>

          <div className="space-y-1.5">
            <Label>内容</Label>
            <Textarea
              placeholder="公告内容（支持 Markdown）"
              value={effectiveContent}
              onChange={(e) => setContent(e.target.value)}
              disabled={!effectiveEnabled}
              rows={5}
            />
            {effectiveContent && (
              <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900 dark:border-amber-800/30 dark:bg-amber-950/30 dark:text-amber-200">
                <p className="text-xs text-amber-600 dark:text-amber-400 mb-1.5 font-medium">预览</p>
                <ReactMarkdown components={{
                  p: ({ children }) => <p className="mb-1 last:mb-0">{children}</p>,
                  strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
                  em: ({ children }) => <em className="italic">{children}</em>,
                  a: ({ href, children }) => (
                    <a href={href} target="_blank" rel="noopener noreferrer" className="underline underline-offset-2">
                      {children}
                    </a>
                  ),
                  ul: ({ children }) => <ul className="list-disc ml-4 space-y-0.5 mb-1">{children}</ul>,
                  ol: ({ children }) => <ol className="list-decimal ml-4 space-y-0.5 mb-1">{children}</ol>,
                  h1: ({ children }) => <p className="font-semibold mb-1">{children}</p>,
                  h2: ({ children }) => <p className="font-semibold mb-1">{children}</p>,
                  h3: ({ children }) => <p className="font-medium mb-1">{children}</p>,
                  code: ({ children }) => <code className="font-mono text-xs bg-amber-100 dark:bg-amber-900/40 px-1 rounded">{children}</code>,
                }}>
                  {effectiveContent}
                </ReactMarkdown>
              </div>
            )}
          </div>

          <Button onClick={save} disabled={saving}>
            {saving ? "保存中…" : "保存"}
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">升级通道</CardTitle>
          <CardDescription>
            决定 UI 上「跟随通道」升级节点时使用 stable 还是 rc 的最新版。
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex gap-2">
            {(["stable", "rc"] as const).map((c) => {
              const active = sysVersion?.channel === c;
              return (
                <button
                  key={c}
                  type="button"
                  onClick={() => setChannel(c)}
                  disabled={channelSaving}
                  className={`flex-1 rounded-md border px-3 py-2 text-sm text-left ${
                    active
                      ? "border-primary bg-primary/5 text-foreground"
                      : "border-border text-muted-foreground hover:border-foreground/30"
                  } disabled:opacity-50`}
                >
                  <div className="font-medium">{c === "stable" ? "稳定版" : "预发布"}</div>
                  <div className="text-xs">
                    {c === "stable"
                      ? sysVersion?.latest_stable?.tag ?? "（暂无）"
                      : sysVersion?.latest_rc?.tag ?? "（暂无）"}
                  </div>
                </button>
              );
            })}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base flex items-center gap-2">
            Master 版本
            <Badge variant="outline" className="font-mono text-xs">
              {masterVersion}
            </Badge>
          </CardTitle>
          <CardDescription>
            Master 端不支持远程升级，请 SSH 到 master 服务器手动执行下列命令。
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex gap-0.5 rounded-lg bg-muted p-0.5 text-xs">
              {(["global", "cn"] as const).map((r) => (
                <button
                  key={r}
                  onClick={() => setUpgradeRegion(r)}
                  className={cn(
                    "rounded-md px-2.5 py-1 font-medium transition-colors",
                    upgradeRegion === r
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground"
                  )}
                >
                  {r === "global" ? "国际线路" : "国内线路"}
                </button>
              ))}
            </div>
          </div>
          <div className="flex gap-2 items-stretch">
            <code className="flex-1 rounded-md border bg-muted/30 px-3 py-2 text-xs font-mono break-all">
              {masterUpgradeCmd(upgradeRegion)}
            </code>
            <Button variant="outline" size="sm" onClick={copyCmd} className="flex-shrink-0">
              {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            </Button>
          </div>
          {sysVersion?.latest_stable?.tag && (
            <p className="text-xs text-muted-foreground">
              最新稳定版：{sysVersion.latest_stable.tag}
              {sysVersion?.latest_rc?.tag && <> · 最新预发布：{sysVersion.latest_rc.tag}</>}
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
