export function Disconnect({ large = false }: { large?: boolean }) {
  return (
    <svg
      width={large ? 190 : 31}
      height={large ? 72 : 28}
      viewBox="0 0 190 72"
      fill="none"
      aria-hidden="true"
      className={large ? "disconnect-large" : ""}
    >
      <path
        d="M18 30H166"
        stroke="currentColor"
        strokeWidth={large ? 1.4 : 4}
      />
      <circle
        cx="20"
        cy="30"
        r="7"
        fill="var(--background)"
        stroke="currentColor"
        strokeWidth={large ? 1.5 : 4}
      />
      <circle
        cx="166"
        cy="30"
        r="7"
        fill="var(--background)"
        stroke="currentColor"
        strokeWidth={large ? 1.5 : 4}
      />
      <circle cx="96" cy="52" r="7" fill="var(--accent)" />
      <path d="M96 29V36" stroke="var(--accent)" strokeDasharray="2 3" />
    </svg>
  );
}
