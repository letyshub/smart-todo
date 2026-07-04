import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { AnchorHTMLAttributes, ImgHTMLAttributes } from 'react'
import { open } from '@tauri-apps/plugin-shell'

const SAFE_URL = /^(https?:|mailto:|#)/i

interface Props {
  content: string
}

export default function MarkdownRenderer({ content }: Props) {
  function handleLinkClick(e: React.MouseEvent<HTMLAnchorElement>, href: string) {
    e.preventDefault()
    if (/^https?:/i.test(href) || /^mailto:/i.test(href)) {
      open(href).catch(console.error)
    }
  }

  return (
    <div className="prose prose-sm dark:prose-invert max-w-none">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          script: () => null,
          a: ({ href, children, ...props }: AnchorHTMLAttributes<HTMLAnchorElement>) => {
            const safeHref = href && SAFE_URL.test(href) ? href : undefined
            return (
              <a
                href={safeHref}
                {...props}
                onClick={safeHref ? (e) => handleLinkClick(e as React.MouseEvent<HTMLAnchorElement>, safeHref) : undefined}
                className="cursor-pointer"
              >
                {children}
              </a>
            )
          },
          img: ({ src, ...props }: ImgHTMLAttributes<HTMLImageElement>) =>
            src && SAFE_URL.test(src) ? <img src={src} {...props} /> : null,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  )
}
