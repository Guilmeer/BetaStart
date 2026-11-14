import { Fragment, useMemo, type ReactNode } from "react";
import { useI18n } from "./i18n";

export interface MarkdownHandlers {
  /** Resolve um link `[[destino]]` para o caminho real do arquivo. */
  resolveWikiLink?: (label: string) => string | null;
  /** Chamado ao clicar em um link `[[destino]]`. */
  onOpenWikiLink?: (label: string, path: string | null) => void;
  /** Chamado ao clicar em um link externo. */
  onOpenUrl?: (url: string) => void;
}

const LIST_RE = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
const HEADING_RE = /^(#{1,6})\s+(.*)$/;
const FENCE_RE = /^\s*(```|~~~)(.*)$/;
const HR_RE = /^\s*([-*_])(?:\s*\1){2,}\s*$/;
const QUOTE_RE = /^\s*>\s?(.*)$/;
const TABLE_SEPARATOR_RE =
  /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)+\|?\s*$/;

const INLINE_PATTERN = [
  "`[^`\\n]+`",
  "\\[\\[[^\\]\\n]+\\]\\]",
  "!\\[[^\\]\\n]*\\]\\([^)\\s]+\\)",
  "\\[[^\\]\\n]+\\]\\([^)\\s]+\\)",
  "\\*\\*[^*\\n]+\\*\\*",
  "__[^_\\n]+__",
  "~~[^~\\n]+~~",
  "==[^=\\n]+==",
  "\\*[^*\\n]+\\*",
  "(?<![\\w])_[^_\\n]+_(?![\\w])",
  "https?:\\/\\/[^\\s<]+",
].join("|");

function leadingSpaces(value: string): number {
  return value.match(/^\s*/)?.[0].length ?? 0;
}

function isSetextUnderline(line: string | undefined): boolean {
  return !!line && /^\s*(=+|-{2,})\s*$/.test(line);
}

function isTableStart(lines: string[], index: number): boolean {
  const line = lines[index];
  const next = lines[index + 1];
  return (
    !!line &&
    !!next &&
    line.includes("|") &&
    TABLE_SEPARATOR_RE.test(next)
  );
}

function isBlockStart(line: string): boolean {
  return (
    HEADING_RE.test(line) ||
    FENCE_RE.test(line) ||
    HR_RE.test(line) ||
    QUOTE_RE.test(line) ||
    LIST_RE.test(line)
  );
}

function plainText(value: string, keyBase: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  value.split("\n").forEach((part, index) => {
    if (index > 0) nodes.push(<br key={`${keyBase}-br-${index}`} />);
    if (part) nodes.push(part);
  });
  return nodes;
}

function renderInline(
  text: string,
  keyBase: string,
  handlers: MarkdownHandlers,
): ReactNode[] {
  const nodes: ReactNode[] = [];
  const regex = new RegExp(INLINE_PATTERN, "g");
  let cursor = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(text)) !== null) {
    if (match.index > cursor) {
      nodes.push(
        ...plainText(text.slice(cursor, match.index), `${keyBase}-t-${cursor}`),
      );
    }
    nodes.push(renderToken(match[0], `${keyBase}-${match.index}`, handlers));
    cursor = match.index + match[0].length;
  }

  if (cursor < text.length) {
    nodes.push(...plainText(text.slice(cursor), `${keyBase}-t-${cursor}`));
  }
  return nodes;
}

