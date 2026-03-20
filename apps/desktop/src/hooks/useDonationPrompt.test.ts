import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useDonationPrompt } from "./useDonationPrompt";

const STORAGE_KEY = "donation-last-shown";
const THIRTY_DAYS_MS = 30 * 24 * 60 * 60 * 1000;

describe("useDonationPrompt", () => {
	beforeEach(() => {
		localStorage.clear();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("does not show when enabled=false", () => {
		const { result } = renderHook(() => useDonationPrompt({ enabled: false }));
		expect(result.current.shouldShow).toBe(false);
	});

	it("shows on first launch (no stored key)", () => {
		const { result } = renderHook(() => useDonationPrompt({ enabled: true }));
		expect(result.current.shouldShow).toBe(true);
	});

	it("does not show if dismissed less than 30 days ago", () => {
		const recent = Date.now() - (THIRTY_DAYS_MS - 1000);
		localStorage.setItem(STORAGE_KEY, String(recent));
		const { result } = renderHook(() => useDonationPrompt({ enabled: true }));
		expect(result.current.shouldShow).toBe(false);
	});

	it("shows if dismissed exactly 30 days ago", () => {
		const old = Date.now() - THIRTY_DAYS_MS;
		localStorage.setItem(STORAGE_KEY, String(old));
		const { result } = renderHook(() => useDonationPrompt({ enabled: true }));
		expect(result.current.shouldShow).toBe(true);
	});

	it("dismiss() sets shouldShow=false and writes timestamp", () => {
		const before = Date.now();
		const { result } = renderHook(() => useDonationPrompt({ enabled: true }));
		expect(result.current.shouldShow).toBe(true);

		act(() => result.current.dismiss());

		expect(result.current.shouldShow).toBe(false);
		const stored = parseInt(localStorage.getItem(STORAGE_KEY) ?? "0", 10);
		expect(stored).toBeGreaterThanOrEqual(before);
	});
});
