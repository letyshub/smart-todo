import { useState, useEffect, useRef } from 'react'
import { api } from '../lib/tauri'
import type { Tag } from '../types'

interface Props {
  tags: Tag[]
  onChange: (tagNames: string[]) => void
}

export default function TagInput({ tags, onChange }: Props) {
  const [input, setInput] = useState('')
  const [suggestions, setSuggestions] = useState<Tag[]>([])
  const [allTags, setAllTags] = useState<Tag[]>([])
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    api.getAllTags().then(setAllTags)
  }, [])

  useEffect(() => {
    if (!input.trim()) { setSuggestions([]); return }
    const lower = input.toLowerCase()
    const currentNames = tags.map((t) => t.name.toLowerCase())
    setSuggestions(
      allTags.filter(
        (t) => t.name.toLowerCase().includes(lower) && !currentNames.includes(t.name.toLowerCase())
      ).slice(0, 5)
    )
  }, [input, allTags, tags])

  function addTag(name: string) {
    const trimmed = name.trim()
    if (!trimmed) return
    if (tags.some((t) => t.name.toLowerCase() === trimmed.toLowerCase())) return
    onChange([...tags.map((t) => t.name), trimmed])
    setInput('')
    setSuggestions([])
  }

  function removeTag(name: string) {
    onChange(tags.filter((t) => t.name !== name).map((t) => t.name))
  }

  return (
    <div className="flex flex-wrap gap-1 p-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 min-h-[38px]">
      {tags.map((tag) => (
        <span
          key={tag.id ?? tag.name}
          className="flex items-center gap-1 text-xs px-1.5 py-0.5 rounded bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300"
        >
          {tag.name}
          <button
            type="button"
            onClick={() => removeTag(tag.name)}
            className="hover:text-red-500 leading-none"
            aria-label={`Remove tag ${tag.name}`}
          >
            ×
          </button>
        </span>
      ))}
      <div className="relative flex-1 min-w-[100px]">
        <input
          ref={inputRef}
          type="text"
          className="w-full text-xs outline-none bg-transparent text-gray-900 dark:text-gray-100 placeholder-gray-400"
          placeholder={tags.length === 0 ? 'Add tags…' : ''}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') { e.preventDefault(); addTag(input) }
            if (e.key === 'Backspace' && !input && tags.length > 0) {
              removeTag(tags[tags.length - 1].name)
            }
          }}
        />
        {suggestions.length > 0 && (
          <ul className="absolute top-full left-0 mt-1 w-48 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded shadow-lg z-10">
            {suggestions.map((tag) => (
              <li key={tag.id}>
                <button
                  type="button"
                  className="w-full text-left text-xs px-3 py-1.5 hover:bg-indigo-50 dark:hover:bg-indigo-900/50 text-gray-700 dark:text-gray-300"
                  onMouseDown={(e) => { e.preventDefault(); addTag(tag.name) }}
                >
                  {tag.name}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}
