/**
 * Original illustrations for the Home banner (drawn for this app as SVG: no photos, no
 * third-party artwork). Each shows what the feature does.
 */

/** Spoken Hinglish becoming typed chat messages. */
export function ChatArt() {
  return (
    <svg viewBox="0 0 420 260" width="100%" height="100%" role="img" aria-label="A phone chat where spoken Hinglish becomes typed text">
      <defs>
        <linearGradient id="ca-glow" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#5bdacd" stopOpacity="0.55" />
          <stop offset="1" stopColor="#6366f1" stopOpacity="0.25" />
        </linearGradient>
        <linearGradient id="ca-phone" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#ffffff" stopOpacity="0.16" />
          <stop offset="1" stopColor="#ffffff" stopOpacity="0.06" />
        </linearGradient>
        <filter id="ca-blur">
          <feGaussianBlur stdDeviation="28" />
        </filter>
      </defs>
      <circle cx="300" cy="120" r="95" fill="url(#ca-glow)" filter="url(#ca-blur)" />
      <rect x="205" y="18" width="170" height="232" rx="26" fill="url(#ca-phone)" stroke="#ffffff" strokeOpacity="0.28" />
      <rect x="262" y="30" width="56" height="7" rx="3.5" fill="#ffffff" fillOpacity="0.3" />
      {/* incoming */}
      <rect x="220" y="56" width="118" height="34" rx="14" fill="#ffffff" fillOpacity="0.18" />
      <rect x="232" y="68" width="78" height="5" rx="2.5" fill="#ffffff" fillOpacity="0.7" />
      <rect x="232" y="78" width="52" height="5" rx="2.5" fill="#ffffff" fillOpacity="0.45" />
      {/* typed by voice */}
      <rect x="236" y="104" width="126" height="46" rx="14" fill="#5bdacd" />
      <text x="248" y="122" fontSize="11.5" fontWeight="700" fill="#03241f" fontFamily="Segoe UI, sans-serif">
        Kal meeting 4 baje,
      </text>
      <text x="248" y="138" fontSize="11.5" fontWeight="700" fill="#03241f" fontFamily="Segoe UI, sans-serif">
        time pe aa jana 👍
      </text>
      <rect x="250" y="164" width="112" height="30" rx="14" fill="#ffffff" fillOpacity="0.92" />
      <text x="262" y="183" fontSize="11.5" fontWeight="700" fill="#0b4f4b" fontFamily="Segoe UI, sans-serif">
        Done in 0.8 s ✓
      </text>
      {/* voice waves into the phone */}
      <g fill="#ffffff">
        {[22, 40, 62, 88, 64, 104, 72, 50, 30].map((h, i) => (
          <rect key={i} x={40 + i * 16} y={130 - h / 2} width="8" height={h} rx="4" opacity={0.5 + (i % 3) * 0.17} />
        ))}
      </g>
      <path d="M190 130 C 200 130, 205 128, 214 126" stroke="#ffffff" strokeOpacity="0.5" strokeWidth="2.5" strokeDasharray="4 6" fill="none" />
      <g transform="translate(70 196)">
        <rect width="46" height="30" rx="8" fill="#ffffff" fillOpacity="0.95" />
        <text x="23" y="20" textAnchor="middle" fontSize="12" fontWeight="800" fill="#0b4f4b" fontFamily="Segoe UI, sans-serif">
          Ctrl
        </text>
        <text x="58" y="20" textAnchor="middle" fontSize="14" fontWeight="800" fill="#ffffff">
          +
        </text>
        <rect x="70" width="46" height="30" rx="8" fill="#ffffff" fillOpacity="0.95" />
        <text x="93" y="20" textAnchor="middle" fontSize="12" fontWeight="800" fill="#0b4f4b" fontFamily="Segoe UI, sans-serif">
          Win
        </text>
      </g>
    </svg>
  );
}

