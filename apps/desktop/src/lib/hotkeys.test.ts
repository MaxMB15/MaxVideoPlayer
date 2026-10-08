import { describe, it, expect } from "vitest";
import { resolvePlayerHotkey, SEEK_LONG, SEEK_SHORT, VOLUME_STEP } from "./hotkeys";

const key = (
	k: string,
	mods: Partial<{ shiftKey: boolean; ctrlKey: boolean; metaKey: boolean; altKey: boolean }> = {}
) => ({
	key: k,
	shiftKey: false,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	...mods,
});

describe("resolvePlayerHotkey", () => {
	it("maps space and K to play/pause", () => {
		expect(resolvePlayerHotkey(key(" "))).toEqual({ type: "togglePlay" });
		expect(resolvePlayerHotkey(key("k"))).toEqual({ type: "togglePlay" });
	});
	it("maps F and M regardless of case", () => {
		expect(resolvePlayerHotkey(key("F"))).toEqual({ type: "toggleFullscreen" });
		expect(resolvePlayerHotkey(key("m"))).toEqual({ type: "toggleMute" });
	});
	it("seeks with arrows, longer with shift", () => {
		expect(resolvePlayerHotkey(key("ArrowLeft"))).toEqual({
			type: "seekBy",
			seconds: -SEEK_SHORT,
		});
		expect(resolvePlayerHotkey(key("ArrowRight", { shiftKey: true }))).toEqual({
			type: "seekBy",
			seconds: SEEK_LONG,
		});
	});
	it("changes volume with up/down", () => {
		expect(resolvePlayerHotkey(key("ArrowUp"))).toEqual({
			type: "volumeBy",
			delta: VOLUME_STEP,
		});
		expect(resolvePlayerHotkey(key("ArrowDown"))).toEqual({
			type: "volumeBy",
			delta: -VOLUME_STEP,
		});
	});
	it("maps digits to percentage seeks", () => {
		expect(resolvePlayerHotkey(key("0"))).toEqual({ type: "seekToPercent", percent: 0 });
		expect(resolvePlayerHotkey(key("7"))).toEqual({ type: "seekToPercent", percent: 70 });
	});
	it("ignores modifier combinations", () => {
		expect(resolvePlayerHotkey(key("f", { metaKey: true }))).toBeNull();
		expect(resolvePlayerHotkey(key("m", { ctrlKey: true }))).toBeNull();
		expect(resolvePlayerHotkey(key(" ", { altKey: true }))).toBeNull();
	});
	it("returns null for unbound keys", () => {
		expect(resolvePlayerHotkey(key("z"))).toBeNull();
	});
});
