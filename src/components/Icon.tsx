import type { SVGProps } from "react";

export type IconName =
  | "spark"
  | "clipboard"
  | "copy"
  | "history"
  | "settings"
  | "shield"
  | "arrow"
  | "trash"
  | "check"
  | "warning"
  | "close"
  | "code";

const paths: Record<IconName, React.ReactNode> = {
  spark: (
    <path d="m12 2 1.4 5.1L18 9l-4.6 1.9L12 16l-1.4-5.1L6 9l4.6-1.9L12 2Zm6 12 .8 2.2L21 17l-2.2.8L18 20l-.8-2.2L15 17l2.2-.8L18 14Z" />
  ),
  clipboard: (
    <path d="M9 5h6m-7 2H6a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-2M9 3h6a1 1 0 0 1 1 1v2H8V4a1 1 0 0 1 1-1Z" />
  ),
  copy: (
    <path d="M8 8h11a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V10a2 2 0 0 1 2-2Zm8-3V4a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h1" />
  ),
  history: <path d="M3 12a9 9 0 1 0 3-6.7L3 8m0 0V3m0 5h5m4-2v6l4 2" />,
  settings: (
    <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm7-3.5 2-1-2-3.5-2.2.6a7 7 0 0 0-1.6-.9L14.5 5h-5l-.7 2.2a7 7 0 0 0-1.6.9L5 7.5 3 11l2 1a7 7 0 0 0 0 1.8l-2 1 2 3.5 2.2-.6a7 7 0 0 0 1.6.9l.7 2.2h5l.7-2.2a7 7 0 0 0 1.6-.9l2.2.6 2-3.5-2-1a7 7 0 0 0 0-1.8Z" />
  ),
  shield: <path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Zm-3-10 2 2 4-5" />,
  arrow: <path d="m9 18 6-6-6-6" />,
  trash: <path d="M4 7h16M9 7V4h6v3m3 0-1 14H7L6 7m4 4v6m4-6v6" />,
  check: <path d="m5 12 4 4L19 6" />,
  warning: <path d="M12 3 2 21h20L12 3Zm0 6v5m0 3h.01" />,
  close: <path d="m6 6 12 12M18 6 6 18" />,
  code: <path d="m8 9-4 3 4 3m8-6 4 3-4 3m-2-9-4 12" />,
};

interface IconProps extends SVGProps<SVGSVGElement> {
  name: IconName;
  size?: number;
}

export function Icon({ name, size = 18, ...props }: IconProps) {
  return (
    <svg
      aria-hidden="true"
      fill="none"
      height={size}
      viewBox="0 0 24 24"
      width={size}
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth="1.8"
      {...props}
    >
      {paths[name]}
    </svg>
  );
}