/** A paragraph turning into a bulleted list by voice. */
export function CommandArt() {
  return (
    <svg viewBox="0 0 420 260" width="100%" height="100%" role="img" aria-label="Selected text turning into a bulleted list">
      <defs>
        <linearGradient id="cm-glow" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#f6a623" stopOpacity="0.45" />
          <stop offset="1" stopColor="#5bdacd" stopOpacity="0.3" />
        </linearGradient>
        <filter id="cm-blur">
          <feGaussianBlur stdDeviation="26" />
        </filter>
      </defs>
      <circle cx="250" cy="130" r="100" fill="url(#cm-glow)" filter="url(#cm-blur)" />
      {/* before */}
      <rect x="28" y="52" width="150" height="150" rx="16" fill="#ffffff" fillOpacity="0.12" stroke="#ffffff" strokeOpacity="0.25" />
      <rect x="44" y="74" width="118" height="44" rx="6" fill="#5bdacd" fillOpacity="0.35" />
      {[78, 88, 98, 108].map((y, i) => (
        <rect key={y} x="50" y={y} width={[104, 96, 108, 70][i]} height="5" rx="2.5" fill="#ffffff" fillOpacity="0.85" />
      ))}
      {[136, 148, 160, 172].map((y, i) => (
        <rect key={y} x="44" y={y} width={[112, 100, 116, 64][i]} height="5" rx="2.5" fill="#ffffff" fillOpacity="0.4" />
      ))}
      {/* speech bubble */}
      <g transform="translate(150 20)">
        <rect width="150" height="34" rx="17" fill="#ffffff" />
        <path d="M30 34 L24 46 L42 34 Z" fill="#ffffff" />
        <text x="75" y="22" textAnchor="middle" fontSize="11.5" fontWeight="700" fill="#0b4f4b" fontFamily="Segoe UI, sans-serif">
          “make this a list”
        </text>
      </g>
      <path d="M190 128 h40" stroke="#ffffff" strokeWidth="3" strokeLinecap="round" />
      <path d="m224 120 10 8-10 8" stroke="#ffffff" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" fill="none" />
      {/* after */}
      <rect x="246" y="52" width="150" height="150" rx="16" fill="#ffffff" fillOpacity="0.95" />
      {[80, 112, 144].map((y, i) => (
        <g key={y}>
          <circle cx="266" cy={y + 2} r="5" fill="#0b8a80" />
          <rect x="280" y={y} width={[96, 80, 90][i]} height="5" rx="2.5" fill="#0f1419" fillOpacity="0.8" />
          <rect x="280" y={y + 10} width={[70, 88, 56][i]} height="5" rx="2.5" fill="#0f1419" fillOpacity="0.35" />
        </g>
      ))}
    </svg>
  );
}

/** The same sentence in three writing styles. */
export function StyleArt() {
  const cards: [string, string, number][] = [
    ["Casual", "running 10 mins late!", 0],
    ["Clear", "I'm running 10 minutes late.", 1],
    ["Formal", "I will be ten minutes late.", 2],
  ];
  return (
    <svg viewBox="0 0 420 260" width="100%" height="100%" role="img" aria-label="One sentence in casual, clear and formal style">
      <defs>
        <linearGradient id="st-glow" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#6366f1" stopOpacity="0.4" />
          <stop offset="1" stopColor="#5bdacd" stopOpacity="0.35" />
        </linearGradient>
        <filter id="st-blur">
          <feGaussianBlur stdDeviation="30" />
        </filter>
      </defs>
      <circle cx="230" cy="130" r="110" fill="url(#st-glow)" filter="url(#st-blur)" />
      {cards.map(([label, text, i]) => (
        <g key={label} transform={`translate(${60 + i * 26} ${36 + i * 62}) rotate(${-4 + i * 3})`}>
          <rect width="250" height="56" rx="14" fill="#ffffff" fillOpacity={0.96 - i * 0.04} />
          <rect x="14" y="13" width="58" height="18" rx="9" fill={["#f6a623", "#0b8a80", "#4f46e5"][i]} />
          <text x="43" y="26" textAnchor="middle" fontSize="10.5" fontWeight="800" fill="#ffffff" fontFamily="Segoe UI, sans-serif">
            {label}
          </text>
          <text x="16" y="46" fontSize="12" fontWeight="600" fill="#0f1419" fontFamily="Segoe UI, sans-serif">
            {text}
          </text>
        </g>
      ))}
    </svg>
  );
}
