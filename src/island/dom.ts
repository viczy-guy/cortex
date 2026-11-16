/**
 * The only way the island creates DOM. Dynamic data always goes through
 * `textContent` — never innerHTML — so nothing an agent sends can become markup.
 */
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function button(
  className: string,
  label: string,
  onClick: () => void,
): HTMLButtonElement {
  const node = el("button", `btn ${className}`, label);
  node.addEventListener("click", (event) => {
    event.stopPropagation();
    onClick();
  });
  return node;
}