function renderToken(
  token: string,
  key: string,
  handlers: MarkdownHandlers,
): ReactNode {
  if (token.startsWith("`") && token.endsWith("`")) {
    return (
      <code className="md-inline-code" key={key}>
        {token.slice(1, -1)}
      </code>
    );
  }

  if (token.startsWith("[[") && token.endsWith("]]")) {
    const raw = token.slice(2, -2);
    const [labelPart, alias] = raw.split("|");
    const label = (labelPart ?? "").trim();
    const target = label.split("#")[0].trim();
    const path = handlers.resolveWikiLink?.(target) ?? null;
    return (
      <button
        type="button"
        key={key}
        className={`md-wikilink${path ? "" : " unresolved"}`}
        onClick={() => handlers.onOpenWikiLink?.(target, path)}
      >
        {(alias ?? label).trim() || label || target}
      </button>
    );
  }

  if (token.startsWith("![")) {
    const image = token.match(/^!\[([^\]]*)\]\(([^)\s]+)\)$/);
    if (image) {
      const [, alt, url] = image;
      if (/^(https?:|data:image\/)/i.test(url)) {
        return (
          <img
            key={key}
            className="md-image"
            src={url}
            alt={alt}
            loading="lazy"
          />
        );
      }
      return (
        <span key={key} className="md-image-missing" title={url}>
          {alt || url}
        </span>
      );
    }
  }

  const link = token.match(/^\[([^\]]+)\]\(([^)\s]+)\)$/);
  if (link) {
    const [, label, url] = link;
    return (
      <a
        key={key}
        className="md-link"
        href={url}
        title={url}
        onClick={(event) => {
          event.preventDefault();
          handlers.onOpenUrl?.(url);
        }}
      >
        {renderInline(label, `${key}-label`, handlers)}
      </a>
    );
  }

  if (token.startsWith("**") || token.startsWith("__")) {
    return (
      <strong key={key}>
        {renderInline(token.slice(2, -2), `${key}-strong`, handlers)}
      </strong>
    );
  }

  if (token.startsWith("~~")) {
    return (
      <del key={key}>
        {renderInline(token.slice(2, -2), `${key}-del`, handlers)}
      </del>
    );
  }

  if (token.startsWith("==")) {
    return (
      <mark key={key}>
        {renderInline(token.slice(2, -2), `${key}-mark`, handlers)}
      </mark>
    );
  }

  if (token.startsWith("*") || token.startsWith("_")) {
    return (
      <em key={key}>
        {renderInline(token.slice(1, -1), `${key}-em`, handlers)}
      </em>
    );
  }

  const trailing = token.match(/[.,;:!?]+$/)?.[0] ?? "";
  const url = trailing ? token.slice(0, -trailing.length) : token;
  const anchor = (
    <a
      key={key}
      className="md-url"
      href={url}
      title={url}
      onClick={(event) => {
        event.preventDefault();
        handlers.onOpenUrl?.(url);
      }}
    >
      {url}
    </a>
  );
  if (!trailing) return anchor;
  return (
    <Fragment key={`${key}-frag`}>
      {anchor}
      {trailing}
    </Fragment>
  );
}

function splitRow(row: string): string[] {
  let value = row.trim();
  if (value.startsWith("|")) value = value.slice(1);
  if (value.endsWith("|")) value = value.slice(0, -1);
  return value.split("|").map((cell) => cell.trim());
}

interface ListParseResult {
  node: ReactNode;
  next: number;
}

function parseList(
  lines: string[],
  start: number,
  keyBase: string,
  handlers: MarkdownHandlers,
): ListParseResult {
  const first = lines[start].match(LIST_RE)!;
  const baseIndent = first[1].length;
  const ordered = /^\d/.test(first[2]);
  const items: string[][] = [];
  let current: string[] | null = null;
  let i = start;

  while (i < lines.length) {
    const line = lines[i];
    const match = line.match(LIST_RE);

    if (match && match[1].length <= baseIndent) {
      if (match[1].length < baseIndent) break;
      current = [match[3]];
      items.push(current);
      i += 1;
      continue;
    }

    if (!line.trim()) {
      let next = i + 1;
      while (next < lines.length && !lines[next].trim()) next += 1;
      if (next >= lines.length) {
        i = next;
        break;
      }
      const nextMatch = lines[next].match(LIST_RE);
      const nextIndent = nextMatch ? nextMatch[1].length : -1;
      if (nextIndent >= baseIndent || leadingSpaces(lines[next]) >= baseIndent + 2) {
        current?.push("");
        i += 1;
        continue;
      }
      break;
    }

    if (!current) break;

    const strip = Math.min(leadingSpaces(line), baseIndent + 2);
    current.push(line.slice(strip));
    i += 1;
  }

  const Tag = ordered ? "ol" : "ul";
  const node = (
    <Tag
      className={`md-list ${ordered ? "md-ordered" : "md-unordered"}`}
      key={`${keyBase}-list-${start}`}
    >
      {items.map((item, index) => {
        const checkbox = item[0]?.match(/^\[([ xX])\]\s*(.*)$/);
        if (checkbox) {
          const content = [...item];
          content[0] = checkbox[2];
          return (
            <li className="md-task-item" key={`${keyBase}-li-${start}-${index}`}>
              <input
                className="md-checkbox"
                type="checkbox"
                checked={checkbox[1].toLowerCase() === "x"}
                readOnly
                tabIndex={-1}
              />
              <div className="md-task-text">
                {parseBlocks(content, `${keyBase}-li-${start}-${index}`, handlers)}
              </div>
            </li>
          );
        }
        return (
          <li key={`${keyBase}-li-${start}-${index}`}>
            {parseBlocks(item, `${keyBase}-li-${start}-${index}`, handlers)}
          </li>
        );
      })}
    </Tag>
  );

  return { node, next: i };
}

