/** Line icons drawn for this app (24 x 24 grid, 1.8 stroke, currentColor). */
import type { ReactNode } from "react";

function Svg({ size = 20, children, label }: { size?: number; children: ReactNode; label?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden={label ? undefined : true}
      role={label ? "img" : undefined}
      aria-label={label}
    >
      {children}
    </svg>
  );
}

type P = { size?: number };

export const Icon = {
  Logo: ({ size = 20 }: P) => (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
      <g fill="currentColor">
        <rect x="3" y="9" width="2.4" height="6" rx="1.2" />
        <rect x="7.2" y="5.5" width="2.4" height="13" rx="1.2" />
        <rect x="11.4" y="2.5" width="2.4" height="19" rx="1.2" />
        <rect x="15.6" y="6.5" width="2.4" height="11" rx="1.2" />
        <rect x="19.8" y="9.5" width="2.4" height="5" rx="1.2" />
      </g>
    </svg>
  ),
  Home: (p: P) => (
    <Svg {...p}>
      <path d="M4 10.5 12 4l8 6.5V19a1 1 0 0 1-1 1h-4.5v-5.5h-5V20H5a1 1 0 0 1-1-1z" />
    </Svg>
  ),
  Chart: (p: P) => (
    <Svg {...p}>
      <path d="M4 20h16" />
      <rect x="6" y="11" width="3" height="6" rx="1" />
      <rect x="10.5" y="6" width="3" height="11" rx="1" />
      <rect x="15" y="9" width="3" height="8" rx="1" />
    </Svg>
  ),
  Clock: (p: P) => (
    <Svg {...p}>
      <circle cx="12" cy="12" r="8" />
      <path d="M12 7.5V12l3 2" />
    </Svg>
  ),
  Note: (p: P) => (
    <Svg {...p}>
      <path d="M6 4h9l3 3v13H6z" />
      <path d="M9 10h6M9 13.5h6M9 17h3.5" />
    </Svg>
  ),
  Meeting: (p: P) => (
    <Svg {...p}>
      <rect x="3.5" y="5" width="17" height="11" rx="2" />
      <path d="M9 20h6M12 16v4" />
      <path d="M8 10.5h1M11.5 9v3M15 10.5h1" />
    </Svg>
  ),
  Book: (p: P) => (
    <Svg {...p}>
      <path d="M5 5.5A2.5 2.5 0 0 1 7.5 3H19v15H7.5A2.5 2.5 0 0 0 5 20.5z" />
      <path d="M5 20.5A2.5 2.5 0 0 1 7.5 18H19v3H7.5" />
    </Svg>
  ),
  Quote: (p: P) => (
    <Svg {...p}>
      <path d="M5 6h14v9H9l-4 4z" />
      <path d="M9 10h6" />
    </Svg>
  ),
  Compass: (p: P) => (
    <Svg {...p}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="m15.5 8.5-2 5-5 2 2-5z" />
    </Svg>
  ),
  Gear: (p: P) => (
    <Svg {...p}>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2.8v2.4M12 18.8v2.4M4.2 7.5l2.1 1.2M17.7 15.3l2.1 1.2M4.2 16.5l2.1-1.2M17.7 8.7l2.1-1.2" />
      <circle cx="12" cy="12" r="6.6" />
    </Svg>
  ),
  Sun: (p: P) => (
    <Svg {...p}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2.5v2M12 19.5v2M4.6 4.6 6 6M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4 6 18M18 6l1.4-1.4" />
    </Svg>
  ),
  Moon: (p: P) => (
    <Svg {...p}>
      <path d="M19.5 14.5A8 8 0 0 1 9.5 4.5a7.5 7.5 0 1 0 10 10z" />
    </Svg>
  ),
  Monitor: (p: P) => (
    <Svg {...p}>
      <rect x="3.5" y="4.5" width="17" height="11.5" rx="2" />
      <path d="M9 20h6M12 16v4" />
    </Svg>
  ),
  Mic: (p: P) => (
    <Svg {...p}>
      <rect x="9" y="3" width="6" height="11" rx="3" />
      <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
    </Svg>
  ),
  Flame: (p: P) => (
    <Svg {...p}>
      <path d="M12 21c-3.9 0-6.5-2.6-6.5-6.1 0-3.4 2.6-5.3 3.6-8.4.3.1 2.2 1.4 2.4 4 1-1 1.6-2.6 1.6-4.5 3.3 2.2 5.4 5.4 5.4 8.9 0 3.5-2.6 6.1-6.5 6.1z" />
    </Svg>
  ),
  Gauge: (p: P) => (
    <Svg {...p}>
      <path d="M4 16a8 8 0 1 1 16 0" />
      <path d="m12 16 4-5" />
    </Svg>
  ),
  Words: (p: P) => (
    <Svg {...p}>
      <path d="M4 6h16M4 10h11M4 14h16M4 18h8" />
    </Svg>
  ),
  Hourglass: (p: P) => (
    <Svg {...p}>
      <path d="M7 3h10M7 21h10M8 3c0 5 8 5 8 9s-8 4-8 9M16 3c0 5-8 5-8 9" />
    </Svg>
  ),
  Sparkles: (p: P) => (
    <Svg {...p}>
      <path d="M12 3.5 13.6 8 18 9.6 13.6 11.2 12 15.7 10.4 11.2 6 9.6 10.4 8z" />
      <path d="M18.5 15.5l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z" />
    </Svg>
  ),
  Check: (p: P) => (
    <Svg {...p}>
      <path d="m5 12.5 4.5 4.5L19 7.5" />
    </Svg>
  ),
  Chevron: (p: P) => (
    <Svg {...p}>
      <path d="m9.5 6 6 6-6 6" />
    </Svg>
  ),
  Copy: (p: P) => (
    <Svg {...p}>
      <rect x="8.5" y="8.5" width="11" height="11" rx="2" />
      <path d="M15.5 8.5V6a1.5 1.5 0 0 0-1.5-1.5H6A1.5 1.5 0 0 0 4.5 6v8A1.5 1.5 0 0 0 6 15.5h2.5" />
    </Svg>
  ),
  Trash: (p: P) => (
    <Svg {...p}>
      <path d="M4.5 7h15M10 11v6M14 11v6M6.5 7l1 13h9l1-13M9.5 7V4h5v3" />
    </Svg>
  ),
  Edit: (p: P) => (
    <Svg {...p}>
      <path d="M4.5 19.5h4l10-10-4-4-10 10z" />
      <path d="m13 7 4 4" />
    </Svg>
  ),
  Search: (p: P) => (
    <Svg {...p}>
      <circle cx="11" cy="11" r="6.5" />
      <path d="m16 16 4 4" />
    </Svg>
  ),
  Key: (p: P) => (
    <Svg {...p}>
      <circle cx="8" cy="12" r="4" />
      <path d="M12 12h8.5M17 12v3M20 12v2" />
    </Svg>
  ),
  Shield: (p: P) => (
    <Svg {...p}>
      <path d="M12 3 5 6v5.5c0 4.4 3 7.9 7 9.5 4-1.6 7-5.1 7-9.5V6z" />
      <path d="m9 12 2.2 2.2L15.5 10" />
    </Svg>
  ),
  Download: (p: P) => (
    <Svg {...p}>
      <path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14" />
    </Svg>
  ),
  Refresh: (p: P) => (
    <Svg {...p}>
      <path d="M19 12a7 7 0 1 1-2.1-5M19 4.5V8h-3.5" />
    </Svg>
  ),
  Globe: (p: P) => (
    <Svg {...p}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M3.5 12h17M12 3.5c2.4 2.5 3.5 5.3 3.5 8.5s-1.1 6-3.5 8.5c-2.4-2.5-3.5-5.3-3.5-8.5s1.1-6 3.5-8.5z" />
    </Svg>
  ),
  Keyboard: (p: P) => (
    <Svg {...p}>
      <rect x="3" y="6.5" width="18" height="11" rx="2" />
      <path d="M7 10.5h.01M10 10.5h.01M13 10.5h.01M16 10.5h.01M7.5 14h9" />
    </Svg>
  ),
  Wand: (p: P) => (
    <Svg {...p}>
      <path d="m4.5 19.5 10-10M13 6l1-2.5L15 6l2.5 1L15 8l-1 2.5L13 8l-2.5-1z" />
    </Svg>
  ),
  Pen: (p: P) => (
    <Svg {...p}>
      <path d="M4.5 19.5 6 14 15.5 4.5l4 4L10 18z" />
    </Svg>
  ),
  Cloud: (p: P) => (
    <Svg {...p}>
      <path d="M7.5 18.5h10a4 4 0 0 0 .4-8A6 6 0 0 0 6.3 11a3.8 3.8 0 0 0 1.2 7.5z" />
    </Svg>
  ),
  Info: (p: P) => (
    <Svg {...p}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 11v5M12 8h.01" />
    </Svg>
  ),
  X: (p: P) => (
    <Svg {...p}>
      <path d="m6.5 6.5 11 11M17.5 6.5l-11 11" />
    </Svg>
  ),
};
