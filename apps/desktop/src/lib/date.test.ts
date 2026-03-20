import { describe, it, expect } from "vitest";
import { parseDateMs } from "./date";

describe("parseDateMs", () => {
	it("parses a valid ISO string", () => {
		const ms = parseDateMs("2024-01-15T10:00:00Z");
		expect(ms).toBe(new Date("2024-01-15T10:00:00Z").getTime());
	});

	it("returns 0 for null", () => {
		expect(parseDateMs(null)).toBe(0);
	});

	it("returns 0 for undefined", () => {
		expect(parseDateMs(undefined)).toBe(0);
	});

	it("returns 0 for empty string", () => {
		expect(parseDateMs("")).toBe(0);
	});

	it("returns 0 for garbage string", () => {
		expect(parseDateMs("not-a-date")).toBe(0);
	});

	it("returns 0 for NaN-producing input", () => {
		expect(parseDateMs("NaN")).toBe(0);
	});
});
