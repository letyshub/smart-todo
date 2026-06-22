import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { AnchorHTMLAttributes, ImgHTMLAttributes } from 'react'

const SAFE_URL = /^(https?:|mailto:|#)/i

interface Props {
  content: string
}

export default function MarkdownRenderer({ content }: Props) {
  return (
    <div className="prose prose-sm dark:prose-invert max-w-none">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          script: () => null,
          a: ({ href, children, ...props }: AnchorHTMLAttributes<HTMLAnchorElement>) => (
            <a href={href && SAFE_URL.test(href) ? href : undefined} {...props}>
              {children}
            </a>
          ),
          img: ({ src, ...props }: ImgHTMLAttributes<HTMLImageElement>) =>
            src && SAFE_URL.test(src) ? <img src={src} {...props} /> : null,
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  )
}
