import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

type Props = {
  text: string;
  className?: string;
};

/** Yazg chat markdown via react-markdown + remark-gfm. */
export function YazgMarkdown({ text, className }: Props) {
  const source = text.trim();
  if (!source) {
    return <div className={className} />;
  }

  return (
    <div className={className}>
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{source}</ReactMarkdown>
    </div>
  );
}
