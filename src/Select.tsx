import type { SelectHTMLAttributes } from "react";
import { Icon } from "./Icons";

/**
 * Native select with a custom chevron that is not a CSS background.
 * Styling the arrow with background-image makes WebView2 tile the image
 * inside the native dropdown popup, so the chevron lives in the wrapper.
 */
export function Select({
  className,
  children,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <span className="select-wrap">
      <select
        className={`text-input${className ? ` ${className}` : ""}`}
        {...props}
      >
        {children}
      </select>
      <Icon className="select-chevron" name="chevron-down" size={12} />
    </span>
  );
}
