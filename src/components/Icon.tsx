import type { RowKind } from '../lib/format';

interface KindStyle {
  cls: string;
  paths: string[];
}

const KIND: Record<RowKind, KindStyle> = {
  text: {
    cls: 'bg-[#eef2ff] text-[#4f46e5] dark:bg-[#312e81] dark:text-[#a5b4fc]',
    paths: ['M5 6h14', 'M5 12h14', 'M5 18h9'],
  },
  link: {
    cls: 'bg-[#ecfdf5] text-[#059669] dark:bg-[#064e3b] dark:text-[#6ee7b7]',
    paths: [
      'M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1',
      'M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1',
    ],
  },
  code: {
    cls: 'bg-[#fff7ed] text-[#ea580c] dark:bg-[#7c2d12] dark:text-[#fdba74]',
    paths: ['m8 8-4 4 4 4', 'm16 8 4 4-4 4'],
  },
  file: {
    cls: 'bg-[#eff6ff] text-[#2563eb] dark:bg-[#1e3a8a] dark:text-[#93c5fd]',
    paths: ['M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z', 'M14 3v5h5'],
  },
  image: {
    cls: 'bg-[#fdf2f8] text-[#db2777] dark:bg-[#831843] dark:text-[#f9a8d4]',
    paths: ['M3 5h18v14H3z', 'm21 16-5-5-9 9'],
  },
};

export function Icon({ kind, className = '' }: { kind: RowKind; className?: string }) {
  const s = KIND[kind];
  return (
    <span
      className={`inline-flex items-center justify-center w-[18px] h-[18px] rounded-[5px] shrink-0 ${s.cls} ${className}`}
      aria-hidden
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
        {s.paths.map((d, i) => (
          <path key={i} d={d} />
        ))}
      </svg>
    </span>
  );
}

export function SearchIcon({ className = '' }: { className?: string }) {
  return (
    <svg className={className} width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round">
      <circle cx="11" cy="11" r="7" />
      <path d="m21 21-4.3-4.3" />
    </svg>
  );
}

export function PinIcon({ active = false, className = '' }: { active?: boolean; className?: string }) {
  return (
    <svg
      className={className}
      width="13" height="13" viewBox="0 0 24 24"
      fill={active ? 'currentColor' : 'none'}
      stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round"
    >
      <path d="M12 17v5" />
      <path d="M9 3h6l-1 6 4 3v2H6v-2l4-3-1-6z" />
    </svg>
  );
}

export function TrashIcon({ className = '' }: { className?: string }) {
  return (
    <svg
      className={className}
      width="13" height="13" viewBox="0 0 24 24"
      fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round"
    >
      <path d="M4 7h16" />
      <path d="M10 11v6M14 11v6" />
      <path d="M6 7l1 13h10l1-13" />
      <path d="M9 7V4h6v3" />
    </svg>
  );
}
