import { describe, it, expect, beforeEach } from "vitest";
import {
	readDefaultVolume,
	writeDefaultVolume,
	readVolumePreference,
	writeVolumePreference,
	readHwdecEnabled,
	writeHwdecEnabled,
} from "./player-prefs";

describe("player-prefs", () => {
	beforeEach(() => {
		localStorage.clear();
		sessionStorage.clear();
	});

	describe("default volume", () => {
		it("is 100 when never set", () => {
			expect(readDefaultVolume()).toBe(100);
		});

		it("round-trips a saved value", () => {
			writeDefaultVolume(65);
			expect(readDefaultVolume()).toBe(65);
		});

		it("clamps to the range mpv accepts", () => {
			writeDefaultVolume(400);
			expect(readDefaultVolume()).toBe(150);
			writeDefaultVolume(-5);
			expect(readDefaultVolume()).toBe(0);
		});

		it("ignores a value that isn't a number", () => {
			localStorage.setItem("mvp_default_volume", "loud");
			expect(readDefaultVolume()).toBe(100);
		});
	});

	describe("volume preference", () => {
		it("falls back to the default volume", () => {
			writeDefaultVolume(70);
			expect(readVolumePreference()).toEqual({ volume: 70, preMute: 70 });
		});

		it("unmutes to 100 when the default volume is 0", () => {
			writeDefaultVolume(0);
			expect(readVolumePreference()).toEqual({ volume: 0, preMute: 100 });
		});

		it("prefers the volume chosen this session", () => {
			writeDefaultVolume(70);
			writeVolumePreference({ volume: 30, preMute: 50 });
			expect(readVolumePreference()).toEqual({ volume: 30, preMute: 50 });
		});

		it("drops this session's volume when the default changes", () => {
			writeVolumePreference({ volume: 30, preMute: 50 });
			writeDefaultVolume(90);
			expect(readVolumePreference()).toEqual({ volume: 90, preMute: 90 });
		});
	});

	describe("hardware decoding", () => {
		it("is on when never set", () => {
			expect(readHwdecEnabled()).toBe(true);
		});

		it("round-trips the setting", () => {
			writeHwdecEnabled(false);
			expect(readHwdecEnabled()).toBe(false);
			writeHwdecEnabled(true);
			expect(readHwdecEnabled()).toBe(true);
		});
	});
});
