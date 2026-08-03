import { useEffect } from "react";
import { Icon } from "./Icon";

interface ToastProps {
  message: string;
  tone?: "success" | "error" | "info";
  onDismiss: () => void;
}

export function Toast({ message, tone = "info", onDismiss }: ToastProps) {
  useEffect(() => {
    const timer = window.setTimeout(onDismiss, 2800);
    return () => window.clearTimeout(timer);
  }, [onDismiss]);

  return (
    <div className={`toast toast--${tone}`} role="status">
      <Icon name={tone === "error" ? "warning" : "check"} size={17} />
      <span>{message}</span>
    </div>
  );
}
