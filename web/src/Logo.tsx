// Flatrate mark: a price line flattened inside a band.
export function Logo({ size = 22 }: { size?: number }) {
  return (
    <svg className="logo" width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <rect x="1" y="1" width="22" height="22" rx="6" fill="var(--accent-soft)" stroke="var(--accent)" strokeOpacity="0.6" />
      <path d="M5 8h14M5 16h14" stroke="var(--accent-2)" strokeOpacity="0.45" strokeWidth="1.4" strokeLinecap="round" />
      <path d="M5 14.5l3-3 2.5 1.5L13 12h6" stroke="var(--text)" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
