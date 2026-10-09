import { useEffect, type RefObject } from "react";

const FOCUSABLE = [
	"button:not([disabled])",
	"a[href]",
	"input:not([disabled])",
	"select:not([disabled])",
	"textarea:not([disabled])",
	'[tabindex]:not([tabindex="-1"])',
].join(",");

/**
 * Keeps keyboard focus inside a modal while it is mounted: focus moves to its
 * first control on open, Tab / Shift+Tab wrap around its controls, and focus
 * returns to where it was on close unless something else has taken it.
 */
export const useFocusTrap = (ref: RefObject<HTMLElement | null>): void => {
	useEffect(() => {
		const node = ref.current;
		if (!node) return;
		const previous = document.activeElement;
		const focusables = () => Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE));
		(focusables()[0] ?? node).focus();

		const handleKey = (e: KeyboardEvent) => {
			if (e.key !== "Tab") return;
			const items = focusables();
			if (items.length === 0) {
				e.preventDefault();
				node.focus();
				return;
			}
			const first = items[0];
			const last = items[items.length - 1];
			const active = document.activeElement;
			const outside = !node.contains(active);
			if (e.shiftKey && (active === first || outside)) {
				e.preventDefault();
				last.focus();
			} else if (!e.shiftKey && (active === last || outside)) {
				e.preventDefault();
				first.focus();
			}
		};
		document.addEventListener("keydown", handleKey);
		return () => {
			document.removeEventListener("keydown", handleKey);
			// Focus falls back to <body> when the focused control unmounts with the modal.
			const active = document.activeElement;
			const lost = !active || active === document.body || node.contains(active);
			if (lost && previous instanceof HTMLElement && previous.isConnected) previous.focus();
		};
	}, [ref]);
};