function parseBlocks(
  lines: string[],
  keyBase: string,
  handlers: MarkdownHandlers,
): ReactNode[] {
  const nodes: ReactNode[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    if (!line.trim()) {
      i += 1;
      continue;
    }

    const fence = line.match(FENCE_RE);
    if (fence) {
      const marker = fence[1];
      const language = fence[2].trim();
      const code: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i].trimStart().startsWith(marker)) {
        code.push(lines[i]);
        i += 1;
      }
      if (i < lines.length) i += 1;
      nodes.push(
        <pre className="md-code-block" key={`${keyBase}-code-${i}`}>
          <code data-language={language || undefined}>{code.join("\n")}</code>
        </pre>,
      );
      continue;
    }

    const heading = line.match(HEADING_RE);
    if (heading) {
      const level = heading[1].length;
      const Tag = `h${level}` as "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
      nodes.push(
        <Tag className="md-heading" key={`${keyBase}-h-${i}`}>
          {renderInline(heading[2], `${keyBase}-h-${i}`, handlers)}
        </Tag>,
      );
      i += 1;
      continue;
    }

    if (!isBlockStart(line) && isSetextUnderline(lines[i + 1])) {
      const level = lines[i + 1].trim().startsWith("=") ? 1 : 2;
      const Tag = `h${level}` as "h1" | "h2";
      nodes.push(
        <Tag className="md-heading" key={`${keyBase}-sh-${i}`}>
          {renderInline(line.trim(), `${keyBase}-sh-${i}`, handlers)}
        </Tag>,
      );
      i += 2;
      continue;
    }

    if (HR_RE.test(line)) {
      nodes.push(<hr className="md-hr" key={`${keyBase}-hr-${i}`} />);
      i += 1;
      continue;
    }

    const quote = line.match(QUOTE_RE);
    if (quote) {
      const quoteLines: string[] = [];
      while (i < lines.length) {
        const inner = lines[i].match(QUOTE_RE);
        if (!inner) break;
        quoteLines.push(inner[1]);
        i += 1;
      }
      nodes.push(
        <blockquote className="md-quote" key={`${keyBase}-quote-${i}`}>
          {parseBlocks(quoteLines, `${keyBase}-quote-${i}`, handlers)}
        </blockquote>,
      );
      continue;
    }

    if (LIST_RE.test(line)) {
      const list = parseList(lines, i, keyBase, handlers);
      nodes.push(list.node);
      i = list.next;
      continue;
    }

    if (isTableStart(lines, i)) {
      const header = splitRow(lines[i]);
      i += 2;
      const rows: string[][] = [];
      while (i < lines.length && lines[i].trim() && lines[i].includes("|")) {
        rows.push(splitRow(lines[i]));
        i += 1;
      }
      nodes.push(
        <div className="md-table-wrap" key={`${keyBase}-table-${i}`}>
          <table className="md-table">
            <thead>
              <tr>
                {header.map((cell, index) => (
                  <th key={`${keyBase}-th-${index}`}>
                    {renderInline(
                      cell,
                      `${keyBase}-th-${i}-${index}`,
                      handlers,
                    )}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, rowIndex) => (
                <tr key={`${keyBase}-tr-${rowIndex}`}>
                  {row.map((cell, cellIndex) => (
                    <td key={`${keyBase}-td-${rowIndex}-${cellIndex}`}>
                      {renderInline(
                        cell,
                        `${keyBase}-td-${rowIndex}-${cellIndex}`,
                        handlers,
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }

    const paragraph: string[] = [line];
    i += 1;
    while (
      i < lines.length &&
      lines[i].trim() &&
      !isBlockStart(lines[i]) &&
      !isTableStart(lines, i) &&
      !isSetextUnderline(lines[i])
    ) {
      paragraph.push(lines[i]);
      i += 1;
    }
    nodes.push(
      <p className="md-paragraph" key={`${keyBase}-p-${i}`}>
        {renderInline(paragraph.join("\n"), `${keyBase}-p-${i}`, handlers)}
      </p>,
    );
  }

  return nodes;
}

export function MarkdownView({
  source,
  className,
  ...handlers
}: {
  source: string;
  className?: string;
} & MarkdownHandlers) {
  const { t } = useI18n();
  const blocks = useMemo(
    () => parseBlocks(source.replace(/\r\n?/g, "\n").split("\n"), "md", handlers),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      source,
      handlers.resolveWikiLink,
      handlers.onOpenWikiLink,
      handlers.onOpenUrl,
    ],
  );

  return (
    <div className={`markdown ${className ?? ""}`.trim()}>
      {blocks.length > 0 ? (
        blocks
      ) : (
        <p className="md-empty">
          {t('No content. Use "Edit" to start writing.')}
        </p>
      )}
    </div>
  );
}

export function MarkdownInline({
  text,
  className,
  ...handlers
}: {
  text: string;
  className?: string;
} & MarkdownHandlers) {
  return <span className={className}>{renderInline(text, "inline", handlers)}</span>;
}
