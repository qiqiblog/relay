import useSWR from "swr";
import ReactMarkdown from "react-markdown";
import { Api } from "@/lib/api";

const mdComponents: React.ComponentProps<typeof ReactMarkdown>["components"] = {
  p: ({ children }) => <span className="mr-1">{children}</span>,
  strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
  em: ({ children }) => <em className="italic">{children}</em>,
  a: ({ href, children }) => (
    <a href={href} target="_blank" rel="noopener noreferrer" className="underline underline-offset-2">
      {children}
    </a>
  ),
  ul: ({ children }) => <ul className="list-disc ml-4">{children}</ul>,
  ol: ({ children }) => <ol className="list-decimal ml-4">{children}</ol>,
  code: ({ children }) => <code className="font-mono text-xs bg-amber-100 dark:bg-amber-900/40 px-1 rounded">{children}</code>,
};

export default function AnnouncementBanner() {
  const { data: cfg } = useSWR("system-config", Api.getConfig, {
    revalidateOnFocus: false,
  });

  if (!cfg?.announcement_enabled) return null;

  return (
    <div className="border-b border-amber-200 bg-amber-50 px-6 py-2.5 text-sm text-amber-900 dark:border-amber-800/30 dark:bg-amber-950/40 dark:text-amber-200">
      {cfg.announcement_title && (
        <span className="font-medium mr-2">{cfg.announcement_title}</span>
      )}
      <span className="opacity-80">
        <ReactMarkdown components={mdComponents}>{cfg.announcement_content}</ReactMarkdown>
      </span>
    </div>
  );
}
